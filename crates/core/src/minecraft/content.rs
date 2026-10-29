//! An instance's other content: resource packs, shader packs, worlds (with
//! backups) and screenshots.

use super::instance::{game_dir, instance_dir};
use super::nbt;
use anyhow::{bail, Context, Result};
use base64::Engine;
use serde::Serialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const OFF: &str = ".disabled";

fn safe_name(name: &str) -> Result<&str> {
    if name.is_empty() || name.contains(['/', '\\']) || name == "." || name == ".." {
        bail!("Not a valid file name");
    }
    Ok(name)
}

fn dir_size(p: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(p) else { return 0 };
    rd.flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            _ => e.metadata().map(|m| m.len()).unwrap_or(0),
        })
        .sum()
}

fn mtime(p: &Path) -> u64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn png_data_url(b: &[u8]) -> Option<String> {
    (b.starts_with(b"\x89PNG") && b.len() < 300_000).then(|| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b)))
}

// ── packs ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pack {
    pub file_name: String,
    pub name: String,
    pub enabled: bool,
    pub folder: bool,
    pub size: u64,
    pub description: Option<String>,
    pub icon: Option<String>,
}

pub fn pack_dir(kind: &str) -> Result<&'static str> {
    Ok(match kind {
        "resourcepacks" | "resourcepack" => "resourcepacks",
        "shaderpacks" | "shader" | "shaders" => "shaderpacks",
        _ => bail!("Unknown pack type"),
    })
}

/// `pack.mcmeta`'s description can be a string or chat-component JSON.
fn describe(v: &serde_json::Value) -> Option<String> {
    let s = super::servers::motd_text(v);
    (!s.is_empty()).then_some(s)
}

/// Remove § colour codes.
pub fn strip_formatting(s: &str) -> String {
    let mut out = String::new();
    let mut skip = false;
    for c in s.chars() {
        if skip {
            skip = false;
        } else if c == '§' {
            skip = true;
        } else {
            out.push(c);
        }
    }
    out.trim().to_string()
}

fn pack_meta(path: &Path) -> (Option<String>, Option<String>) {
    let read = |name: &str| -> Option<Vec<u8>> {
        if path.is_dir() {
            std::fs::read(path.join(name)).ok()
        } else {
            let f = std::fs::File::open(path).ok()?;
            let mut z = zip::ZipArchive::new(f).ok()?;
            let mut e = z.by_name(name).ok()?;
            let mut b = Vec::new();
            (&mut e).take(400_000).read_to_end(&mut b).ok()?;
            Some(b)
        }
    };
    let desc = read("pack.mcmeta")
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(trim_bom(&b)).ok())
        .and_then(|v| describe(&v["pack"]["description"]))
        .filter(|s| !s.is_empty());
    let icon = read("pack.png").and_then(|b| png_data_url(&b));
    (desc, icon)
}

fn trim_bom(b: &[u8]) -> &[u8] {
    b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(b)
}

pub fn packs(id: &str, kind: &str) -> Result<Vec<Pack>> {
    let dir = game_dir(id).join(pack_dir(kind)?);
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(&dir) else { return Ok(out) };
    for e in rd.flatten() {
        let file_name = e.file_name().to_string_lossy().to_string();
        let p = e.path();
        let folder = p.is_dir();
        let enabled = !file_name.ends_with(OFF);
        let base = file_name.trim_end_matches(OFF);
        if !folder && !base.to_ascii_lowercase().ends_with(".zip") {
            continue;
        }
        let (description, icon) = pack_meta(&p);
        out.push(Pack {
            name: base.trim_end_matches(".zip").to_string(),
            size: if folder { dir_size(&p) } else { e.metadata().map(|m| m.len()).unwrap_or(0) },
            file_name,
            enabled,
            folder,
            description,
            icon,
        });
    }
    out.sort_by_key(|p| p.name.to_lowercase());
    Ok(out)
}

/// Turn a pack on or off; returns its new file name.
pub fn set_pack_enabled(id: &str, kind: &str, file_name: &str, on: bool) -> Result<String> {
    let dir = game_dir(id).join(pack_dir(kind)?);
    let file_name = safe_name(file_name)?;
    let base = file_name.trim_end_matches(OFF);
    let new = if on { base.to_string() } else { format!("{base}{OFF}") };
    if new != file_name {
        std::fs::rename(dir.join(file_name), dir.join(&new)).context("renaming pack")?;
    }
    Ok(new)
}

pub fn remove_pack(id: &str, kind: &str, file_name: &str) -> Result<()> {
    let p = game_dir(id).join(pack_dir(kind)?).join(safe_name(file_name)?);
    if p.is_dir() {
        std::fs::remove_dir_all(p)?;
    } else {
        std::fs::remove_file(p)?;
    }
    Ok(())
}

