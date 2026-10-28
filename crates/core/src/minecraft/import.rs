//! Bring packs and instances in from elsewhere, without the other launcher:
//!
//! * Modrinth `.mrpack`
//! * CurseForge modpack `.zip` (manifest.json)
//! * Prism Launcher / MultiMC instance folders or exported `.zip`s
//! * CurseForge app instances already on disk

use super::download::{fetch_all, Job};
use super::instance::{self, Instance, ManualDownload, PackSource};
use super::loaders::Loader;
use super::{curseforge, progress, OnProgress};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub instance: Instance,
    pub warnings: Vec<String>,
}

/// Hosts Modrinth itself accepts in packs; anything else is flagged.
const MODRINTH_HOSTS: &[&str] = &["cdn.modrinth.com", "github.com", "raw.githubusercontent.com", "gitlab.com"];

fn open_zip(path: &Path) -> Result<zip::ZipArchive<std::fs::File>> {
    let f = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    zip::ZipArchive::new(f).context("that file isn't a valid zip archive")
}

fn read_text(z: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<String> {
    let mut e = z.by_name(name).ok()?;
    let mut s = String::new();
    e.read_to_string(&mut s).ok()?;
    Some(s)
}

/// Extract every entry under `prefix/` into `dest` (paths are validated).
fn extract_prefix(z: &mut zip::ZipArchive<std::fs::File>, prefix: &str, dest: &Path) -> Result<usize> {
    let prefix = format!("{}/", prefix.trim_end_matches('/'));
    let mut n = 0;
    for i in 0..z.len() {
        let mut e = z.by_index(i)?;
        let name = e.name().replace('\\', "/");
        let Some(rest) = name.strip_prefix(&prefix) else { continue };
        if rest.is_empty() || e.is_dir() {
            continue;
        }
        let Some(rel) = super::safe_rel_path(rest) else { continue };
        let out = dest.join(rel);
        std::fs::create_dir_all(out.parent().unwrap())?;
        let mut w = std::fs::File::create(&out)?;
        std::io::copy(&mut e, &mut w)?;
        n += 1;
    }
    Ok(n)
}

fn loader_from_id(id: &str) -> Result<(Loader, String)> {
    // CurseForge style: "neoforge-21.1.77", "forge-47.2.0", "fabric-0.16.9"
    for (p, l) in [("neoforge-", Loader::Neoforge), ("forge-", Loader::Forge), ("fabric-", Loader::Fabric)] {
        if let Some(v) = id.strip_prefix(p) {
            return Ok((l, v.to_string()));
        }
    }
    if id.starts_with("quilt") {
        bail!("Quilt packs aren't supported yet");
    }
    bail!("unknown mod loader \"{id}\"")
}

// ───────────────────────── Modrinth .mrpack ─────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrIndex {
    name: String,
    #[serde(default)]
    version_id: String,
    files: Vec<MrFile>,
    dependencies: HashMap<String, String>,
}
#[derive(Deserialize)]
struct MrFile {
    path: String,
    hashes: HashMap<String, String>,
    #[serde(default)]
    env: Option<HashMap<String, String>>,
    downloads: Vec<String>,
    #[serde(rename = "fileSize", default)]
    file_size: u64,
}

