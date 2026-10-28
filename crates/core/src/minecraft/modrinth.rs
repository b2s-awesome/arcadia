//! Modrinth API (no key needed; identifying User-Agent required).

use super::download::{fetch, Job};
use super::instance;
use super::loaders::Loader;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub source: String,
}

#[derive(Deserialize)]
struct SearchResp {
    hits: Vec<RawHit>,
}
#[derive(Deserialize)]
struct RawHit {
    project_id: String,
    slug: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    author: String,
    icon_url: Option<String>,
    #[serde(default)]
    downloads: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    #[serde(default)]
    pub name: String,
    pub version_number: String,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    pub files: Vec<VFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VFile {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    pub hashes: HashMap<String, String>,
    #[serde(default)]
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub dependency_type: String,
}

fn base() -> String {
    format!("{}/v2", super::urls::modrinth())
}

/// `kind` is "mod", "modpack", "resourcepack" or "shader".
pub async fn search(query: &str, kind: &str, mc: Option<&str>, loader: Option<Loader>) -> Result<Vec<Hit>> {
    let mut facets = vec![vec![format!("project_type:{kind}")]];
    if let Some(mc) = mc {
        facets.push(vec![format!("versions:{mc}")]);
    }
    if let Some(l) = loader.and_then(|l| l.modrinth()) {
        if kind == "mod" || kind == "modpack" {
            facets.push(vec![format!("categories:{l}")]);
        }
    }
    let r: SearchResp = super::http()
        .get(format!("{}/search", base()))
        .query(&[("query", query), ("limit", "30"), ("facets", &serde_json::to_string(&facets)?), ("index", "relevance")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .context("Modrinth search failed")?;
    Ok(r.hits
        .into_iter()
        .map(|h| Hit { id: h.project_id, slug: h.slug, title: h.title, description: h.description, author: h.author, icon_url: h.icon_url, downloads: h.downloads, source: "modrinth".into() })
        .collect())
}

pub async fn versions(project: &str, mc: Option<&str>, loader: Option<Loader>) -> Result<Vec<Version>> {
    let mut q: Vec<(&str, String)> = vec![];
    if let Some(l) = loader.and_then(|l| l.modrinth()) {
        q.push(("loaders", serde_json::to_string(&[l])?));
    }
    if let Some(mc) = mc {
        q.push(("game_versions", serde_json::to_string(&[mc])?));
    }
    Ok(super::http().get(format!("{}/project/{project}/version", base())).query(&q).send().await?.error_for_status()?.json().await?)
}

pub async fn version(id: &str) -> Result<Version> {
    Ok(super::http().get(format!("{}/version/{id}", base())).send().await?.error_for_status()?.json().await?)
}

/// sha1 → Modrinth version, for files Modrinth knows about.
pub async fn versions_by_sha1(hashes: &[String]) -> Result<HashMap<String, Version>> {
    if hashes.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(super::http()
        .post(format!("{}/version_files", base()))
        .json(&serde_json::json!({"hashes": hashes, "algorithm": "sha1"}))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectInfo {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub slug: String,
}

pub async fn projects(ids: &[String]) -> Result<Vec<ProjectInfo>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    Ok(super::http().get(format!("{}/projects", base())).query(&[("ids", serde_json::to_string(ids)?)]).send().await?.error_for_status()?.json().await?)
}

fn primary(v: &Version) -> Option<&VFile> {
    v.files.iter().find(|f| f.primary).or_else(|| v.files.first())
}

/// Folder in the game directory for a project type.
pub fn folder_for(kind: &str) -> &'static str {
    match kind {
        "resourcepack" => "resourcepacks",
        "shader" => "shaderpacks",
        _ => "mods",
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub file_name: String,
    pub dependencies_added: Vec<String>,
}

/// Install the newest compatible version of a project into an instance,
/// plus its required dependencies.
pub async fn install_project(instance_id: &str, project: &str, kind: &str) -> Result<Installed> {
    let inst = instance::get(instance_id)?;
    let loader = if kind == "mod" { Some(inst.loader) } else { None };
    let vs = versions(project, Some(&inst.mc_version), loader).await?;
    let v = vs.first().ok_or_else(|| anyhow!("No version of this project works with {} {}", inst.loader.label(), inst.mc_version))?;
    let dir = instance::game_dir(instance_id).join(folder_for(kind));
    let file = install_version_file(v, &dir).await?;

    // Required dependencies (one level deep is what real packs need; deeper
    // chains are followed too, guarded against cycles).
    let mut added = Vec::new();
    let mut seen: HashSet<String> = HashSet::from([project.to_string()]);
    let installed_ids: HashSet<String> = instance::mods(instance_id)?
        .into_iter()
        .filter_map(|m| m.meta.map(|x| x.id))
        .collect();
    let mut queue: Vec<Dependency> = v.dependencies.iter().filter(|d| d.dependency_type == "required").cloned().collect();
    while let Some(d) = queue.pop() {
        let dv = match (&d.version_id, &d.project_id) {
            (Some(vid), _) => version(vid).await.ok(),
            (None, Some(pid)) if seen.insert(pid.clone()) => versions(pid, Some(&inst.mc_version), Some(inst.loader)).await.ok().and_then(|v| v.into_iter().next()),
            _ => None,
        };
        let Some(dv) = dv else { continue };
        if installed_ids.contains(&dv.project_id) {
            continue;
        }
        let name = install_version_file(&dv, &instance::game_dir(instance_id).join("mods")).await?;
        added.push(name);
        queue.extend(dv.dependencies.iter().filter(|d| d.dependency_type == "required").cloned());
    }
    Ok(Installed { file_name: file, dependencies_added: added })
}

async fn install_version_file(v: &Version, dir: &std::path::Path) -> Result<String> {
    let f = primary(v).ok_or_else(|| anyhow!("that version has no files"))?;
    let name = super::safe_rel_path(&f.filename).and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string())).ok_or_else(|| anyhow!("bad file name"))?;
    fetch(&super::http(), &Job { url: f.url.clone(), dest: dir.join(&name), sha1: f.hashes.get("sha1").cloned(), sha512: f.hashes.get("sha512").cloned(), size: Some(f.size) }).await?;
    Ok(name)
}

/// Download a modpack version's .mrpack to a temp file.
pub async fn download_pack(version_id: &str) -> Result<std::path::PathBuf> {
    let v = version(version_id).await?;
    let f = v.files.iter().find(|f| f.filename.ends_with(".mrpack")).or_else(|| primary(&v)).ok_or_else(|| anyhow!("that version has no pack file"))?;
    let dest = super::dir().join("downloads").join(&f.filename);
    fetch(&super::http(), &Job { url: f.url.clone(), dest: dest.clone(), sha1: f.hashes.get("sha1").cloned(), sha512: None, size: Some(f.size) }).await?;
    Ok(dest)
}
