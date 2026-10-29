//! Media keys and the Windows media overlay (SMTC): the keyboard's
//! play/pause/next/previous keys work anywhere, and the volume flyout and
//! lock screen show what's playing with its cover.
//!
//! The controls live on their own thread, which owns them; the player sends
//! it updates over a channel.

use std::sync::mpsc::Sender;
use std::sync::OnceLock;
use tauri::AppHandle;

// Only read by the Windows media controls.
#[cfg_attr(not(windows), allow(dead_code))]
pub enum Update {
    Track { title: String, artist: String, album: String, art: Option<String>, duration: f64 },
    Playing { playing: bool, pos: f64 },
    Stopped,
}

static TX: OnceLock<std::sync::Mutex<Sender<Update>>> = OnceLock::new();

/// Tell the overlay something changed. Does nothing before `start`.
pub fn send(u: Update) {
    if let Some(tx) = TX.get() {
        if let Ok(tx) = tx.lock() {
            let _ = tx.send(u);
        }
    }
}

#[cfg(windows)]
pub fn start(app: &AppHandle) {
    use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig, SeekDirection};
    use std::time::Duration;
    use tauri::Manager;

    let Some(win) = app.get_webview_window("main") else { return };
    let Ok(hwnd) = win.hwnd() else { return };
    let hwnd = hwnd.0 as usize;
    let (tx, rx) = std::sync::mpsc::channel::<Update>();
    if TX.set(std::sync::Mutex::new(tx)).is_err() {
        return;
    }
    let app = app.clone();
    std::thread::Builder::new()
        .name("arcadia-media-keys".into())
        .spawn(move || {
            let config = PlatformConfig { display_name: "Arcadia", dbus_name: "arcadia", hwnd: Some(hwnd as *mut std::ffi::c_void) };
            let mut controls = match MediaControls::new(config) {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("media keys unavailable: {e:?}");
                    return;
                }
            };
            let h = app.clone();
            let attached = controls.attach(move |e: MediaControlEvent| {
                let h = h.clone();
                tauri::async_runtime::spawn(async move {
                    use crate::music_cmds as m;
                    let r = match e {
                        MediaControlEvent::Play => m::music_pause(h, Some(false)).await,
                        MediaControlEvent::Pause | MediaControlEvent::Stop => m::music_pause(h, Some(true)).await,
                        MediaControlEvent::Toggle => m::music_pause(h, None).await,
                        MediaControlEvent::Next => m::music_next(h).await,
                        MediaControlEvent::Previous => m::music_prev(h).await,
                        MediaControlEvent::SetPosition(MediaPosition(p)) => m::music_seek(h, p.as_secs_f64()).await,
                        MediaControlEvent::Seek(dir) => step(h, matches!(dir, SeekDirection::Forward), 10.0).await,
                        MediaControlEvent::SeekBy(dir, by) => step(h, matches!(dir, SeekDirection::Forward), by.as_secs_f64()).await,
                        _ => Ok(()),
                    };
                    if let Err(e) = r {
                        log::warn!("media key: {e}");
                    }
                });
            });
            if let Err(e) = attached {
                log::warn!("media keys unavailable: {e:?}");
                return;
            }
            while let Ok(u) = rx.recv() {
                let r = match u {
                    Update::Track { title, artist, album, art, duration } => {
                        let cover = art.map(|a| format!("file://{}", a.replace('\\', "/")));
                        controls.set_metadata(MediaMetadata {
                            title: Some(&title),
                            artist: Some(&artist),
                            album: Some(&album),
                            cover_url: cover.as_deref(),
                            duration: (duration > 0.0).then(|| Duration::from_secs_f64(duration)),
                        })
                    }
                    Update::Playing { playing, pos } => {
                        let progress = Some(MediaPosition(Duration::from_secs_f64(pos.max(0.0))));
                        controls.set_playback(if playing { MediaPlayback::Playing { progress } } else { MediaPlayback::Paused { progress } })
                    }
                    Update::Stopped => controls.set_playback(MediaPlayback::Stopped),
                };
                if let Err(e) = r {
                    log::debug!("media overlay update: {e:?}");
                }
            }
        })
        .ok();
}

/// Seek forward or back from where the song is now.
#[cfg(windows)]
async fn step(h: AppHandle, forward: bool, by: f64) -> crate::Res<()> {
    let now = crate::music_cmds::music_status(h.clone()).map(|s| s.pos).unwrap_or(0.0);
    crate::music_cmds::music_seek(h, if forward { now + by } else { (now - by).max(0.0) }).await
}

#[cfg(not(windows))]
pub fn start(_app: &AppHandle) {}
