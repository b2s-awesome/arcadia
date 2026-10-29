//! Minecraft+: packs, worlds and backups, screenshots, servers, updates,
//! export, logs and the crash explainer, and skins and capes.

use crate::{err, Res};
use arcadia_core::minecraft::{auth, content, crash, export, instance, java, servers, skins, updates};
use serde::Serialize;
use std::path::PathBuf;
use tauri::AppHandle;

// ── packs ──

#[tauri::command]
pub fn mc_packs(id: String, kind: String) -> Res<Vec<content::Pack>> {
    content::packs(&id, &kind).map_err(err)
}
#[tauri::command]
pub fn mc_pack_toggle(id: String, kind: String, file: String, on: bool) -> Res<String> {
    content::set_pack_enabled(&id, &kind, &file, on).map_err(err)
}
#[tauri::command]
pub fn mc_pack_remove(id: String, kind: String, file: String) -> Res<()> {
    content::remove_pack(&id, &kind, &file).map_err(err)
}
#[tauri::command]
pub fn mc_packs_add(id: String, kind: String, files: Vec<String>) -> Res<usize> {
    content::add_packs(&id, &kind, &files.into_iter().map(PathBuf::from).collect::<Vec<_>>()).map_err(err)
}

// ── worlds ──

#[tauri::command]
pub async fn mc_worlds(id: String) -> Res<Vec<content::World>> {
    tauri::async_runtime::spawn_blocking(move || content::worlds(&id)).await.map_err(err)?.map_err(err)
}
#[tauri::command]
pub async fn mc_world_backup(id: String, folder: String) -> Res<content::Backup> {
    tauri::async_runtime::spawn_blocking(move || content::backup_world(&id, &folder)).await.map_err(err)?.map_err(err)
}
#[tauri::command]
pub async fn mc_world_restore(id: String, folder: String, file: String) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || content::restore_backup(&id, &folder, &file)).await.map_err(err)?.map_err(err)
}
#[tauri::command]
pub fn mc_world_backup_delete(id: String, folder: String, file: String) -> Res<()> {
    content::delete_backup(&id, &folder, &file).map_err(err)
}
#[tauri::command]
pub async fn mc_world_delete(id: String, folder: String, keep_backup: bool) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || content::delete_world(&id, &folder, keep_backup)).await.map_err(err)?.map_err(err)
}

// ── screenshots ──

#[tauri::command]
pub fn mc_screenshots(id: String) -> Vec<content::Shot> {
    content::screenshots(&id)
}
#[tauri::command]
pub fn mc_screenshot_delete(id: String, file: String) -> Res<()> {
    content::delete_screenshot(&id, &file).map_err(err)
}

// ── servers ──

#[tauri::command]
pub fn mc_servers(id: String) -> Vec<servers::SavedServer> {
    servers::list(&id)
}
#[tauri::command]
pub fn mc_server_add(id: String, name: String, address: String) -> Res<()> {
    servers::add(&id, &name, &address).map_err(err)
}
#[tauri::command]
pub fn mc_server_remove(id: String, address: String) -> Res<()> {
    servers::remove(&id, &address).map_err(err)
}
#[tauri::command]
pub async fn mc_server_ping(address: String) -> Res<servers::Status> {
    servers::ping(&address).await.map_err(err)
}

// ── updates and export ──

