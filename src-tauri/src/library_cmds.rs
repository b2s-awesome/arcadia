//! Library+: your info about each game, collections, art from SteamGridDB,
//! and playtime for every game Arcadia launches.

use crate::{err, AppState, Res};
use arcadia_core::library::{self, meta, steamgriddb, Game, LaunchTarget};
use serde::Serialize;
use std::collections::BTreeMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryMeta {
    meta: meta::Meta,
    /// Steam's own playtime: id → (minutes, last played).
    steam: BTreeMap<String, (u64, u64)>,
    /// Roblox and Minecraft playtime from their own trackers: id → (seconds, last played).
    arcadia: BTreeMap<String, (u64, u64)>,
}

#[tauri::command]
pub async fn library_meta() -> Res<LibraryMeta> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut arcadia = BTreeMap::new();
        let h = arcadia_core::roblox::history::load();
        let rb = h.games.iter().fold((0, 0), |(s, l), g| (s + g.seconds, l.max(g.last)));
        if rb.0 > 0 {
            arcadia.insert("roblox:player".to_string(), rb);
        }
        let mc = arcadia_core::minecraft::instance::list().iter().fold((0, 0), |(s, l), i| (s + i.playtime_secs, l.max(i.last_played.unwrap_or(0))));
        if mc.0 > 0 {
            arcadia.insert("minecraft:launcher".to_string(), mc);
        }
        LibraryMeta { meta: meta::load(), steam: meta::steam_playtime(), arcadia }
    })
    .await
    .map_err(err)
}

#[tauri::command]
pub fn library_meta_set(id: String, meta: meta::GameMeta) -> Res<()> {
    meta::set(&id, meta).map_err(err)
}

#[tauri::command]
pub fn library_collections_set(list: Vec<meta::Collection>) -> Res<()> {
    meta::set_collections(list).map_err(err)
}

#[tauri::command]
pub fn library_art_import(path: String) -> Res<String> {
    meta::import_art(std::path::Path::new(&path)).map_err(err)
}

fn key(app: &AppHandle) -> String {
    app.state::<AppState>().settings.lock().steamgriddb_key.clone()
}

#[tauri::command]
pub async fn library_art_search(app: AppHandle, term: String) -> Res<Vec<steamgriddb::Found>> {
    steamgriddb::search(&key(&app), &term).await.map_err(err)
}

#[tauri::command]
pub async fn library_art(app: AppHandle, kind: String, id: u64, steam: bool) -> Res<Vec<steamgriddb::Art>> {
    steamgriddb::art(&key(&app), &kind, id, steam).await.map_err(err)
}

// ───────────────────────── launching with extras ─────────────────────────

/// Run one of your scripts. `wait` blocks until it ends (up to a minute).
fn run_script(cmd: &str, dir: Option<&str>, wait: bool) {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return;
    }
    #[cfg(windows)]
    let mut c = {
        use std::os::windows::process::CommandExt;
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", cmd]).creation_flags(0x0800_0000);
        c
    };
    #[cfg(not(windows))]
    let mut c = {
        let mut c = std::process::Command::new("sh");
        c.args(["-c", cmd]);
        c
    };
    if let Some(d) = dir.filter(|d| std::path::Path::new(d).is_dir()) {
        c.current_dir(d);
    }
    match c.spawn() {
        Ok(mut child) if wait => {
            let t = Instant::now();
            while t.elapsed() < Duration::from_secs(60) {
                if let Ok(Some(_)) = child.try_wait() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            log::warn!("library: script still running after a minute; carrying on");
        }
        Ok(_) => {}
        Err(e) => log::warn!("library: script didn't start: {e}"),
    }
}

/// Launch a launcher game with your arguments and scripts, then watch for
/// it to close to count the time.
pub(crate) fn launch(app: &AppHandle, game: &Game) -> Res<()> {
    let m = meta::get(&game.id);
    let target = meta::with_args(&game.launch, &m.launch_args, game.source);
    let dir = game.install_dir.clone();
    run_script(&m.before, dir.as_deref(), true);
    library::launch(&target).map_err(err)?;
    let track = app.state::<AppState>().settings.lock().track_playtime;
    if !track && m.after.trim().is_empty() {
        return Ok(());
    }
    let (h, id, after, name) = (app.clone(), game.id.clone(), m.after.clone(), game.name.clone());
    let exe_dir = match &game.launch {
        LaunchTarget::Exe { path, .. } => std::path::Path::new(path).parent().map(|p| p.to_string_lossy().to_string()),
        _ => None,
    };
    let watch_dir = dir.or(exe_dir);
    std::thread::spawn(move || {
        let secs = watch_dir.as_deref().map(watch).unwrap_or(0);
        if track && secs > 0 {
            let _ = meta::add_play(&id, secs, now());
            let _ = h.emit("library-played", serde_json::json!({ "id": id, "name": name, "seconds": secs }));
        }
        run_script(&after, watch_dir.as_deref(), false);
    });
    Ok(())
}

/// Wait for a process from `dir` to appear (up to two minutes), then until
/// none are left. Returns how long the game ran, in seconds.
fn watch(dir: &str) -> u64 {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let want = std::path::PathBuf::from(dir);
    let mut sys = System::new();
    let kind = ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet);
    let running = |sys: &mut System| {
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
        sys.processes().values().any(|p| p.exe().is_some_and(|e| e.starts_with(&want)))
    };
    let t = Instant::now();
    while !running(&mut sys) {
        if t.elapsed() > Duration::from_secs(120) {
            return 0;
        }
        std::thread::sleep(Duration::from_secs(3));
    }
    let started = Instant::now();
    let mut misses = 0;
    loop {
        std::thread::sleep(Duration::from_secs(5));
        if running(&mut sys) {
            misses = 0;
        } else {
            // Some games restart themselves (anti-cheat, launchers); allow a gap.
            misses += 1;
            if misses >= 3 {
                break;
            }
        }
    }
    started.elapsed().as_secs().saturating_sub(10)
}

/// Big Picture: the main window full screen, or back.
#[tauri::command]
pub fn library_big_picture(app: AppHandle, on: bool) -> Res<()> {
    if let Some(w) = app.get_webview_window("main") {
        w.set_fullscreen(on).map_err(err)?;
    }
    Ok(())
}
