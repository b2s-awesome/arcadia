//! Backups of Arcadia's setup (settings, looks, library info, addon data,
//! Roblox history, skins, proxy rules) as one `.arcadia` file, and a sync
//! folder (OneDrive, Dropbox, a USB stick) that keeps the latest copy.
//!
//! Never included: Roblox and Minecraft logins (they live in Credential
//! Manager), addon secrets, downloaded games, and API keys unless asked.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const PARTS: &[(&str, &str)] = &[
    ("settings", "Settings, looks and layout"),
    ("library", "Library info, collections and your art"),
    ("addons", "Addon data"),
    ("roblox", "Roblox accounts list and play history"),
    ("skins", "Minecraft skin library"),
    ("proxy", "Asset proxy rules"),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub app: String,
    pub created: u64,
    pub computer: String,
    pub parts: Vec<String>,
    pub keys: bool,
    pub files: usize,
}

const KEY_FIELDS: &[&str] = &["curseforge_api_key", "steamgriddb_key"];

/// Files of a part, as (path inside the backup, path on disk).
fn files_of(part: &str) -> Vec<(String, PathBuf)> {
    let root = crate::paths::root();
    let mut out = Vec::new();
    let mut file = |inside: &str, p: PathBuf| {
        if p.is_file() {
            out.push((inside.to_string(), p));
        }
    };
    match part {
        "settings" => file("settings.json", crate::paths::settings_file()),
        "library" => {
            file("library-meta.json", root.join("library-meta.json"));
            file("library-custom.json", crate::paths::custom_library_file());
        }
        "roblox" => {
            file("Roblox/accounts.json", crate::paths::roblox_dir().join("accounts.json"));
            file("Roblox/history.json", crate::paths::roblox_dir().join("history.json"));
        }
        "skins" => file("Minecraft/skins/library.json", crate::paths::minecraft_dir().join("skins/library.json")),
        "proxy" => file("proxy/rules.json", crate::paths::proxy_dir().join("rules.json")),
        _ => {}
    }
    // Folders.
    let dirs: &[(&str, PathBuf)] = match part {
        "library" => &[("art", root.join("art"))],
        "addons" => &[("addons", crate::addons::root())],
        "skins" => &[("Minecraft/skins", crate::paths::minecraft_dir().join("skins"))],
        _ => &[],
    };
    for (inside, d) in dirs {
        walk(d, d, inside, &mut out);
    }
    out.sort();
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

fn walk(base: &Path, dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        // Addon secrets stay on this PC.
        if name == ".secrets" || name.ends_with(".tmp") {
            continue;
        }
        if p.is_dir() {
            walk(base, &p, prefix, out);
        } else if let Ok(rel) = p.strip_prefix(base) {
            out.push((format!("{prefix}/{}", rel.to_string_lossy().replace('\\', "/")), p));
        }
    }
}

fn computer() -> String {
    std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "this PC".into())
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn strip_keys(settings: &[u8]) -> Vec<u8> {
    let Ok(mut v) = serde_json::from_slice::<serde_json::Value>(settings) else { return settings.to_vec() };
    if let Some(o) = v.as_object_mut() {
        for k in KEY_FIELDS {
            if o.contains_key(*k) {
                o.insert((*k).into(), "".into());
            }
        }
        // The sync folder belongs to this PC.
        o.remove("sync_folder");
        o.remove("sync_last");
    }
    serde_json::to_vec_pretty(&v).unwrap_or_else(|_| settings.to_vec())
}

/// Write a backup of `parts` to `dest`.
pub fn export(dest: &Path, parts: &[String], keys: bool) -> Result<Manifest> {
    let parts: Vec<String> = parts.iter().filter(|p| PARTS.iter().any(|(id, _)| id == p)).cloned().collect();
    if parts.is_empty() {
        bail!("Pick at least one thing to back up");
    }
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p)?;
    }
    let tmp = dest.with_extension("part");
    let mut z = zip::ZipWriter::new(std::fs::File::create(&tmp)?);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut n = 0;
    for part in &parts {
        for (inside, path) in files_of(part) {
            let mut bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            if inside == "settings.json" && !keys {
                bytes = strip_keys(&bytes);
            }
            z.start_file(format!("{part}/{inside}"), opts)?;
            z.write_all(&bytes)?;
            n += 1;
        }
    }
    let m = Manifest { app: env!("CARGO_PKG_VERSION").into(), created: now(), computer: computer(), parts, keys, files: n };
    z.start_file("manifest.json", opts)?;
    z.write_all(&serde_json::to_vec_pretty(&m)?)?;
    z.finish()?.flush()?;
    std::fs::rename(&tmp, dest)?;
    Ok(m)
}

pub fn inspect(path: &Path) -> Result<Manifest> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(path)?).context("That isn't an Arcadia backup")?;
    let mut s = String::new();
    z.by_name("manifest.json").context("That isn't an Arcadia backup (no manifest)")?.take(64 * 1024).read_to_string(&mut s)?;
    Ok(serde_json::from_str(&s)?)
}

/// Where restored files go, for a path inside the backup.
fn target(part: &str, inside: &str) -> Option<PathBuf> {
    let rel = crate::minecraft::safe_rel_path(inside)?;
    let root = crate::paths::root();
    Some(match part {
        "settings" if inside == "settings.json" => crate::paths::settings_file(),
        "library" if inside.starts_with("art/") || inside.starts_with("library-") => root.join(rel),
        "addons" if inside.starts_with("addons/") => root.join(rel),
        "roblox" if inside.starts_with("Roblox/") => root.join(rel),
        "skins" if inside.starts_with("Minecraft/skins/") => root.join(rel),
        "proxy" if inside == "proxy/rules.json" => root.join(rel),
        _ => return None,
    })
}

