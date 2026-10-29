//! Skins and capes: a local skin library, and changing the skin and cape on
//! your Minecraft profile through Mojang's own API (with the signed-in
//! account's token, which never leaves this module except to Mojang).

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const PROFILE: &str = "https://api.minecraftservices.com/minecraft/profile";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Saved {
    pub id: String,
    pub name: String,
    /// "classic" (4px arms) or "slim" (3px arms).
    pub variant: String,
    pub added: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinWithData {
    #[serde(flatten)]
    pub saved: Saved,
    /// The skin picture as a data URL.
    pub data: String,
}

fn dir() -> PathBuf {
    super::dir().join("skins")
}
fn index() -> PathBuf {
    dir().join("library.json")
}

fn load() -> Vec<Saved> {
    std::fs::read(index()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}
fn save(v: &[Saved]) -> Result<()> {
    std::fs::create_dir_all(dir())?;
    crate::paths::write_atomic(&index(), &serde_json::to_vec_pretty(v)?)?;
    Ok(())
}

fn data_url(b: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b))
}

/// A real skin: a PNG that's 64×64 (modern) or 64×32 (old style).
pub fn check_png(b: &[u8]) -> Result<()> {
    if !b.starts_with(b"\x89PNG\r\n\x1a\n") || b.len() < 24 {
        bail!("That isn't a PNG picture");
    }
    let w = u32::from_be_bytes(b[16..20].try_into()?);
    let h = u32::from_be_bytes(b[20..24].try_into()?);
    if w != 64 || (h != 64 && h != 32) {
        bail!("Skins are 64×64 pixels (or 64×32 for old ones); this one is {w}×{h}");
    }
    if b.len() > 64 * 1024 {
        bail!("That skin file is too big");
    }
    Ok(())
}

/// Guess slim or classic from the pixel that's empty on slim arms.
pub fn guess_variant(b: &[u8]) -> &'static str {
    match image::load_from_memory(b) {
        Ok(img) if img.height() == 64 => {
            let px = img.to_rgba8();
            if px.get_pixel(54, 20)[3] == 0 { "slim" } else { "classic" }
        }
        _ => "classic",
    }
}

pub fn library() -> Vec<SkinWithData> {
    load()
        .into_iter()
        .filter_map(|s| {
            let b = std::fs::read(dir().join(format!("{}.png", s.id))).ok()?;
            Some(SkinWithData { data: data_url(&b), saved: s })
        })
        .collect()
}

pub fn add_bytes(name: &str, bytes: &[u8], variant: Option<&str>) -> Result<SkinWithData> {
    check_png(bytes)?;
    let id = uuid::Uuid::new_v4().simple().to_string();
    std::fs::create_dir_all(dir())?;
    std::fs::write(dir().join(format!("{id}.png")), bytes)?;
    let variant = match variant {
        Some("slim") => "slim",
        Some("classic") => "classic",
        _ => guess_variant(bytes),
    };
    let s = Saved { id, name: name.trim().chars().take(40).collect(), variant: variant.into(), added: super::instance::now() };
    let mut all = load();
    all.insert(0, s.clone());
    save(&all)?;
    Ok(SkinWithData { saved: s, data: data_url(bytes) })
}

pub fn add_file(path: &std::path::Path) -> Result<SkinWithData> {
    let b = std::fs::read(path)?;
    let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Skin".into());
    add_bytes(&name, &b, None)
}

pub fn rename(id: &str, name: &str, variant: &str) -> Result<()> {
    let mut all = load();
    if let Some(s) = all.iter_mut().find(|s| s.id == id) {
        s.name = name.trim().chars().take(40).collect();
        if variant == "slim" || variant == "classic" {
            s.variant = variant.into();
        }
    }
    save(&all)
}

pub fn remove(id: &str) -> Result<()> {
    let mut all = load();
    all.retain(|s| s.id != id);
    let _ = std::fs::remove_file(dir().join(format!("{}.png", id.replace(['/', '\\', '.'], ""))));
    save(&all)
}

async fn download_png(url: &str) -> Result<Vec<u8>> {
    let u = url::Url::parse(url)?;
    if !u.host_str().is_some_and(|h| h == "textures.minecraft.net" || h.ends_with(".minecraft.net")) {
        bail!("Skins only come from textures.minecraft.net");
    }
    let u = if u.scheme() == "http" { url.replacen("http://", "https://", 1) } else { url.to_string() };
    Ok(super::http().get(u).send().await?.error_for_status()?.bytes().await?.to_vec())
}

