//! Mods: files in `Modifications\` are overlaid onto the Roblox version folder
//! at launch, mirroring its layout (e.g. `content\sounds\ouch.ogg`). Original
//! files are backed up and restored before each launch, so removing a mod
//! from the folder cleanly undoes it.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const STATE: &str = ".arcadia-mods.json";
const BACKUP: &str = ".arcadia-backup";

#[derive(Default, Serialize, Deserialize)]
struct ModState {
    /// Files that didn't exist before (delete on revert).
    added: Vec<String>,
    /// Files that were overwritten (restore from backup on revert).
    replaced: Vec<String>,
}

fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, base, out);
        } else if let Ok(rel) = p.strip_prefix(base) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// Undo whatever the last launch applied.
pub fn revert(version_dir: &Path) -> Result<()> {
    let state_path = version_dir.join(STATE);
    let Ok(src) = std::fs::read_to_string(&state_path) else { return Ok(()) };
    let st: ModState = serde_json::from_str(&src).unwrap_or_default();
    for rel in &st.added {
        let _ = std::fs::remove_file(version_dir.join(rel));
    }
    for rel in &st.replaced {
        let b = version_dir.join(BACKUP).join(rel);
        if b.exists() {
            std::fs::copy(&b, version_dir.join(rel))?;
        }
    }
    let _ = std::fs::remove_dir_all(version_dir.join(BACKUP));
    std::fs::remove_file(state_path)?;
    Ok(())
}

/// Overlay files onto the version folder. `extra` adds generated files
/// (custom font and its rewritten family definitions) as (relative path, bytes).
pub fn apply(version_dir: &Path, mods_dir: Option<&Path>, extra: Vec<(String, Vec<u8>)>) -> Result<usize> {
    revert(version_dir)?;
    let mut files = Vec::new();
    let mods_dir = mods_dir.unwrap_or(Path::new(""));
    if !mods_dir.as_os_str().is_empty() {
        walk(mods_dir, mods_dir, &mut files);
    }
    let mut st = ModState::default();

    let mut place = |rel: &str, write: &dyn Fn(&Path) -> std::io::Result<()>| -> Result<()> {
        let rel = rel.trim_start_matches('/');
        if rel.split('/').any(|c| c == ".." || c.is_empty()) {
            return Ok(());
        }
        let target = version_dir.join(rel);
        if target.exists() {
            if !st.replaced.iter().any(|r| r == rel) {
                let b = version_dir.join(BACKUP).join(rel);
                if let Some(p) = b.parent() {
                    std::fs::create_dir_all(p)?;
                }
                std::fs::copy(&target, &b)?;
                st.replaced.push(rel.to_string());
            }
        } else {
            if let Some(p) = target.parent() {
                std::fs::create_dir_all(p)?;
            }
            st.added.push(rel.to_string());
        }
        write(&target)?;
        Ok(())
    };

    for rel in &files {
        let src = mods_dir.join(rel);
        place(rel, &|t| std::fs::copy(&src, t).map(|_| ()))?;
    }
    for (rel, bytes) in &extra {
        place(rel, &|t| std::fs::write(t, bytes))?;
    }
    let n = st.added.len() + st.replaced.len();
    std::fs::write(version_dir.join(STATE), serde_json::to_vec_pretty(&st)?)?;
    Ok(n)
}

/// Generate the files that swap every Roblox font family to a custom font:
/// the font itself plus each `content/fonts/families/*.json` with its faces
/// pointed at it. (Same technique as Bloxstrap.)
pub fn custom_font_files(version_dir: &Path, font: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let ext = font
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .filter(|e| e == "ttf" || e == "otf")
        .unwrap_or_else(|| "ttf".into());
    let font_rel = format!("content/fonts/CustomFont.{ext}");
    let mut out = vec![(font_rel.clone(), std::fs::read(font)?)];
    let fam_dir: PathBuf = version_dir.join("content/fonts/families");
    if let Ok(rd) = std::fs::read_dir(&fam_dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Ok(src) = std::fs::read_to_string(&p) else { continue };
            let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&src) else { continue };
            if let Some(faces) = v.get_mut("faces").and_then(|f| f.as_array_mut()) {
                for f in faces {
                    if let Some(o) = f.as_object_mut() {
                        o.insert("assetId".into(), format!("rbxasset://fonts/CustomFont.{ext}").into());
                    }
                }
            }
            let name = e.file_name().to_string_lossy().to_string();
            out.push((format!("content/fonts/families/{name}"), serde_json::to_vec(&v)?));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_and_revert_roundtrip() {
        let t = tempfile::tempdir().unwrap();
        let ver = t.path().join("ver");
        let mods = t.path().join("mods");
        std::fs::create_dir_all(ver.join("content/sounds")).unwrap();
        std::fs::create_dir_all(mods.join("content/sounds")).unwrap();
        std::fs::write(ver.join("content/sounds/ouch.ogg"), b"original").unwrap();
        std::fs::write(mods.join("content/sounds/ouch.ogg"), b"classic oof").unwrap();
        std::fs::write(mods.join("content/sounds/new.ogg"), b"new").unwrap();

        assert_eq!(apply(&ver, Some(&mods), vec![]).unwrap(), 2);
        assert_eq!(std::fs::read(ver.join("content/sounds/ouch.ogg")).unwrap(), b"classic oof");
        assert!(ver.join("content/sounds/new.ogg").exists());

        // Re-applying is idempotent and keeps the true original backed up.
        apply(&ver, Some(&mods), vec![]).unwrap();
        revert(&ver).unwrap();
        assert_eq!(std::fs::read(ver.join("content/sounds/ouch.ogg")).unwrap(), b"original");
        assert!(!ver.join("content/sounds/new.ogg").exists());
    }

    #[test]
    fn custom_font_rewrites_families() {
        let t = tempfile::tempdir().unwrap();
        let ver = t.path();
        std::fs::create_dir_all(ver.join("content/fonts/families")).unwrap();
        std::fs::write(
            ver.join("content/fonts/families/GothamSSm.json"),
            r#"{"name":"Gotham SSm","faces":[{"name":"Regular","weight":400,"assetId":"rbxasset://fonts/GothamSSm-Book.otf"}]}"#,
        )
        .unwrap();
        let font = ver.join("my.ttf");
        std::fs::write(&font, b"FONT").unwrap();
        let files = custom_font_files(ver, &font).unwrap();
        assert_eq!(files[0].0, "content/fonts/CustomFont.ttf");
        let fam: serde_json::Value = serde_json::from_slice(&files[1].1).unwrap();
        assert_eq!(fam["faces"][0]["assetId"], "rbxasset://fonts/CustomFont.ttf");
    }
}
