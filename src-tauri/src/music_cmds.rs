//! The music player: queue + library + audio engine, and its commands.

use crate::{err, Res};
use arcadia_audio::{Engine, Event, OutputInfo};
use arcadia_core::music::{self, stats::Stats, Library, Playlist, Queue, Repeat, ScanReport, ShuffleInfo, ShuffleMode, Track};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Session {
    queue: Queue,
    pos: f64,
    volume: Option<f32>,
    device: Option<String>,
}

struct Player {
    queue: Queue,
    current: Option<i64>,
    pos: f64,
    duration: f64,
    paused: bool,
    /// 0–1 as the slider shows it; the engine gets a curve of it.
    volume: f32,
    device: Option<String>,
    output: Option<OutputInfo>,
    output_error: Option<String>,
    counted: bool,
    /// Consecutive tracks that failed to open, so a broken queue can't spin.
    failures: usize,
    saved_at: std::time::Instant,
    /// An online song being fetched before it can play.
    loading: Option<i64>,
    /// The current song is chosen but nothing is loaded in the engine yet
    /// (an online song restored from last time: fetched when Play is pressed).
    pending: bool,
    /// Listening time on the current song, for stats.
    rec: Option<Rec>,
    /// A–B loop on the current song (Loopy Loop).
    ab: Option<(f64, f64)>,
}

struct Rec {
    id: i64,
    listened: f64,
    last: f64,
    duration: f64,
}

/// Trash Bin + Auto Skip: what to pass over when moving on by itself.
#[derive(Default, Deserialize, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct SkipRules {
    trash_songs: std::collections::HashSet<i64>,
    trash_artists: Vec<String>,
    /// Skip what's in the trash (on unless turned off).
    #[serde(default = "yes")]
    trashed: bool,
    explicit: bool,
    /// Under 30 seconds (intros, skits).
    short: bool,
    live: bool,
    remix: bool,
}

fn yes() -> bool {
    true
}

const TRASH: &str = "trash-bin";
const SHUFFLE_PLUS: &str = "shuffle-plus";

