//! Streaming & downloads addon: search, fetch-to-play, the download queue
//! and synced playlists.

use crate::addon_cmds::installed;
use crate::music_cmds::Music;
use crate::{err, Res};
use arcadia_core::addons::{self, streaming as st};
use arcadia_core::music::library::RemoteMeta;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    id: u64,
    track_id: i64,
    title: String,
    artist: String,
    url: String,
    /// "queued", "running", "done", "failed", "cancelled"
    state: String,
    progress: st::Progress,
    error: Option<String>,
    path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncedSource {
    playlist_id: i64,
    url: String,
    /// Download new songs as they appear (otherwise they're added to stream).
    download: bool,
    last_sync: i64,
}

pub struct Streamer {
    jobs: Mutex<Vec<Job>>,
    next: AtomicU64,
    cancels: Mutex<HashMap<u64, tokio::sync::watch::Sender<bool>>>,
    slots: Arc<tokio::sync::Semaphore>,
    /// Stream fetches in flight, so a song isn't fetched twice at once.
    fetching: Mutex<HashSet<i64>>,
}

impl Streamer {
    pub fn new() -> Streamer {
        Streamer { jobs: Mutex::new(Vec::new()), next: AtomicU64::new(1), cancels: Mutex::new(HashMap::new()), slots: Arc::new(tokio::sync::Semaphore::new(2)), fetching: Mutex::new(HashSet::new()) }
    }
}

fn streamer(app: &AppHandle) -> Arc<Streamer> {
    app.state::<Arc<Streamer>>().inner().clone()
}
fn music(app: &AppHandle) -> Res<Arc<Music>> {
    app.try_state::<Arc<Music>>().map(|m| m.inner().clone()).ok_or_else(|| "The music player didn't start.".to_string())
}
fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}
fn ytdlp(cfg: &st::Config) -> Res<PathBuf> {
    st::find(st::Tool::YtDlp, cfg).ok_or_else(|| "Streaming needs yt-dlp. Get it under Music → Downloads → Setup.".to_string())
}

// ───────────────────────── setup ─────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    path: String,
    version: Option<String>,
    /// Arcadia's own copy (can be updated from here).
    managed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    ytdlp: Option<ToolInfo>,
    ffmpeg: Option<ToolInfo>,
    config: st::Config,
    download_dir: String,
    cache_bytes: u64,
    windows: bool,
}

async fn tool_info(tool: st::Tool, cfg: &st::Config) -> Option<ToolInfo> {
    let p = st::find(tool, cfg)?;
    let version = st::version(&p, tool).await;
    Some(ToolInfo { managed: p.starts_with(st::bin_dir()), path: p.display().to_string(), version })
}

#[tauri::command]
pub async fn streaming_status(app: AppHandle) -> Res<Status> {
    installed(&app, st::ID)?;
    let cfg = st::Config::load();
    let (ytdlp, ffmpeg) = tokio::join!(tool_info(st::Tool::YtDlp, &cfg), tool_info(st::Tool::Ffmpeg, &cfg));
    Ok(Status { ytdlp, ffmpeg, download_dir: cfg.download_dir().display().to_string(), config: cfg, cache_bytes: st::cache_size(), windows: cfg!(windows) })
}

#[tauri::command]
pub async fn streaming_install(app: AppHandle, tool: st::Tool) -> Res<()> {
    installed(&app, st::ID)?;
    let h = app.clone();
    st::install(tool, move |p| {
        let _ = h.emit("streaming-install", p);
    })
    .await
    .map(|_| ())
    .map_err(err)
}

#[tauri::command]
pub async fn streaming_update(app: AppHandle) -> Res<String> {
    installed(&app, st::ID)?;
    let cfg = st::Config::load();
    let p = ytdlp(&cfg)?;
    if !p.starts_with(st::bin_dir()) {
        return Err("This yt-dlp was installed outside Arcadia; update it the way you installed it.".into());
    }
    st::update_ytdlp(&p).await.map_err(err)
}