pub async fn mrpack(path: &Path, source: Option<PackSource>, on: &OnProgress) -> Result<ImportResult> {
    let mut z = open_zip(path)?;
    let idx: MrIndex = serde_json::from_str(&read_text(&mut z, "modrinth.index.json").ok_or_else(|| anyhow!("not a Modrinth pack (no modrinth.index.json)"))?)
        .context("reading modrinth.index.json")?;
    let mc = idx.dependencies.get("minecraft").ok_or_else(|| anyhow!("the pack doesn't say which Minecraft version it needs"))?.clone();
    let (loader, lv) = if let Some(v) = idx.dependencies.get("neoforge") {
        (Loader::Neoforge, Some(v.clone()))
    } else if let Some(v) = idx.dependencies.get("forge") {
        (Loader::Forge, Some(v.clone()))
    } else if let Some(v) = idx.dependencies.get("fabric-loader") {
        (Loader::Fabric, Some(v.clone()))
    } else if idx.dependencies.contains_key("quilt-loader") {
        bail!("Quilt packs aren't supported yet");
    } else {
        (Loader::Vanilla, None)
    };

    let mut inst = instance::create(&idx.name, &mc, loader, lv, default_memory())?;
    inst.source = source.or(Some(PackSource { platform: "modrinth".into(), project_id: None, version_id: Some(idx.version_id.clone()), name: Some(idx.name.clone()) }));
    instance::save(&inst)?;
    let game = instance::game_dir(&inst.id);

    let mut warnings = Vec::new();
    let mut jobs = Vec::new();
    for f in &idx.files {
        if f.env.as_ref().and_then(|e| e.get("client")).map(|c| c == "unsupported").unwrap_or(false) {
            continue;
        }
        let Some(rel) = super::safe_rel_path(&f.path) else {
            warnings.push(format!("Skipped {} (unsafe path)", f.path));
            continue;
        };
        let Some(url) = f.downloads.iter().find(|u| super::download::is_secure_url(u)) else {
            warnings.push(format!("Skipped {} (no secure download link)", f.path));
            continue;
        };
        if let Ok(u) = url::Url::parse(url) {
            let host = u.host_str().unwrap_or_default();
            if !MODRINTH_HOSTS.contains(&host) {
                warnings.push(format!("{} downloads from {host}, which Modrinth doesn't vet", f.path));
            }
        }
        if !f.hashes.contains_key("sha1") && !f.hashes.contains_key("sha512") {
            warnings.push(format!("Skipped {} (no checksum to verify it)", f.path));
            continue;
        }
        jobs.push(Job { url: url.clone(), dest: game.join(rel), sha1: f.hashes.get("sha1").cloned(), sha512: f.hashes.get("sha512").cloned(), size: Some(f.file_size) });
    }
    let task = inst.id.clone();
    let on2 = on.clone();
    fetch_all(jobs, move |d, n| progress(&on2, &task, "Downloading pack files", d, n)).await?;
    extract_prefix(&mut z, "overrides", &game)?;
    extract_prefix(&mut z, "client-overrides", &game)?;
    Ok(ImportResult { instance: inst, warnings })
}