/// Copy any player's current skin into the library, by name.
pub async fn add_from_player(name: &str) -> Result<SkinWithData> {
    #[derive(Deserialize)]
    struct Id {
        id: String,
        name: String,
    }
    let r = super::http().get(format!("https://api.mojang.com/users/profiles/minecraft/{}", urlenc(name.trim()))).send().await?;
    if r.status() == reqwest::StatusCode::NOT_FOUND || r.status() == reqwest::StatusCode::NO_CONTENT {
        bail!("No player is called {name}");
    }
    let who: Id = r.error_for_status()?.json().await?;
    let p: serde_json::Value = super::http().get(format!("https://sessionserver.mojang.com/session/minecraft/profile/{}", who.id)).send().await?.error_for_status()?.json().await?;
    let tex = p["properties"].as_array().and_then(|a| a.iter().find(|x| x["name"] == "textures")).and_then(|x| x["value"].as_str()).ok_or_else(|| anyhow!("{} has no skin", who.name))?;
    let t: serde_json::Value = serde_json::from_slice(&base64::engine::general_purpose::STANDARD.decode(tex)?)?;
    let url = t["textures"]["SKIN"]["url"].as_str().ok_or_else(|| anyhow!("{} uses a default skin", who.name))?;
    let slim = t["textures"]["SKIN"]["metadata"]["model"] == "slim";
    let b = download_png(url).await?;
    add_bytes(&who.name, &b, Some(if slim { "slim" } else { "classic" }))
}

fn urlenc(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').collect()
}

// ── the signed-in profile ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cape {
    pub id: String,
    pub alias: String,
    pub active: bool,
    pub data: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub name: String,
    pub skin: Option<String>,
    pub variant: String,
    pub capes: Vec<Cape>,
}

#[derive(Deserialize)]
struct RawProfile {
    name: String,
    #[serde(default)]
    skins: Vec<RawSkin>,
    #[serde(default)]
    capes: Vec<RawCape>,
}
#[derive(Deserialize)]
struct RawSkin {
    state: String,
    url: String,
    #[serde(default)]
    variant: String,
}
#[derive(Deserialize)]
struct RawCape {
    id: String,
    state: String,
    url: String,
    #[serde(default)]
    alias: String,
}

async fn to_profile(r: RawProfile) -> Profile {
    let active = r.skins.iter().find(|s| s.state == "ACTIVE");
    let skin = match active {
        Some(s) => download_png(&s.url).await.ok().map(|b| data_url(&b)),
        None => None,
    };
    let mut capes = Vec::new();
    for c in r.capes {
        let data = download_png(&c.url).await.ok().map(|b| data_url(&b));
        capes.push(Cape { id: c.id, alias: c.alias, active: c.state == "ACTIVE", data });
    }
    Profile { name: r.name, skin, variant: active.map(|s| s.variant.to_lowercase()).unwrap_or_else(|| "classic".into()), capes }
}

async fn reply(r: reqwest::Response) -> Result<Profile> {
    if r.status() == reqwest::StatusCode::UNAUTHORIZED {
        bail!("Your Minecraft sign-in has expired. Sign in again under Accounts.");
    }
    if r.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        bail!("Mojang is limiting skin changes. Wait a minute and try again.");
    }
    let r = r.error_for_status().context("Mojang didn't accept that")?;
    Ok(to_profile(r.json().await?).await)
}

pub async fn profile(token: &str) -> Result<Profile> {
    reply(super::http().get(PROFILE).bearer_auth(token).send().await?).await
}

/// Wear a skin from the library.
pub async fn wear(token: &str, id: &str) -> Result<Profile> {
    let s = load().into_iter().find(|s| s.id == id).ok_or_else(|| anyhow!("That skin isn't in the library"))?;
    let b = std::fs::read(dir().join(format!("{}.png", s.id)))?;
    check_png(&b)?;
    let form = reqwest::multipart::Form::new()
        .text("variant", s.variant.clone())
        .part("file", reqwest::multipart::Part::bytes(b).file_name("skin.png").mime_str("image/png")?);
    reply(super::http().post(format!("{PROFILE}/skins")).bearer_auth(token).multipart(form).send().await?).await
}

/// Show a cape, or none.
pub async fn set_cape(token: &str, cape: Option<&str>) -> Result<Profile> {
    let req = match cape {
        Some(id) => super::http().put(format!("{PROFILE}/capes/active")).json(&serde_json::json!({ "capeId": id })),
        None => super::http().delete(format!("{PROFILE}/capes/active")),
    };
    reply(req.bearer_auth(token).send().await?).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, slim: bool) -> Vec<u8> {
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([90, 120, 200, 255]));
        if slim && h == 64 {
            img.put_pixel(54, 20, image::Rgba([0, 0, 0, 0]));
        }
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn checks_and_keeps_skins() {
        let (_g, _t) = super::super::test_home();
        assert!(check_png(&png(64, 64, false)).is_ok());
        assert!(check_png(&png(64, 32, false)).is_ok());
        assert!(check_png(&png(128, 128, false)).is_err());
        assert!(check_png(b"nope").is_err());
        assert_eq!(guess_variant(&png(64, 64, true)), "slim");
        assert_eq!(guess_variant(&png(64, 64, false)), "classic");

        let s = add_bytes("Mine", &png(64, 64, true), None).unwrap();
        assert_eq!(s.saved.variant, "slim");
        assert_eq!(library().len(), 1);
        rename(&s.saved.id, "Renamed", "classic").unwrap();
        assert_eq!(library()[0].saved.name, "Renamed");
        remove(&s.saved.id).unwrap();
        assert!(library().is_empty());
    }
}