/// Copy dropped-in pack files into the instance.
pub fn add_packs(id: &str, kind: &str, files: &[PathBuf]) -> Result<usize> {
    let dir = game_dir(id).join(pack_dir(kind)?);
    std::fs::create_dir_all(&dir)?;
    let mut n = 0;
    for f in files {
        let Some(name) = f.file_name() else { continue };
        if f.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("zip")) != Some(true) {
            continue;
        }
        std::fs::copy(f, dir.join(name))?;
        n += 1;
    }
    Ok(n)
}

// ── worlds ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub file: String,
    pub created: u64,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct World {
    pub folder: String,
    pub name: String,
    /// Milliseconds since 1970.
    pub last_played: Option<i64>,
    /// 0 survival, 1 creative, 2 adventure, 3 spectator.
    pub game_mode: Option<i64>,
    pub hardcore: bool,
    pub cheats: bool,
    pub version: Option<String>,
    pub seed: Option<i64>,
    pub size: u64,
    pub icon: Option<String>,
    pub backups: Vec<Backup>,
}

fn backups_dir(id: &str, folder: &str) -> PathBuf {
    instance_dir(id).join("backups").join(folder)
}

pub fn backups(id: &str, folder: &str) -> Vec<Backup> {
    let mut v: Vec<Backup> = std::fs::read_dir(backups_dir(id, folder))
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.file_name().to_string_lossy().ends_with(".zip"))
                .map(|e| Backup { file: e.file_name().to_string_lossy().to_string(), created: mtime(&e.path()), size: e.metadata().map(|m| m.len()).unwrap_or(0) })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| b.created.cmp(&a.created));
    v
}

/// Facts from a world's `level.dat`.
pub fn read_level(dat: &[u8]) -> Option<(Option<String>, Option<i64>, Option<i64>, bool, bool, Option<String>, Option<i64>)> {
    let root = nbt::read(dat).ok()?;
    let d = root.get("Data")?;
    let name = d.get("LevelName").and_then(|t| t.str()).map(str::to_string);
    let last = d.get("LastPlayed").and_then(|t| t.int());
    let mode = d.get("GameType").and_then(|t| t.int());
    let hard = d.get("hardcore").and_then(|t| t.int()).unwrap_or(0) != 0;
    let cheats = d.get("allowCommands").and_then(|t| t.int()).unwrap_or(0) != 0;
    let version = d.get("Version").and_then(|v| v.get("Name")).and_then(|t| t.str()).map(str::to_string);
    let seed = d.get("WorldGenSettings").and_then(|w| w.get("seed")).and_then(|t| t.int()).or_else(|| d.get("RandomSeed").and_then(|t| t.int()));
    Some((name, last, mode, hard, cheats, version, seed))
}

pub fn worlds(id: &str) -> Result<Vec<World>> {
    let dir = game_dir(id).join("saves");
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(&dir) else { return Ok(out) };
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        let Ok(dat) = std::fs::read(p.join("level.dat")) else { continue };
        let folder = e.file_name().to_string_lossy().to_string();
        let (name, last, mode, hard, cheats, version, seed) = read_level(&dat).unwrap_or((None, None, None, false, false, None, None));
        out.push(World {
            name: name.unwrap_or_else(|| folder.clone()),
            last_played: last.or(Some(mtime(&p.join("level.dat")) as i64 * 1000)),
            game_mode: mode,
            hardcore: hard,
            cheats,
            version,
            seed,
            size: dir_size(&p),
            icon: std::fs::read(p.join("icon.png")).ok().and_then(|b| png_data_url(&b)),
            backups: backups(id, &folder),
            folder,
        });
    }
    out.sort_by(|a, b| b.last_played.cmp(&a.last_played));
    Ok(out)
}

fn zip_dir(src: &Path, out: &Path) -> Result<()> {
    if let Some(p) = out.parent() {
        std::fs::create_dir_all(p)?;
    }
    let tmp = out.with_extension("part");
    let f = std::fs::File::create(&tmp)?;
    let mut z = zip::ZipWriter::new(f);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(true);
    fn walk(z: &mut zip::ZipWriter<std::fs::File>, base: &Path, dir: &Path, opts: zip::write::SimpleFileOptions) -> Result<()> {
        for e in std::fs::read_dir(dir)?.flatten() {
            let p = e.path();
            let rel = p.strip_prefix(base)?.to_string_lossy().replace('\\', "/");
            if p.is_dir() {
                z.add_directory(format!("{rel}/"), opts)?;
                walk(z, base, &p, opts)?;
            } else if e.file_name() != "session.lock" {
                z.start_file(rel, opts)?;
                let mut f = std::fs::File::open(&p)?;
                std::io::copy(&mut f, z)?;
            }
        }
        Ok(())
    }
    walk(&mut z, src, src, opts)?;
    z.finish()?.flush()?;
    std::fs::rename(tmp, out)?;
    Ok(())
}

fn stamp() -> String {
    let t = time::OffsetDateTime::now_utc();
    format!("{:04}-{:02}-{:02}_{:02}-{:02}-{:02}", t.year(), t.month() as u8, t.day(), t.hour(), t.minute(), t.second())
}