#[tauri::command]
pub async fn streaming_set_config(app: AppHandle, config: st::Config) -> Res<()> {
    installed(&app, st::ID)?;
    config.save().map_err(err)?;
    trim_cache(&app, &[]);
    Ok(())
}

#[tauri::command]
pub async fn streaming_clear_cache(app: AppHandle) -> Res<u64> {
    installed(&app, st::ID)?;
    let m = music(&app)?;
    let keep = protected(&m);
    let mut freed = 0;
    for (id, path) in m.lib.cached_streams().map_err(err)? {
        if keep.contains(&id) {
            continue;
        }
        freed += std::fs::metadata(&path).map(|x| x.len()).unwrap_or(0);
        let _ = std::fs::remove_file(&path);
        let _ = m.lib.uncache(id);
    }
    Ok(freed)
}

// ───────────────────────── finding ─────────────────────────

#[tauri::command]
pub async fn streaming_search(app: AppHandle, source: st::Source, query: String) -> Res<st::Found> {
    installed(&app, st::ID)?;
    let p = ytdlp(&st::Config::load())?;
    st::search(&p, source, &query).await.map_err(err)
}

#[tauri::command]
pub async fn streaming_resolve(app: AppHandle, url: String) -> Res<st::Found> {
    installed(&app, st::ID)?;
    let p = ytdlp(&st::Config::load())?;
    st::resolve(&p, &url).await.map_err(err)
}

fn meta(i: &st::Item) -> RemoteMeta {
    RemoteMeta { url: i.url.clone(), title: i.title.clone(), artist: i.artist.clone(), album: i.album.clone(), duration: i.duration }
}

/// Library entries for online songs (made if new), so they can be played,
/// queued and added to playlists like any other song. Their covers are
/// fetched in the background.
#[tauri::command]
pub async fn streaming_tracks(app: AppHandle, items: Vec<st::Item>) -> Res<Vec<i64>> {
    installed(&app, st::ID)?;
    let m = music(&app)?;
    let mut ids = Vec::with_capacity(items.len());
    let mut covers = Vec::new();
    for i in &items {
        let id = m.lib.remote(&meta(i)).map_err(err)?;
        if !m.lib.has_art(id) {
            if let Some(t) = &i.thumbnail {
                covers.push((id, t.clone()));
            }
        }
        ids.push(id);
    }
    if !covers.is_empty() {
        let h = app.clone();
        tauri::async_runtime::spawn(async move {
            for (id, url) in covers.into_iter().take(60) {
                if let Ok(bytes) = st::fetch_image(&url).await {
                    let _ = m.lib.set_art(id, &bytes);
                }
            }
            let _ = h.emit("music-queue-changed", ());
        });
    }
    Ok(ids)
}

// ───────────────────────── fetch to play ─────────────────────────

/// The current and next songs: never trimmed from the cache.
fn protected(m: &Music) -> HashSet<i64> {
    m.near_ids().into_iter().collect()
}

/// Keep the stream cache under its limit, oldest first.
fn trim_cache(app: &AppHandle, also_keep: &[i64]) {
    let Ok(m) = music(app) else { return };
    let limit = st::Config::load().cache_limit_mb.max(64) * 1024 * 1024;
    let mut keep = protected(&m);
    keep.extend(also_keep);
    let Ok(list) = m.lib.cached_streams() else { return };
    let mut total: u64 = list.iter().map(|(_, p)| std::fs::metadata(p).map(|x| x.len()).unwrap_or(0)).sum();
    for (id, path) in list {
        if total <= limit {
            break;
        }
        if keep.contains(&id) {
            continue;
        }
        total = total.saturating_sub(std::fs::metadata(&path).map(|x| x.len()).unwrap_or(0));
        let _ = std::fs::remove_file(&path);
        let _ = m.lib.uncache(id);
    }
}

