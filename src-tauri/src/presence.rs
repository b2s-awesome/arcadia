//! Discord Rich Presence: shows what you're doing in Arcadia on your Discord
//! profile. Games win over music; music over "Browsing Arcadia".
//!
//! Everything else just reports what's happening (`game_start`, `game_update`,
//! `game_end`, `music`); one thread owns the Discord connection, reconnects
//! when Discord starts, and sends at most one update every few seconds.

use crate::{err, AppState, Res};
use arcadia_core::discord::{self, Activity};
use parking_lot::{Condvar, Mutex};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const ADDON: &str = "discord-presence";

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Prefs {
    client_id: String,
    idle: bool,
    music: bool,
    roblox: bool,
    roblox_server: bool,
    roblox_join: bool,
    minecraft: bool,
    minecraft_server: bool,
    games: bool,
    elapsed: bool,
    hide_names: bool,
    /// Look up album covers on the web (Discord can't see covers in files).
    covers: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self { client_id: String::new(), idle: false, music: true, roblox: true, roblox_server: true, roblox_join: false, minecraft: true, minecraft_server: true, games: true, elapsed: true, hide_names: false, covers: true }
    }
}

fn prefs(app: &AppHandle) -> Option<Prefs> {
    let st = app.state::<AppState>();
    let s = st.settings.lock();
    let on = s.ui["addons"]["installed"].as_array().is_some_and(|a| a.iter().any(|v| v.as_str() == Some(ADDON)));
    if !on {
        return None;
    }
    let p: Prefs = serde_json::from_value(s.ui["presence"].clone()).unwrap_or_default();
    discord::valid_id(&p.client_id).then_some(p)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Roblox { place: Option<u64>, job: Option<String> },
    Minecraft { version: String, loader: String, server: Option<String> },
    Game { launcher: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    pub kind: Kind,
    pub title: String,
    /// Region for Roblox servers.
    pub detail: Option<String>,
    /// An https picture (game icon or cover).
    pub image: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct Entry {
    key: String,
    game: Game,
    started: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Music {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    pub pos: f64,
    pub duration: f64,
    /// Where an online song came from (its YouTube thumbnail is the cover).
    pub source_url: Option<String>,
}

#[derive(Default)]
struct Inner {
    games: Vec<Entry>,
    music: Option<(Music, u64)>,
    /// The playing song's cover on the web: (track id, address or none).
    cover: Option<(i64, Option<String>)>,
    /// A lookup is running for this track.
    looking: Option<i64>,
    paused_since: Option<u64>,
    dirty: bool,
    status: Status,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    connected: bool,
    user: Option<String>,
    error: Option<String>,
    showing: Option<Activity>,
}

struct Presence {
    inner: Mutex<Inner>,
    cv: Condvar,
}

fn p() -> &'static Presence {
    static P: OnceLock<Presence> = OnceLock::new();
    P.get_or_init(|| Presence { inner: Mutex::new(Inner::default()), cv: Condvar::new() })
}

fn poke_with(f: impl FnOnce(&mut Inner) -> bool) {
    let pr = p();
    let mut i = pr.inner.lock();
    if f(&mut i) {
        i.dirty = true;
        pr.cv.notify_one();
    }
}

/// Settings changed: look again.
pub fn poke() {
    poke_with(|_| true);
}

pub fn game_start(key: &str, game: Game) {
    poke_with(|i| {
        i.games.retain(|e| e.key != key);
        i.games.push(Entry { key: key.into(), game, started: now_ms() });
        true
    });
}

pub fn game_update(key: &str, f: impl FnOnce(&mut Game)) {
    poke_with(|i| match i.games.iter_mut().find(|e| e.key == key) {
        Some(e) => {
            let before = e.game.clone();
            f(&mut e.game);
            e.game != before
        }
        None => false,
    });
}

pub fn game_end(key: &str) {
    poke_with(|i| {
        let n = i.games.len();
        i.games.retain(|e| e.key != key);
        i.games.len() != n
    });
}

/// From the music player. Only real changes (new song, play/pause, a seek)
/// count, not the clock ticking.
pub fn music(m: Option<Music>) {
    poke_with(|i| {
        let now = now_ms();
        let changed = match (&i.music, &m) {
            (None, None) => false,
            (Some((old, at)), Some(new)) => {
                let expected = if old.playing { old.pos + (now - at) as f64 / 1000.0 } else { old.pos };
                old.id != new.id || old.playing != new.playing || (new.pos - expected).abs() > 3.0
            }
            _ => true,
        };
        if changed {
            match &m {
                Some(x) if !x.playing => {
                    i.paused_since.get_or_insert(now);
                }
                _ => i.paused_since = None,
            }
            i.music = m.map(|x| (x, now));
        }
        changed
    });
}

/// Start looking up the playing song's cover, if it isn't known yet.
fn want_cover(i: &mut Inner) {
    let Some((m, _)) = &i.music else { return };
    if i.cover.as_ref().is_some_and(|(id, _)| *id == m.id) || i.looking == Some(m.id) {
        return;
    }
    let m = m.clone();
    i.looking = Some(m.id);
    tauri::async_runtime::spawn(async move {
        let url = arcadia_core::music::covers::find(&m.artist, &m.album, &m.title, m.source_url.as_deref()).await;
        poke_with(|i| {
            if i.looking == Some(m.id) {
                i.looking = None;
            }
            i.cover = Some((m.id, url));
            true
        });
    });
}

fn hide(p: &Prefs, s: &str, generic: &str) -> String {
    if p.hide_names { generic.to_string() } else { s.to_string() }
}

/// What to show right now.
fn desired(i: &Inner, p: &Prefs) -> Option<Activity> {
    let now = now_ms();
    let shown = i.games.iter().rev().find(|e| match e.game.kind {
        Kind::Roblox { .. } => p.roblox,
        Kind::Minecraft { .. } => p.minecraft,
        Kind::Game { .. } => p.games,
    });
    if let Some(e) = shown {
        let g = &e.game;
        let start = p.elapsed.then_some(e.started);
        let mut a = Activity { kind: 0, start, small_image: "arcadia".into(), small_text: "via Arcadia".into(), ..Default::default() };
        match &g.kind {
            Kind::Roblox { place, job } => {
                a.details = hide(p, &g.title, "Playing a Roblox game");
                a.state = match (&g.detail, p.roblox_server) {
                    (Some(r), true) => format!("Roblox · {r}"),
                    _ => "Roblox".into(),
                };
                a.large_image = if p.hide_names { "game".into() } else { g.image.clone().unwrap_or_else(|| "game".into()) };
                a.large_text = hide(p, &g.title, "Roblox");
                if let (Some(pl), false) = (place, p.hide_names) {
                    if let (Some(j), true) = (job, p.roblox_join) {
                        a.buttons.push(("Join my server".into(), format!("https://www.roblox.com/games/start?placeId={pl}&gameInstanceId={j}")));
                    }
                    a.buttons.push(("View game".into(), format!("https://www.roblox.com/games/{pl}")));
                }
            }
            Kind::Minecraft { version, loader, server } => {
                a.details = hide(p, &g.title, "Playing Minecraft");
                a.state = match (server, p.minecraft_server && !p.hide_names) {
                    (Some(s), true) => format!("On {s}"),
                    _ => format!("Minecraft {version}{}", if loader.is_empty() || loader == "Vanilla" { String::new() } else { format!(" · {loader}") }),
                };
                a.large_image = "cube".into();
                a.large_text = format!("Minecraft {version}");
            }
            Kind::Game { launcher } => {
                a.details = hide(p, &g.title, "Playing a game");
                a.state = format!("via {launcher}");
                a.large_image = if p.hide_names { "game".into() } else { g.image.clone().unwrap_or_else(|| "game".into()) };
                a.large_text = hide(p, &g.title, "A game");
            }
        }
        return Some(a);
    }
    if p.music {
        if let Some((m, at)) = &i.music {
            let paused_long = i.paused_since.is_some_and(|t| now - t > 10 * 60 * 1000);
            if !paused_long {
                let mut a = Activity {
                    kind: 2,
                    details: m.title.clone(),
                    state: if m.artist.is_empty() { "Arcadia Music".into() } else { format!("by {}", m.artist) },
                    large_image: match &i.cover {
                        Some((id, Some(url))) if *id == m.id && p.covers && !p.hide_names => url.clone(),
                        _ => "music".into(),
                    },
                    large_text: if m.album.is_empty() { m.title.clone() } else { m.album.clone() },
                    ..Default::default()
                };
                if m.playing {
                    // A progress bar: when the song started and when it ends.
                    let elapsed = (m.pos * 1000.0) as u64 + (now - at);
                    let start = now.saturating_sub(elapsed);
                    a.start = Some(start);
                    if m.duration > 0.0 {
                        a.end = Some(start + (m.duration * 1000.0) as u64);
                    }
                } else {
                    a.small_image = "paused".into();
                    a.small_text = "Paused".into();
                    a.state = format!("Paused · {}", if m.artist.is_empty() { &m.title } else { &m.artist });
                }
                return Some(a);
            }
        }
    }
    p.idle.then(|| Activity { kind: 0, details: "Browsing Arcadia".into(), large_image: "arcadia".into(), large_text: "Arcadia".into(), ..Default::default() })
}

/// The thread that talks to Discord.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let pr = p();
        let mut client: Option<(discord::Client, String)> = None;
        let mut last: Option<Option<Activity>> = None;
        let mut last_send = Instant::now() - Duration::from_secs(60);
        let mut next_try = Instant::now();
        loop {
            {
                let mut i = pr.inner.lock();
                if !i.dirty {
                    pr.cv.wait_for(&mut i, Duration::from_secs(15));
                }
                i.dirty = false;
            }
            let Some(prefs) = prefs(&app) else {
                if let Some((mut c, _)) = client.take() {
                    let _ = c.set(None);
                }
                last = None;
                pr.inner.lock().status = Status::default();
                continue;
            };
            // A different app id: start over.
            if client.as_ref().is_some_and(|(_, id)| *id != prefs.client_id) {
                if let Some((mut c, _)) = client.take() {
                    let _ = c.set(None);
                }
                last = None;
            }
            if client.is_none() {
                if Instant::now() < next_try {
                    continue;
                }
                match discord::Client::connect(&prefs.client_id) {
                    Ok(c) => {
                        let mut i = pr.inner.lock();
                        i.status.connected = true;
                        i.status.user = Some(c.user.clone()).filter(|u| !u.is_empty());
                        i.status.error = None;
                        client = Some((c, prefs.client_id.clone()));
                        last = None;
                    }
                    Err(e) => {
                        next_try = Instant::now() + Duration::from_secs(15);
                        let mut i = pr.inner.lock();
                        i.status = Status { error: Some(format!("{e:#}")), ..Default::default() };
                        continue;
                    }
                }
            }
            if prefs.music && prefs.covers && !prefs.hide_names {
                want_cover(&mut pr.inner.lock());
            }
            let want = desired(&pr.inner.lock(), &prefs);
            if last.as_ref() == Some(&want) {
                continue;
            }
            // Discord allows about five updates every 20 seconds.
            let wait = Duration::from_secs(4).saturating_sub(last_send.elapsed());
            if !wait.is_zero() {
                std::thread::sleep(wait);
            }
            let want = desired(&pr.inner.lock(), &prefs);
            let (c, _) = client.as_mut().unwrap();
            last_send = Instant::now();
            match c.set(want.as_ref()) {
                Ok(()) => {
                    pr.inner.lock().status.showing = want.clone();
                    last = Some(want);
                }
                Err(e) => {
                    log::warn!("discord: {e:#}");
                    client = None;
                    next_try = Instant::now() + Duration::from_secs(5);
                    let mut i = pr.inner.lock();
                    i.status = Status { error: Some(format!("{e:#}")), ..Default::default() };
                    i.dirty = true;
                }
            }
        }
    });
}

