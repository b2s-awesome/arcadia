//! Tauri commands for the Minecraft launcher.

use crate::{err, hide_for_game, AppState, Res};
use arcadia_core::minecraft::{
    self, auth, curseforge, import, instance, launch, loaders, modrinth, security, OnProgress,
};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::io::{AsyncBufReadExt, BufReader};

fn settings(app: &AppHandle) -> arcadia_core::Settings {
    app.state::<AppState>().settings.lock().clone()
}

fn on_progress(app: &AppHandle) -> OnProgress {
    let h = app.clone();
    Arc::new(move |p| {
        let _ = h.emit("mc-progress", p);
    })
}

// ───────────────────────── versions & instances ─────────────────────────

#[tauri::command]
pub async fn mc_versions(snapshots: bool) -> Res<Vec<minecraft::version::ManifestEntry>> {
    let m = minecraft::install::manifest().await.map_err(err)?;
    Ok(m.versions.into_iter().filter(|v| v.kind == "release" || (snapshots && v.kind == "snapshot")).collect())
}

#[tauri::command]
pub async fn mc_loader_versions(loader: loaders::Loader, mc: String) -> Res<Vec<loaders::LoaderVersion>> {
    loaders::versions(loader, &mc).await.map_err(err)
}

#[tauri::command]
pub fn mc_instances() -> Vec<instance::Instance> {
    instance::list()
}

#[tauri::command]
pub fn mc_create(name: String, mc: String, loader: loaders::Loader, loader_version: Option<String>, memory_mb: u32) -> Res<instance::Instance> {
    instance::create(&name, &mc, loader, loader_version, memory_mb).map_err(err)
}

/// Save editable fields (name, memory, JVM arguments).
#[tauri::command]
pub fn mc_update(id: String, name: String, memory_mb: u32, java_args: String) -> Res<instance::Instance> {
    let mut i = instance::get(&id).map_err(err)?;
    if !name.trim().is_empty() {
        i.name = name.trim().to_string();
    }
    i.memory_mb = memory_mb.clamp(512, 65536);
    i.java_args = java_args;
    instance::save(&i).map_err(err)?;
    Ok(i)
}

#[tauri::command]
pub fn mc_delete(app: AppHandle, id: String) -> Res<()> {
    if app.state::<AppState>().mc_running.lock().contains_key(&id) {
        return Err("Close the game before deleting this instance".into());
    }
    instance::delete(&id).map_err(err)
}

#[tauri::command]
pub fn mc_duplicate(id: String, name: String) -> Res<instance::Instance> {
    instance::duplicate(&id, &name).map_err(err)
}

#[tauri::command]
pub fn mc_mods(id: String) -> Res<Vec<instance::ModFile>> {
    instance::mods(&id).map_err(err)
}

#[tauri::command]
pub fn mc_mod_toggle(id: String, file: String, enabled: bool) -> Res<String> {
    instance::set_mod_enabled(&id, &file, enabled).map_err(err)
}

#[tauri::command]
pub fn mc_mod_remove(id: String, file: String) -> Res<()> {
    instance::remove_mod(&id, &file).map_err(err)
}

#[tauri::command]
pub fn mc_open_folder(id: String, sub: Option<String>) -> Res<()> {
    let mut p = instance::game_dir(&id);
    if let Some(s) = sub.filter(|s| !s.is_empty()) {
        p = p.join(minecraft::safe_rel_path(&s).ok_or("bad folder")?);
    }
    std::fs::create_dir_all(&p).map_err(err)?;
    crate::open_path(&p)
}

// ───────────────────────── content ─────────────────────────