/// Put `parts` of a backup back. What's there now is saved first to
/// `backups/before-restore-<time>.arcadia`.
pub fn restore(path: &Path, parts: &[String]) -> Result<usize> {
    let m = inspect(path)?;
    let wanted: Vec<&String> = parts.iter().filter(|p| m.parts.contains(p)).collect();
    if wanted.is_empty() {
        bail!("That backup has none of those parts");
    }
    let safety = crate::paths::root().join("backups").join(format!("before-restore-{}.arcadia", now()));
    export(&safety, &m.parts, true).ok();
    let mut z = zip::ZipArchive::new(std::fs::File::open(path)?)?;
    let mut n = 0;
    for i in 0..z.len() {
        let mut e = z.by_index(i)?;
        let name = e.name().to_string();
        let Some((part, inside)) = name.split_once('/') else { continue };
        if !wanted.iter().any(|w| *w == part) || e.is_dir() {
            continue;
        }
        let Some(out) = target(part, inside) else { continue };
        let mut bytes = Vec::new();
        (&mut e).take(256 << 20).read_to_end(&mut bytes)?;
        if inside == "settings.json" {
            bytes = merge_settings(&bytes)?;
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        crate::paths::write_atomic(&out, &bytes)?;
        n += 1;
    }
    Ok(n)
}

/// Keep this PC's own keys and sync folder when the backup left them out.
fn merge_settings(incoming: &[u8]) -> Result<Vec<u8>> {
    let mut v: serde_json::Value = serde_json::from_slice(incoming)?;
    let cur: serde_json::Value = std::fs::read(crate::paths::settings_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    if let (Some(o), Some(c)) = (v.as_object_mut(), cur.as_object()) {
        for k in KEY_FIELDS.iter().chain(&["sync_folder", "sync_last"]) {
            let empty = o.get(*k).map(|x| x.as_str().is_some_and(str::is_empty) || x.is_null()).unwrap_or(true);
            if empty {
                if let Some(x) = c.get(*k) {
                    o.insert((*k).into(), x.clone());
                }
            }
        }
    }
    Ok(serde_json::to_vec_pretty(&v)?)
}

// ── sync folder ──────────────────────────────────────────────

pub const SYNC_FILE: &str = "Arcadia sync.arcadia";
const SYNC_PARTS: &[&str] = &["settings", "library", "addons", "roblox", "skins", "proxy"];

pub fn sync_file(folder: &str) -> Option<PathBuf> {
    let f = PathBuf::from(folder.trim());
    (!folder.trim().is_empty() && f.is_dir()).then(|| f.join(SYNC_FILE))
}

/// Write this PC's setup to the sync folder.
pub fn sync_write(folder: &str) -> Result<Manifest> {
    let dest = sync_file(folder).context("The sync folder isn't there")?;
    export(&dest, &SYNC_PARTS.iter().map(|s| s.to_string()).collect::<Vec<_>>(), false)
}

/// A copy in the sync folder that another PC wrote after our last sync.
pub fn sync_newer(folder: &str, last: u64) -> Option<Manifest> {
    let m = inspect(&sync_file(folder)?).ok()?;
    (m.created > last && m.computer != computer()).then_some(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backs_up_and_restores_without_keys_or_secrets() {
        let (_g, t) = crate::minecraft::test_home();
        let root = t.path();
        std::fs::write(root.join("settings.json"), r##"{"accent":"#fff","steamgriddb_key":"SECRET","sync_folder":"D:\\Sync"}"##).unwrap();
        std::fs::create_dir_all(root.join("addons/discord-profile")).unwrap();
        std::fs::write(root.join("addons/discord-profile/data.json"), "{}").unwrap();
        std::fs::write(root.join("addons/discord-profile/.secrets"), "TOKEN").unwrap();
        std::fs::write(root.join("library-meta.json"), r#"{"games":{}}"#).unwrap();

        let dest = root.join("out/b.arcadia");
        let m = export(&dest, &["settings".into(), "addons".into(), "library".into()], false).unwrap();
        assert_eq!(m.files, 3);
        let mut z = zip::ZipArchive::new(std::fs::File::open(&dest).unwrap()).unwrap();
        let names: Vec<String> = z.file_names().map(String::from).collect();
        assert!(!names.iter().any(|n| n.contains(".secrets")));
        let mut s = String::new();
        z.by_name("settings/settings.json").unwrap().read_to_string(&mut s).unwrap();
        assert!(!s.contains("SECRET") && !s.contains("Sync"));

        // Change things, then restore: the key and sync folder stay, data comes back.
        std::fs::write(root.join("settings.json"), r##"{"accent":"#000","steamgriddb_key":"MINE","sync_folder":"E:\\S"}"##).unwrap();
        std::fs::remove_file(root.join("addons/discord-profile/data.json")).unwrap();
        assert_eq!(inspect(&dest).unwrap().parts.len(), 3);
        restore(&dest, &["settings".into(), "addons".into()]).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join("settings.json")).unwrap()).unwrap();
        assert_eq!(v["accent"], "#fff");
        assert_eq!(v["steamgriddb_key"], "MINE");
        assert_eq!(v["sync_folder"], "E:\\S");
        assert!(root.join("addons/discord-profile/data.json").exists());
        assert!(std::fs::read_dir(root.join("backups")).unwrap().count() == 1);
    }
}