// ───────────────────────── CurseForge .zip ─────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfManifest {
    minecraft: CfMc,
    #[serde(default)]
    name: String,
    #[serde(default)]
    files: Vec<CfManFile>,
    #[serde(default = "default_overrides")]
    overrides: String,
}
fn default_overrides() -> String {
    "overrides".into()
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMc {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<CfLoader>,
}
#[derive(Deserialize)]
struct CfLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfManFile {
    #[serde(rename = "projectID")]
    #[allow(dead_code)]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
    #[serde(default = "yes")]
    required: bool,
}
fn yes() -> bool {
    true
}

pub async fn curseforge_zip(path: &Path, key: &str, source: Option<PackSource>, on: &OnProgress) -> Result<ImportResult> {
    let mut z = open_zip(path)?;
    let man: CfManifest = serde_json::from_str(&read_text(&mut z, "manifest.json").ok_or_else(|| anyhow!("not a CurseForge pack (no manifest.json)"))?)
        .context("reading manifest.json")?;
    let primary = man.minecraft.mod_loaders.iter().find(|l| l.primary).or_else(|| man.minecraft.mod_loaders.first());
    let (loader, lv) = match primary {
        Some(l) => {
            let (l, v) = loader_from_id(&l.id)?;
            (l, Some(v))
        }
        None => (Loader::Vanilla, None),
    };
    let name = if man.name.is_empty() { "CurseForge pack".to_string() } else { man.name.clone() };
    let mut inst = instance::create(&name, &man.minecraft.version, loader, lv, default_memory())?;
    inst.source = source.or(Some(PackSource { platform: "curseforge".into(), project_id: None, version_id: None, name: Some(name.clone()) }));
    let game = instance::game_dir(&inst.id);
    let task = inst.id.clone();

    let required: Vec<&CfManFile> = man.files.iter().filter(|f| f.required).collect();
    let mut warnings = Vec::new();
    if !required.is_empty() {
        progress(on, &task, "Looking up pack files on CurseForge", 0, 1);
        let ids: Vec<u64> = required.iter().map(|f| f.file_id).collect();
        let files = curseforge::files_by_id(key, &ids).await?;
        let mod_ids: Vec<u64> = files.iter().map(|f| f.mod_id).collect();
        let mods: HashMap<u64, curseforge::CfMod> = curseforge::mods_by_id(key, &mod_ids).await.unwrap_or_default().into_iter().map(|m| (m.id, m)).collect();
        let mut jobs = Vec::new();
        for f in &files {
            let m = mods.get(&f.mod_id);
            let folder = match m.and_then(|m| m.class_id) {
                Some(curseforge::CLASS_RESOURCEPACKS) => "resourcepacks",
                Some(curseforge::CLASS_SHADERS) => "shaderpacks",
                _ => "mods",
            };
            let Some(rel) = super::safe_rel_path(&f.file_name) else { continue };
            match &f.download_url {
                Some(url) => jobs.push(Job { url: url.clone(), dest: game.join(folder).join(rel), sha1: f.sha1(), sha512: None, size: Some(f.file_length) }),
                None => inst.manual_downloads.push(ManualDownload {
                    name: m.map(|m| m.name.clone()).unwrap_or_else(|| f.display_name.clone()),
                    file_name: f.file_name.clone(),
                    page_url: curseforge::page_url(m, f.id),
                    folder: folder.into(),
                }),
            }
        }
        if files.len() < required.len() {
            warnings.push(format!("CurseForge couldn't find {} of the pack's files", required.len() - files.len()));
        }
        let on2 = on.clone();
        let t = task.clone();
        fetch_all(jobs, move |d, n| progress(&on2, &t, "Downloading pack files", d, n)).await?;
    }
    extract_prefix(&mut z, &man.overrides, &game)?;
    if !inst.manual_downloads.is_empty() {
        warnings.push(format!(
            "{} file(s) can only be downloaded from CurseForge's website. They're listed on the instance page.",
            inst.manual_downloads.len()
        ));
    }
    instance::save(&inst)?;
    Ok(ImportResult { instance: inst, warnings })
}

// ───────────────────────── Prism / MultiMC ─────────────────────────

#[derive(Deserialize)]
struct MmcPack {
    components: Vec<MmcComponent>,
}
#[derive(Deserialize)]
struct MmcComponent {
    uid: String,
    #[serde(default)]
    version: Option<String>,
}

fn parse_ini(s: &str) -> HashMap<String, String> {
    s.lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

fn find_instance_root(dir: &Path, depth: u32) -> Option<PathBuf> {
    if dir.join("instance.cfg").exists() && dir.join("mmc-pack.json").exists() {
        return Some(dir.to_path_buf());
    }
    if depth == 0 {
        return None;
    }
    std::fs::read_dir(dir).ok()?.flatten().filter(|e| e.path().is_dir()).find_map(|e| find_instance_root(&e.path(), depth - 1))
}

pub fn prism_folder(dir: &Path) -> Result<ImportResult> {
    let root = find_instance_root(dir, 2).ok_or_else(|| anyhow!("no Prism or MultiMC instance found there (looking for instance.cfg and mmc-pack.json)"))?;
    let cfg = parse_ini(&std::fs::read_to_string(root.join("instance.cfg"))?);
    let pack: MmcPack = serde_json::from_str(&std::fs::read_to_string(root.join("mmc-pack.json"))?)?;
    let mut mc = None;
    let mut loader = (Loader::Vanilla, None);
    let mut warnings = Vec::new();
    for c in &pack.components {
        match c.uid.as_str() {
            "net.minecraft" => mc = c.version.clone(),
            "net.fabricmc.fabric-loader" => loader = (Loader::Fabric, c.version.clone()),
            "net.neoforged" => loader = (Loader::Neoforge, c.version.clone()),
            "net.minecraftforge" => loader = (Loader::Forge, c.version.clone()),
            "org.quiltmc.quilt-loader" => bail!("Quilt instances aren't supported yet"),
            "org.lwjgl" | "org.lwjgl3" | "net.fabricmc.intermediary" => {}
            other => warnings.push(format!("Ignored component {other}")),
        }
    }
    let mc = mc.ok_or_else(|| anyhow!("the instance doesn't list a Minecraft version"))?;
    let name = cfg.get("name").cloned().unwrap_or_else(|| root.file_name().unwrap().to_string_lossy().to_string());
    let mem = cfg.get("MaxMemAlloc").and_then(|v| v.parse().ok()).unwrap_or_else(default_memory);
    let mut inst = instance::create(&name, &mc, loader.0, loader.1, mem)?;
    inst.java_args = cfg.get("JvmArgs").cloned().unwrap_or_default();
    inst.source = Some(PackSource { platform: "prism".into(), project_id: None, version_id: None, name: Some(name.clone()) });
    let src = [".minecraft", "minecraft"].iter().map(|d| root.join(d)).find(|p| p.is_dir());
    match src {
        Some(s) => instance::copy_dir(&s, &instance::game_dir(&inst.id))?,
        None => warnings.push("The instance had no game folder to copy".into()),
    }
    instance::save(&inst)?;
    Ok(ImportResult { instance: inst, warnings })
}

fn prism_zip(path: &Path) -> Result<ImportResult> {
    let tmp = super::dir().join("import-tmp").join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&tmp)?;
    let mut z = open_zip(path)?;
    for i in 0..z.len() {
        let mut e = z.by_index(i)?;
        if e.is_dir() {
            continue;
        }
        let Some(rel) = super::safe_rel_path(e.name()) else { continue };
        let out = tmp.join(rel);
        std::fs::create_dir_all(out.parent().unwrap())?;
        std::io::copy(&mut e, &mut std::fs::File::create(&out)?)?;
    }
    let r = prism_folder(&tmp);
    let _ = std::fs::remove_dir_all(&tmp);
    r
}

// ───────────────────────── CurseForge app instances ─────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfAppInstance {
    name: String,
    game_version: String,
    #[serde(default)]
    base_mod_loader: Option<CfAppLoader>,
}
#[derive(Deserialize)]
struct CfAppLoader {
    name: String,
}