#[tauri::command]
pub async fn mc_updates(id: String) -> Res<Vec<updates::Update>> {
    updates::check(&id).await.map_err(err)
}
#[tauri::command]
pub async fn mc_update_apply(id: String, update: updates::Update) -> Res<String> {
    updates::apply(&id, &update).await.map_err(err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exported {
    path: String,
    linked: usize,
    packed: usize,
}
#[tauri::command]
pub async fn mc_export(id: String, dest: String, options: export::ExportOptions) -> Res<Exported> {
    let (linked, packed) = export::export(&id, std::path::Path::new(&dest), &options).await.map_err(err)?;
    Ok(Exported { path: dest, linked, packed })
}

// ── Java and auto-join ──

#[derive(Serialize)]
pub struct Runtime {
    id: &'static str,
    label: &'static str,
}
#[tauri::command]
pub fn mc_java_runtimes() -> Vec<Runtime> {
    java::RUNTIMES.iter().map(|(id, label)| Runtime { id, label }).collect()
}
#[tauri::command]
pub fn mc_set_launch(id: String, java: String, java_path: Option<String>, auto_join: String) -> Res<()> {
    let mut i = instance::get(&id).map_err(err)?;
    if !(java.is_empty() || java == "custom" || java::RUNTIMES.iter().any(|(r, _)| *r == java)) {
        return Err("Unknown Java choice".into());
    }
    i.java = java;
    i.java_path = java_path.filter(|p| !p.trim().is_empty());
    i.auto_join = auto_join.trim().to_string();
    instance::save(&i).map_err(err)
}

// ── logs and crashes ──

#[tauri::command]
pub fn mc_logs(id: String) -> Vec<crash::LogFile> {
    crash::logs(&id)
}
#[tauri::command]
pub fn mc_log_read(id: String, path: String) -> Res<String> {
    crash::read_log(&id, &path, 512 * 1024).map_err(err)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashInfo {
    pub instance: String,
    /// Log or crash report it was read from.
    pub path: String,
    pub diagnoses: Vec<crash::Diagnosis>,
    pub last_error: Option<String>,
}

fn diagnose_file(id: &str, f: &crash::LogFile) -> Option<CrashInfo> {
    let text = crash::read_log(id, &f.path, 2 * 1024 * 1024).ok()?;
    Some(CrashInfo { instance: id.into(), path: f.path.clone(), diagnoses: crash::explain(&text), last_error: crash::last_error(&text) })
}

/// After a crash: the newest crash report since `since`, else latest.log.
pub fn diagnose_after(id: &str, since: std::time::SystemTime) -> Option<CrashInfo> {
    let since = since.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0).saturating_sub(2);
    let logs = crash::logs(id);
    let report = logs.iter().find(|l| (l.kind == "crash" || l.kind == "jvm") && l.modified >= since);
    let f = report.or_else(|| logs.iter().find(|l| l.path == "logs/latest.log"))?;
    diagnose_file(id, f)
}

/// Explain a log file, or the newest crash report when `path` is empty.
#[tauri::command]
pub fn mc_explain(id: String, path: Option<String>) -> Res<Option<CrashInfo>> {
    let logs = crash::logs(&id);
    let f = match path.filter(|p| !p.is_empty()) {
        Some(p) => logs.iter().find(|l| l.path == p),
        None => logs.iter().find(|l| l.kind == "crash").or_else(|| logs.iter().find(|l| l.path == "logs/latest.log")),
    };
    Ok(f.and_then(|f| diagnose_file(&id, f)))
}

// ── skins and capes ──

#[tauri::command]
pub fn mc_skins() -> Vec<skins::SkinWithData> {
    skins::library()
}
#[tauri::command]
pub fn mc_skin_add_file(path: String) -> Res<skins::SkinWithData> {
    skins::add_file(std::path::Path::new(&path)).map_err(err)
}
#[tauri::command]
pub async fn mc_skin_add_player(name: String) -> Res<skins::SkinWithData> {
    skins::add_from_player(&name).await.map_err(err)
}
#[tauri::command]
pub fn mc_skin_edit(id: String, name: String, variant: String) -> Res<()> {
    skins::rename(&id, &name, &variant).map_err(err)
}
#[tauri::command]
pub fn mc_skin_remove(id: String) -> Res<()> {
    skins::remove(&id).map_err(err)
}

async fn token(app: &AppHandle) -> Res<String> {
    use tauri::Manager;
    let cid = app.state::<crate::AppState>().settings.lock().minecraft_client_id.clone();
    Ok(auth::session(&cid, None).await.map_err(err)?.access_token)
}
#[tauri::command]
pub async fn mc_profile(app: AppHandle) -> Res<skins::Profile> {
    skins::profile(&token(&app).await?).await.map_err(err)
}
#[tauri::command]
pub async fn mc_skin_wear(app: AppHandle, id: String) -> Res<skins::Profile> {
    skins::wear(&token(&app).await?, &id).await.map_err(err)
}
#[tauri::command]
pub async fn mc_cape_set(app: AppHandle, cape: Option<String>) -> Res<skins::Profile> {
    skins::set_cape(&token(&app).await?, cape.as_deref()).await.map_err(err)
}
