//! SteamGridDB: community covers, hero banners and logos for any game.
//! Needs a free API key from steamgriddb.com (Preferences → API).

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

const BASE: &str = "https://www.steamgriddb.com/api/v2";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub year: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Art {
    pub url: String,
    pub thumb: String,
    pub width: u32,
    pub height: u32,
    pub style: String,
}

fn none<T>() -> Option<T> {
    None
}

async fn get<T: for<'de> Deserialize<'de>>(key: &str, path: &str, query: &[(&str, &str)]) -> Result<T> {
    if key.trim().is_empty() {
        bail!("Add a SteamGridDB API key in Settings first (it's free at steamgriddb.com → Preferences → API)");
    }
    #[derive(Deserialize)]
    #[serde(bound = "T: Deserialize<'de>")]
    struct Resp<T> {
        success: bool,
        #[serde(default = "none")]
        data: Option<T>,
        #[serde(default)]
        errors: Vec<String>,
    }
    let r = reqwest::Client::builder()
        .user_agent(concat!("Arcadia/", env!("CARGO_PKG_VERSION")))
        .build()?
        .get(format!("{BASE}{path}"))
        .bearer_auth(key.trim())
        .query(query)
        .send()
        .await?;
    if r.status() == reqwest::StatusCode::UNAUTHORIZED {
        bail!("SteamGridDB didn't accept the API key");
    }
    if r.status() == reqwest::StatusCode::NOT_FOUND {
        bail!("SteamGridDB has nothing for that game yet");
    }
    let body: Resp<T> = r.error_for_status()?.json().await?;
    match (body.success, body.data) {
        (true, Some(d)) => Ok(d),
        _ => bail!("SteamGridDB: {}", body.errors.join(", ")),
    }
}

pub async fn search(key: &str, term: &str) -> Result<Vec<Found>> {
    #[derive(Deserialize)]
    struct Raw {
        id: u64,
        name: String,
        #[serde(default)]
        release_date: Option<i64>,
    }
    let q: String = term.chars().filter(|c| !matches!(c, '/' | '?' | '#' | '%')).collect();
    let raw: Vec<Raw> = get(key, &format!("/search/autocomplete/{}", q.trim()), &[]).await?;
    Ok(raw
        .into_iter()
        .map(|r| Found {
            id: r.id,
            name: r.name,
            year: r.release_date.map(|t| time::OffsetDateTime::from_unix_timestamp(t).map(|d| d.year()).unwrap_or(0)).filter(|y| *y > 0),
        })
        .collect())
}

/// "grids" (covers), "heroes" or "logos" for a SteamGridDB game id, or for a
/// Steam app id when `steam` is true.
pub async fn art(key: &str, kind: &str, id: u64, steam: bool) -> Result<Vec<Art>> {
    let kind = match kind {
        "cover" | "grids" => "grids",
        "hero" | "heroes" => "heroes",
        "logo" | "logos" => "logos",
        _ => bail!("Unknown art type"),
    };
    let path = format!("/{kind}/{}/{id}", if steam { "steam" } else { "game" });
    let mut q = vec![("nsfw", "false"), ("humor", "false")];
    if kind == "grids" {
        q.push(("dimensions", "600x900,342x482,660x930"));
    }
    #[derive(Deserialize)]
    struct Raw {
        url: String,
        thumb: String,
        #[serde(default)]
        width: u32,
        #[serde(default)]
        height: u32,
        #[serde(default)]
        style: String,
    }
    let raw: Vec<Raw> = get(key, &path, &q).await?;
    Ok(raw.into_iter().filter(|r| r.url.starts_with("https://")).take(60).map(|r| Art { url: r.url, thumb: r.thumb, width: r.width, height: r.height, style: r.style }).collect())
}