pub fn curseforge_app_folder(dir: &Path) -> Result<ImportResult> {
    let ci: CfAppInstance = serde_json::from_str(&std::fs::read_to_string(dir.join("minecraftinstance.json")).context("not a CurseForge app instance")?)?;
    let (loader, lv) = match &ci.base_mod_loader {
        Some(l) => {
            let (l, v) = loader_from_id(&l.name)?;
            (l, Some(v))
        }
        None => (Loader::Vanilla, None),
    };
    let mut inst = instance::create(&ci.name, &ci.game_version, loader, lv, default_memory())?;
    inst.source = Some(PackSource { platform: "curseforge".into(), project_id: None, version_id: None, name: Some(ci.name.clone()) });
    let game = instance::game_dir(&inst.id);
    for sub in ["mods", "config", "resourcepacks", "shaderpacks", "saves", "kubejs", "defaultconfigs", "scripts", "options.txt", "servers.dat"] {
        let p = dir.join(sub);
        if p.is_dir() {
            instance::copy_dir(&p, &game.join(sub))?;
        } else if p.is_file() {
            std::fs::copy(&p, game.join(sub))?;
        }
    }
    instance::save(&inst)?;
    Ok(ImportResult { instance: inst, warnings: vec![] })
}

/// Instances other launchers keep in their default folders.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub launcher: String,
    pub name: String,
    pub path: String,
}

pub fn discover() -> Vec<Found> {
    let mut out = Vec::new();
    let mut prism_roots = vec![];
    if let Some(d) = dirs::data_dir() {
        prism_roots.push(("Prism Launcher", d.join("PrismLauncher").join("instances")));
    }
    if let Some(d) = dirs::data_local_dir() {
        prism_roots.push(("Prism Launcher", d.join("PrismLauncher").join("instances")));
    }
    for (label, root) in prism_roots {
        for e in std::fs::read_dir(&root).into_iter().flatten().flatten() {
            let p = e.path();
            if p.join("instance.cfg").exists() {
                let cfg = std::fs::read_to_string(p.join("instance.cfg")).map(|s| parse_ini(&s)).unwrap_or_default();
                out.push(Found { launcher: label.into(), name: cfg.get("name").cloned().unwrap_or_else(|| e.file_name().to_string_lossy().into()), path: p.to_string_lossy().into() });
            }
        }
    }
    if let Some(home) = dirs::home_dir() {
        for e in std::fs::read_dir(home.join("curseforge").join("minecraft").join("Instances")).into_iter().flatten().flatten() {
            if e.path().join("minecraftinstance.json").exists() {
                out.push(Found { launcher: "CurseForge app".into(), name: e.file_name().to_string_lossy().into(), path: e.path().to_string_lossy().into() });
            }
        }
    }
    out
}

