//! Customisation+: backups, the sync folder, and window helpers.

use crate::{err, AppState, Res};
use arcadia_core::backup;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Serialize)]
pub struct Part {
    id: &'static str,
    label: &'static str,
}

#[tauri::command]
pub fn backup_parts() -> Vec<Part> {
    backup::PARTS.iter().map(|(id, label)| Part { id, label }).collect()
}

#[tauri::command]
pub async fn backup_export(dest: String, parts: Vec<String>, keys: bool) -> Res<backup::Manifest> {
    tauri::async_runtime::spawn_blocking(move || backup::export(std::path::Path::new(&dest), &parts, keys)).await.map_err(err)?.map_err(err)
}

#[tauri::command]
pub fn backup_inspect(path: String) -> Res<backup::Manifest> {
    backup::inspect(std::path::Path::new(&path)).map_err(err)
}

/// Restore, then make every window pick up the restored settings.
#[tauri::command]
pub async fn backup_restore(app: AppHandle, path: String, parts: Vec<String>) -> Res<usize> {
    let n = tauri::async_runtime::spawn_blocking(move || backup::restore(std::path::Path::new(&path), &parts)).await.map_err(err)?.map_err(err)?;
    reload(&app);
    Ok(n)
}

fn reload(app: &AppHandle) {
    let fresh = arcadia_core::Settings::load();
    *app.state::<AppState>().settings.lock() = fresh;
    let _ = app.emit("settings-changed", "backup");
}

/// Write the sync copy now.
#[tauri::command]
pub async fn sync_now(app: AppHandle) -> Res<backup::Manifest> {
    let folder = app.state::<AppState>().settings.lock().sync_folder.clone();
    let m = tauri::async_runtime::spawn_blocking(move || backup::sync_write(&folder)).await.map_err(err)?.map_err(err)?;
    mark_synced(&app, m.created);
    Ok(m)
}

/// A newer copy another PC left in the sync folder, if there is one.
#[tauri::command]
pub fn sync_check(app: AppHandle) -> Option<backup::Manifest> {
    let s = app.state::<AppState>().settings.lock().clone();
    backup::sync_newer(&s.sync_folder, s.sync_last)
}

#[tauri::command]
pub async fn sync_restore(app: AppHandle) -> Res<usize> {
    let folder = app.state::<AppState>().settings.lock().sync_folder.clone();
    let file = backup::sync_file(&folder).ok_or("The sync folder isn't there")?;
    let m = backup::inspect(&file).map_err(err)?;
    let n = tauri::async_runtime::spawn_blocking(move || backup::restore(&file, &m.parts)).await.map_err(err)?.map_err(err)?;
    let created = backup::inspect(&backup::sync_file(&folder).unwrap()).map(|m| m.created).unwrap_or(0);
    reload(&app);
    mark_synced(&app, created);
    Ok(n)
}

fn mark_synced(app: &AppHandle, at: u64) {
    let st = app.state::<AppState>();
    let mut s = st.settings.lock();
    s.sync_last = at;
    let _ = s.save();
}

static GEN: AtomicU64 = AtomicU64::new(0);

/// After settings change: write the sync copy once things have been quiet
/// for a few seconds.
pub(crate) fn sync_soon(app: &AppHandle) {
    let folder = app.state::<AppState>().settings.lock().sync_folder.clone();
    if backup::sync_file(&folder).is_none() {
        return;
    }
    let g = GEN.fetch_add(1, Ordering::Relaxed) + 1;
    let h = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(6));
        if GEN.load(Ordering::Relaxed) != g {
            return;
        }
        match backup::sync_write(&folder) {
            Ok(m) => mark_synced(&h, m.created),
            Err(e) => log::warn!("sync: {e:#}"),
        }
    });
}
