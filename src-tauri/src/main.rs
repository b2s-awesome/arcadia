// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use arcadia_core::library::{self, Game, LaunchTarget};
use arcadia_core::proxy::rules::{parse_ids, RuleConfig};
use arcadia_core::proxy::{inspector, Proxy, ProxyConfig};
use arcadia_core::roblox::{self, cache, deploy, fflags, ProxyHook};
use arcadia_core::{paths, Settings};
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::{Map, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};

mod addon_cmds;
mod winfix;
mod minecraft_cmds;
mod custom_cmds;
mod library_cmds;
mod lyrics_cmds;
mod mc_extra_cmds;
mod media_keys;
mod music_cmds;
mod presence;
mod roblox_cmds;
mod streaming_cmds;

pub(crate) struct AppState {
    settings: Mutex<Settings>,
    games: Mutex<Vec<Game>>,
    rules: Mutex<RuleConfig>,
    proxy: tokio::sync::Mutex<Option<Proxy>>,
    sessions: Arc<AtomicUsize>,
    exe: String,
    /// Running Minecraft instances: instance id → process id.
    mc_running: Mutex<std::collections::HashMap<String, u32>>,
    /// Windows were created see-through this run (Translucent window addon).
    /// Transparency can only be chosen when a window is made, so turning the
    /// addon on or off takes a restart.
    see_through: bool,
}

pub(crate) type Res<T> = Result<T, String>;
pub(crate) fn err(e: impl std::fmt::Display) -> String {
    format!("{e:#}")
}

// ───────────────────────── settings ─────────────────────────

#[tauri::command]
fn get_settings(st: State<'_, AppState>) -> Settings {
    st.settings.lock().clone()
}

/// Saves settings, then tells the *other* windows to reload them, so the hub
/// and the popped-out Minecraft window never overwrite each other's changes.
#[tauri::command]
async fn save_settings(app: AppHandle, window: tauri::Window, st: State<'_, AppState>, mut settings: Settings) -> Res<()> {
    // The UI may hold an older sync time than the backend just wrote.
    settings.sync_last = settings.sync_last.max(st.settings.lock().sync_last);
    settings.save().map_err(err)?;
    let boot = settings.run_on_boot.then(|| format!("\"{}\" --tray", st.exe));
    arcadia_core::registry::set_run_on_boot("Arcadia", boot.as_deref()).map_err(err)?;
    if let Some(p) = st.proxy.lock().await.as_ref() {
        p.set_fflags(live_fflags(&settings));
        p.inspector()
            .capture_bodies
            .store(settings.proxy_inspector_enabled, Ordering::Relaxed);
    }
    *st.settings.lock() = settings;
    let _ = app.emit("settings-changed", window.label());
    custom_cmds::sync_soon(&app);
    presence::poke();
    Ok(())
}

// ───────────────────────── library ─────────────────────────

#[tauri::command]
async fn scan_library(st: State<'_, AppState>) -> Res<library::ScanReport> {
    let report = tauri::async_runtime::spawn_blocking(library::scan).await.map_err(err)?;
    *st.games.lock() = report.games.clone();
    Ok(report)
}

#[tauri::command]
async fn launch_game(app: AppHandle, st: State<'_, AppState>, id: String) -> Res<()> {
    let game = st.games.lock().iter().find(|g| g.id == id).cloned();
    let game = game.ok_or("That game is no longer installed. Refresh the library.")?;
    if game.launch == LaunchTarget::Roblox {
        return roblox_cmds::roblox_play(app, None, None, None).await.map(|_| ());
    }
    if game.launch == LaunchTarget::Minecraft {
        return minecraft_cmds::open_minecraft(&app, false);
    }
    library_cmds::launch(&app, &game)?;
    let settings = st.settings.lock().clone();
    hide_for_game(&app, &settings);
    Ok(())
}

/// Hide Arcadia's windows when a game starts, if the user asked for that.
pub(crate) fn hide_for_game(app: &AppHandle, settings: &Settings) {
    if settings.hide_on_game_launch {
        for (_, w) in app.webview_windows() {
            let _ = w.hide();
        }
    }
}

