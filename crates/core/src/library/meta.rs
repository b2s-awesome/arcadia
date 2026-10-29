//! Your own information about each game: favourite, hidden, collections,
//! notes, launch options, scripts, custom art, playtime and how long it is
//! to beat. Kept in one file, keyed by game id.

use super::{Game, LaunchTarget, Source};
use anyhow::Result;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Beat {
    /// Hours for the main story, main + extras, and everything.
    pub main: Option<f32>,
    pub extra: Option<f32>,
    pub complete: Option<f32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct GameMeta {
    pub favourite: bool,
    pub hidden: bool,
    pub collections: Vec<String>,
    pub notes: String,
    /// Extra arguments for the game.
    pub launch_args: String,
    /// Commands run before the game starts and after it closes.
    pub before: String,
    pub after: String,
    /// Your own art: a URL or a file in Arcadia's art folder.
    pub cover: Option<String>,
    pub hero: Option<String>,
    pub logo: Option<String>,
    /// Seconds played while Arcadia was watching.
    pub playtime: u64,
    pub sessions: u32,
    pub last_played: Option<u64>,
    pub beat: Beat,
    /// Your own name for it.
    pub rename: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Collection {
    pub name: String,
    /// An emoji or short label shown beside the name.
    pub icon: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Meta {
    pub games: BTreeMap<String, GameMeta>,
    pub collections: Vec<Collection>,
}

fn file() -> PathBuf {
    crate::paths::root().join("library-meta.json")
}

/// Folder for art you chose from your own files.
pub fn art_dir() -> PathBuf {
    crate::paths::root().join("art")
}

static LOCK: Mutex<()> = Mutex::new(());

pub fn load() -> Meta {
    std::fs::read(file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn edit<T>(f: impl FnOnce(&mut Meta) -> T) -> Result<T> {
    let _g = LOCK.lock();
    let mut m = load();
    let out = f(&mut m);
    // Don't keep empty entries around.
    m.games.retain(|_, g| *g != GameMeta::default());
    let p = file();
    std::fs::create_dir_all(p.parent().unwrap())?;
    crate::paths::write_atomic(&p, &serde_json::to_vec_pretty(&m)?)?;
    Ok(out)
}

pub fn get(id: &str) -> GameMeta {
    load().games.get(id).cloned().unwrap_or_default()
}

/// Replace a game's info, keeping playtime (which only Arcadia changes).
pub fn set(id: &str, mut g: GameMeta) -> Result<()> {
    edit(|m| {
        let old = m.games.get(id).cloned().unwrap_or_default();
        g.playtime = old.playtime;
        g.sessions = old.sessions;
        g.last_played = old.last_played;
        g.notes = g.notes.chars().take(20_000).collect();
        m.games.insert(id.to_string(), g);
    })
}

pub fn add_play(id: &str, secs: u64, ended: u64) -> Result<()> {
    edit(|m| {
        let g = m.games.entry(id.to_string()).or_default();
        g.playtime += secs;
        g.sessions += 1;
        g.last_played = Some(ended);
    })
}

pub fn set_collections(list: Vec<Collection>) -> Result<()> {
    edit(|m| {
        let mut seen = std::collections::HashSet::new();
        m.collections = list.into_iter().filter(|c| !c.name.trim().is_empty() && seen.insert(c.name.trim().to_lowercase())).collect();
        // Drop games' links to collections that are gone.
        let names: Vec<String> = m.collections.iter().map(|c| c.name.clone()).collect();
        for g in m.games.values_mut() {
            g.collections.retain(|c| names.contains(c));
        }
    })
}

/// Copy a picture into Arcadia's art folder; returns its new path.
pub fn import_art(src: &std::path::Path) -> Result<String> {
    let ext = src.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default();
    if !["png", "jpg", "jpeg", "webp", "gif"].contains(&ext.as_str()) {
        anyhow::bail!("Pick a PNG, JPEG, WebP or GIF picture");
    }
    let d = art_dir();
    std::fs::create_dir_all(&d)?;
    let out = d.join(format!("{}.{ext}", uuid::Uuid::new_v4().simple()));
    std::fs::copy(src, &out)?;
    Ok(out.to_string_lossy().into_owned())
}

/// The launch target with your extra arguments added.
pub fn with_args(target: &LaunchTarget, args: &str, source: Source) -> LaunchTarget {
    let extra = crate::minecraft::launch::split_args(args);
    if extra.is_empty() {
        return target.clone();
    }
    match target {
        LaunchTarget::Exe { path, args, cwd } => LaunchTarget::Exe { path: path.clone(), args: args.iter().cloned().chain(extra).collect(), cwd: cwd.clone() },
        // Steam takes arguments after the app id: steam://run/<id>//<args>
        LaunchTarget::Uri { uri } if source == Source::Steam => {
            let id = uri.rsplit('/').next().unwrap_or_default();
            let joined = extra.join(" ");
            LaunchTarget::Uri { uri: format!("steam://run/{id}//{}", joined.replace(' ', "%20")) }
        }
        other => other.clone(),
    }
}

/// Playtime and last-played times Steam keeps for each app, per Steam user,
/// merged by taking the largest. Minutes and Unix seconds.
pub fn steam_playtime() -> BTreeMap<String, (u64, u64)> {
    let mut out = BTreeMap::new();
    let Some(root) = super::steam::root() else { return out };
    let Ok(users) = std::fs::read_dir(root.join("userdata")) else { return out };
    for u in users.flatten() {
        let Ok(src) = std::fs::read_to_string(u.path().join("config/localconfig.vdf")) else { continue };
        merge_steam(&src, &mut out);
    }
    out
}

pub(crate) fn merge_steam(src: &str, out: &mut BTreeMap<String, (u64, u64)>) {
    let v = super::vdf::parse(src);
    let apps = v.get("UserLocalConfigStore").and_then(|x| x.get("Software")).and_then(|x| x.get("Valve")).and_then(|x| x.get("Steam")).and_then(|x| x.get("apps"));
    let Some(apps) = apps else { return };
    for (id, a) in apps.entries() {
        let mins: u64 = a.str("Playtime").and_then(|s| s.parse().ok()).unwrap_or(0);
        let last: u64 = a.str("LastPlayed").and_then(|s| s.parse().ok()).unwrap_or(0);
        if mins == 0 && last == 0 {
            continue;
        }
        let e = out.entry(format!("steam:{id}")).or_insert((0, 0));
        e.0 = e.0.max(mins);
        e.1 = e.1.max(last);
    }
}

/// A game as the library shows it: launcher info plus your own.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    #[serde(flatten)]
    pub game: Game,
    pub meta: GameMeta,
    /// Minutes Steam has recorded, for Steam games.
    pub steam_minutes: Option<u64>,
    pub steam_last: Option<u64>,
}

pub fn entries(games: &[Game]) -> Vec<Entry> {
    let m = load();
    let steam = steam_playtime();
    games
        .iter()
        .map(|g| {
            let s = steam.get(&g.id);
            Entry { game: g.clone(), meta: m.games.get(&g.id).cloned().unwrap_or_default(), steam_minutes: s.map(|x| x.0).filter(|x| *x > 0), steam_last: s.map(|x| x.1).filter(|x| *x > 0) }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_launch_arguments() {
        let exe = LaunchTarget::Exe { path: "g.exe".into(), args: vec!["-a".into()], cwd: None };
        assert_eq!(with_args(&exe, "-windowed \"-name x\"", Source::Custom), LaunchTarget::Exe { path: "g.exe".into(), args: vec!["-a".into(), "-windowed".into(), "-name x".into()], cwd: None });
        let steam = LaunchTarget::Uri { uri: "steam://rungameid/730".into() };
        assert_eq!(with_args(&steam, "-novid -high", Source::Steam), LaunchTarget::Uri { uri: "steam://run/730//-novid%20-high".into() });
        assert_eq!(with_args(&steam, "  ", Source::Steam), steam);
    }

    #[test]
    fn reads_steam_playtime() {
        let src = r#""UserLocalConfigStore" { "Software" { "Valve" { "Steam" { "apps" {
            "730" { "LastPlayed" "1700000000" "Playtime" "6123" }
            "1086940" { "Playtime" "90" }
            "5" { "cloud" { "x" "y" } }
        } } } } }"#;
        let mut out = BTreeMap::new();
        merge_steam(src, &mut out);
        assert_eq!(out["steam:730"], (6123, 1_700_000_000));
        assert_eq!(out["steam:1086940"], (90, 0));
        assert!(!out.contains_key("steam:5"));
    }
}