pub struct Music {
    pub lib: Library,
    engine: Engine,
    p: Mutex<Player>,
    app: AppHandle,
    rules: Mutex<SkipRules>,
    /// The song the media overlay shows.
    media_track: Mutex<Option<i64>>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    track: Option<Track>,
    playing: bool,
    pub(crate) pos: f64,
    duration: f64,
    volume: f32,
    shuffle: bool,
    repeat: Repeat,
    queue_index: Option<usize>,
    queue_length: usize,
    output: Option<OutputInfo>,
    output_error: Option<String>,
    device: Option<String>,
    /// Fetching the current online song.
    loading: bool,
    shuffle_mode: ShuffleMode,
    /// The A–B loop, in seconds.
    loop_ab: Option<(f64, f64)>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueView {
    index: Option<usize>,
    tracks: Vec<Track>,
}

fn gain(v: f32) -> f32 {
    // Loudness follows the slider: roughly −40 dB at the bottom, 0 at the top.
    let v = v.clamp(0.0, 1.0);
    if v <= 0.0 {
        0.0
    } else {
        v * v * v
    }
}

impl Music {
    pub fn start(app: &AppHandle) -> anyhow::Result<Arc<Music>> {
        let lib = Library::open(&music::dir())?;
        if lib.folders()?.is_empty() {
            if let Some(d) = dirs_audio() {
                lib.set_folders(&[d])?;
            }
        }
        let session: Session = std::fs::read(music::session_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let h = app.clone();
        let engine = Engine::start(move |e| {
            if let Some(m) = h.try_state::<Arc<Music>>() {
                m.inner().clone().on_event(&h, e);
            } else if let Event::Output(Err(e)) = e {
                // Before the player is registered; a working output is
                // remembered by the engine itself.
                log::warn!("audio output: {e}");
            }
        });
        let volume = session.volume.unwrap_or(0.8);
        engine.set_volume(gain(volume));
        if session.device.is_some() {
            engine.set_device(session.device.clone());
        }
        let m = Arc::new(Music {
            lib,
            engine,
            p: Mutex::new(Player {
                queue: session.queue,
                current: None,
                pos: session.pos,
                duration: 0.0,
                paused: true,
                volume,
                device: session.device,
                output: None,
                output_error: None,
                counted: false,
                failures: 0,
                saved_at: std::time::Instant::now(),
                loading: None,
                pending: false,
                rec: None,
                ab: None,
            }),
            app: app.clone(),
            rules: Mutex::new(load_rules()),
            media_track: Mutex::new(None),
        });
        // Pick up where the last session left off, paused.
        let resume = m.p.lock().queue.current();
        if let Some(id) = resume {
            let pos = m.p.lock().pos;
            m.load(id, pos, true);
        }
        Ok(m)
    }

    fn load(&self, id: i64, start: f64, paused: bool) {
        self.finish_rec();
        {
            let mut p = self.p.lock();
            p.ab = None;
            p.current = Some(id);
            p.pos = start;
            p.duration = self.lib.track(id).ok().flatten().map(|t| t.duration).unwrap_or(0.0);
            p.counted = false;
            p.paused = paused;
            p.pending = false;
            p.loading = None;
        }
        if let Some(path) = self.lib.file_of(id) {
            self.engine.load(id as u64, path, start, paused);
            return;
        }
        if self.lib.source_of(id).is_none() {
            let _ = self.app.emit("music-error", "That song's file is missing. Rescan your library under Customise.");
            return;
        }
        // An online song: don't fetch just to sit paused (e.g. on startup).
        if paused {
            self.engine.stop();
            self.p.lock().pending = true;
            return;
        }
        self.engine.stop();
        self.p.lock().loading = Some(id);
        self.changed(&self.app);
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let r = crate::streaming_cmds::ensure_cached(&app, id).await;
            let Some(m) = app.try_state::<Arc<Music>>().map(|m| m.inner().clone()) else { return };
            let still = {
                let mut p = m.p.lock();
                let still = p.current == Some(id) && p.loading == Some(id);
                if still {
                    p.loading = None;
                }
                still
            };
            if !still {
                return;
            }
            match r {
                Ok(path) => {
                    m.engine.load(id as u64, path, start, false);
                    m.preload_next();
                }
                Err(e) => {
                    let _ = app.emit("music-error", e);
                    // Move on, as with a file that won't open.
                    let next = m.p.lock().queue.next_where(|i| m.skips(i));
                    match next {
                        Some(n) if n != id => m.load(n, 0.0, false),
                        _ => m.changed(&app),
                    }
                }
            }
        });
    }

