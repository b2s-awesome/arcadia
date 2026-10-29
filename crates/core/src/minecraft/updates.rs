//! Updates for an instance's mods, resource packs and shaders, from Modrinth.
//! Files are matched by their SHA-1, so this works for anything that came
//! from Modrinth, whoever installed it.

use super::download::{fetch, sha1_file, Job};
use super::instance::{self, game_dir};
use super::modrinth::{self, Version};
use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Update {
    /// "mods", "resourcepacks" or "shaderpacks".
    pub folder: String,
    pub file_name: String,
    pub project_id: String,
    pub title: String,
    pub icon: Option<String>,
    pub current: String,
    pub latest: String,
    pub latest_id: String,
    pub latest_file: String,
}

fn files_in(dir: &std::path::Path, exts: &[&str]) -> Vec<(String, std::path::PathBuf)> {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_file())
                .filter_map(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    let base = n.trim_end_matches(".disabled").to_ascii_lowercase();
                    exts.iter().any(|x| base.ends_with(x)).then(|| (n, e.path()))
                })
                .collect()
        })
        .unwrap_or_default()
}

async fn latest_for(hashes: &[String], loaders: &[&str], mc: &str) -> Result<HashMap<String, Version>> {
    if hashes.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(super::http()
        .post(format!("{}/v2/version_files/update", super::urls::modrinth()))
        .json(&serde_json::json!({ "hashes": hashes, "algorithm": "sha1", "loaders": loaders, "game_versions": [mc] }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

fn primary(v: &Version) -> Option<&modrinth::VFile> {
    v.files.iter().find(|f| f.primary).or_else(|| v.files.first())
}

/// Everything in the instance that has a newer Modrinth version for this
/// Minecraft version and loader.
pub async fn check(id: &str) -> Result<Vec<Update>> {
    let inst = instance::get(id)?;
    let gd = game_dir(id);
    let groups: Vec<(&str, Vec<(String, std::path::PathBuf)>, Vec<&str>)> = vec![
        ("mods", files_in(&gd.join("mods"), &[".jar"]), vec![inst.loader.modrinth().unwrap_or("minecraft")]),
        ("resourcepacks", files_in(&gd.join("resourcepacks"), &[".zip"]), vec!["minecraft"]),
        ("shaderpacks", files_in(&gd.join("shaderpacks"), &[".zip"]), vec!["iris", "optifine", "canvas"]),
    ];
    let mut out = Vec::new();
    for (folder, files, loaders) in groups {
        if files.is_empty() || (folder == "mods" && inst.loader == super::loaders::Loader::Vanilla) {
            continue;
        }
        let mut by_hash = HashMap::new();
        for (name, path) in &files {
            if let Ok(h) = sha1_file(path) {
                by_hash.insert(h, name.clone());
            }
        }
        let hashes: Vec<String> = by_hash.keys().cloned().collect();
        let current = modrinth::versions_by_sha1(&hashes).await.unwrap_or_default();
        let latest = latest_for(&hashes, &loaders, &inst.mc_version).await?;
        for (hash, v) in latest {
            let Some(f) = primary(&v) else { continue };
            if f.hashes.get("sha1") == Some(&hash) {
                continue;
            }
            let Some(name) = by_hash.get(&hash) else { continue };
            let cur = current.get(&hash);
            // Only offer a newer version of the same project.
            if cur.is_some_and(|c| c.project_id != v.project_id || c.id == v.id) {
                continue;
            }
            out.push(Update {
                folder: folder.into(),
                file_name: name.clone(),
                project_id: v.project_id.clone(),
                title: String::new(),
                icon: None,
                current: cur.map(|c| c.version_number.clone()).unwrap_or_default(),
                latest: v.version_number.clone(),
                latest_id: v.id.clone(),
                latest_file: f.filename.clone(),
            });
        }
    }
    // Names and icons, in one request.
    let ids: Vec<String> = out.iter().map(|u| u.project_id.clone()).collect();
    if let Ok(ps) = project_cards(&ids).await {
        for u in &mut out {
            if let Some((t, i)) = ps.get(&u.project_id) {
                u.title = t.clone();
                u.icon = i.clone();
            }
        }
    }
    for u in &mut out {
        if u.title.is_empty() {
            u.title = u.file_name.trim_end_matches(".disabled").to_string();
        }
    }
    out.sort_by_key(|u| (u.folder.clone(), u.title.to_lowercase()));
    Ok(out)
}

async fn project_cards(ids: &[String]) -> Result<HashMap<String, (String, Option<String>)>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    #[derive(serde::Deserialize)]
    struct P {
        id: String,
        title: String,
        icon_url: Option<String>,
    }
    let ps: Vec<P> = super::http()
        .get(format!("{}/v2/projects", super::urls::modrinth()))
        .query(&[("ids", serde_json::to_string(ids)?)])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(ps.into_iter().map(|p| (p.id, (p.title, p.icon_url))).collect())
}

/// Swap one file for its newer version, keeping it on or off as it was.
pub async fn apply(id: &str, u: &Update) -> Result<String> {
    if !["mods", "resourcepacks", "shaderpacks"].contains(&u.folder.as_str()) || u.file_name.contains(['/', '\\']) {
        bail!("Not an update Arcadia offered");
    }
    let v = modrinth::version(&u.latest_id).await?;
    if v.project_id != u.project_id {
        bail!("That version belongs to another project");
    }
    let f = primary(&v).ok_or_else(|| anyhow!("That version has no files"))?;
    let name = super::safe_rel_path(&f.filename).and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string())).ok_or_else(|| anyhow!("bad file name"))?;
    let dir = game_dir(id).join(&u.folder);
    let old = dir.join(&u.file_name);
    let was_off = u.file_name.ends_with(".disabled");
    let new_name = if was_off { format!("{name}.disabled") } else { name.clone() };
    let tmp = dir.join(format!("{name}.download"));
    fetch(&super::http(), &Job { url: f.url.clone(), dest: tmp.clone(), sha1: f.hashes.get("sha1").cloned(), sha512: f.hashes.get("sha512").cloned(), size: Some(f.size) }).await?;
    if old.exists() {
        std::fs::remove_file(&old)?;
    }
    std::fs::rename(&tmp, dir.join(&new_name))?;
    Ok(new_name)
}
