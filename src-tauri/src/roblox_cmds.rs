//! Roblox+: accounts, server regions, play history, auto-rejoin and the mod
//! studio. Login cookies only ever go to roblox.com and Windows Credential
//! Manager; they're never written to a file or a log.

use crate::{err, AppState, Res};
use arcadia_core::roblox::{accounts, activity::Activity, history, mods, web};
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashMap;
use std::process::Child;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

/// What a launch was for, so history and rejoin know the account and game.
#[derive(Debug, Clone, Default)]
pub(crate) struct Ctx {
    pub account: Option<u64>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
fn now_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

fn tracker() -> u64 {
    let b = uuid::Uuid::new_v4().as_u128();
    100_000_000_000 + (b % 900_000_000_000) as u64
}

fn next_session() -> u64 {
    static N: OnceLock<AtomicU64> = OnceLock::new();
    N.get_or_init(|| AtomicU64::new(now_ms() as u64)).fetch_add(1, Ordering::Relaxed)
}

// ───────────────────────── launching ─────────────────────────

/// Start Roblox signed in as `account` (None = the default account, 0 = the
/// Roblox app's own sign-in), at a game and server if given.
#[tauri::command]
pub async fn roblox_play(app: AppHandle, account: Option<u64>, place: Option<u64>, job: Option<String>) -> Res<String> {
    let acct = match account {
        Some(0) => None,
        Some(id) => Some(id),
        None => accounts::load().default,
    };
    let uri = uri_for(acct, place, job.as_deref()).await?;
    crate::launch_roblox_as(app, uri, Ctx { account: acct }).await
}

async fn uri_for(acct: Option<u64>, place: Option<u64>, job: Option<&str>) -> Res<Option<String>> {
    let target = match place {
        Some(p) => web::Target::Place { place: p, job: job.map(str::to_string) },
        None => web::Target::Home,
    };
    Ok(match acct {
        Some(id) => {
            let t = accounts::ticket(id).await.map_err(err)?;
            accounts::touch(id);
            Some(web::player_uri(&t, &target, now_ms(), tracker()))
        }
        None => place.map(|p| web::deeplink(p, job)),
    })
}

/// A link from the browser or a shortcut. Game links that don't carry their
/// own sign-in use the default account, if one is set.
pub(crate) async fn resolve_link(uri: String) -> (Option<String>, Ctx) {
    let lower = uri.to_ascii_lowercase();
    if lower.starts_with("roblox://") {
        if let (Some(place), Some(id)) = (web::place_from(&uri), accounts::load().default) {
            let job = query_value(&uri, "gameinstanceid");
            if let Ok(Some(u)) = uri_for(Some(id), Some(place), job.as_deref()).await {
                return (Some(u), Ctx { account: Some(id) });
            }
        }
    }
    (Some(uri), Ctx::default())
}

fn query_value(uri: &str, key: &str) -> Option<String> {
    let q = uri.split_once('?')?.1;
    q.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        k.eq_ignore_ascii_case(key).then(|| v.to_string())
    })
}

