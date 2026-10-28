//! Make sure everything a version needs is on disk: version JSON, client
//! jar, libraries, natives, assets and the (patched) logging config.

use super::download::{fetch, fetch_all, Job};
use super::version::{self, maven_path, Manifest, VersionJson};
use super::{progress, OnProgress};
use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Mojang's version list, cached so the launcher works offline once warmed.
pub async fn manifest() -> Result<Manifest> {
    let cache = super::versions_dir().join("version_manifest_v2.json");
    match async {
        let m: Manifest = super::http()
            .get(super::urls::version_manifest())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        anyhow::Ok(m)
    }
    .await
    {
        Ok(m) => {
            std::fs::create_dir_all(cache.parent().unwrap())?;
            std::fs::write(&cache, serde_json::to_vec(&m)?)?;
            Ok(m)
        }
        Err(e) => {
            let s = std::fs::read_to_string(&cache).map_err(|_| e)?;
            Ok(serde_json::from_str(&s)?)
        }
    }
}

/// Download `versions/<id>/<id>.json` for a vanilla version if needed.
pub async fn ensure_vanilla_json(id: &str) -> Result<()> {
    if version::local_path(id).exists() {
        return Ok(());
    }
    let m = manifest().await?;
    let e = m.versions.iter().find(|v| v.id == id).ok_or_else(|| anyhow!("Minecraft {id} doesn't exist"))?;
    let job = Job { url: e.url.clone(), dest: version::local_path(id), sha1: e.sha1.clone(), sha512: None, size: None };
    fetch(&super::http(), &job).await
}

/// What the launcher needs after installing.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub classpath: Vec<PathBuf>,
    pub native_jars: Vec<PathBuf>,
    pub asset_index: String,
    /// For very old versions whose assets must be laid out by name.
    pub legacy_assets: Option<LegacyAssets>,
    pub logging_arg: Option<String>,
}

#[derive(Debug, Clone)]
pub enum LegacyAssets {
    /// `assets/virtual/legacy` (1.6-ish).
    Virtual(PathBuf),
    /// Copy into `<game dir>/resources` (pre-1.6).
    MapToResources(PathBuf),
}

#[derive(Deserialize)]
struct AssetIndex {
    objects: HashMap<String, AssetObject>,
    #[serde(default, rename = "virtual")]
    is_virtual: bool,
    #[serde(default)]
    map_to_resources: bool,
}
#[derive(Deserialize)]
struct AssetObject {
    hash: String,
    size: u64,
}

fn lib_job(l: &version::Library) -> Option<(Job, bool)> {
    let libs = super::libraries_dir();
    if let Some(a) = l.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
        let path = a.path.clone().or_else(|| maven_path(&l.name))?;
        let rel = super::safe_rel_path(&path)?;
        // Forge/NeoForge installers generate some jars locally (empty URL).
        let local_only = a.url.is_empty();
        return Some((Job { url: a.url.clone(), dest: libs.join(rel), sha1: a.sha1.clone(), sha512: None, size: a.size }, local_only));
    }
    if l.downloads.as_ref().is_some_and(|d| d.classifiers.is_some()) && l.downloads.as_ref().unwrap().artifact.is_none() {
        return None; // natives-only entry
    }
    let path = maven_path(&l.name)?;
    let base = l.url.clone().unwrap_or_else(|| format!("{}/", super::urls::libraries()));
    let base = if base.ends_with('/') { base } else { format!("{base}/") };
    let rel = super::safe_rel_path(&path)?;
    Some((Job { url: format!("{base}{path}"), dest: libs.join(rel), sha1: l.sha1.clone(), sha512: None, size: l.size }, false))
}

fn native_job(l: &version::Library) -> Option<Job> {
    let classifier = l.natives.as_ref()?.get(version::os_name())?;
    let bits = if version::arch_name() == "x86" { "32" } else { "64" };
    let classifier = classifier.replace("${arch}", bits);
    let a = l.downloads.as_ref()?.classifiers.as_ref()?.get(&classifier)?;
    let path = a.path.clone().or_else(|| maven_path(&format!("{}:{classifier}", l.name)))?;
    Some(Job { url: a.url.clone(), dest: super::libraries_dir().join(super::safe_rel_path(&path)?), sha1: a.sha1.clone(), sha512: None, size: a.size })
}

