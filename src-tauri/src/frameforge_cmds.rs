//! Tauri commands for FrameForge (the Performance section).
//!
//! The commands are thin: every decision lives in `arcadia_core::frameforge`.
//! Anything that scans hardware, talks to a driver or lists processes runs on
//! the blocking pool so the UI thread never waits on it.

use crate::{err, AppState, Res};
use arcadia_core::frameforge::{self as ff, FrameForge};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// FrameForge is created on first use, not at start-up, so opening Arcadia
/// does not load any GPU driver library until the user needs it.
pub(crate) fn forge(app: &AppHandle) -> Arc<FrameForge> {
    let st = app.state::<AppState>();
    st.ff
        .get_or_init(|| {
            let f = Arc::new(FrameForge::new());
            f.set_games(&st.games.lock());
            f
        })
        .clone()
}

/// Run blocking FrameForge work off the UI thread.
async fn blocking<T: Send + 'static>(app: &AppHandle, f: impl FnOnce(Arc<FrameForge>) -> T + Send + 'static) -> Res<T> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || f(forge(&app))).await.map_err(err)
}

#[tauri::command]
pub async fn ff_overview(app: AppHandle, rescan: bool) -> Res<ff::service::Overview> {
    blocking(&app, move |f| f.overview(rescan)).await
}

#[tauri::command]
pub async fn ff_telemetry(app: AppHandle) -> Res<ff::telemetry::Sample> {
    blocking(&app, |f| f.telemetry()).await
}

#[tauri::command]
pub async fn ff_processes(app: AppHandle) -> Res<Vec<ff::processes::ProcessInfo>> {
    blocking(&app, |f| f.processes()).await
}

/// The running process most likely to be a library game, if any.
#[tauri::command]
pub async fn ff_running_game(app: AppHandle) -> Res<Option<ff::processes::ProcessInfo>> {
    blocking(&app, |f| ff::processes::pick_running_game(&f.processes()).cloned()).await
}

#[tauri::command]
pub async fn ff_presentmon(app: AppHandle) -> Res<ff::presentmon::Availability> {
    blocking(&app, |f| f.presentmon()).await
}

#[tauri::command]
pub async fn ff_config_get(app: AppHandle) -> Res<ff::config::Config> {
    blocking(&app, |f| f.config()).await
}

#[tauri::command]
pub async fn ff_config_set(app: AppHandle, config: ff::config::Config) -> Res<ff::config::Config> {
    blocking(&app, move |f| f.set_config(config).map(|_| f.config())).await?.map_err(err)
}

#[tauri::command]
pub async fn ff_monitor_start(app: AppHandle, pid: u32, name: String) -> Res<()> {
    blocking(&app, move |f| f.monitor_start(pid, name)).await?.map_err(err)
}

#[tauri::command]
pub async fn ff_monitor_stop(app: AppHandle) -> Res<()> {
    blocking(&app, |f| f.monitor_stop()).await
}

#[tauri::command]
pub async fn ff_monitor_snapshot(app: AppHandle) -> Res<ff::monitor::Snapshot> {
    blocking(&app, |f| f.monitor_snapshot()).await
}
