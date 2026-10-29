//! Lyrics Plus: finding lyrics, saving fixed timings, and the pop-up
//! lyrics window and desktop lyrics bar.

use crate::addon_cmds::installed;
use crate::music_cmds::Music;
use crate::{err, Res};
use arcadia_core::addons;
use arcadia_core::music::lyrics::{self, Lyrics};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tauri::window::{Effect, EffectsBuilder};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const ID: &str = "lyrics-plus";
/// After LRCLIB had nothing, ask again after this long.
const RETRY_AFTER: Duration = Duration::from_secs(3 * 24 * 3600);

fn cache_dir() -> PathBuf {
    addons::root().join(ID).join("cache")
}

fn music(app: &AppHandle) -> Res<Arc<Music>> {
    app.try_state::<Arc<Music>>().map(|s| s.inner().clone()).ok_or_else(|| "The music player didn't start.".to_string())
}

fn remember(id: i64, l: &Lyrics) {
    let dir = cache_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(b) = serde_json::to_vec(l) {
        let _ = std::fs::write(dir.join(format!("{id}.json")), b);
    }
    let _ = std::fs::remove_file(dir.join(format!("{id}.none")));
}

/// Lyrics for a song. Looks on this PC first; with `online`, asks LRCLIB.
/// With `sidecar`, lyrics found online are also saved as a `.lrc` beside
/// the song (local files only, never over an existing one).
#[tauri::command]
pub async fn lyrics_get(app: AppHandle, id: i64, online: bool, sidecar: bool) -> Res<Option<Lyrics>> {
    installed(&app, ID)?;
    let m = music(&app)?;
    let m2 = m.clone();
    let local = tauri::async_runtime::spawn_blocking(move || m2.lib.local_lyrics(id, &cache_dir())).await.map_err(err)?;
    if local.is_some() || !online {
        return Ok(local);
    }
    let none = cache_dir().join(format!("{id}.none"));
    let recent_miss = std::fs::metadata(&none).and_then(|m| m.modified()).ok().and_then(|t| SystemTime::now().duration_since(t).ok()).is_some_and(|age| age < RETRY_AFTER);
    if recent_miss {
        return Ok(None);
    }
    let Some(track) = m.lib.track(id).map_err(err)? else { return Ok(None) };
    match lyrics::lrclib(&track).await {
        Ok(Some(l)) => {
            remember(id, &l);
            if sidecar && l.synced && track.origin == "local" {
                if let Some(p) = lyrics::sidecar(&track).filter(|p| !p.exists()) {
                    let _ = std::fs::write(p, lyrics::to_lrc(&l));
                }
            }
            Ok(Some(l))
        }
        Ok(None) => {
            let _ = std::fs::create_dir_all(cache_dir());
            let _ = std::fs::write(none, b"");
            Ok(None)
        }
        Err(e) => {
            log::warn!("lrclib: {e}");
            Err("Couldn't reach LRCLIB for lyrics. Check your connection.".into())
        }
    }
}

/// Save lyrics from the timing editor (LRC text). They win over anything
/// found online from now on. With `sidecar`, also written beside the song.
#[tauri::command]
pub async fn lyrics_save(app: AppHandle, id: i64, lrc: String, sidecar: bool) -> Res<Lyrics> {
    installed(&app, ID)?;
    let m = music(&app)?;
    let l = lyrics::parse(&lrc, "yours");
    remember(id, &l);
    if sidecar {
        if let Some(t) = m.lib.track(id).map_err(err)? {
            if t.origin == "local" {
                if let Some(p) = lyrics::sidecar(&t) {
                    std::fs::write(&p, lyrics::to_lrc(&l)).map_err(|e| format!("Couldn't save {}: {e}", p.display()))?;
                }
            }
        }
    }
    Ok(l)
}

/// Forget cached lyrics for a song, so they're looked up again.
#[tauri::command]
pub async fn lyrics_forget(app: AppHandle, id: i64) -> Res<()> {
    installed(&app, ID)?;
    let _ = std::fs::remove_file(cache_dir().join(format!("{id}.json")));
    let _ = std::fs::remove_file(cache_dir().join(format!("{id}.none")));
    Ok(())
}

/// Open or close the pop-up lyrics window ("popup") or the click-through
/// desktop lyrics bar ("bar"; `top` puts it at the top of the screen).
#[tauri::command]
pub async fn lyrics_window(app: AppHandle, kind: String, open: bool, top: Option<bool>) -> Res<()> {
    installed(&app, ID)?;
    let label = match kind.as_str() {
        "popup" => "lyrics-popup",
        "bar" => "lyrics-bar",
        other => return Err(format!("Unknown lyrics window “{other}”")),
    };
    if let Some(w) = app.get_webview_window(label) {
        if open {
            let _ = w.show();
            if label == "lyrics-popup" {
                let _ = w.set_focus();
            } else {
                // The bar may have moved between top and bottom.
                let _ = w.close();
                return Box::pin(lyrics_window(app.clone(), kind, true, top)).await;
            }
        } else {
            w.close().map_err(err)?;
        }
        return Ok(());
    }
    if !open {
        return Ok(());
    }
    let (sw, sh, scale) = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| (m.size().width as f64, m.size().height as f64, m.scale_factor()))
        .unwrap_or((1920.0, 1080.0, 1.0));
    let (sw, sh) = (sw / scale, sh / scale);
    let popup = kind == "popup";
    let (w, h) = if popup { (440.0, 240.0) } else { (sw.min(1100.0), 96.0) };
    let (x, y) = if popup {
        (sw - w - 32.0, 72.0)
    } else if top.unwrap_or(false) {
        ((sw - w) / 2.0, 24.0)
    } else {
        ((sw - w) / 2.0, sh - h - 72.0)
    };
    let mode = label;
    let win = WebviewWindowBuilder::new(&app, label, WebviewUrl::App(format!("index.html?mode={mode}").into()))
        .title(if popup { "Lyrics" } else { "Desktop lyrics" })
        .transparent(true)
        .decorations(false)
        .shadow(popup)
        .always_on_top(true)
        .skip_taskbar(!popup)
        .resizable(popup)
        .focused(popup)
        .inner_size(w, h)
        .min_inner_size(300.0, if popup { 140.0 } else { 60.0 })
        .position(x, y)
        .initialization_script(format!("window.__ARCADIA_MODE__ = '{mode}';"))
        .build()
        .map_err(|e| format!("Couldn't open the lyrics window: {e}"))?;
    if popup {
        // Frosted glass behind the pop-up; the page draws its own tint.
        let _ = win.set_effects(EffectsBuilder::new().effect(Effect::Acrylic).build());
    } else {
        // Clicks go straight through to whatever is underneath.
        let _ = win.set_ignore_cursor_events(true);
    }
    Ok(())
}