#[tauri::command]
fn add_custom_game(name: String, target: LaunchTarget, cover: Option<String>) -> Res<()> {
    library::add_custom(name, target, cover).map(|_| ()).map_err(err)
}

#[tauri::command]
fn remove_custom_game(id: String) -> Res<()> {
    library::remove_custom(&id).map_err(err)
}

// ───────────────────────── roblox ─────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RobloxStatus {
    installed_version: Option<String>,
    channel: Option<String>,
    protocol_registered: bool,
    running_sessions: usize,
}

#[tauri::command]
fn roblox_status(st: State<'_, AppState>) -> RobloxStatus {
    let s = deploy::installed();
    RobloxStatus {
        installed_version: s.version,
        channel: s.channel,
        protocol_registered: cfg!(not(windows)) || roblox::is_protocol_registered(&st.exe),
        running_sessions: st.sessions.load(Ordering::Relaxed),
    }
}

#[tauri::command]
async fn roblox_latest(channel: String) -> Res<deploy::ClientVersion> {
    deploy::latest(&channel).await.map_err(err)
}

#[tauri::command]
async fn roblox_launch(app: AppHandle, uri: Option<String>) -> Res<String> {
    launch_roblox(app, uri).await
}

#[tauri::command]
fn roblox_reinstall() -> Res<()> {
    deploy::force_reinstall().map_err(err)
}

#[tauri::command]
fn roblox_register_protocol(st: State<'_, AppState>) -> Res<()> {
    roblox::register_protocols(&st.exe).map_err(err)
}

#[tauri::command]
fn clear_roblox_cache(kill: bool) -> cache::ClearReport {
    if kill {
        roblox::kill_all();
        std::thread::sleep(std::time::Duration::from_millis(600));
    }
    cache::clear()
}

#[tauri::command]
fn fflag_presets() -> Vec<fflags::Preset> {
    fflags::presets()
}

#[tauri::command]
fn parse_fflags(json: String) -> Res<Map<String, Value>> {
    fflags::parse_import(&json).map_err(err)
}

#[tauri::command]
fn open_folder(which: String) -> Res<()> {
    let p = match which.as_str() {
        "mods" => paths::mods_dir(),
        "roblox" => paths::roblox_versions_dir(),
        "proxy" => paths::proxy_dir(),
        _ => paths::root(),
    };
    std::fs::create_dir_all(&p).map_err(err)?;
    open_path(&p)
}

pub(crate) fn open_path(p: &std::path::Path) -> Res<()> {
    #[cfg(windows)]
    std::process::Command::new("explorer").arg(p).spawn().map_err(err)?;
    #[cfg(not(windows))]
    library::open_uri(&p.to_string_lossy()).map_err(err)?;
    Ok(())
}

/// Flags the proxy merges into Roblox's ClientSettings: the certificate flag,
/// plus the FPS cap and live FastFlags once the ban risk has been accepted.
fn live_fflags(settings: &Settings) -> Map<String, Value> {
    fflags::server_flags(settings, true)
}

async fn ensure_proxy(app: &AppHandle) -> anyhow::Result<ProxyHook> {
    let st = app.state::<AppState>();
    let settings = st.settings.lock().clone();
    let mut guard = st.proxy.lock().await;
    if guard.is_none() {
        let p = Proxy::start(ProxyConfig {
            port: settings.proxy_port,
            data_dir: paths::proxy_dir(),
            test_upstream: None,
        })
        .await?;
        let mut rx = p.inspector().tx.subscribe();
        let h = app.clone();
        tauri::async_runtime::spawn(async move {
            while let Ok(e) = rx.recv().await {
                let _ = h.emit("proxy-entry", e);
            }
        });
        *guard = Some(p);
    }
    let p = guard.as_ref().unwrap();
    p.set_rules(&st.rules.lock());
    p.set_fflags(live_fflags(&settings));
    p.inspector()
        .capture_bodies
        .store(settings.proxy_inspector_enabled, Ordering::Relaxed);
    p.prepare_launch();
    Ok(ProxyHook { port: p.port, ca_pem: p.ca_pem(), intercept: p.intercept_switch() })
}