#[tauri::command]
pub async fn mc_search(app: AppHandle, source: String, query: String, kind: String, instance_id: Option<String>) -> Res<Vec<modrinth::Hit>> {
    let inst = instance_id.as_deref().and_then(|i| instance::get(i).ok());
    let mc = inst.as_ref().map(|i| i.mc_version.clone());
    let loader = inst.as_ref().map(|i| i.loader).filter(|l| *l != loaders::Loader::Vanilla);
    match source.as_str() {
        "curseforge" => curseforge::search(&settings(&app).curseforge_api_key, &query, &kind, mc.as_deref(), loader).await.map_err(err),
        _ => modrinth::search(&query, &kind, mc.as_deref(), loader).await.map_err(err),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    file_name: String,
    dependencies_added: Vec<String>,
}

#[tauri::command]
pub async fn mc_install(app: AppHandle, source: String, project_id: String, kind: String, instance_id: String) -> Res<InstallResult> {
    match source.as_str() {
        "curseforge" => {
            let id: u64 = project_id.parse().map_err(err)?;
            let f = curseforge::install_project(&settings(&app).curseforge_api_key, &instance_id, id, &kind).await.map_err(err)?;
            Ok(InstallResult { file_name: f, dependencies_added: vec![] })
        }
        _ => {
            let r = modrinth::install_project(&instance_id, &project_id, &kind).await.map_err(err)?;
            Ok(InstallResult { file_name: r.file_name, dependencies_added: r.dependencies_added })
        }
    }
}

#[tauri::command]
pub async fn mc_install_pack(app: AppHandle, source: String, project_id: String) -> Res<import::ImportResult> {
    let on = on_progress(&app);
    let s = settings(&app);
    match source.as_str() {
        "curseforge" => {
            let id: u64 = project_id.parse().map_err(err)?;
            let path = curseforge::download_pack(&s.curseforge_api_key, id).await.map_err(err)?;
            let src = instance::PackSource { platform: "curseforge".into(), project_id: Some(project_id), version_id: None, name: None };
            import::curseforge_zip(&path, &s.curseforge_api_key, Some(src), &on).await.map_err(err)
        }
        _ => {
            let versions = modrinth::versions(&project_id, None, None).await.map_err(err)?;
            let v = versions.first().ok_or("This modpack has no versions to install")?;
            let path = modrinth::download_pack(&v.id).await.map_err(err)?;
            let src = instance::PackSource { platform: "modrinth".into(), project_id: Some(project_id), version_id: Some(v.id.clone()), name: Some(v.name.clone()) };
            import::mrpack(&path, Some(src), &on).await.map_err(err)
        }
    }
}

#[tauri::command]
pub async fn mc_import(app: AppHandle, path: String) -> Res<import::ImportResult> {
    let on = on_progress(&app);
    import::any(&PathBuf::from(path), &settings(&app).curseforge_api_key, &on).await.map_err(err)
}

#[tauri::command]
pub fn mc_discover() -> Vec<import::Found> {
    import::discover()
}

// ───────────────────────── accounts ─────────────────────────

#[tauri::command]
pub fn mc_accounts() -> Vec<auth::Account> {
    auth::accounts()
}

#[tauri::command]
pub async fn mc_login_start(app: AppHandle) -> Res<auth::DeviceCode> {
    auth::start(&settings(&app).minecraft_client_id).await.map_err(err)
}

#[tauri::command]
pub async fn mc_login_finish(app: AppHandle, code: auth::DeviceCode) -> Res<auth::Account> {
    auth::finish(&settings(&app).minecraft_client_id, &code).await.map_err(err)
}

#[tauri::command]
pub fn mc_account_select(uuid: String) -> Res<()> {
    auth::set_active(&uuid).map_err(err)
}

#[tauri::command]
pub fn mc_account_remove(uuid: String) -> Res<()> {
    auth::remove(&uuid).map_err(err)
}

// ───────────────────────── play ─────────────────────────

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogLine {
    instance: String,
    line: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Exited {
    instance: String,
    code: Option<i32>,
    seconds: u64,
}

#[tauri::command]
pub async fn mc_play(app: AppHandle, id: String) -> Res<()> {
    let st = app.state::<AppState>();
    if st.mc_running.lock().contains_key(&id) {
        return Err("This instance is already running".into());
    }
    let s = settings(&app);
    let session = auth::session(&s.minecraft_client_id, None).await.map_err(err)?;
    let mut inst = instance::get(&id).map_err(err)?;
    let on = on_progress(&app);
    let ready = launch::prepare(&mut inst, &session, &on).await.map_err(err)?;

    let mut cmd = tokio::process::Command::new(&ready.java);
    cmd.args(&ready.args).current_dir(&ready.game_dir).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        cmd.creation_flags(0x0800_0000); // no console window
    }
    let mut child = cmd.spawn().map_err(|e| format!("Couldn't start Java: {e}"))?;
    if let Some(pid) = child.id() {
        st.mc_running.lock().insert(id.clone(), pid);
    }
    let _ = app.emit("mc-started", &id);

    for stream in [child.stdout.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>), child.stderr.take().map(|s| Box::new(s) as _)].into_iter().flatten() {
        let h = app.clone();
        let iid = id.clone();
        let session = session.clone();
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stream).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                // Never let the access token reach the UI or logs.
                let line = l.replace(&session.access_token, "<token>");
                let _ = h.emit("mc-log", LogLine { instance: iid.clone(), line });
            }
        });
    }

    let started = std::time::Instant::now();
    inst.last_played = Some(instance::now());
    let _ = instance::save(&inst);
    let h = app.clone();
    let iid = id.clone();
    tauri::async_runtime::spawn(async move {
        let code = child.wait().await.ok().and_then(|s| s.code());
        let secs = started.elapsed().as_secs();
        h.state::<AppState>().mc_running.lock().remove(&iid);
        if let Ok(mut i) = instance::get(&iid) {
            i.playtime_secs += secs;
            let _ = instance::save(&i);
        }
        let _ = h.emit("mc-exited", Exited { instance: iid, code, seconds: secs });
    });
    hide_for_game(&app, &s);
    Ok(())
}

#[tauri::command]
pub fn mc_running(app: AppHandle) -> Vec<String> {
    app.state::<AppState>().mc_running.lock().keys().cloned().collect()
}

