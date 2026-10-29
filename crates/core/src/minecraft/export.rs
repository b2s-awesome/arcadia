//! Export an instance as a Modrinth pack (`.mrpack`): files Modrinth hosts
//! are listed by link and checksum, everything else goes in `overrides/`.
//! A "full" export puts every file in `overrides/`, so it works offline.
//! Arcadia, Prism and the Modrinth app all import either.

use super::download::sha1_file;
use super::instance::{self, game_dir};
use super::loaders::Loader;
use anyhow::{bail, Result};
use serde::Deserialize;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    /// Link to Modrinth downloads instead of packing the files in.
    pub link_downloads: bool,
    pub include_config: bool,
    pub include_worlds: bool,
    /// options.txt: keybinds, video settings.
    pub include_options: bool,
    pub include_resourcepacks: bool,
    pub include_shaderpacks: bool,
    pub version: String,
}

/// Folders and files never worth sharing.
const SKIP: &[&str] = &["logs", "crash-reports", "screenshots", ".fabric", ".mixin.out", "debug", "natives", "backups", "usercache.json", "usernamecache.json"];

fn walk(base: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(base, &p, out);
        } else if let Ok(rel) = p.strip_prefix(base) {
            out.push(rel.to_path_buf());
        }
    }
}

fn wanted(rel: &Path, o: &ExportOptions) -> bool {
    let s = rel.to_string_lossy().replace('\\', "/");
    let top = s.split('/').next().unwrap_or("");
    if SKIP.contains(&top) || s.ends_with(".disabled") || s.ends_with(".part") || s.ends_with(".download") {
        return false;
    }
    match top {
        "mods" => true,
        "config" | "defaultconfigs" | "kubejs" => o.include_config,
        "saves" => o.include_worlds && !s.ends_with("session.lock"),
        "resourcepacks" => o.include_resourcepacks,
        "shaderpacks" => o.include_shaderpacks,
        "options.txt" | "optionsof.txt" | "optionsshaders.txt" => o.include_options,
        "servers.dat" => true,
        _ => o.include_config && !top.ends_with(".log"),
    }
}

/// Build the pack at `dest`. Returns how many files were linked and packed.
pub async fn export(id: &str, dest: &Path, o: &ExportOptions) -> Result<(usize, usize)> {
    let inst = instance::get(id)?;
    let gd = game_dir(id);
    let mut files = Vec::new();
    walk(&gd, &gd, &mut files);
    files.retain(|r| wanted(r, o));
    files.sort();

    // Which files can be linked to Modrinth instead of packed in.
    let linkable: Vec<(PathBuf, String)> = if o.link_downloads {
        files
            .iter()
            .filter(|r| {
                let s = r.to_string_lossy().replace('\\', "/");
                ["mods/", "resourcepacks/", "shaderpacks/"].iter().any(|p| s.starts_with(p)) && !s[s.find('/').unwrap() + 1..].contains('/')
            })
            .filter_map(|r| Some((r.clone(), sha1_file(&gd.join(r)).ok()?)))
            .collect()
    } else {
        Vec::new()
    };
    let hashes: Vec<String> = linkable.iter().map(|(_, h)| h.clone()).collect();
    let known = super::modrinth::versions_by_sha1(&hashes).await.unwrap_or_default();

    let mut index_files = Vec::new();
    let mut linked = std::collections::HashSet::new();
    for (rel, h) in &linkable {
        let Some(v) = known.get(h) else { continue };
        let Some(f) = v.files.iter().find(|f| f.hashes.get("sha1") == Some(h)) else { continue };
        let Some(sha512) = f.hashes.get("sha512") else { continue };
        let path = rel.to_string_lossy().replace('\\', "/");
        index_files.push(serde_json::json!({
            "path": path,
            "hashes": { "sha1": h, "sha512": sha512 },
            "env": { "client": "required", "server": if path.starts_with("mods/") { "required" } else { "unsupported" } },
            "downloads": [f.url],
            "fileSize": f.size,
        }));
        linked.insert(rel.clone());
    }

    let mut deps = serde_json::Map::new();
    deps.insert("minecraft".into(), inst.mc_version.clone().into());
    if let Some(lv) = &inst.loader_version {
        let key = match inst.loader {
            Loader::Fabric => Some("fabric-loader"),
            Loader::Forge => Some("forge"),
            Loader::Neoforge => Some("neoforge"),
            Loader::Vanilla => None,
        };
        if let Some(k) = key {
            deps.insert(k.into(), lv.clone().into());
        }
    }
    let index = serde_json::json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": if o.version.trim().is_empty() { "1.0.0" } else { o.version.trim() },
        "name": inst.name,
        "summary": "Exported from Arcadia",
        "files": index_files,
        "dependencies": deps,
    });

    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p)?;
    }
    let tmp = dest.with_extension("part");
    let mut z = zip::ZipWriter::new(std::fs::File::create(&tmp)?);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(true);
    z.start_file("modrinth.index.json", opts)?;
    z.write_all(serde_json::to_string_pretty(&index)?.as_bytes())?;
    let mut packed = 0;
    for rel in files.iter().filter(|r| !linked.contains(*r)) {
        let name = format!("overrides/{}", rel.to_string_lossy().replace('\\', "/"));
        z.start_file(name, opts)?;
        let mut f = std::fs::File::open(gd.join(rel))?;
        std::io::copy(&mut f, &mut z)?;
        packed += 1;
    }
    z.finish()?.flush()?;
    if dest.exists() {
        std::fs::remove_file(dest)?;
    }
    std::fs::rename(&tmp, dest)?;
    if packed + linked.len() == 0 {
        bail!("There was nothing to export");
    }
    Ok((linked.len(), packed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn exports_a_pack_that_imports_again() {
        let (_g, t) = super::super::test_home();
        let i = instance::create("Export Me", "1.21.1", Loader::Fabric, Some("0.16.9".into()), 2048).unwrap();
        let gd = game_dir(&i.id);
        for (p, b) in [("mods/a.jar", "A"), ("mods/off.jar.disabled", "B"), ("config/a.toml", "x=1"), ("logs/latest.log", "L"), ("options.txt", "fov:1"), ("saves/W/level.dat", "D")] {
            std::fs::create_dir_all(gd.join(p).parent().unwrap()).unwrap();
            std::fs::write(gd.join(p), b).unwrap();
        }
        let o = ExportOptions { link_downloads: false, include_config: true, include_worlds: false, include_options: true, include_resourcepacks: true, include_shaderpacks: true, version: "2.0".into() };
        let dest = t.path().join("out/pack.mrpack");
        let (linked, packed) = export(&i.id, &dest, &o).await.unwrap();
        assert_eq!((linked, packed), (0, 3));
        let mut z = zip::ZipArchive::new(std::fs::File::open(&dest).unwrap()).unwrap();
        let names: Vec<String> = z.file_names().map(String::from).collect();
        assert!(names.contains(&"overrides/mods/a.jar".to_string()));
        assert!(names.contains(&"overrides/options.txt".to_string()));
        assert!(!names.iter().any(|n| n.contains("logs") || n.contains("saves") || n.ends_with(".disabled")));
        let idx: serde_json::Value = serde_json::from_reader(z.by_name("modrinth.index.json").unwrap()).unwrap();
        assert_eq!(idx["dependencies"]["fabric-loader"], "0.16.9");
        assert_eq!(idx["versionId"], "2.0");
    }
}