/// `--roblox-play <place> [--account <id>]` from a desktop shortcut.
pub(crate) fn shortcut_args(args: &[String]) -> Option<(u64, Option<u64>)> {
    let i = args.iter().position(|a| a == "--roblox-play")?;
    let place = args.get(i + 1)?.parse().ok()?;
    let acct = args.iter().position(|a| a == "--account").and_then(|j| args.get(j + 1)).and_then(|a| a.parse().ok());
    Some((place, acct))
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RejoinEvent {
    state: &'static str,
    place_id: u64,
    seconds: u32,
    message: Option<String>,
}

#[derive(Default)]
struct Current {
    place: u64,
    job: String,
}

/// Follow a running client: history, server region, auto-rejoin, and the
/// end of the session.
pub(crate) fn follow(app: &AppHandle, started: SystemTime, mut child: Child, ctx: Ctx, clear_on_exit: bool) {
    let session = next_session();
    let st = app.state::<AppState>();
    let sessions = st.sessions.clone();
    sessions.fetch_add(1, Ordering::Relaxed);
    let stop = Arc::new(AtomicBool::new(false));
    let kill = Arc::new(AtomicBool::new(false));
    let current = Arc::new(Mutex::new(None::<Current>));

    let h = app.clone();
    let (k, cur, acct) = (kill.clone(), current.clone(), ctx.account);
    arcadia_core::roblox::activity::watch(started, stop.clone(), move |a| {
        let _ = h.emit("roblox-activity", &a);
        match a {
            Activity::Joining { place_id, job_id } => {
                let _ = history::joined(session, place_id, &job_id, acct, now());
                *cur.lock() = Some(Current { place: place_id, job: job_id });
                let h = h.clone();
                tauri::async_runtime::spawn(async move {
                    if let Ok(g) = web::games(&[place_id]).await {
                        if let Some(g) = g.first() {
                            let _ = history::set_info(place_id, &g.name, &g.creator, g.icon.as_deref());
                        }
                    }
                    let _ = h.emit("roblox-history", ());
                });
            }
            Activity::ServerIp { ip } => {
                let job = cur.lock().as_ref().map(|c| c.job.clone());
                let h = h.clone();
                tauri::async_runtime::spawn(async move {
                    if let Ok(p) = web::locate(&ip).await {
                        let text = region_text(&p);
                        if let Some(j) = job {
                            let _ = history::set_region(&j, &text);
                        }
                        let _ = h.emit("roblox-region", text);
                    }
                });
            }
            Activity::Disconnected { .. } => {
                let settings = h.state::<AppState>().settings.lock().clone();
                if let Some(c) = cur.lock().as_ref().filter(|_| settings.roblox_auto_rejoin) {
                    let job = settings.roblox_rejoin_same_server.then(|| c.job.clone());
                    rejoin(&h, acct, c.place, job, settings.roblox_rejoin_delay, k.clone());
                }
            }
            Activity::Left => {
                let _ = history::left(session, now());
            }
        }
    });

    let h = app.clone();
    std::thread::spawn(move || {
        loop {
            if kill.swap(false, Ordering::Relaxed) {
                let _ = child.kill();
            }
            match child.try_wait() {
                Ok(None) => std::thread::sleep(Duration::from_millis(300)),
                _ => break,
            }
        }
        stop.store(true, Ordering::Relaxed);
        let _ = history::left(session, now());
        let left = sessions.fetch_sub(1, Ordering::Relaxed) - 1;
        if left == 0 && clear_on_exit {
            std::thread::sleep(Duration::from_millis(800));
            arcadia_core::roblox::cache::clear();
        }
        let _ = h.emit("roblox-exited", left);
        let _ = h.emit("roblox-history", ());
    });
}

fn region_text(p: &web::Place) -> String {
    [p.city.as_str(), p.region.as_str(), p.country.as_str()]
        .iter()
        .filter(|s| !s.is_empty())
        .take(2)
        .copied()
        .collect::<Vec<_>>()
        .join(", ")
}

fn rejoin_cancel() -> &'static Mutex<Option<Arc<AtomicBool>>> {
    static C: OnceLock<Mutex<Option<Arc<AtomicBool>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

/// Close the kicked client and join again after `delay` seconds, unless
/// cancelled. At most five times in half an hour, so a game that keeps
/// kicking you doesn't loop forever.
fn rejoin(app: &AppHandle, acct: Option<u64>, place: u64, job: Option<String>, delay: u32, kill: Arc<AtomicBool>) {
    static RECENT: Mutex<Vec<Instant>> = Mutex::new(Vec::new());
    {
        let mut r = RECENT.lock();
        r.retain(|t| t.elapsed() < Duration::from_secs(30 * 60));
        if r.len() >= 5 {
            let _ = app.emit("roblox-rejoin", RejoinEvent { state: "limit", place_id: place, seconds: 0, message: None });
            return;
        }
        r.push(Instant::now());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    if let Some(old) = rejoin_cancel().lock().replace(cancel.clone()) {
        old.store(true, Ordering::Relaxed);
    }
    let h = app.clone();
    let delay = delay.clamp(3, 120);
    tauri::async_runtime::spawn(async move {
        let _ = h.emit("roblox-rejoin", RejoinEvent { state: "waiting", place_id: place, seconds: delay, message: None });
        for _ in 0..delay {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if cancel.load(Ordering::Relaxed) {
                let _ = h.emit("roblox-rejoin", RejoinEvent { state: "cancelled", place_id: place, seconds: 0, message: None });
                return;
            }
        }
        kill.store(true, Ordering::Relaxed);
        tokio::time::sleep(Duration::from_millis(2500)).await;
        let _ = h.emit("roblox-rejoin", RejoinEvent { state: "joining", place_id: place, seconds: 0, message: None });
        let res = match uri_for(acct, Some(place), job.as_deref()).await {
            Ok(uri) => crate::launch_roblox_as(h.clone(), uri, Ctx { account: acct }).await,
            Err(e) => Err(e),
        };
        let (state, message) = match res {
            Ok(_) => ("done", None),
            Err(e) => ("failed", Some(e)),
        };
        let _ = h.emit("roblox-rejoin", RejoinEvent { state, place_id: place, seconds: 0, message });
    });
}

#[tauri::command]
pub fn roblox_rejoin_cancel() {
    if let Some(c) = rejoin_cancel().lock().take() {
        c.store(true, Ordering::Relaxed);
    }
}

// ───────────────────────── accounts ─────────────────────────

#[tauri::command]
pub fn roblox_accounts() -> accounts::Accounts {
    accounts::load()
}

const LOGIN: &str = "roblox-login";

/// Open Roblox's own sign-in page in a private window and wait for the
/// person to sign in. The login cookie goes straight to Credential Manager.
#[tauri::command]
pub async fn roblox_account_login(app: AppHandle) -> Res<accounts::Account> {
    if let Some(w) = app.get_webview_window(LOGIN) {
        let _ = w.close();
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    let url: tauri::Url = "https://www.roblox.com/login".parse().map_err(err)?;
    tauri::WebviewWindowBuilder::new(&app, LOGIN, tauri::WebviewUrl::External(url))
        .title("Sign in to Roblox")
        .inner_size(500.0, 760.0)
        .center()
        .incognito(true)
        .build()
        .map_err(err)?;
    let site: tauri::Url = "https://www.roblox.com/".parse().map_err(err)?;
    // Ten minutes is plenty for a captcha or two-step code.
    for _ in 0..1200 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let Some(w) = app.get_webview_window(LOGIN) else {
            return Err("Sign-in was closed before it finished".into());
        };
        let site = site.clone();
        // WebView2 deadlocks if cookies are read on the event loop thread.
        let found = tauri::async_runtime::spawn_blocking(move || {
            w.cookies_for_url(site).ok().and_then(|cs| cs.into_iter().find(|c| c.name() == ".ROBLOSECURITY" && !c.value().is_empty()).map(|c| c.value().to_string()))
        })
        .await
        .ok()
        .flatten();
        if let Some(cookie) = found {
            if let Some(w) = app.get_webview_window(LOGIN) {
                let _ = w.close();
            }
            return accounts::add(&cookie).await.map_err(err);
        }
    }
    if let Some(w) = app.get_webview_window(LOGIN) {
        let _ = w.close();
    }
    Err("Sign-in timed out".into())
}

#[tauri::command]
pub fn roblox_account_cancel_login(app: AppHandle) {
    if let Some(w) = app.get_webview_window(LOGIN) {
        let _ = w.close();
    }
}

#[tauri::command]
pub fn roblox_account_remove(id: u64) -> Res<()> {
    accounts::remove(id).map_err(err)
}

#[tauri::command]
pub fn roblox_account_default(id: Option<u64>) -> Res<()> {
    accounts::set_default(id).map_err(err)
}

#[tauri::command]
pub fn roblox_account_note(id: u64, note: String) -> Res<()> {
    accounts::set_note(id, &note).map_err(err)
}

#[tauri::command]
pub fn roblox_account_reorder(ids: Vec<u64>) -> Res<()> {
    accounts::reorder(&ids).map_err(err)
}

/// Is this account still signed in? Refreshes its name and picture.
#[tauri::command]
pub async fn roblox_account_check(id: u64) -> Res<bool> {
    let Some(c) = accounts::cookie(id) else { return Ok(false) };
    match web::whoami(&c).await {
        Ok(_) => {
            let _ = accounts::add(&c).await;
            Ok(true)
        }
        Err(_) => Ok(false),
    }
}

// ───────────────────────── servers ─────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerPage {
    servers: Vec<web::Server>,
    next: Option<String>,
}

#[tauri::command]
pub async fn roblox_servers(place: u64, cursor: Option<String>) -> Res<ServerPage> {
    let (servers, next) = web::servers(place, cursor.as_deref()).await.map_err(err)?;
    Ok(ServerPage { servers, next })
}

/// Where a server is. Needs a signed-in account: Roblox only tells the
/// address to someone who could join.
#[tauri::command]
pub async fn roblox_server_region(place: u64, job: String) -> Res<Option<String>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(r) = cache.lock().get(&job) {
        return Ok(r.clone());
    }
    let all = accounts::load();
    let id = all.default.or_else(|| all.list.first().map(|a| a.id)).ok_or("Add a Roblox account to see where servers are")?;
    let cookie = accounts::cookie(id).ok_or("Sign in to your default account again to see where servers are")?;
    let region = match web::server_address(&cookie, place, &job).await.map_err(err)? {
        Some(ip) => web::locate(&ip).await.ok().map(|p| region_text(&p)),
        None => None,
    };
    cache.lock().insert(job, region.clone());
    Ok(region)
}