#[tauri::command]
pub fn mc_kill(app: AppHandle, id: String) -> Res<()> {
    let pid = app.state::<AppState>().mc_running.lock().get(&id).copied().ok_or("That instance isn't running")?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).creation_flags(0x0800_0000).status().map_err(err)?;
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("kill").arg(pid.to_string()).status().map_err(err)?;
    }
    Ok(())
}

// ───────────────────────── security ─────────────────────────

#[tauri::command]
pub fn mc_incidents() -> Vec<security::Incident> {
    security::INCIDENTS.to_vec()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityReport {
    findings: Vec<security::Finding>,
    leftovers: Vec<security::Leftover>,
    pipeblocker_advised: Vec<String>,
    checked_instances: usize,
}

#[tauri::command]
pub async fn mc_security_scan() -> Res<SecurityReport> {
    tauri::async_runtime::spawn_blocking(|| {
        let list = instance::list();
        SecurityReport {
            findings: list.iter().flat_map(security::scan_instance).collect(),
            leftovers: security::check_leftovers(),
            pipeblocker_advised: list.iter().filter(|i| security::pipeblocker_advised(i)).map(|i| i.name.clone()).collect(),
            checked_instances: list.len(),
        }
    })
    .await
    .map_err(err)
}

#[tauri::command]
pub async fn mc_verify(app: AppHandle, id: String) -> Res<Vec<security::Verified>> {
    security::verify_instance(&id, &settings(&app).curseforge_api_key).await.map_err(err)
}

// ───────────────────────── standalone window ─────────────────────────

/// Open Minecraft according to the user's setting: as a page in Arcadia,
/// or as the standalone launcher window.
pub fn open_minecraft(app: &AppHandle, force_window: bool) {
    let mode = settings(app).minecraft_open_mode;
    if !force_window && mode != "window" {
        crate::show_main(app);
        let _ = app.emit("navigate", "minecraft");
        return;
    }
    if let Some(w) = app.get_webview_window("minecraft") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "minecraft", WebviewUrl::App("index.html?mode=minecraft".into()))
        .title("Minecraft")
        .inner_size(1180.0, 760.0)
        .min_inner_size(900.0, 580.0)
        .decorations(false)
        .center()
        .build();
}

#[tauri::command]
pub fn open_minecraft_launcher(app: AppHandle) {
    open_minecraft(&app, false);
}

/// "Pop out": move the launcher into its own window.
#[tauri::command]
pub fn minecraft_pop_out(app: AppHandle) {
    open_minecraft(&app, true);
    let _ = app.emit("navigate", "library");
}

/// "Dock": close the standalone window and show the launcher inside Arcadia.
#[tauri::command]
pub fn minecraft_dock(app: AppHandle) {
    crate::show_main(&app);
    let _ = app.emit("navigate", "minecraft");
    if let Some(w) = app.get_webview_window("minecraft") {
        let _ = w.destroy();
    }
}

/// Open a web page (mod pages, manual downloads, incident write-ups).
#[tauri::command]
pub fn open_url(url: String) -> Res<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Only web links can be opened".into());
    }
    arcadia_core::library::open_uri(&url).map_err(err)
}

/// Put "<name> Minecraft" shortcuts on the desktop and in the Start menu that
/// open the standalone launcher.
#[tauri::command]
pub fn create_minecraft_shortcut(app: AppHandle, name: String) -> Res<Vec<String>> {
    let exe = app.state::<AppState>().exe.clone();
    let clean: String = name.chars().filter(|c| c.is_alphanumeric() || " -_()".contains(*c)).collect();
    let label = if clean.trim().is_empty() { "Arcadia Minecraft".to_string() } else { clean.trim().to_string() };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut made = Vec::new();
        let mut targets = Vec::new();
        if let Some(d) = dirs_desktop() {
            targets.push(d.join(format!("{label}.lnk")));
        }
        if let Ok(appdata) = std::env::var("APPDATA") {
            targets.push(PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs").join(format!("{label}.lnk")));
        }
        for t in targets {
            let ps = format!(
                "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$s.TargetPath='{}';$s.Arguments='--minecraft';$s.IconLocation='{},0';$s.Save()",
                t.display().to_string().replace('\'', "''"),
                exe.replace('\'', "''"),
                exe.replace('\'', "''")
            );
            let ok = std::process::Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &ps])
                .creation_flags(0x0800_0000)
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                made.push(t.display().to_string());
            }
        }
        if made.is_empty() {
            return Err("Windows didn't let Arcadia create the shortcuts".into());
        }
        Ok(made)
    }
    #[cfg(not(windows))]
    {
        let _ = (exe, label);
        Err("Shortcuts can only be created on Windows; run Arcadia with --minecraft instead".into())
    }
}

#[cfg(windows)]
fn dirs_desktop() -> Option<PathBuf> {
    std::env::var("USERPROFILE").ok().map(|p| PathBuf::from(p).join("Desktop")).filter(|p| p.is_dir())
}