    /// Queue up the next song for a gapless start, fetching it first if
    /// it's online.
    fn preload_next(&self) {
        let next = self.p.lock().queue.peek_next_where(|i| self.skips(i));
        let Some(id) = next else {
            self.engine.set_next(None);
            return;
        };
        if let Some(path) = self.lib.file_of(id) {
            self.engine.set_next(Some((id as u64, path)));
            return;
        }
        self.engine.set_next(None);
        if self.lib.source_of(id).is_none() {
            return;
        }
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            if crate::streaming_cmds::ensure_cached(&app, id).await.is_ok() {
                if let Some(m) = app.try_state::<Arc<Music>>() {
                    // Still the next one? Then it can follow on gaplessly.
                    let next = m.p.lock().queue.peek_next_where(|i| m.skips(i));
                    if next == Some(id) {
                        m.preload_next();
                    }
                }
            }
        });
    }

    /// The current and next songs (kept when trimming the stream cache).
    pub fn near_ids(&self) -> Vec<i64> {
        let p = self.p.lock();
        [p.current, p.queue.peek_next(), p.loading].into_iter().flatten().collect()
    }

    /// Whether moving on by itself should pass over this song.
    fn skips(&self, id: i64) -> bool {
        if crate::addon_cmds::installed(&self.app, TRASH).is_err() {
            return false;
        }
        let r = self.rules.lock().clone();
        if r.trashed && r.trash_songs.contains(&id) {
            return true;
        }
        if !(r.trashed && !r.trash_artists.is_empty()) && !r.explicit && !r.short && !r.live && !r.remix {
            return false;
        }
        let Ok(Some(t)) = self.lib.track(id) else { return false };
        let title = t.title.to_lowercase();
        (r.trashed && r.trash_artists.iter().any(|a| a.eq_ignore_ascii_case(&t.artist) || a.eq_ignore_ascii_case(&t.album_artist)))
            || (r.short && t.duration > 0.0 && t.duration < 30.0)
            || (r.live && (title.contains("(live") || title.contains("[live") || title.contains("- live") || title.contains(" live at ")))
            || (r.remix && title.contains("remix"))
            || (r.explicit && self.lib.explicit_of(id))
    }

    /// Count up listening on the song now playing.
    fn track_listening(&self, id: i64, pos: f64) {
        let done = {
            let mut p = self.p.lock();
            match &mut p.rec {
                Some(r) if r.id == id => {
                    let d = pos - r.last;
                    // Ordinary playback; a seek doesn't count as listening.
                    if d > 0.0 && d < 2.0 {
                        r.listened += d;
                    }
                    r.last = pos;
                    None
                }
                _ => {
                    let duration = p.duration;
                    p.rec.replace(Rec { id, listened: 0.0, last: pos, duration })
                }
            }
        };
        if let Some(r) = done {
            self.save_rec(r);
        }
    }

    /// The song changed or stopped: log what was heard of it.
    fn finish_rec(&self) {
        let r = self.p.lock().rec.take();
        if let Some(r) = r {
            self.save_rec(r);
        }
    }

    fn save_rec(&self, r: Rec) {
        // A few seconds of a song isn't really listening.
        if r.listened < 5.0 {
            return;
        }
        let skipped = r.listened < if r.duration > 0.0 { 30f64.min(r.duration * 0.5) } else { 30.0 };
        let app = self.app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Some(m) = app.try_state::<Arc<Music>>() {
                if let Err(e) = m.lib.record_play(r.id, r.listened, skipped) {
                    log::warn!("couldn't log a play: {e}");
                }
            }
        });
    }

    /// Give the queue what Shuffle+ needs to know about `ids`.
    fn shuffle_info(&self, ids: &[i64]) {
        let mode = self.p.lock().queue.mode;
        let info = self.info_for(mode, ids);
        self.p.lock().queue.info = info;
    }

    fn info_for(&self, mode: ShuffleMode, ids: &[i64]) -> std::collections::HashMap<i64, ShuffleInfo> {
        if mode == ShuffleMode::Random {
            return Default::default();
        }
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        self.lib
            .tracks_by_id(ids)
            .unwrap_or_default()
            .into_iter()
            .map(|t| (t.id, ShuffleInfo { artist: t.artist, plays: t.play_count, since_played: t.last_played.map(|l| now - l) }))
            .collect()
    }

    pub fn status(&self) -> Status {
        let p = self.p.lock();
        Status {
            track: p.current.and_then(|id| self.lib.track(id).ok().flatten()),
            playing: !p.paused && p.current.is_some(),
            pos: p.pos,
            duration: p.duration,
            volume: p.volume,
            shuffle: p.queue.shuffle,
            repeat: p.queue.repeat,
            queue_index: p.queue.pos,
            queue_length: p.queue.list.len(),
            output: p.output.clone().or_else(|| self.engine.output()),
            output_error: p.output_error.clone(),
            device: p.device.clone(),
            loading: p.loading.is_some(),
            shuffle_mode: p.queue.mode,
            loop_ab: p.ab,
        }
    }

    fn save(&self) {
        let p = self.p.lock();
        let s = Session { queue: p.queue.clone(), pos: p.pos, volume: Some(p.volume), device: p.device.clone() };
        drop(p);
        let path = music::session_file();
        if let Ok(b) = serde_json::to_vec(&s) {
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, b).is_ok() {
                let _ = std::fs::rename(tmp, path);
            }
        }
    }

    fn changed(&self, app: &AppHandle) {
        let s = self.status();
        // The Windows media overlay and media keys.
        let id = s.track.as_ref().map(|t| t.id);
        let new_track = {
            let mut last = self.media_track.lock();
            let changed = *last != id;
            *last = id;
            changed
        };
        match &s.track {
            Some(t) => {
                if new_track {
                    crate::media_keys::send(crate::media_keys::Update::Track { title: t.title.clone(), artist: t.artist.clone(), album: t.album.clone(), art: t.art.clone(), duration: s.duration });
                }
                crate::media_keys::send(crate::media_keys::Update::Playing { playing: s.playing, pos: s.pos });
            }
            None if new_track => crate::media_keys::send(crate::media_keys::Update::Stopped),
            None => {}
        }
        crate::presence::music(s.track.as_ref().map(|t| crate::presence::Music {
            id: t.id,
            title: t.title.clone(),
            artist: t.artist.clone(),
            album: t.album.clone(),
            playing: s.playing,
            pos: s.pos,
            duration: s.duration,
            source_url: t.source_url.clone(),
        }));
        let _ = app.emit("music-status", s);
    }

    fn on_event(self: Arc<Self>, app: &AppHandle, e: Event) {
        match e {
            Event::Track { token, pos, duration } => {
                let id = token as i64;
                let switching = self.p.lock().rec.as_ref().is_some_and(|r| r.id != id);
                if switching {
                    self.finish_rec();
                }
                {
                    let mut p = self.p.lock();
                    if p.queue.current() != Some(id) && p.queue.peek_next_where(|i| self.skips(i)) == Some(id) {
                        p.queue.advance_where(|i| self.skips(i));
                    }
                    if p.current != Some(id) {
                        p.ab = None;
                    }
                    p.current = Some(id);
                    p.pos = pos;
                    p.duration = duration.unwrap_or(p.duration);
                    p.counted = false;
                    p.failures = 0;
                }
                let missing = self.p.lock().duration <= 0.0;
                if missing {
                    if let Ok(Some(t)) = self.lib.track(id) {
                        self.p.lock().duration = t.duration;
                    }
                }
                self.preload_next();
                self.save();
                self.changed(app);
            }
            Event::Position { token, pos } => {
                let id = token as i64;
                if !self.p.lock().paused {
                    self.track_listening(id, pos);
                }
                let count = {
                    let mut p = self.p.lock();
                    if p.current != Some(id) {
                        return;
                    }
                    p.pos = pos;
                    // A play counts after 30 seconds or half the track.
                    let due = !p.counted && (pos >= 30.0 || (p.duration > 0.0 && pos >= p.duration * 0.5));
                    if due {
                        p.counted = true;
                    }
                    due
                };
                if count {
                    let _ = self.lib.mark_played(id);
                }
                // Every few seconds, so quitting mid-song resumes there.
                let due_save = {
                    let mut p = self.p.lock();
                    let due = p.saved_at.elapsed() >= std::time::Duration::from_secs(5);
                    if due {
                        p.saved_at = std::time::Instant::now();
                    }
                    due
                };
                if due_save {
                    self.save();
                }
                let _ = app.emit("music-position", pos);
            }
            Event::Paused(paused) => {
                self.p.lock().paused = paused;
                self.save();
                self.changed(app);
            }
            Event::Ended => {
                self.finish_rec();
                {
                    let mut p = self.p.lock();
                    p.paused = true;
                    p.pos = 0.0;
                }
                // Back to the start of the last track, ready to play again.
                let cur = self.p.lock().current;
                if let Some(id) = cur {
                    self.load(id, 0.0, true);
                }
                self.save();
                self.changed(app);
            }
            Event::Failed { token, message } => {
                let id = token as i64;
                let _ = app.emit("music-error", message);
                enum Then {
                    Stop,
                    Load(i64, bool),
                    Preload,
                    Nothing,
                }
                let then = {
                    let mut p = self.p.lock();
                    p.failures += 1;
                    if p.failures > p.queue.list.len().max(1) {
                        p.paused = true;
                        Then::Stop
                    } else if p.queue.current() == Some(id) {
                        // The current track wouldn't open: try the next one.
                        let paused = p.paused;
                        match p.queue.next_where(|i| self.skips(i)) {
                            Some(n) => Then::Load(n, paused),
                            None => Then::Stop,
                        }
                    } else if p.queue.peek_next() == Some(id) {
                        // The queued one wouldn't open: skip past it.
                        if let Some(i) = p.queue.pos.map(|x| x + 1) {
                            p.queue.remove(i);
                        }
                        Then::Preload
                    } else {
                        Then::Nothing
                    }
                };
                match then {
                    Then::Load(n, paused) => self.load(n, 0.0, paused),
                    Then::Preload => self.preload_next(),
                    Then::Stop | Then::Nothing => {}
                }
                self.changed(app);
            }
            Event::Output(r) => {
                {
                    let mut p = self.p.lock();
                    match r {
                        Ok(i) => {
                            p.output = Some(i);
                            p.output_error = None;
                        }
                        Err(e) => p.output_error = Some(e),
                    }
                }
                self.changed(app);
            }
        }
    }
}