#[tauri::command]
pub async fn roblox_games(places: Vec<u64>) -> Res<Vec<web::Game>> {
    web::games(&places).await.map_err(err)
}

/// A place id from a link, a share link or a number.
#[tauri::command]
pub fn roblox_place_from(text: String) -> Option<u64> {
    web::place_from(&text)
}

// ───────────────────────── history ─────────────────────────

#[tauri::command]
pub fn roblox_history() -> history::History {
    history::load()
}

#[tauri::command]
pub async fn roblox_history_add(app: AppHandle, place: u64) -> Res<()> {
    history::add(place).map_err(err)?;
    if let Ok(g) = web::games(&[place]).await {
        if let Some(g) = g.first() {
            let _ = history::set_info(place, &g.name, &g.creator, g.icon.as_deref());
        }
    }
    let _ = app.emit("roblox-history", ());
    Ok(())
}

#[tauri::command]
pub fn roblox_history_favourite(place: u64, on: bool) -> Res<()> {
    history::set_favourite(place, on).map_err(err)
}

#[tauri::command]
pub fn roblox_history_forget(place: u64) -> Res<()> {
    history::forget(place).map_err(err)
}

#[tauri::command]
pub fn roblox_history_clear() -> Res<()> {
    history::clear_visits().map_err(err)
}

