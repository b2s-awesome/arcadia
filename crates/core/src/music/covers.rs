//! Album covers on the web, for Discord.
//!
//! Discord can only show pictures it can download from an https address:
//! it can't see the cover inside a file on this PC. So for the song you're
//! playing, Arcadia looks up the same album's cover on the web:
//!
//! 1. Songs from YouTube use the video's thumbnail (no lookup needed).
//! 2. Apple's iTunes Search (free, no account): the album, then the song.
//! 3. Deezer's search (free, no account) if Apple has nothing.
//!
//! Only the artist, album and title are sent, and only when Discord status
//! is on with covers allowed. Answers (including "nothing found") are kept in
//! `Music/covers.json`, so each album is looked up once; misses are retried
//! after a week.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

const RETRY_MISSES_AFTER: u64 = 7 * 24 * 3600;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Entry {
    url: Option<String>,
    at: u64,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn file() -> std::path::PathBuf {
    super::dir().join("covers.json")
}

fn load() -> HashMap<String, Entry> {
    std::fs::read(file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// The cache key: one per album (or per song when there's no album).
pub fn key(artist: &str, album: &str, title: &str) -> String {
    let a = fold(artist);
    let al = fold(album);
    if al.is_empty() || is_unknown(&al) { format!("{a}\u{0}song\u{0}{}", fold(title)) } else { format!("{a}\u{0}{al}") }
}

fn is_unknown(s: &str) -> bool {
    matches!(s, "unknown album" | "unknown" | "singles" | "single")
}

fn fold(s: &str) -> String {
    s.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A cached answer: `Some(Some(url))` found, `Some(None)` known miss,
/// `None` not looked up yet (or a miss old enough to try again).
pub fn cached(key: &str) -> Option<Option<String>> {
    let e = load().remove(key)?;
    if e.url.is_none() && now().saturating_sub(e.at) > RETRY_MISSES_AFTER {
        return None;
    }
    Some(e.url)
}

fn remember(key: &str, url: Option<String>) {
    static LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
    let _held = LOCK.lock();
    let mut all = load();
    all.insert(key.to_string(), Entry { url, at: now() });
    // Keep the file small: the newest 3000 albums.
    if all.len() > 3000 {
        let mut v: Vec<_> = all.into_iter().collect();
        v.sort_by(|a, b| b.1.at.cmp(&a.1.at));
        v.truncate(3000);
        all = v.into_iter().collect();
    }
    if let Ok(b) = serde_json::to_vec(&all) {
        let _ = crate::paths::write_atomic(&file(), &b);
    }
}

/// The YouTube thumbnail for a YouTube or YouTube Music link.
pub fn youtube_thumb(url: &str) -> Option<String> {
    let u = url::Url::parse(url).ok()?;
    let host = u.host_str()?.trim_start_matches("www.").trim_start_matches("m.").trim_start_matches("music.");
    let id = match host {
        "youtube.com" => u.query_pairs().find(|(k, _)| k == "v").map(|(_, v)| v.to_string()),
        "youtu.be" => u.path_segments()?.next().map(str::to_string),
        _ => None,
    }?;
    (id.len() == 11 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')).then(|| format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"))
}

/// How closely two names match, 0–1 (shared words over all words).
fn close(a: &str, b: &str) -> f64 {
    let (a, b) = (fold(a), fold(b));
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b || a.contains(&b) || b.contains(&a) {
        return 1.0;
    }
    let wa: std::collections::BTreeSet<&str> = a.split(' ').collect();
    let wb: std::collections::BTreeSet<&str> = b.split(' ').collect();
    wa.intersection(&wb).count() as f64 / wa.union(&wb).count().max(1) as f64
}

#[derive(Deserialize)]
struct ItunesResp {
    #[serde(default)]
    results: Vec<ItunesHit>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItunesHit {
    #[serde(default)]
    artist_name: String,
    #[serde(default)]
    collection_name: String,
    #[serde(default)]
    track_name: String,
    artwork_url100: Option<String>,
}

async fn itunes(http: &reqwest::Client, artist: &str, album: Option<&str>, title: &str) -> Option<String> {
    let (term, entity) = match album {
        Some(al) => (format!("{artist} {al}"), "album"),
        None => (format!("{artist} {title}"), "song"),
    };
    let r: ItunesResp = http
        .get("https://itunes.apple.com/search")
        .query(&[("term", term.as_str()), ("entity", entity), ("limit", "10"), ("media", "music")])
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    let best = r
        .results
        .into_iter()
        .filter_map(|h| {
            let s = close(&h.artist_name, artist) * 0.5
                + match album {
                    Some(al) => close(&h.collection_name, al),
                    None => close(&h.track_name, title),
                } * 0.5;
            Some((s, h.artwork_url100?))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))?;
    // Bigger art from the same address.
    (best.0 >= 0.7).then(|| best.1.replace("100x100bb", "600x600bb"))
}

#[derive(Deserialize)]
struct DeezerResp {
    #[serde(default)]
    data: Vec<DeezerHit>,
}
#[derive(Deserialize)]
struct DeezerHit {
    #[serde(default)]
    title: String,
    artist: Option<DeezerName>,
    album: Option<DeezerAlbum>,
}
#[derive(Deserialize)]
struct DeezerName {
    #[serde(default)]
    name: String,
}
#[derive(Deserialize)]
struct DeezerAlbum {
    #[serde(default)]
    title: String,
    cover_xl: Option<String>,
    cover_big: Option<String>,
}

async fn deezer(http: &reqwest::Client, artist: &str, album: Option<&str>, title: &str) -> Option<String> {
    let q = match album {
        Some(al) => format!("artist:\"{artist}\" album:\"{al}\""),
        None => format!("artist:\"{artist}\" track:\"{title}\""),
    };
    let r: DeezerResp = http.get("https://api.deezer.com/search").query(&[("q", q.as_str()), ("limit", "10")]).send().await.ok()?.json().await.ok()?;
    let best = r
        .data
        .into_iter()
        .filter_map(|h| {
            let al = h.album?;
            let s = close(&h.artist.map(|a| a.name).unwrap_or_default(), artist) * 0.5
                + match album {
                    Some(want) => close(&al.title, want),
                    None => close(&h.title, title),
                } * 0.5;
            Some((s, al.cover_xl.or(al.cover_big)?))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))?;
    (best.0 >= 0.7 && best.1.starts_with("https://")).then_some(best.1)
}

/// Find a cover for a song, from the cache or the web. `source_url` is
/// where an online song came from.
pub async fn find(artist: &str, album: &str, title: &str, source_url: Option<&str>) -> Option<String> {
    if let Some(u) = source_url.and_then(youtube_thumb) {
        return Some(u);
    }
    if artist.trim().is_empty() || fold(artist) == "unknown artist" {
        return None;
    }
    let key = key(artist, album, title);
    if let Some(hit) = cached(&key) {
        return hit;
    }
    let http = reqwest::Client::builder().user_agent(concat!("Arcadia/", env!("CARGO_PKG_VERSION"))).timeout(std::time::Duration::from_secs(8)).build().ok()?;
    let album = Some(album).filter(|a| !a.trim().is_empty() && !is_unknown(&fold(a)));
    let mut url = itunes(&http, artist, album, title).await;
    if url.is_none() && album.is_some() {
        url = itunes(&http, artist, None, title).await;
    }
    if url.is_none() {
        url = deezer(&http, artist, album, title).await;
    }
    if url.is_none() && album.is_some() {
        url = deezer(&http, artist, None, title).await;
    }
    remember(&key, url.clone());
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn youtube_links() {
        assert_eq!(youtube_thumb("https://music.youtube.com/watch?v=dQw4w9WgXcQ&list=x").as_deref(), Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg"));
        assert_eq!(youtube_thumb("https://youtu.be/dQw4w9WgXcQ").as_deref(), Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg"));
        assert!(youtube_thumb("https://soundcloud.com/a/b").is_none());
        assert!(youtube_thumb("https://youtube.com/watch?v=../../x").is_none());
    }

    #[test]
    fn keys_share_an_album() {
        assert_eq!(key("Type R", "Sextrance Worldwide Vol. 2", "PUREHEART"), key("type r", "Sextrance Worldwide Vol 2", "Other"));
        assert_ne!(key("x", "Unknown album", "a"), key("x", "Unknown album", "b"));
    }

    #[test]
    fn closeness() {
        assert_eq!(close("BUCKSHOT", "Buckshot"), 1.0);
        assert!(close("Cyber Trance", "Cyber Trance (Deluxe)") >= 0.99);
        assert!(close("Glow", "Lantern") < 0.1);
    }
}