#[tauri::command]
pub fn presence_status() -> Status {
    p().inner.lock().status.clone()
}

/// Try an Application ID: connect, show a test status for a moment, clear.
#[tauri::command]
pub async fn presence_test(client_id: String) -> Res<String> {
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<String> {
        let mut c = discord::Client::connect(&client_id)?;
        c.set(Some(&Activity { details: "Setting up Arcadia".into(), state: "Rich presence works!".into(), large_image: "arcadia".into(), large_text: "Arcadia".into(), start: Some(now_ms()), ..Default::default() }))?;
        std::thread::sleep(Duration::from_secs(8));
        let _ = c.set(None);
        Ok(c.user)
    })
    .await
    .map_err(err)?
    .map_err(err)
    .inspect(|_| poke())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inner(games: Vec<Entry>, music: Option<Music>) -> Inner {
        Inner { games, music: music.map(|m| (m, now_ms())), ..Default::default() }
    }

    #[test]
    fn picks_what_to_show() {
        let p = Prefs { client_id: "1234567890123456789".into(), ..Default::default() };
        let rbx = Entry {
            key: "rbx:1".into(),
            started: 5,
            game: Game { kind: Kind::Roblox { place: Some(42), job: Some("j".into()) }, title: "Blox Fruits".into(), detail: Some("London, United Kingdom".into()), image: Some("https://tr.rbxcdn.com/x".into()) },
        };
        let song = Music { id: 1, title: "Lantern Lights".into(), artist: "Ivy".into(), album: "Glow".into(), playing: true, pos: 30.0, duration: 200.0, source_url: None };

        // A game beats music.
        let a = desired(&inner(vec![rbx.clone()], Some(song.clone())), &p).unwrap();
        assert_eq!((a.details.as_str(), a.state.as_str()), ("Blox Fruits", "Roblox · London, United Kingdom"));
        assert_eq!(a.buttons.len(), 1, "no join button unless asked");
        let join = Prefs { roblox_join: true, ..p.clone() };
        assert!(desired(&inner(vec![rbx.clone()], None), &join).unwrap().buttons[0].1.contains("gameInstanceId=j"));

        // Hiding names hides the game and buttons.
        let hidden = desired(&inner(vec![rbx.clone()], None), &Prefs { hide_names: true, ..p.clone() }).unwrap();
        assert_eq!(hidden.details, "Playing a Roblox game");
        assert!(hidden.buttons.is_empty() && hidden.large_image == "game");

        // Music with a progress bar.
        let m = desired(&inner(vec![], Some(song.clone())), &p).unwrap();
        assert_eq!(m.kind, 2);
        assert_eq!(m.end.unwrap() - m.start.unwrap(), 200_000);
        assert_eq!(m.large_image, "music", "no cover known yet");
        let mut with_cover = inner(vec![], Some(song.clone()));
        with_cover.cover = Some((1, Some("https://is1-ssl.mzstatic.com/x/600x600bb.jpg".into())));
        assert!(desired(&with_cover, &p).unwrap().large_image.starts_with("https://"));
        with_cover.cover = Some((2, Some("https://other".into())));
        assert_eq!(desired(&with_cover, &p).unwrap().large_image, "music", "a cover for another song isn't used");

        // Turned off, nothing (idle is off by default).
        assert!(desired(&inner(vec![rbx], Some(song)), &Prefs { roblox: false, music: false, ..p.clone() }).is_none());
        assert_eq!(desired(&inner(vec![], None), &Prefs { idle: true, ..p }).unwrap().details, "Browsing Arcadia");
    }
}