/// Import whatever `path` is.
pub async fn any(path: &Path, cf_key: &str, on: &OnProgress) -> Result<ImportResult> {
    if path.is_dir() {
        if path.join("minecraftinstance.json").exists() {
            return curseforge_app_folder(path);
        }
        return prism_folder(path);
    }
    let lower = path.to_string_lossy().to_lowercase();
    if lower.ends_with(".mrpack") {
        return mrpack(path, None, on).await;
    }
    let names: Vec<String> = {
        let z = open_zip(path)?;
        z.file_names().map(String::from).collect()
    };
    if names.iter().any(|n| n == "modrinth.index.json") {
        mrpack(path, None, on).await
    } else if names.iter().any(|n| n == "manifest.json") {
        curseforge_zip(path, cf_key, None, on).await
    } else if names.iter().any(|n| n.ends_with("instance.cfg")) {
        prism_zip(path)
    } else {
        bail!("couldn't tell what kind of pack that is (expected a .mrpack, a CurseForge .zip, or a Prism/MultiMC export)")
    }
}

fn default_memory() -> u32 {
    crate::Settings::load().minecraft_default_memory_mb.max(1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prism_instance_import() {
        let (_g, t) = super::super::test_home();
        let src = t.path().join("prism/My Fabric");
        std::fs::create_dir_all(src.join(".minecraft/mods")).unwrap();
        std::fs::write(src.join("instance.cfg"), "InstanceType=OneSix\nname=My Fabric\nMaxMemAlloc=6144\nJvmArgs=-XX:+UseZGC\n").unwrap();
        std::fs::write(
            src.join("mmc-pack.json"),
            r#"{"components":[{"uid":"org.lwjgl3","version":"3.3.3"},{"uid":"net.minecraft","version":"1.21.1"},{"uid":"net.fabricmc.intermediary","version":"1.21.1"},{"uid":"net.fabricmc.fabric-loader","version":"0.16.9"}],"formatVersion":1}"#,
        )
        .unwrap();
        std::fs::write(src.join(".minecraft/mods/sodium.jar"), b"jar").unwrap();
        std::fs::write(src.join(".minecraft/options.txt"), b"fov:1").unwrap();

        let r = prism_folder(&t.path().join("prism")).unwrap();
        assert_eq!(r.instance.name, "My Fabric");
        assert_eq!(r.instance.loader, Loader::Fabric);
        assert_eq!(r.instance.loader_version.as_deref(), Some("0.16.9"));
        assert_eq!(r.instance.memory_mb, 6144);
        assert_eq!(r.instance.java_args, "-XX:+UseZGC");
        assert!(instance::game_dir(&r.instance.id).join("mods/sodium.jar").exists());
        assert!(instance::game_dir(&r.instance.id).join("options.txt").exists());
    }

    #[test]
    fn curseforge_app_instance_import() {
        let (_g, t) = super::super::test_home();
        let src = t.path().join("cf/All the Mods");
        std::fs::create_dir_all(src.join("mods")).unwrap();
        std::fs::write(src.join("mods/a.jar"), b"jar").unwrap();
        std::fs::write(src.join("minecraftinstance.json"), r#"{"name":"All the Mods","gameVersion":"1.21.1","baseModLoader":{"name":"neoforge-21.1.77"}}"#).unwrap();
        let r = curseforge_app_folder(&src).unwrap();
        assert_eq!(r.instance.loader, Loader::Neoforge);
        assert_eq!(r.instance.loader_version.as_deref(), Some("21.1.77"));
        assert!(instance::game_dir(&r.instance.id).join("mods/a.jar").exists());
    }

    #[test]
    fn loader_ids() {
        assert_eq!(loader_from_id("neoforge-21.1.77").unwrap(), (Loader::Neoforge, "21.1.77".into()));
        assert_eq!(loader_from_id("forge-47.2.0").unwrap().0, Loader::Forge);
        assert!(loader_from_id("quilt-0.26").is_err());
    }
}