fn dirs_audio() -> Option<String> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok()?;
    let d = std::path::Path::new(&home).join("Music");
    d.is_dir().then(|| d.display().to_string())
}

fn m(app: &AppHandle) -> Res<Arc<Music>> {
    app.try_state::<Arc<Music>>().map(|s| s.inner().clone()).ok_or_else(|| "The music player didn't start.".to_string())
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(err)?.map_err(err)
}

// ───────────────────────── library ─────────────────────────

#[tauri::command]
pub async fn music_library(app: AppHandle) -> Res<Vec<Track>> {
    let m = m(&app)?;
    blocking(move || m.lib.tracks()).await
}

#[tauri::command]
pub async fn music_folders(app: AppHandle) -> Res<Vec<String>> {
    let m = m(&app)?;
    blocking(move || m.lib.folders()).await
}

#[tauri::command]
pub async fn music_set_folders(app: AppHandle, folders: Vec<String>) -> Res<()> {
    let m = m(&app)?;
    blocking(move || m.lib.set_folders(&folders)).await
}

#[tauri::command]
pub async fn music_scan(app: AppHandle) -> Res<ScanReport> {
    let m = m(&app)?;
    let h = app.clone();
    let r = blocking(move || m.lib.scan(|p| {
        let _ = h.emit("music-scan", p);
    }))
    .await?;
    let _ = app.emit("music-library-changed", ());
    Ok(r)
}