/// Make sure an online song has a file to play, fetching it if needed.
pub async fn ensure_cached(app: &AppHandle, id: i64) -> Res<PathBuf> {
    let m = music(app)?;
    if let Some(p) = m.lib.file_of(id) {
        return Ok(p);
    }
    if installed(app, st::ID).is_err() {
        return Err("Add Streaming & downloads from the Marketplace to play online songs.".into());
    }
    let url = m.lib.source_of(id).ok_or("That song's file is missing.")?;
    let s = streamer(app);
    // Someone else is already fetching it: wait for them.
    let mut waited = 0;
    while s.fetching.lock().contains(&id) && waited < 600 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        waited += 1;
    }
    if let Some(p) = m.lib.file_of(id) {
        return Ok(p);
    }
    s.fetching.lock().insert(id);
    let r = fetch_stream(app, &m, id, &url).await;
    s.fetching.lock().remove(&id);
    if r.is_ok() {
        trim_cache(app, &[id]);
        let _ = app.emit("music-queue-changed", ());
    }
    r
}

async fn fetch_stream(app: &AppHandle, m: &Music, id: i64, url: &str) -> Res<PathBuf> {
    let cfg = st::Config::load();
    let exe = ytdlp(&cfg)?;
    let ff = st::find(st::Tool::Ffmpeg, &cfg);
    std::fs::create_dir_all(st::cache_dir()).map_err(err)?;
    let tpl = st::cache_dir().join(format!("{}.%(ext)s", st::cache_key(url))).display().to_string();
    let (_tx, rx) = tokio::sync::watch::channel(false);
    let h = app.clone();
    // Streams always take what plays without conversion; no originals (they can be huge WAVs).
    let f = st::fetch(&exe, ff.as_deref(), url, st::Format::M4a, false, &tpl, move |p| {
        let _ = h.emit("streaming-fetch", (id, p));
    }, rx)
    .await
    .map_err(err)?;
    let fallback = m.lib.track(id).ok().flatten().map(|t| RemoteMeta { url: url.into(), title: t.title, artist: t.artist, album: Some(t.album), duration: Some(t.duration) });
    m.lib.attach_file(id, &f.path, "stream", fallback.as_ref(), None).map_err(err)?;
    Ok(f.path)
}

// ───────────────────────── downloads ─────────────────────────

fn emit_jobs(app: &AppHandle) {
    let jobs = streamer(app).jobs.lock().clone();
    let _ = app.emit("streaming-jobs", jobs);
}

fn update(app: &AppHandle, job: u64, f: impl FnOnce(&mut Job)) {
    let s = streamer(app);
    let mut jobs = s.jobs.lock();
    if let Some(j) = jobs.iter_mut().find(|j| j.id == job) {
        f(j);
    }
}

#[tauri::command]
pub fn streaming_jobs(app: AppHandle) -> Res<Vec<Job>> {
    installed(&app, st::ID)?;
    Ok(streamer(&app).jobs.lock().clone())
}

/// Download these songs (online songs only) in the configured format.
#[tauri::command]
pub async fn streaming_download(app: AppHandle, ids: Vec<i64>) -> Res<usize> {
    installed(&app, st::ID)?;
    let cfg = st::Config::load();
    ytdlp(&cfg)?;
    let m = music(&app)?;
    let s = streamer(&app);
    let mut n = 0;
    for id in ids {
        let Some(t) = m.lib.track(id).map_err(err)? else { continue };
        let Some(url) = t.source_url.clone() else { continue };
        if t.origin == "download" && m.lib.file_of(id).is_some() {
            continue;
        }
        // Already queued or running.
        if s.jobs.lock().iter().any(|j| j.track_id == id && (j.state == "queued" || j.state == "running")) {
            continue;
        }
        let job = Job { id: s.next.fetch_add(1, Ordering::Relaxed), track_id: id, title: t.title, artist: t.artist, url, state: "queued".into(), progress: Default::default(), error: None, path: None };
        let jid = job.id;
        s.jobs.lock().push(job);
        spawn_job(&app, jid);
        n += 1;
    }
    emit_jobs(&app);
    Ok(n)
}

