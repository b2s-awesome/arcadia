//! Instances: one self-contained game folder each, sharing libraries/assets.

use super::loaders::Loader;
use super::modmeta::{self, ModMeta};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackSource {
    /// "modrinth", "curseforge", "prism", "multimc" or "file".
    pub platform: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

/// A file a pack needs that its author doesn't allow other apps to download.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManualDownload {
    pub name: String,
    pub file_name: String,
    pub page_url: String,
    /// Folder inside the game directory it belongs in, e.g. "mods".
    pub folder: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    pub loader: Loader,
    #[serde(default)]
    pub loader_version: Option<String>,
    /// Version id to launch, once the loader is installed.
    #[serde(default)]
    pub version_id: Option<String>,
    pub memory_mb: u32,
    #[serde(default)]
    pub java_args: String,
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub last_played: Option<u64>,
    #[serde(default)]
    pub playtime_secs: u64,
    #[serde(default)]
    pub source: Option<PackSource>,
    #[serde(default)]
    pub manual_downloads: Vec<ManualDownload>,
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn instance_dir(id: &str) -> PathBuf {
    super::instances_dir().join(id)
}

pub fn game_dir(id: &str) -> PathBuf {
    instance_dir(id).join("minecraft")
}

fn slug(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if s.is_empty() { "instance".into() } else { s.chars().take(40).collect() }
}

fn unique_id(name: &str) -> String {
    let base = slug(name);
    let mut id = base.clone();
    let mut i = 2;
    while instance_dir(&id).exists() {
        id = format!("{base}-{i}");
        i += 1;
    }
    id
}

pub fn list() -> Vec<Instance> {
    let mut v: Vec<Instance> = std::fs::read_dir(super::instances_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| get(&e.file_name().to_string_lossy()).ok())
        .collect();
    v.sort_by(|a, b| b.last_played.unwrap_or(b.created).cmp(&a.last_played.unwrap_or(a.created)));
    v
}

pub fn get(id: &str) -> Result<Instance> {
    if id.contains(['/', '\\']) || id.contains("..") {
        bail!("invalid instance id");
    }
    let s = std::fs::read_to_string(instance_dir(id).join("instance.json")).with_context(|| format!("no instance called {id}"))?;
    Ok(serde_json::from_str(&s)?)
}

pub fn save(i: &Instance) -> Result<()> {
    let d = instance_dir(&i.id);
    std::fs::create_dir_all(d.join("minecraft"))?;
    let tmp = d.join("instance.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(i)?)?;
    std::fs::rename(tmp, d.join("instance.json"))?;
    Ok(())
}

pub fn create(name: &str, mc: &str, loader: Loader, loader_version: Option<String>, memory_mb: u32) -> Result<Instance> {
    if name.trim().is_empty() {
        bail!("Give the instance a name");
    }
    if loader != Loader::Vanilla && loader_version.is_none() {
        bail!("Pick a {} version", loader.label());
    }
    let i = Instance {
        id: unique_id(name),
        name: name.trim().to_string(),
        mc_version: mc.to_string(),
        loader,
        loader_version,
        version_id: None,
        memory_mb: memory_mb.max(512),
        java_args: String::new(),
        created: now(),
        last_played: None,
        playtime_secs: 0,
        source: None,
        manual_downloads: vec![],
    };
    save(&i)?;
    for sub in ["mods", "resourcepacks", "shaderpacks", "config", "saves"] {
        std::fs::create_dir_all(game_dir(&i.id).join(sub))?;
    }
    Ok(i)
}

pub fn delete(id: &str) -> Result<()> {
    get(id)?; // validates the id
    std::fs::remove_dir_all(instance_dir(id)).context("couldn't delete the instance folder (is the game still running?)")
}

pub fn duplicate(id: &str, new_name: &str) -> Result<Instance> {
    let mut i = get(id)?;
    i.id = unique_id(new_name);
    i.name = new_name.to_string();
    i.created = now();
    i.last_played = None;
    i.playtime_secs = 0;
    copy_dir(&game_dir(id), &game_dir(&i.id))?;
    save(&i)?;
    Ok(i)
}

pub fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let to = dst.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &to)?;
        } else {
            std::fs::copy(e.path(), to)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModFile {
    pub file_name: String,
    pub enabled: bool,
    pub size: u64,
    pub meta: Option<ModMeta>,
}

/// Mods in an instance (`.jar`, and `.jar.disabled` for turned-off ones).
pub fn mods(id: &str) -> Result<Vec<ModFile>> {
    let dir = game_dir(id).join("mods");
    let mut out = Vec::new();
    for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let enabled = name.ends_with(".jar");
        if !enabled && !name.ends_with(".jar.disabled") {
            continue;
        }
        let size = e.metadata().map(|m| m.len()).unwrap_or(0);
        out.push(ModFile { meta: modmeta::read(&e.path()), file_name: name, enabled, size });
    }
    out.sort_by_key(|m| m.meta.as_ref().map(|x| x.name.to_lowercase()).unwrap_or_else(|| m.file_name.to_lowercase()));
    Ok(out)
}

fn mod_path(id: &str, file_name: &str) -> Result<PathBuf> {
    if file_name.contains(['/', '\\']) || file_name.contains("..") {
        bail!("invalid file name");
    }
    let p = game_dir(id).join("mods").join(file_name);
    if !p.exists() {
        return Err(anyhow!("{file_name} isn't in this instance"));
    }
    Ok(p)
}

pub fn set_mod_enabled(id: &str, file_name: &str, enabled: bool) -> Result<String> {
    let p = mod_path(id, file_name)?;
    let new = match (enabled, file_name.strip_suffix(".disabled")) {
        (true, Some(base)) => base.to_string(),
        (false, None) => format!("{file_name}.disabled"),
        _ => return Ok(file_name.to_string()),
    };
    std::fs::rename(&p, p.with_file_name(&new))?;
    Ok(new)
}

pub fn remove_mod(id: &str, file_name: &str) -> Result<()> {
    std::fs::remove_file(mod_path(id, file_name)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_lifecycle_and_mod_toggle() {
        let (_g, _t) = super::super::test_home();
        let i = create("My Pack!", "1.21.1", Loader::Fabric, Some("0.16.9".into()), 4096).unwrap();
        assert_eq!(i.id, "my-pack");
        assert_eq!(create("My Pack!", "1.21.1", Loader::Vanilla, None, 2048).unwrap().id, "my-pack-2");
        assert!(create("x", "1.21.1", Loader::Neoforge, None, 2048).is_err());

        let jar = game_dir(&i.id).join("mods/sodium.jar");
        super::super::modmeta::tests::make_jar(&jar, &[("fabric.mod.json", r#"{"id":"sodium","name":"Sodium","version":"0.6"}"#)]);
        let m = mods(&i.id).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].meta.as_ref().unwrap().name, "Sodium");

        let off = set_mod_enabled(&i.id, "sodium.jar", false).unwrap();
        assert_eq!(off, "sodium.jar.disabled");
        assert!(!mods(&i.id).unwrap()[0].enabled);
        set_mod_enabled(&i.id, &off, true).unwrap();
        assert!(mods(&i.id).unwrap()[0].enabled);
        assert!(set_mod_enabled(&i.id, "../../evil.jar", true).is_err());

        assert_eq!(list().len(), 2);
        delete("my-pack-2").unwrap();
        assert!(get("../etc").is_err());
    }
}
