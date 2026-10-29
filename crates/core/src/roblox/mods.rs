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

// ── Mod studio ──────────────────────────────────────────────

/// Mods made from a few choices instead of hand-placed files.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ModStudio {
    /// "" keeps Roblox's cursor; a built-in style ("dot", "ring", "cross",
    /// "arrow"); or "custom" for your own pictures.
    pub cursor: String,
    /// Colour for the built-in styles, as #rrggbb.
    pub cursor_colour: String,
    /// Size of the built-in styles, 50–200 (%).
    pub cursor_scale: u32,
    pub cursor_arrow: Option<String>,
    pub cursor_far: Option<String>,
    /// Shift-lock crosshair: "", a built-in style, or "custom".
    pub shiftlock: String,
    pub shiftlock_file: Option<String>,
    /// Sound for when your character resets or dies.
    pub death_sound: Option<String>,
}

impl Default for ModStudio {
    fn default() -> Self {
        Self {
            cursor: String::new(),
            cursor_colour: "#ffffff".into(),
            cursor_scale: 100,
            cursor_arrow: None,
            cursor_far: None,
            shiftlock: String::new(),
            shiftlock_file: None,
            death_sound: None,
        }
    }
}

pub const CURSOR_STYLES: &[&str] = &["dot", "ring", "ring-dot", "cross", "arrow"];
const ARROW: &str = "content/textures/Cursors/KeyboardMouse/ArrowCursor.png";
const ARROW_FAR: &str = "content/textures/Cursors/KeyboardMouse/ArrowFarCursor.png";
const LOCKED: &str = "content/textures/MouseLockedCursor.png";
const OUCH: &str = "content/sounds/ouch.ogg";

impl ModStudio {
    pub fn is_empty(&self) -> bool {
        self.cursor.is_empty() && self.shiftlock.is_empty() && self.death_sound.is_none()
    }

    /// The files these choices add, as (path in the version folder, bytes).
    pub fn files(&self) -> Result<Vec<(String, Vec<u8>)>> {
        let mut out = Vec::new();
        let colour = parse_colour(&self.cursor_colour).unwrap_or([255, 255, 255]);
        let scale = self.cursor_scale.clamp(50, 200) as f32 / 100.0;
        let read = |p: &Option<String>| -> Result<Option<Vec<u8>>> {
            match p.as_deref().filter(|p| !p.is_empty()) {
                Some(p) => Ok(Some(std::fs::read(p).map_err(|e| anyhow::anyhow!("{p}: {e}"))?)),
                None => Ok(None),
            }
        };
        match self.cursor.as_str() {
            "" => {}
            "custom" => {
                let arrow = read(&self.cursor_arrow)?;
                let far = read(&self.cursor_far)?.or_else(|| arrow.clone());
                if let Some(a) = arrow {
                    out.push((ARROW.into(), a));
                }
                if let Some(f) = far {
                    out.push((ARROW_FAR.into(), f));
                }
            }
            style => {
                out.push((ARROW.into(), cursor_png(style, colour, scale, 1.0)));
                out.push((ARROW_FAR.into(), cursor_png(style, colour, scale, 0.6)));
            }
        }
        match self.shiftlock.as_str() {
            "" => {}
            "custom" => {
                if let Some(b) = read(&self.shiftlock_file)? {
                    out.push((LOCKED.into(), b));
                }
            }
            style => out.push((LOCKED.into(), cursor_png(style, colour, scale, 1.0))),
        }
        if let Some(b) = read(&self.death_sound)? {
            out.push((OUCH.into(), b));
        }
        Ok(out)
    }
}

fn parse_colour(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}

/// Signed distance from a point to a convex or concave polygon.
fn sd_polygon(p: (f32, f32), v: &[(f32, f32)]) -> f32 {
    let mut d = f32::MAX;
    let mut inside = false;
    let mut j = v.len() - 1;
    for i in 0..v.len() {
        let (a, b) = (v[i], v[j]);
        let e = (b.0 - a.0, b.1 - a.1);
        let w = (p.0 - a.0, p.1 - a.1);
        let t = ((w.0 * e.0 + w.1 * e.1) / (e.0 * e.0 + e.1 * e.1)).clamp(0.0, 1.0);
        let q = (w.0 - e.0 * t, w.1 - e.1 * t);
        d = d.min((q.0 * q.0 + q.1 * q.1).sqrt());
        if ((a.1 > p.1) != (b.1 > p.1)) && (p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0) {
            inside = !inside;
        }
        j = i;
    }
    if inside { -d } else { d }
}