pub(crate) async fn launch_roblox(app: AppHandle, uri: Option<String>) -> Res<String> {
    let (uri, ctx) = match uri {
        Some(u) => roblox_cmds::resolve_link(u).await,
        None => (None, roblox_cmds::Ctx::default()),
    };
    launch_roblox_as(app, uri, ctx).await
}

pub(crate) async fn launch_roblox_as(app: AppHandle, uri: Option<String>, ctx: roblox_cmds::Ctx) -> Res<String> {
    let st = app.state::<AppState>();
    let settings = st.settings.lock().clone();
    // Roblox's own updater re-registers its protocol; take it back each launch.
    let _ = roblox::register_protocols(&st.exe);

    let hook = if settings.proxy_enabled {
        Some(ensure_proxy(&app).await.map_err(err)?)
    } else {
        None
    };
    let h = app.clone();
    let on = move |p: deploy::Progress| {
        let _ = h.emit("roblox-progress", p);
    };
    let started = std::time::SystemTime::now();
    let launched = roblox::launch(&settings, uri.as_deref(), hook.clone(), &on)
        .await
        .map_err(err)?;
    let version = launched.version.clone();
    let _ = app.emit("roblox-launched", &version);
    if hook.is_some() {
        use arcadia_core::roblox::flagcache::Seed;
        let notice = match &launched.flag_cache {
            Seed::Missing => Some(
                "First launch with the proxy: Roblox is setting itself up, so replacements are off this time. \
                 Close Roblox and launch again and they'll apply."
                    .to_string(),
            ),
            Seed::Unsupported { reason } => Some(format!(
                "Arcadia couldn't prepare Roblox's flag cache ({reason}), so replacements are off this launch. \
                 Roblox may have changed its format."
            )),
            _ => None,
        };
        if let Some(n) = notice {
            let _ = app.emit("proxy-notice", n);
        }
    }

    // Follow the client: now playing, history, server region, rejoin.
    let clear_on_exit = hook.is_some() && settings.proxy_clear_cache_on_exit;
    roblox_cmds::follow(&app, started, launched.child, ctx, clear_on_exit);

    hide_for_game(&app, &settings);
    Ok(version)
}

// ───────────────────────── proxy ─────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProxyStatus {
    running: bool,
    port: Option<u16>,
    stats: Option<inspector::Stats>,
    ca_path: String,
}

#[tauri::command]
async fn proxy_status(st: State<'_, AppState>) -> Res<ProxyStatus> {
    let g = st.proxy.lock().await;
    Ok(ProxyStatus {
        running: g.is_some(),
        port: g.as_ref().map(|p| p.port),
        stats: g.as_ref().map(|p| p.inspector().stats()),
        ca_path: paths::proxy_dir().join("ca.pem").to_string_lossy().into(),
    })
}

#[tauri::command]
async fn proxy_start(app: AppHandle) -> Res<u16> {
    ensure_proxy(&app).await.map(|h| h.port).map_err(err)
}

#[tauri::command]
async fn proxy_stop(st: State<'_, AppState>) -> Res<()> {
    if let Some(p) = st.proxy.lock().await.take() {
        p.stop();
    }
    Ok(())
}

#[tauri::command]
fn rules_get(st: State<'_, AppState>) -> RuleConfig {
    st.rules.lock().clone()
}

#[tauri::command]
async fn rules_save(st: State<'_, AppState>, config: RuleConfig) -> Res<()> {
    config.save().map_err(err)?;
    if let Some(p) = st.proxy.lock().await.as_ref() {
        p.set_rules(&config);
    }
    *st.rules.lock() = config;
    Ok(())
}

#[derive(serde::Serialize)]
struct RulesImported {
    config: RuleConfig,
    report: arcadia_core::proxy::rules::ImportReport,
}