#[tauri::command]
pub async fn music_favourite(app: AppHandle, id: i64, on: bool) -> Res<()> {
    let m = m(&app)?;
    blocking(move || m.lib.set_favourite(id, on)).await
}

// ───────────────────────── playback ─────────────────────────

#[tauri::command]
pub fn music_status(app: AppHandle) -> Res<Status> {
    Ok(m(&app)?.status())
}

/// Play `ids` as the new queue, starting at `ids[start]`.
#[tauri::command]
pub async fn music_play(app: AppHandle, ids: Vec<i64>, start: usize) -> Res<()> {
    let m = m(&app)?;
    if m.p.lock().queue.shuffle {
        let (m2, ids2) = (m.clone(), ids.clone());
        let _ = tauri::async_runtime::spawn_blocking(move || m2.shuffle_info(&ids2)).await;
    }
    let first = m.p.lock().queue.set(ids, start);
    match first {
        Some(id) => m.load(id, 0.0, false),
        None => m.engine.stop(),
    }
    m.save();
    m.changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn music_pause(app: AppHandle, paused: Option<bool>) -> Res<()> {
    let m = m(&app)?;
    let (now, has, pending, cur, pos) = {
        let p = m.p.lock();
        (p.paused, p.current.is_some(), p.pending, p.current, p.pos)
    };
    if !has {
        return Ok(());
    }
    if pending && !paused.unwrap_or(!now) {
        if let Some(id) = cur {
            m.load(id, pos, false);
        }
        return Ok(());
    }
    m.engine.pause(paused.unwrap_or(!now));
    Ok(())
}

#[tauri::command]
pub async fn music_next(app: AppHandle) -> Res<()> {
    let m = m(&app)?;
    let (next, paused) = {
        let mut p = m.p.lock();
        (p.queue.next_where(|i| m.skips(i)), p.paused)
    };
    if let Some(id) = next {
        m.load(id, 0.0, paused);
    }
    m.changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn music_prev(app: AppHandle) -> Res<()> {
    let m = m(&app)?;
    let (pos, paused) = {
        let p = m.p.lock();
        (p.pos, p.paused)
    };
    // Like every player: Previous restarts the track unless it's just begun.
    if pos > 3.0 {
        m.engine.seek(0.0);
        m.p.lock().pos = 0.0;
    } else {
        let prev = m.p.lock().queue.prev();
        if let Some(id) = prev {
            m.load(id, 0.0, paused);
        }
    }
    m.changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn music_seek(app: AppHandle, secs: f64) -> Res<()> {
    let m = m(&app)?;
    m.engine.seek(secs.max(0.0));
    m.p.lock().pos = secs.max(0.0);
    let playing = !m.p.lock().paused;
    crate::media_keys::send(crate::media_keys::Update::Playing { playing, pos: secs.max(0.0) });
    Ok(())
}

#[tauri::command]
pub async fn music_volume(app: AppHandle, volume: f32) -> Res<()> {
    let m = m(&app)?;
    let v = volume.clamp(0.0, 1.0);
    m.p.lock().volume = v;
    m.engine.set_volume(gain(v));
    m.save();
    Ok(())
}

#[tauri::command]
pub async fn music_shuffle(app: AppHandle, on: bool) -> Res<()> {
    let m = m(&app)?;
    if on {
        let (m2, ids) = (m.clone(), m.p.lock().queue.list.clone());
        let _ = tauri::async_runtime::spawn_blocking(move || m2.shuffle_info(&ids)).await;
    }
    m.p.lock().queue.set_shuffle(on);
    m.preload_next();
    m.save();
    m.changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn music_repeat(app: AppHandle, mode: Repeat) -> Res<()> {
    let m = m(&app)?;
    m.p.lock().queue.repeat = mode;
    m.preload_next();
    m.save();
    m.changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn music_devices() -> Vec<String> {
    tauri::async_runtime::spawn_blocking(arcadia_audio::devices).await.unwrap_or_default()
}

#[tauri::command]
pub async fn music_set_device(app: AppHandle, name: Option<String>) -> Res<()> {
    let m = m(&app)?;
    m.p.lock().device = name.clone();
    m.engine.set_device(name);
    m.save();
    Ok(())
}

// ───────────────────────── queue ─────────────────────────

#[tauri::command]
pub async fn music_queue(app: AppHandle) -> Res<QueueView> {
    let m = m(&app)?;
    let (ids, index) = {
        let p = m.p.lock();
        (p.queue.list.clone(), p.queue.pos)
    };
    let tracks = blocking({
        let m = m.clone();
        move || m.lib.tracks_by_id(&ids)
    })
    .await?;
    Ok(QueueView { index, tracks })
}

fn after_queue_edit(m: &Music, app: &AppHandle) {
    m.preload_next();
    m.save();
    m.changed(app);
    let _ = app.emit("music-queue-changed", ());
}

#[tauri::command]
pub async fn music_queue_add(app: AppHandle, ids: Vec<i64>, next: bool) -> Res<()> {
    let m = m(&app)?;
    let start = {
        let mut p = m.p.lock();
        let empty = p.current.is_none();
        p.queue.add(&ids, next);
        if empty {
            p.queue.current()
        } else {
            None
        }
    };
    if let Some(id) = start {
        m.load(id, 0.0, false);
    }
    after_queue_edit(&m, &app);
    Ok(())
}

#[tauri::command]
pub async fn music_queue_remove(app: AppHandle, index: usize) -> Res<()> {
    let m = m(&app)?;
    let (was_current, now, paused) = {
        let mut p = m.p.lock();
        let was = p.queue.pos == Some(index);
        p.queue.remove(index);
        (was, p.queue.current(), p.paused)
    };
    if was_current {
        match now {
            Some(id) => m.load(id, 0.0, paused),
            None => {
                m.engine.stop();
                m.p.lock().current = None;
            }
        }
    }
    after_queue_edit(&m, &app);
    Ok(())
}

#[tauri::command]
pub async fn music_queue_move(app: AppHandle, from: usize, to: usize) -> Res<()> {
    let m = m(&app)?;
    m.p.lock().queue.move_item(from, to);
    after_queue_edit(&m, &app);
    Ok(())
}

#[tauri::command]
pub async fn music_queue_jump(app: AppHandle, index: usize) -> Res<()> {
    let m = m(&app)?;
    let id = m.p.lock().queue.jump(index);
    if let Some(id) = id {
        m.load(id, 0.0, false);
    }
    after_queue_edit(&m, &app);
    Ok(())
}

#[tauri::command]
pub async fn music_queue_clear(app: AppHandle) -> Res<()> {
    let m = m(&app)?;
    {
        let mut p = m.p.lock();
        // Keep what's playing; drop everything else.
        let cur = p.queue.current();
        p.queue.clear();
        if let Some(c) = cur {
            p.queue.set(vec![c], 0);
        }
    }
    after_queue_edit(&m, &app);
    Ok(())
}

// ───────────────────────── playlists ─────────────────────────

#[tauri::command]
pub async fn music_playlists(app: AppHandle) -> Res<Vec<Playlist>> {
    let m = m(&app)?;
    blocking(move || m.lib.playlists()).await
}

#[tauri::command]
pub async fn music_playlist_items(app: AppHandle, id: i64) -> Res<Vec<i64>> {
    let m = m(&app)?;
    blocking(move || m.lib.playlist_items(id)).await
}

#[tauri::command]
pub async fn music_playlist_create(app: AppHandle, name: String, ids: Vec<i64>) -> Res<i64> {
    let m = m(&app)?;
    blocking(move || m.lib.playlist_create(&name, &ids)).await
}

#[tauri::command]
pub async fn music_playlist_rename(app: AppHandle, id: i64, name: String) -> Res<()> {
    let m = m(&app)?;
    blocking(move || m.lib.playlist_rename(id, &name)).await
}

#[tauri::command]
pub async fn music_playlist_delete(app: AppHandle, id: i64) -> Res<()> {
    let m = m(&app)?;
    blocking(move || m.lib.playlist_delete(id)).await
}

#[tauri::command]
pub async fn music_playlist_set(app: AppHandle, id: i64, ids: Vec<i64>) -> Res<()> {
    let m = m(&app)?;
    blocking(move || m.lib.playlist_set(id, &ids)).await
}

#[tauri::command]
pub async fn music_playlist_add(app: AppHandle, id: i64, ids: Vec<i64>) -> Res<()> {
    let m = m(&app)?;
    blocking(move || m.lib.playlist_add(id, &ids)).await
}

// ───────────────────────── listening tools ─────────────────────────

fn load_rules() -> SkipRules {
    arcadia_core::addons::get(TRASH, "rules").ok().flatten().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_else(|| SkipRules { trashed: true, ..Default::default() })
}

/// Trash Bin saved new rules: pick them up.
#[tauri::command]
pub async fn music_reload_rules(app: AppHandle) -> Res<()> {
    let m = m(&app)?;
    *m.rules.lock() = tauri::async_runtime::spawn_blocking(load_rules).await.map_err(err)?;
    m.preload_next();
    Ok(())
}

/// Shuffle+: how shuffle orders songs.
#[tauri::command]
pub async fn music_shuffle_mode(app: AppHandle, mode: ShuffleMode) -> Res<()> {
    if mode != ShuffleMode::Random {
        crate::addon_cmds::installed(&app, SHUFFLE_PLUS)?;
    }
    let m = m(&app)?;
    if m.p.lock().queue.mode == mode {
        return Ok(());
    }
    // Look up what the new mode needs, then reshuffle what's still to come.
    let ids = m.p.lock().queue.list.clone();
    let m2 = m.clone();
    let info = tauri::async_runtime::spawn_blocking(move || m2.info_for(mode, &ids)).await.map_err(err)?;
    {
        let mut p = m.p.lock();
        p.queue.info = info;
        p.queue.set_mode(mode);
    }
    after_queue_edit(&m, &app);
    Ok(())
}

/// Loopy Loop: loop the current song between `a` and `b` seconds, or stop
/// looping when either is missing.
#[tauri::command]
pub async fn music_loop(app: AppHandle, a: Option<f64>, b: Option<f64>) -> Res<()> {
    let m = m(&app)?;
    let cur = m.p.lock().current;
    let ab = match (cur, a, b) {
        (Some(id), Some(a), Some(b)) if b > a + 0.05 => Some((id, a.max(0.0), b)),
        _ => None,
    };
    m.p.lock().ab = ab.map(|(_, a, b)| (a, b));
    m.engine.set_loop(ab.map(|(id, a, b)| (id as u64, a, b)));
    m.changed(&app);
    Ok(())
}

/// Levels for the Full App Display visualiser, low notes first, 0–1.
#[tauri::command]
pub async fn music_spectrum(app: AppHandle, bands: usize) -> Res<Vec<f32>> {
    Ok(m(&app)?.engine.spectrum(bands.clamp(1, 256)))
}

#[tauri::command]
pub async fn music_spectrum_stop(app: AppHandle) -> Res<()> {
    m(&app)?.engine.stop_spectrum();
    Ok(())
}

/// Queue tools: "dedupe", "clearPlayed" or "shuffleRest".
#[tauri::command]
pub async fn music_queue_tool(app: AppHandle, tool: String) -> Res<()> {
    let m = m(&app)?;
    if tool == "shuffleRest" {
        let (m2, ids) = (m.clone(), m.p.lock().queue.list.clone());
        let _ = tauri::async_runtime::spawn_blocking(move || m2.shuffle_info(&ids)).await;
    }
    {
        let mut p = m.p.lock();
        match tool.as_str() {
            "dedupe" => p.queue.dedupe(),
            "clearPlayed" => p.queue.clear_played(),
            "shuffleRest" => p.queue.shuffle_rest(),
            other => return Err(format!("Unknown queue tool “{other}”")),
        }
    }
    after_queue_edit(&m, &app);
    Ok(())
}

/// Listening stats from `since` (unix seconds, or all time). `tz` is the
/// local offset from UTC in minutes, as JavaScript's getTimezoneOffset
/// gives it (so negated).
#[tauri::command]
pub async fn music_stats(app: AppHandle, since: Option<i64>, tz: i64, days: Option<usize>) -> Res<Stats> {
    crate::addon_cmds::installed(&app, "stats")?;
    let m = m(&app)?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    blocking(move || m.lib.stats(since, now, -tz * 60, days.unwrap_or(182).clamp(7, 400))).await
}

// ───────────────────────── duplicates ─────────────────────────

/// Duplicate songs, best copy first in each group. `min`: the lowest
/// certainty to show, 50–100.
#[tauri::command]
pub async fn music_duplicates(app: AppHandle, min: f64) -> Res<Vec<arcadia_core::music::dupes::Group>> {
    let m = m(&app)?;
    blocking(move || {
        let tracks = m.lib.tracks()?;
        let sizes = m.lib.sizes()?;
        Ok(arcadia_core::music::dupes::find(&tracks, min.clamp(50.0, 100.0), |t| sizes.get(&t.id).copied()))
    })
    .await
}

/// Move songs' files to the Recycle Bin (so they can be restored from
/// there) and take them out of the library. Online songs without a file
/// of their own are only taken out.
#[tauri::command]
pub async fn music_recycle(app: AppHandle, ids: Vec<i64>) -> Res<usize> {
    let m = m(&app)?;
    let h = app.clone();
    let n = blocking(move || {
        let mut files = Vec::new();
        for id in &ids {
            if let Some(p) = m.lib.file_of(*id).filter(|p| p.is_file()) {
                files.push(p);
            }
        }
        if !files.is_empty() {
            trash::delete_all(&files).map_err(|e| anyhow::anyhow!("Couldn't move the files to the Recycle Bin: {e}"))?;
        }
        m.lib.forget(&ids)?;
        Ok(ids.len())
    })
    .await?;
    let _ = h.emit("music-library-changed", ());
    Ok(n)
}
