//! CurseForge API. Needs a free API key from console.curseforge.com.
//!
//! Some authors turn off third-party distribution; those files come back
//! without a download URL and are listed for the player to download from
//! CurseForge's website instead.

use super::download::{fetch, Job};
use super::instance;
use super::loaders::Loader;
use super::modrinth::Hit;
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const MINECRAFT: u32 = 432;
pub const CLASS_MODS: u32 = 6;
pub const CLASS_MODPACKS: u32 = 4471;
pub const CLASS_RESOURCEPACKS: u32 = 12;
pub const CLASS_SHADERS: u32 = 6552;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CfFile {
    pub id: u64,
    pub mod_id: u64,
    #[serde(default)]
    pub display_name: String,
    pub file_name: String,
    pub download_url: Option<String>,
    #[serde(default)]
    pub hashes: Vec<CfHash>,
    #[serde(default)]
    pub file_length: u64,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub file_fingerprint: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CfHash {
    pub value: String,
    /// 1 = sha1, 2 = md5
    pub algo: u32,
}

impl CfFile {
    pub fn sha1(&self) -> Option<String> {
        self.hashes.iter().find(|h| h.algo == 1).map(|h| h.value.clone())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CfMod {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub download_count: f64,
    #[serde(default)]
    pub logo: Option<CfLogo>,
    #[serde(default)]
    pub authors: Vec<CfAuthor>,
    #[serde(default)]
    pub links: Option<CfLinks>,
    #[serde(default)]
    pub class_id: Option<u32>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CfLogo {
    pub thumbnail_url: Option<String>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CfAuthor {
    pub name: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CfLinks {
    pub website_url: Option<String>,
}

#[derive(Deserialize)]
struct Data<T> {
    data: T,
}

fn client(key: &str) -> Result<(reqwest::Client, String)> {
    if key.trim().is_empty() {
        bail!("Add your CurseForge API key in Minecraft settings to use CurseForge");
    }
    Ok((super::http(), key.trim().to_string()))
}

fn url(path: &str) -> String {
    format!("{}{path}", super::urls::curseforge())
}

pub fn class_for(kind: &str) -> u32 {
    match kind {
        "modpack" => CLASS_MODPACKS,
        "resourcepack" => CLASS_RESOURCEPACKS,
        "shader" => CLASS_SHADERS,
        _ => CLASS_MODS,
    }
}

pub async fn search(key: &str, query: &str, kind: &str, mc: Option<&str>, loader: Option<Loader>) -> Result<Vec<Hit>> {
    let (c, key) = client(key)?;
    let mut q: Vec<(&str, String)> = vec![
        ("gameId", MINECRAFT.to_string()),
        ("classId", class_for(kind).to_string()),
        ("searchFilter", query.to_string()),
        ("sortField", "2".into()), // popularity
        ("sortOrder", "desc".into()),
        ("pageSize", "30".into()),
    ];
    if let Some(mc) = mc {
        q.push(("gameVersion", mc.to_string()));
        if let Some(l) = loader.and_then(|l| l.curseforge()) {
            if kind == "mod" {
                q.push(("modLoaderType", l.to_string()));
            }
        }
    }
    let r: Data<Vec<CfMod>> = c.get(url("/v1/mods/search")).header("x-api-key", key).query(&q).send().await?.error_for_status().map_err(friendly)?.json().await?;
    Ok(r.data
        .into_iter()
        .map(|m| Hit {
            id: m.id.to_string(),
            slug: m.slug,
            title: m.name,
            description: m.summary,
            author: m.authors.first().map(|a| a.name.clone()).unwrap_or_default(),
            icon_url: m.logo.and_then(|l| l.thumbnail_url),
            downloads: m.download_count as u64,
            source: "curseforge".into(),
        })
        .collect())
}

fn friendly(e: reqwest::Error) -> anyhow::Error {
    match e.status().map(|s| s.as_u16()) {
        Some(403) | Some(401) => anyhow!("CurseForge rejected the API key. Check it in Minecraft settings."),
        _ => e.into(),
    }
}

pub async fn files(key: &str, mod_id: u64, mc: Option<&str>, loader: Option<Loader>) -> Result<Vec<CfFile>> {
    let (c, key) = client(key)?;
    let mut q: Vec<(&str, String)> = vec![("pageSize", "50".into())];
    if let Some(mc) = mc {
        q.push(("gameVersion", mc.into()));
    }
    if let Some(l) = loader.and_then(|l| l.curseforge()) {
        q.push(("modLoaderType", l.to_string()));
    }
    let r: Data<Vec<CfFile>> = c.get(url(&format!("/v1/mods/{mod_id}/files"))).header("x-api-key", key).query(&q).send().await?.error_for_status().map_err(friendly)?.json().await?;
    Ok(r.data)
}

pub async fn files_by_id(key: &str, ids: &[u64]) -> Result<Vec<CfFile>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let (c, key) = client(key)?;
    let r: Data<Vec<CfFile>> = c.post(url("/v1/mods/files")).header("x-api-key", key).json(&serde_json::json!({"fileIds": ids})).send().await?.error_for_status().map_err(friendly)?.json().await?;
    Ok(r.data)
}

pub async fn mods_by_id(key: &str, ids: &[u64]) -> Result<Vec<CfMod>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let (c, key) = client(key)?;
    let r: Data<Vec<CfMod>> = c.post(url("/v1/mods")).header("x-api-key", key).json(&serde_json::json!({"modIds": ids})).send().await?.error_for_status().map_err(friendly)?.json().await?;
    Ok(r.data)
}

/// CurseForge's file fingerprint: MurmurHash2 (seed 1) over the file with
/// whitespace bytes (tab, LF, CR, space) removed.
pub fn fingerprint(data: &[u8]) -> u32 {
    let bytes: Vec<u8> = data.iter().copied().filter(|b| !matches!(b, 9 | 10 | 13 | 32)).collect();
    murmur2(&bytes, 1)
}

fn murmur2(data: &[u8], seed: u32) -> u32 {
    const M: u32 = 0x5bd1_e995;
    let len = data.len() as u32;
    let mut h = seed ^ len;
    let mut chunks = data.chunks_exact(4);
    for c in &mut chunks {
        let mut k = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        k = k.wrapping_mul(M);
        k ^= k >> 24;
        k = k.wrapping_mul(M);
        h = h.wrapping_mul(M);
        h ^= k;
    }
    let r = chunks.remainder();
    match r.len() {
        3 => {
            h ^= (r[2] as u32) << 16;
            h ^= (r[1] as u32) << 8;
            h ^= r[0] as u32;
            h = h.wrapping_mul(M);
        }
        2 => {
            h ^= (r[1] as u32) << 8;
            h ^= r[0] as u32;
            h = h.wrapping_mul(M);
        }
        1 => {
            h ^= r[0] as u32;
            h = h.wrapping_mul(M);
        }
        _ => {}
    }
    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^= h >> 15;
    h
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FpResp {
    exact_matches: Vec<FpMatch>,
}
#[derive(Deserialize)]
struct FpMatch {
    file: CfFile,
}

/// fingerprint → file, for files CurseForge knows about.
pub async fn match_fingerprints(key: &str, fps: &[u32]) -> Result<HashMap<u32, CfFile>> {
    if fps.is_empty() {
        return Ok(HashMap::new());
    }
    let (c, key) = client(key)?;
    let r: Data<FpResp> = c
        .post(url(&format!("/v1/fingerprints/{MINECRAFT}")))
        .header("x-api-key", key)
        .json(&serde_json::json!({"fingerprints": fps}))
        .send()
        .await?
        .error_for_status()
        .map_err(friendly)?
        .json()
        .await?;
    Ok(r.data.exact_matches.into_iter().map(|m| (m.file.file_fingerprint as u32, m.file)).collect())
}

pub fn page_url(m: Option<&CfMod>, file_id: u64) -> String {
    match m.and_then(|m| m.links.as_ref()).and_then(|l| l.website_url.clone()) {
        Some(site) => format!("{}/files/{file_id}", site.trim_end_matches('/')),
        None => format!("https://www.curseforge.com/minecraft/mc-mods/{}/files/{file_id}", m.map(|m| m.slug.as_str()).unwrap_or("")),
    }
}

/// Install the newest compatible file of a mod into an instance.
pub async fn install_project(key: &str, instance_id: &str, mod_id: u64, kind: &str) -> Result<String> {
    let inst = instance::get(instance_id)?;
    let loader = if kind == "mod" { Some(inst.loader) } else { None };
    let fs = files(key, mod_id, Some(&inst.mc_version), loader).await?;
    let f = fs.first().ok_or_else(|| anyhow!("No file of this project works with {} {}", inst.loader.label(), inst.mc_version))?;
    let Some(dl) = &f.download_url else {
        let m = mods_by_id(key, &[mod_id]).await.ok().and_then(|v| v.into_iter().next());
        bail!("This mod's author only allows downloads from CurseForge's website: {}", page_url(m.as_ref(), f.id));
    };
    let name = super::safe_rel_path(&f.file_name).and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string())).context("bad file name")?;
    let dir = instance::game_dir(instance_id).join(super::modrinth::folder_for(kind));
    fetch(&super::http(), &Job { url: dl.clone(), dest: dir.join(&name), sha1: f.sha1(), sha512: None, size: Some(f.file_length) }).await?;
    Ok(name)
}

pub async fn download_pack(key: &str, mod_id: u64) -> Result<std::path::PathBuf> {
    let fs = files(key, mod_id, None, None).await?;
    let f = fs.first().ok_or_else(|| anyhow!("that modpack has no files"))?;
    let Some(dl) = &f.download_url else {
        let m = mods_by_id(key, &[mod_id]).await.ok().and_then(|v| v.into_iter().next());
        bail!("This modpack's author only allows downloads from CurseForge's website: {}", page_url(m.as_ref(), f.id));
    };
    let dest = super::dir().join("downloads").join(&f.file_name);
    fetch(&super::http(), &Job { url: dl.clone(), dest: dest.clone(), sha1: f.sha1(), sha512: None, size: Some(f.file_length) }).await?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn murmur2_known_values() {
        // Reference values for MurmurHash2 (32-bit, seed 1).
        assert_eq!(murmur2(b"", 1), 0x5bd15e36);
        // Whitespace is ignored by the fingerprint.
        assert_eq!(fingerprint(b"a b\nc"), fingerprint(b"abc"));
    }
}