/// Save imported rules, copy their files into Arcadia's folder and hand them
/// to a running proxy.
async fn finish_import(
    st: &State<'_, AppState>,
    run: impl FnOnce(&mut RuleConfig) -> anyhow::Result<arcadia_core::proxy::rules::ImportReport>,
) -> Res<RulesImported> {
    let (config, report) = {
        let mut r = st.rules.lock();
        let report = run(&mut r).map_err(err)?;
        r.adopt_files(&report.profiles);
        r.save().map_err(err)?;
        (r.clone(), report)
    };
    if let Some(p) = st.proxy.lock().await.as_ref() {
        p.set_rules(&config);
    }
    Ok(RulesImported { config, report })
}

/// Pasted JSON: an Arcadia export or a Fleasion config.
#[tauri::command]
async fn rules_import(st: State<'_, AppState>, json: String) -> Res<RulesImported> {
    finish_import(&st, |r| r.import(&json, None)).await
}

/// An opened `.json` file: an Arcadia export or a Fleasion config (named after the file).
#[tauri::command]
async fn rules_import_file(st: State<'_, AppState>, path: String) -> Res<RulesImported> {
    let path = std::path::PathBuf::from(path);
    let raw = std::fs::read(&path).map_err(err)?;
    let text = arcadia_core::proxy::fleasion::decode_text(&raw);
    let name = path.file_stem().map(|s| s.to_string_lossy().to_string());
    finish_import(&st, |r| r.import(&text, name.as_deref())).await
}

/// Every config from this PC's Fleasion install, keeping which were on.
#[tauri::command]
async fn rules_import_fleasion(st: State<'_, AppState>) -> Res<RulesImported> {
    let dir = arcadia_core::proxy::fleasion::data_dir();
    finish_import(&st, |r| r.import_fleasion_folder(&dir)).await
}

#[tauri::command]
fn rules_export(st: State<'_, AppState>, profile: Option<String>) -> Res<String> {
    let r = st.rules.lock();
    let out = match profile {
        Some(name) => {
            let p = r.profiles.iter().find(|p| p.name == name).ok_or("No profile with that name")?;
            serde_json::to_string_pretty(p)
        }
        None => serde_json::to_string_pretty(&*r),
    };
    out.map_err(err)
}

/// Copy a replacement file into Arcadia's own folder so rules keep working
/// if the original is moved.
#[tauri::command]
fn proxy_import_file(src: String) -> Res<String> {
    arcadia_core::proxy::rules::adopt_file(std::path::Path::new(&src)).map_err(err)
}

/// Copy a chosen wallpaper into Arcadia's folder (the only place the UI is
/// allowed to load local images from) and return the new path.
#[tauri::command]
fn set_wallpaper(src: String) -> Res<String> {
    let src = std::path::PathBuf::from(src);
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .filter(|e| ["png", "jpg", "jpeg", "webp", "gif", "bmp"].contains(&e.as_str()))
        .ok_or("Choose a PNG, JPG, WebP, GIF or BMP image")?;
    let dir = paths::wallpapers_dir();
    std::fs::create_dir_all(&dir).map_err(err)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let dest = dir.join(format!("wallpaper-{stamp}.{ext}"));
    std::fs::copy(&src, &dest).map_err(err)?;
    Ok(dest.to_string_lossy().into())
}

#[tauri::command]
fn parse_asset_ids(text: String) -> Vec<u64> {
    parse_ids(&text)
}

#[tauri::command]
async fn inspector_snapshot(st: State<'_, AppState>) -> Res<Vec<inspector::Entry>> {
    Ok(st.proxy.lock().await.as_ref().map(|p| p.inspector().snapshot()).unwrap_or_default())
}

#[tauri::command]
async fn inspector_clear(st: State<'_, AppState>) -> Res<()> {
    if let Some(p) = st.proxy.lock().await.as_ref() {
        p.inspector().clear();
    }
    Ok(())
}

// ───────────────────────── app ─────────────────────────