fn sd_box(p: (f32, f32), c: (f32, f32), half: (f32, f32)) -> f32 {
    let q = ((p.0 - c.0).abs() - half.0, (p.1 - c.1).abs() - half.1);
    let out = (q.0.max(0.0).powi(2) + q.1.max(0.0).powi(2)).sqrt();
    out + q.0.max(q.1).min(0.0)
}

/// Distance to one of Arcadia's own cursor shapes, centred on (32, 32)
/// (Roblox draws cursor pictures centred on the pointer).
fn shape(style: &str, p: (f32, f32), k: f32) -> f32 {
    let c = (32.0, 32.0);
    let r = ((p.0 - c.0).powi(2) + (p.1 - c.1).powi(2)).sqrt();
    match style {
        "ring" => (r - 9.0 * k).abs() - 1.6 * k,
        "ring-dot" => ((r - 9.0 * k).abs() - 1.6 * k).min(r - 2.2 * k),
        "cross" => {
            let gap = 3.5 * k;
            let len = 7.0 * k;
            let arm = |dx: f32, dy: f32| sd_box(p, (c.0 + dx * (gap + len / 2.0), c.1 + dy * (gap + len / 2.0)), if dx != 0.0 { (len / 2.0, 1.2 * k) } else { (1.2 * k, len / 2.0) });
            arm(1.0, 0.0).min(arm(-1.0, 0.0)).min(arm(0.0, 1.0)).min(arm(0.0, -1.0)).min(r - 1.4 * k)
        }
        "arrow" => {
            let pts = [(0.0, 0.0), (0.0, 19.0), (4.6, 14.8), (8.2, 22.4), (11.4, 21.0), (7.9, 13.6), (13.6, 13.6)];
            let v: Vec<(f32, f32)> = pts.iter().map(|(x, y)| (c.0 + x * k, c.1 + y * k)).collect();
            sd_polygon(p, &v) - 0.6 * k
        }
        _ => r - 4.0 * k,
    }
}

/// Draw one of Arcadia's cursor shapes as a 64×64 PNG: a fill in `colour`
/// with a soft dark outline so it shows on any background.
pub fn cursor_png(style: &str, colour: [u8; 3], scale: f32, alpha: f32) -> Vec<u8> {
    const N: u32 = 64;
    const SS: u32 = 4;
    let k = scale;
    let outline = 1.6 * k.max(0.8);
    let mut img = image::RgbaImage::new(N, N);
    for y in 0..N {
        for x in 0..N {
            let (mut fill, mut edge) = (0.0f32, 0.0f32);
            for sy in 0..SS {
                for sx in 0..SS {
                    let p = (x as f32 + (sx as f32 + 0.5) / SS as f32, y as f32 + (sy as f32 + 0.5) / SS as f32);
                    let d = shape(style, p, k);
                    if d < 0.0 {
                        fill += 1.0;
                    } else if d < outline {
                        edge += 1.0;
                    }
                }
            }
            let n = (SS * SS) as f32;
            let (f, e) = (fill / n, edge / n);
            let a = f + e * 0.7;
            if a <= 0.0 {
                continue;
            }
            let mix = |ch: u8| ((ch as f32 * f + 12.0 * e * 0.7) / a).round().clamp(0.0, 255.0) as u8;
            img.put_pixel(x, y, image::Rgba([mix(colour[0]), mix(colour[1]), mix(colour[2]), (a * alpha * 255.0).round().min(255.0) as u8]));
        }
    }
    let mut out = std::io::Cursor::new(Vec::new());
    let _ = img.write_to(&mut out, image::ImageFormat::Png);
    out.into_inner()
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

    #[test]
    fn studio_makes_cursor_files() {
        let mut m = ModStudio { cursor: "ring".into(), shiftlock: "cross".into(), ..Default::default() };
        let f = m.files().unwrap();
        assert_eq!(f.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), [ARROW, ARROW_FAR, LOCKED]);
        let img = image::load_from_memory(&f[0].1).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (64, 64));
        // The ring is drawn, the middle of it is empty.
        assert!(img.get_pixel(41, 32)[3] > 200);
        assert_eq!(img.get_pixel(32, 32)[3], 0);
        // The far cursor is fainter.
        let far = image::load_from_memory(&f[1].1).unwrap().to_rgba8();
        assert!(far.get_pixel(41, 32)[3] < img.get_pixel(41, 32)[3]);
        for s in CURSOR_STYLES {
            let i = image::load_from_memory(&cursor_png(s, [255, 0, 0], 1.0, 1.0)).unwrap().to_rgba8();
            assert!(i.pixels().any(|p| p[3] > 0), "{s} draws something");
        }
        m.cursor = "custom".into();
        m.cursor_arrow = Some("/no/such/file.png".into());
        assert!(m.files().is_err());
        assert!(ModStudio::default().files().unwrap().is_empty());
    }
}