fn spawn_job(app: &AppHandle, jid: u64) {
    let app = app.clone();
    let (tx, rx) = tokio::sync::watch::channel(false);
    streamer(&app).cancels.lock().insert(jid, tx);
    tauri::async_runtime::spawn(async move {
        let s = streamer(&app);
        let Ok(_permit) = s.slots.clone().acquire_owned().await else { return };
        // Cancelled while waiting.
        if s.jobs.lock().iter().any(|j| j.id == jid && j.state != "queued") {
            return;
        }
        update(&app, jid, |j| j.state = "running".into());
        emit_jobs(&app);
        let r = run_job(&app, jid, rx).await;
        update(&app, jid, |j| match &r {
            Ok(p) => {
                j.state = "done".into();
                j.path = Some(p.display().to_string());
                j.progress.percent = 100.0;
            }
            Err(e) if e == "Cancelled" => j.state = "cancelled".into(),
            Err(e) => {
                j.state = "failed".into();
                j.error = Some(e.clone());
            }
        });
        s.cancels.lock().remove(&jid);
        emit_jobs(&app);
        if r.is_ok() {
            let _ = app.emit("music-library-changed", ());
        }
    });
}

async fn run_job(app: &AppHandle, jid: u64, cancel: tokio::sync::watch::Receiver<bool>) -> Res<PathBuf> {
    let (track_id, url) = {
        let s = streamer(app);
        let jobs = s.jobs.lock();
        let j = jobs.iter().find(|j| j.id == jid).ok_or("gone")?;
        (j.track_id, j.url.clone())
    };
    let cfg = st::Config::load();
    let exe = ytdlp(&cfg)?;
    let ff = st::find(st::Tool::Ffmpeg, &cfg);
    let dir = cfg.download_dir();
    std::fs::create_dir_all(&dir).map_err(err)?;
    let tpl = dir.join("%(artist,creator,uploader,channel|Unknown artist)s").join("%(album|Singles)s").join("%(track,title)s.%(ext)s").display().to_string();
    let h = app.clone();
    let mut last = std::time::Instant::now();
    let f = st::fetch(&exe, ff.as_deref(), &url, cfg.format, cfg.sc_original, &tpl, move |p| {
        if last.elapsed().as_millis() >= 250 || p.percent >= 100.0 {
            last = std::time::Instant::now();
            update(&h, jid, |j| j.progress = p.clone());
            emit_jobs(&h);
        }
    }, cancel)
    .await
    .map_err(err)?;
    let m = music(app)?;
    let old = m.lib.file_of(track_id);
    let fallback = f.item.as_ref().map(meta);
    // No ffmpeg means no embedded cover: use the thumbnail instead.
    let cover = match (&ff, f.item.as_ref().and_then(|i| i.thumbnail.clone())) {
        (None, Some(t)) => st::fetch_image(&t).await.ok(),
        _ => None,
    };
    m.lib.attach_file(track_id, &f.path, "download", fallback.as_ref(), cover.as_deref()).map_err(err)?;
    // A cached stream copy isn't needed any more.
    if let Some(o) = old {
        if o.starts_with(st::cache_dir()) {
            let _ = std::fs::remove_file(o);
        }
    }
    ensure_library_folder(&m, &dir);
    Ok(f.path)
}

/// Downloads show up on rescans too: make sure their folder is a library folder.
fn ensure_library_folder(m: &Music, dir: &Path) {
    if let Ok(folders) = m.lib.folders() {
        if !folders.iter().any(|f| dir.starts_with(f)) {
            let mut v = folders;
            v.push(dir.display().to_string());
            let _ = m.lib.set_folders(&v);
        }
    }
}