pub(crate) fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Handle a command line (first start, or forwarded from a second start).
fn handle_args(app: &AppHandle, args: &[String]) -> bool {
    if args.iter().any(|a| a == "--minecraft") {
        if let Err(e) = minecraft_cmds::open_minecraft(app, true) {
            // No launcher window: fall back to the hub so the user sees something.
            show_main(app);
            let _ = app.emit_to("main", "navigate", "minecraft");
            log::error!("{e}");
        }
        return true;
    }
    if let Some((place, account)) = roblox_cmds::shortcut_args(args) {
        let h = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = roblox_cmds::roblox_play(h.clone(), account, Some(place), None).await {
                show_main(&h);
                let _ = h.emit("roblox-error", e);
            }
        });
        return true;
    }
    if let Some(uri) = args.iter().skip(1).find(|a| roblox::is_roblox_uri(a)).cloned() {
        let h = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = launch_roblox(h.clone(), Some(uri)).await {
                show_main(&h);
                let _ = h.emit("roblox-error", e);
            }
        });
        return true;
    }
    false
}

fn main() {
    let _ = paths::ensure_all();
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let settings = Settings::load();
    let see_through = addon_cmds::see_through_wanted(&settings);

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if !handle_args(app, &argv) {
                show_main(app);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            settings: Mutex::new(settings),
            games: Mutex::new(Vec::new()),
            rules: Mutex::new(RuleConfig::load()),
            proxy: tokio::sync::Mutex::new(None),
            sessions: Arc::new(AtomicUsize::new(0)),
            exe,
            mc_running: Mutex::new(Default::default()),
            see_through,
        })
        .setup(move |app| {
            let h = app.handle().clone();
            // The main window comes from tauri.conf.json, but is built here so
            // it can be see-through when the Translucent window addon is on.
            let conf = app.config().app.windows.iter().find(|w| w.label == "main").cloned().expect("main window config");
            let builder = tauri::WebviewWindowBuilder::from_config(&h, &conf)?;
            let builder = if see_through {
                builder.transparent(true)
            } else {
                builder.background_color(tauri::window::Color(11, 11, 13, 255))
            };
            let main = builder.build()?;
            if see_through {
                winfix::hide_native_buttons(&main);
            }
            let show = MenuItem::with_id(app, "show", "Open Arcadia", true, None::<&str>)?;
            let play = MenuItem::with_id(app, "roblox", "Launch Roblox", true, None::<&str>)?;
            let cache_item = MenuItem::with_id(app, "cache", "Clear Roblox cache", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &play, &cache_item, &quit])?;
            TrayIconBuilder::with_id("tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Arcadia")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, e| match e.id.as_ref() {
                    "show" => show_main(app),
                    "roblox" => {
                        let h = app.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = roblox_cmds::roblox_play(h.clone(), None, None, None).await {
                                show_main(&h);
                                let _ = h.emit("roblox-error", e);
                            }
                        });
                    }
                    "cache" => {
                        let r = cache::clear();
                        let _ = app.emit("cache-cleared", r);
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, e| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                        show_main(tray.app_handle());
                    }
                })
                .build(app)?;

            app.manage(Arc::new(streaming_cmds::Streamer::new()));
            // The music player. A missing sound card isn't fatal: the page says so.
            match music_cmds::Music::start(&h) {
                Ok(m) => {
                    app.manage(m);
                    media_keys::start(&h);
                }
                Err(e) => log::error!("music player didn't start: {e:#}"),
            }
            let _ = app.asset_protocol_scope().allow_directory(arcadia_core::music::dir().join("art"), false);

            presence::start(&h);
            let _ = app.asset_protocol_scope().allow_directory(library::meta::art_dir(), false);
            // Let the UI show Steam's locally cached cover art.
            if let Some(dir) = library::steam_art_dir() {
                let _ = app.asset_protocol_scope().allow_directory(&dir, true);
            }

            let args: Vec<String> = std::env::args().collect();
            let launched_roblox = handle_args(&h, &args);
            let tray_start = args.iter().any(|a| a == "--tray");
            if launched_roblox || tray_start {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            Ok(())
        })
        .on_window_event(|w, e| {
            if let WindowEvent::CloseRequested { api, .. } = e {
                let app = w.app_handle();
                let to_tray = app.state::<AppState>().settings.lock().minimize_to_tray;
                if to_tray {
                    api.prevent_close();
                    let _ = w.hide();
                } else if w.label() == "minecraft" {
                    // Standalone launcher closed with the hub hidden: quit.
                    let main_visible = app.get_webview_window("main").and_then(|m| m.is_visible().ok()).unwrap_or(false);
                    if !main_visible {
                        app.exit(0);
                    }
                }
            }
        })
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            presence::presence_status,
            presence::presence_test,
            custom_cmds::backup_parts,
            custom_cmds::backup_export,
            custom_cmds::backup_inspect,
            custom_cmds::backup_restore,
            custom_cmds::sync_now,
            custom_cmds::sync_check,
            custom_cmds::sync_restore,
            save_settings,
            scan_library,
            launch_game,
            library_cmds::library_meta,
            library_cmds::library_meta_set,
            library_cmds::library_collections_set,
            library_cmds::library_art_import,
            library_cmds::library_art_search,
            library_cmds::library_art,
            library_cmds::library_big_picture,
            add_custom_game,
            remove_custom_game,
            roblox_status,
            roblox_latest,
            roblox_launch,
            roblox_reinstall,
            roblox_register_protocol,
            clear_roblox_cache,
            roblox_cmds::roblox_play,
            roblox_cmds::roblox_rejoin_cancel,
            roblox_cmds::roblox_accounts,
            roblox_cmds::roblox_account_login,
            roblox_cmds::roblox_account_cancel_login,
            roblox_cmds::roblox_account_remove,
            roblox_cmds::roblox_account_default,
            roblox_cmds::roblox_account_note,
            roblox_cmds::roblox_account_reorder,
            roblox_cmds::roblox_account_check,
            roblox_cmds::roblox_servers,
            roblox_cmds::roblox_server_region,
            roblox_cmds::roblox_games,
            roblox_cmds::roblox_place_from,
            roblox_cmds::roblox_history,
            roblox_cmds::roblox_history_add,
            roblox_cmds::roblox_history_favourite,
            roblox_cmds::roblox_history_forget,
            roblox_cmds::roblox_history_clear,
            roblox_cmds::roblox_shortcut,
            roblox_cmds::roblox_cursor_preview,
            roblox_cmds::roblox_flag_allowlist,
            fflag_presets,
            parse_fflags,
            open_folder,
            proxy_status,
            proxy_start,
            proxy_stop,
            rules_get,
            rules_save,
            rules_import,
            rules_import_file,
            rules_import_fleasion,
            rules_export,
            proxy_import_file,
            parse_asset_ids,
            set_wallpaper,
            minecraft_cmds::mc_versions,
            minecraft_cmds::mc_loader_versions,
            minecraft_cmds::mc_instances,
            minecraft_cmds::mc_create,
            minecraft_cmds::mc_update,
            minecraft_cmds::mc_delete,
            minecraft_cmds::mc_duplicate,
            minecraft_cmds::mc_mods,
            minecraft_cmds::mc_mod_toggle,
            minecraft_cmds::mc_mod_remove,
            minecraft_cmds::mc_open_folder,
            minecraft_cmds::mc_search,
            minecraft_cmds::mc_install,
            minecraft_cmds::mc_install_pack,
            minecraft_cmds::mc_import,
            minecraft_cmds::mc_discover,
            minecraft_cmds::mc_accounts,
            minecraft_cmds::mc_login_start,
            minecraft_cmds::mc_login_finish,
            minecraft_cmds::mc_account_select,
            minecraft_cmds::mc_account_remove,
            minecraft_cmds::mc_play,
            mc_extra_cmds::mc_packs,
            mc_extra_cmds::mc_pack_toggle,
            mc_extra_cmds::mc_pack_remove,
            mc_extra_cmds::mc_packs_add,
            mc_extra_cmds::mc_worlds,
            mc_extra_cmds::mc_world_backup,
            mc_extra_cmds::mc_world_restore,
            mc_extra_cmds::mc_world_backup_delete,
            mc_extra_cmds::mc_world_delete,
            mc_extra_cmds::mc_screenshots,
            mc_extra_cmds::mc_screenshot_delete,
            mc_extra_cmds::mc_servers,
            mc_extra_cmds::mc_server_add,
            mc_extra_cmds::mc_server_remove,
            mc_extra_cmds::mc_server_ping,
            mc_extra_cmds::mc_updates,
            mc_extra_cmds::mc_update_apply,
            mc_extra_cmds::mc_export,
            mc_extra_cmds::mc_java_runtimes,
            mc_extra_cmds::mc_set_launch,
            mc_extra_cmds::mc_logs,
            mc_extra_cmds::mc_log_read,
            mc_extra_cmds::mc_explain,
            mc_extra_cmds::mc_skins,
            mc_extra_cmds::mc_skin_add_file,
            mc_extra_cmds::mc_skin_add_player,
            mc_extra_cmds::mc_skin_edit,
            mc_extra_cmds::mc_skin_remove,
            mc_extra_cmds::mc_profile,
            mc_extra_cmds::mc_skin_wear,
            mc_extra_cmds::mc_cape_set,
            minecraft_cmds::mc_running,
            minecraft_cmds::mc_kill,
            minecraft_cmds::mc_incidents,
            minecraft_cmds::mc_security_scan,
            minecraft_cmds::mc_verify,
            minecraft_cmds::open_minecraft_launcher,
            minecraft_cmds::create_minecraft_shortcut,
            minecraft_cmds::minecraft_pop_out,
            minecraft_cmds::minecraft_dock,
            minecraft_cmds::open_url,
            inspector_snapshot,
            inspector_clear,
            addon_cmds::addon_get,
            addon_cmds::addon_set,
            addon_cmds::addon_keys,
            addon_cmds::addon_clear,
            addon_cmds::addon_secret_set,
            addon_cmds::addon_secrets,
            addon_cmds::discord_lookup,
            addon_cmds::discord_image,
            addon_cmds::window_effects_status,
            addon_cmds::window_effect_set,
            addon_cmds::app_restart,
            music_cmds::music_library,
            music_cmds::music_folders,
            music_cmds::music_set_folders,
            music_cmds::music_scan,
            music_cmds::music_favourite,
            music_cmds::music_status,
            music_cmds::music_play,
            music_cmds::music_pause,
            music_cmds::music_next,
            music_cmds::music_prev,
            music_cmds::music_seek,
            music_cmds::music_volume,
            music_cmds::music_shuffle,
            music_cmds::music_repeat,
            music_cmds::music_devices,
            music_cmds::music_set_device,
            music_cmds::music_queue,
            music_cmds::music_queue_add,
            music_cmds::music_queue_remove,
            music_cmds::music_queue_move,
            music_cmds::music_queue_jump,
            music_cmds::music_queue_clear,
            music_cmds::music_playlists,
            music_cmds::music_playlist_items,
            music_cmds::music_playlist_create,
            music_cmds::music_playlist_rename,
            music_cmds::music_playlist_delete,
            music_cmds::music_playlist_set,
            music_cmds::music_playlist_add,
            music_cmds::music_reload_rules,
            music_cmds::music_shuffle_mode,
            music_cmds::music_loop,
            music_cmds::music_spectrum,
            music_cmds::music_spectrum_stop,
            music_cmds::music_queue_tool,
            music_cmds::music_stats,
            music_cmds::music_duplicates,
            music_cmds::music_recycle,
            lyrics_cmds::lyrics_get,
            lyrics_cmds::lyrics_save,
            lyrics_cmds::lyrics_forget,
            lyrics_cmds::lyrics_translate,
            lyrics_cmds::lyrics_window,
            streaming_cmds::streaming_status,
            streaming_cmds::streaming_install,
            streaming_cmds::streaming_update,
            streaming_cmds::streaming_set_config,
            streaming_cmds::streaming_clear_cache,
            streaming_cmds::streaming_search,
            streaming_cmds::streaming_resolve,
            streaming_cmds::streaming_tracks,
            streaming_cmds::streaming_jobs,
            streaming_cmds::streaming_download,
            streaming_cmds::streaming_cancel,
            streaming_cmds::streaming_retry,
            streaming_cmds::streaming_clear_finished,
            streaming_cmds::streaming_sources,
            streaming_cmds::streaming_import,
            streaming_cmds::streaming_sync,
            streaming_cmds::streaming_unlink,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Arcadia");
}