/// Download everything `v` needs. `mc_version` names the vanilla version
/// whose client jar is used (loader versions share it).
pub async fn ensure_files(v: &VersionJson, mc_version: &str, task: &str, on: &OnProgress) -> Result<Prepared> {
    let features: HashSet<&str> = HashSet::new();
    let mut jobs = Vec::new();
    let mut classpath = Vec::new();
    let mut native_jars = Vec::new();

    for l in &v.libraries {
        if !version::allowed(l.rules.as_deref(), &features) {
            continue;
        }
        if let Some((job, local_only)) = lib_job(l) {
            classpath.push(job.dest.clone());
            if !local_only {
                jobs.push(job);
            } else if !job.dest.exists() {
                bail!("{} is missing; reinstall the mod loader", l.name);
            }
        }
        if let Some(n) = native_job(l) {
            native_jars.push(n.dest.clone());
            jobs.push(n);
        }
    }

    // Client jar.
    let client = super::versions_dir().join(mc_version).join(format!("{mc_version}.jar"));
    let dl = v.downloads.as_ref().and_then(|d| d.get("client")).ok_or_else(|| anyhow!("{mc_version} has no client download"))?;
    jobs.push(Job { url: dl.url.clone(), dest: client.clone(), sha1: dl.sha1.clone(), sha512: None, size: dl.size });
    classpath.push(client);

    // Logging config (Mojang's patched configs also close Log4Shell).
    let mut logging_arg = None;
    if let Some(lc) = v.logging.as_ref().and_then(|l| l.client.as_ref()) {
        let p = super::assets_dir().join("log_configs").join(super::safe_rel_path(&lc.file.id).context("bad logging id")?);
        jobs.push(Job { url: lc.file.url.clone(), dest: p.clone(), sha1: lc.file.sha1.clone(), sha512: None, size: lc.file.size });
        logging_arg = Some(lc.argument.replace("${path}", &p.to_string_lossy()));
    }

    // Asset index, then objects.
    let ai = v.asset_index.as_ref().ok_or_else(|| anyhow!("{} has no asset index", v.id))?;
    let index_path = super::assets_dir().join("indexes").join(format!("{}.json", ai.id));
    fetch(&super::http(), &Job { url: ai.url.clone(), dest: index_path.clone(), sha1: ai.sha1.clone(), sha512: None, size: None })
        .await
        .context("downloading the asset index")?;
    let index: AssetIndex = serde_json::from_slice(&std::fs::read(&index_path)?)?;
    let objects = super::assets_dir().join("objects");
    let res = super::urls::resources();
    for o in index.objects.values() {
        if o.hash.len() < 2 || !o.hash.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let pre = &o.hash[..2];
        jobs.push(Job {
            url: format!("{res}/{pre}/{}", o.hash),
            dest: objects.join(pre).join(&o.hash),
            sha1: Some(o.hash.clone()),
            sha512: None,
            size: Some(o.size),
        });
    }

    // Skip files already verified on disk before counting progress.
    let pending: Vec<Job> = tokio::task::spawn_blocking(move || jobs.into_iter().filter(|j| !super::download::is_valid(j)).collect())
        .await?;
    let on2 = on.clone();
    let t = task.to_string();
    fetch_all(pending, move |d, n| progress(&on2, &t, "Downloading game files", d, n)).await?;

    let legacy_assets = if index.is_virtual || index.map_to_resources {
        let dir = super::assets_dir().join("virtual").join(&ai.id);
        for (name, o) in &index.objects {
            let Some(rel) = super::safe_rel_path(name) else { continue };
            let dst = dir.join(rel);
            if !dst.exists() {
                std::fs::create_dir_all(dst.parent().unwrap())?;
                std::fs::copy(objects.join(&o.hash[..2]).join(&o.hash), &dst)?;
            }
        }
        Some(if index.map_to_resources { LegacyAssets::MapToResources(dir) } else { LegacyAssets::Virtual(dir) })
    } else {
        None
    };

    Ok(Prepared { classpath, native_jars, asset_index: ai.id.clone(), legacy_assets, logging_arg })
}

/// Unpack old-style native jars (LWJGL 2) into `dest`.
pub fn extract_natives(jars: &[PathBuf], dest: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for jar in jars {
        let f = std::fs::File::open(jar).with_context(|| format!("opening {}", jar.display()))?;
        let mut z = zip::ZipArchive::new(f)?;
        for i in 0..z.len() {
            let mut e = z.by_index(i)?;
            let name = e.name().to_string();
            if e.is_dir() || name.starts_with("META-INF/") {
                continue;
            }
            let Some(rel) = super::safe_rel_path(&name) else { continue };
            let out = dest.join(rel);
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut w = std::fs::File::create(&out)?;
            std::io::copy(&mut e, &mut w)?;
        }
    }
    Ok(())
}
