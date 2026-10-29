//! The music player: queue + library + audio engine, and its commands.

use crate::{err, Res};
use arcadia_audio::{Engine, Event, OutputInfo};
use arcadia_core::music::{self, Library, Playlist, Queue, Repeat, ScanReport, Track};
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
}

pub struct Music {
    pub lib: Library,
    engine: Engine,
    p: Mutex<Player>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    track: Option<Track>,
    playing: bool,
    pos: f64,
    duration: f64,
    volume: f32,
    shuffle: bool,
    repeat: Repeat,
    queue_index: Option<usize>,
    queue_length: usize,
    output: Option<OutputInfo>,
    output_error: Option<String>,
    device: Option<String>,
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
            }),
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
        let Some(path) = self.lib.path_of(id) else { return };
        {
            let mut p = self.p.lock();
            p.current = Some(id);
            p.pos = start;
            p.duration = self.lib.track(id).ok().flatten().map(|t| t.duration).unwrap_or(0.0);
            p.counted = false;
            p.paused = paused;
        }
        self.engine.load(id as u64, path, start, paused);
    }

    fn preload_next(&self) {
        let next = self.p.lock().queue.peek_next();
        self.engine.set_next(next.and_then(|id| self.lib.path_of(id).map(|p| (id as u64, p))));
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
        let _ = app.emit("music-status", self.status());
    }

    fn on_event(self: Arc<Self>, app: &AppHandle, e: Event) {
        match e {
            Event::Track { token, pos, duration } => {
                let id = token as i64;
                {
                    let mut p = self.p.lock();
                    if p.queue.current() != Some(id) && p.queue.peek_next() == Some(id) {
                        p.queue.advance();
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
                        match p.queue.next() {
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
    let (now, has) = {
        let p = m.p.lock();
        (p.paused, p.current.is_some())
    };
    if !has {
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
        (p.queue.next(), p.paused)
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