/// A desktop shortcut that starts a game through Arcadia.
#[tauri::command]
pub fn roblox_shortcut(app: AppHandle, place: u64, name: String, account: Option<u64>) -> Res<String> {
    let exe = app.state::<AppState>().exe.clone();
    let clean: String = name.chars().filter(|c| c.is_alphanumeric() || " -_()'".contains(*c)).collect();
    let label = if clean.trim().is_empty() { format!("Roblox {place}") } else { clean.trim().to_string() };
    let mut args = format!("--roblox-play {place}");
    if let Some(a) = account {
        args.push_str(&format!(" --account {a}"));
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let desk = std::env::var("USERPROFILE").ok().map(|p| std::path::PathBuf::from(p).join("Desktop")).filter(|p| p.is_dir()).ok_or("Couldn't find your desktop")?;
        let target = desk.join(format!("{label}.lnk"));
        let q = |s: &str| s.replace('\'', "''");
        let ps = format!(
            "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$s.TargetPath='{}';$s.Arguments='{}';$s.IconLocation='{},0';$s.Save()",
            q(&target.display().to_string()),
            q(&exe),
            q(&args),
            q(&exe)
        );
        let ok = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &ps])
            .creation_flags(0x0800_0000)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            return Err("Windows didn't let Arcadia create the shortcut".into());
        }
        Ok(target.display().to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = (exe, label);
        Err(format!("Shortcuts can only be made on Windows; run Arcadia with {args} instead"))
    }
}

// ───────────────────────── mods and flags ─────────────────────────

/// A picture of one of Arcadia's cursor styles, for the mod studio.
#[tauri::command]
pub fn roblox_cursor_preview(style: String, colour: String, scale: u32) -> String {
    use base64::Engine;
    let c = u32::from_str_radix(colour.trim_start_matches('#'), 16).unwrap_or(0xffffff);
    let png = mods::cursor_png(&style, [(c >> 16) as u8, (c >> 8) as u8, c as u8], scale.clamp(50, 200) as f32 / 100.0, 1.0);
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))
}

#[tauri::command]
pub fn roblox_flag_allowlist() -> Vec<&'static str> {
    arcadia_core::roblox::fflags::ALLOWLIST.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_shortcut_and_link_args() {
        let a: Vec<String> = ["arcadia.exe", "--roblox-play", "920587237", "--account", "7"].iter().map(|s| s.to_string()).collect();
        assert_eq!(shortcut_args(&a), Some((920587237, Some(7))));
        assert_eq!(shortcut_args(&a[..3]), Some((920587237, None)));
        assert_eq!(query_value("roblox://experiences/start?placeId=1&gameInstanceId=abc", "gameinstanceid").as_deref(), Some("abc"));
    }
}