#[tauri::command]
pub fn streaming_cancel(app: AppHandle, id: u64) -> Res<()> {
    installed(&app, st::ID)?;
    let s = streamer(&app);
    if let Some(tx) = s.cancels.lock().get(&id) {
        let _ = tx.send(true);
    }
    update(&app, id, |j| {
        if j.state == "queued" {
            j.state = "cancelled".into();
        }
    });
    emit_jobs(&app);
    Ok(())
}

#[tauri::command]
pub fn streaming_retry(app: AppHandle, id: u64) -> Res<()> {
    installed(&app, st::ID)?;
    update(&app, id, |j| {
        j.state = "queued".into();
        j.error = None;
        j.progress = Default::default();
    });
    spawn_job(&app, id);
    emit_jobs(&app);
    Ok(())
}

#[tauri::command]
pub fn streaming_clear_finished(app: AppHandle) -> Res<()> {
    installed(&app, st::ID)?;
    streamer(&app).jobs.lock().retain(|j| j.state == "queued" || j.state == "running");
    emit_jobs(&app);
    Ok(())
}

// ───────────────────────── synced playlists ─────────────────────────

fn sources() -> Vec<SyncedSource> {
    addons::get(st::ID, "sources").ok().flatten().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
}
fn save_sources(v: &[SyncedSource]) -> Res<()> {
    addons::set(st::ID, "sources", Some(&serde_json::to_value(v).map_err(err)?)).map_err(err)
}

#[tauri::command]
pub fn streaming_sources(app: AppHandle) -> Res<Vec<SyncedSource>> {
    installed(&app, st::ID)?;
    Ok(sources())
}

/// Turn a playlist/album/set link into an Arcadia playlist that can be
/// re-checked for new songs.
#[tauri::command]
pub async fn streaming_import(app: AppHandle, url: String, name: Option<String>, download: bool) -> Res<i64> {
    installed(&app, st::ID)?;
    let found = streaming_resolve(app.clone(), url.clone()).await?;
    if found.items.is_empty() {
        return Err("That link has no songs in it.".into());
    }
    let ids = streaming_tracks(app.clone(), found.items).await?;
    let m = music(&app)?;
    let name = name.filter(|n| !n.trim().is_empty()).or(found.playlist).unwrap_or_else(|| "Imported".into());
    let pid = m.lib.playlist_create(&name, &ids).map_err(err)?;
    let mut v = sources();
    v.push(SyncedSource { playlist_id: pid, url, download, last_sync: now() });
    save_sources(&v)?;
    if download {
        streaming_download(app.clone(), ids).await?;
    }
    Ok(pid)
}

/// Add songs that have appeared at the source since the last check.
#[tauri::command]
pub async fn streaming_sync(app: AppHandle, playlist_id: i64) -> Res<usize> {
    installed(&app, st::ID)?;
    let src = sources().into_iter().find(|s| s.playlist_id == playlist_id).ok_or("That playlist isn't linked to anything online.")?;
    let found = streaming_resolve(app.clone(), src.url.clone()).await?;
    let ids = streaming_tracks(app.clone(), found.items).await?;
    let m = music(&app)?;
    let have: HashSet<i64> = m.lib.playlist_items(playlist_id).map_err(err)?.into_iter().collect();
    let new: Vec<i64> = ids.into_iter().filter(|i| !have.contains(i)).collect();
    if !new.is_empty() {
        m.lib.playlist_add(playlist_id, &new).map_err(err)?;
        if src.download {
            streaming_download(app.clone(), new.clone()).await?;
        }
    }
    let mut v = sources();
    if let Some(s) = v.iter_mut().find(|s| s.playlist_id == playlist_id) {
        s.last_sync = now();
    }
    save_sources(&v)?;
    Ok(new.len())
}

#[tauri::command]
pub fn streaming_unlink(app: AppHandle, playlist_id: i64) -> Res<()> {
    installed(&app, st::ID)?;
    let v: Vec<_> = sources().into_iter().filter(|s| s.playlist_id != playlist_id).collect();
    save_sources(&v)
}