/// Zip a world into the instance's backups folder.
pub fn backup_world(id: &str, folder: &str) -> Result<Backup> {
    let src = game_dir(id).join("saves").join(safe_name(folder)?);
    if !src.join("level.dat").exists() {
        bail!("That world doesn't exist");
    }
    let name = format!("{}.zip", stamp());
    let out = backups_dir(id, folder).join(&name);
    zip_dir(&src, &out)?;
    Ok(Backup { file: name, created: mtime(&out), size: std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0) })
}

/// Put a backup back. The world as it is now is backed up first.
pub fn restore_backup(id: &str, folder: &str, file: &str) -> Result<()> {
    let zip_path = backups_dir(id, safe_name(folder)?).join(safe_name(file)?);
    let dst = game_dir(id).join("saves").join(folder);
    if dst.join("level.dat").exists() {
        backup_world(id, folder)?;
        std::fs::remove_dir_all(&dst)?;
    }
    std::fs::create_dir_all(&dst)?;
    crate::roblox::deploy::extract_zip(&zip_path, &dst)
}

pub fn delete_backup(id: &str, folder: &str, file: &str) -> Result<()> {
    std::fs::remove_file(backups_dir(id, safe_name(folder)?).join(safe_name(file)?))?;
    Ok(())
}

/// Delete a world, keeping one last backup unless told not to.
pub fn delete_world(id: &str, folder: &str, keep_backup: bool) -> Result<()> {
    let p = game_dir(id).join("saves").join(safe_name(folder)?);
    if keep_backup {
        backup_world(id, folder)?;
    }
    std::fs::remove_dir_all(p)?;
    Ok(())
}

// ── screenshots ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shot {
    pub file: String,
    pub path: String,
    pub taken: u64,
    pub size: u64,
}

pub fn screenshots(id: &str) -> Vec<Shot> {
    let dir = game_dir(id).join("screenshots");
    let mut v: Vec<Shot> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().to_ascii_lowercase();
                    n.ends_with(".png") || n.ends_with(".jpg")
                })
                .map(|e| Shot {
                    file: e.file_name().to_string_lossy().to_string(),
                    path: e.path().to_string_lossy().to_string(),
                    taken: mtime(&e.path()),
                    size: e.metadata().map(|m| m.len()).unwrap_or(0),
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| b.taken.cmp(&a.taken));
    v
}

pub fn delete_screenshot(id: &str, file: &str) -> Result<()> {
    std::fs::remove_file(game_dir(id).join("screenshots").join(safe_name(file)?))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nbt::Tag;

    #[test]
    fn reads_level_dat_and_pack_meta() {
        let level = Tag::Compound(vec![(
            "Data".into(),
            Tag::Compound(vec![
                ("LevelName".into(), Tag::String("My World".into())),
                ("LastPlayed".into(), Tag::Long(1_700_000_000_000)),
                ("GameType".into(), Tag::Int(1)),
                ("hardcore".into(), Tag::Byte(0)),
                ("allowCommands".into(), Tag::Byte(1)),
                ("Version".into(), Tag::Compound(vec![("Name".into(), Tag::String("1.21.4".into()))])),
                ("WorldGenSettings".into(), Tag::Compound(vec![("seed".into(), Tag::Long(42))])),
            ]),
        )]);
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gz.write_all(&nbt::write(&level)).unwrap();
        let (name, last, mode, hard, cheats, ver, seed) = read_level(&gz.finish().unwrap()).unwrap();
        assert_eq!(name.as_deref(), Some("My World"));
        assert_eq!((last, mode, hard, cheats, seed), (Some(1_700_000_000_000), Some(1), false, true, Some(42)));
        assert_eq!(ver.as_deref(), Some("1.21.4"));

        let d: serde_json::Value = serde_json::from_str(r#"{"text":"§aFaithful ","extra":[{"text":"32x"}]}"#).unwrap();
        assert_eq!(describe(&d).unwrap(), "Faithful 32x");
        assert_eq!(strip_formatting("§l§6Gold§r"), "Gold");
        assert!(safe_name("../x").is_err());
    }

    #[test]
    fn backs_up_and_restores_a_world() {
        let t = tempfile::tempdir().unwrap();
        let w = t.path().join("w");
        std::fs::create_dir_all(w.join("region")).unwrap();
        std::fs::write(w.join("level.dat"), b"L").unwrap();
        std::fs::write(w.join("region/r.0.0.mca"), b"R").unwrap();
        std::fs::write(w.join("session.lock"), b"S").unwrap();
        let z = t.path().join("b/one.zip");
        zip_dir(&w, &z).unwrap();
        let out = t.path().join("out");
        crate::roblox::deploy::extract_zip(&z, &out).unwrap();
        assert_eq!(std::fs::read(out.join("region/r.0.0.mca")).unwrap(), b"R");
        assert!(!out.join("session.lock").exists());
    }
}
