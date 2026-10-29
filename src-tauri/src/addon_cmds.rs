//! Tauri commands for marketplace addons.
//!
//! Every command names the addon it acts for, and is refused unless that
//! addon is installed. Addons only ever reach their own storage.

use crate::{err, AppState, Res};
use arcadia_core::addons::{self, discord, secrets};
use serde_json::Value;
use serde::Serialize;
use tauri::window::{Color, Effect, EffectsBuilder};
use tauri::{AppHandle, Manager};

const DISCORD: &str = "discord-profile";

/// The UI owns the installed list (in settings.ui.addons.installed).
fn installed(app: &AppHandle, addon: &str) -> Res<()> {
    let st = app.state::<AppState>();
    let s = st.settings.lock();
    let ok = s.ui["addons"]["installed"].as_array().is_some_and(|a| a.iter().any(|v| v.as_str() == Some(addon)));
    if ok {
        Ok(())
    } else {
        Err(format!("The “{addon}” addon isn't installed. Add it from the Marketplace."))
    }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(err)?.map_err(err)
}

#[tauri::command]
pub async fn addon_get(app: AppHandle, addon: String, key: String) -> Res<Option<Value>> {
    installed(&app, &addon)?;
    blocking(move || addons::get(&addon, &key)).await
}

#[tauri::command]
pub async fn addon_set(app: AppHandle, addon: String, key: String, value: Option<Value>) -> Res<()> {
    installed(&app, &addon)?;
    if key.starts_with('_') {
        return Err("Keys starting with _ are reserved".into());
    }
    blocking(move || addons::set(&addon, &key, value.as_ref())).await
}

#[tauri::command]
pub async fn addon_keys(app: AppHandle, addon: String) -> Res<Vec<String>> {
    installed(&app, &addon)?;
    blocking(move || Ok(addons::keys(&addon)?.into_iter().filter(|k| !k.starts_with('_')).collect())).await
}

/// Wipe an addon's data and secrets. Allowed after uninstalling, too.
#[tauri::command]
pub async fn addon_clear(addon: String) -> Res<()> {
    blocking(move || addons::clear(&addon)).await
}

/// Store (or with `None`, forget) a secret. It can't be read back.
#[tauri::command]
pub async fn addon_secret_set(app: AppHandle, addon: String, name: String, value: Option<String>) -> Res<()> {
    installed(&app, &addon)?;
    let value = value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    blocking(move || secrets::set(&addon, &name, value.as_deref())).await
}

/// Which secrets are saved, by name.
#[tauri::command]
pub async fn addon_secrets(app: AppHandle, addon: String) -> Res<Vec<String>> {
    installed(&app, &addon)?;
    blocking(move || Ok(secrets::present(&addon))).await
}

// ───────────────────────── Discord Profile Studio ─────────────────────────

#[tauri::command]
pub async fn discord_lookup(app: AppHandle, query: String) -> Res<discord::Found> {
    installed(&app, DISCORD)?;
    let token = blocking(|| Ok(secrets::get(DISCORD, "bot_token"))).await?;
    discord::lookup(&query, token.as_deref()).await.map_err(err)
}

#[tauri::command]
pub async fn discord_image(app: AppHandle, url: String) -> Res<String> {
    installed(&app, DISCORD)?;
    discord::fetch_image(&url).await.map_err(err)
}

// ───────────────────────── Translucent window ─────────────────────────

const SEE_THROUGH: &str = "see-through";

/// Whether windows should be created see-through: the addon is installed and
/// its switch is on. Read once at startup.
pub fn see_through_wanted(s: &arcadia_core::Settings) -> bool {
    let installed = s.ui["addons"]["installed"].as_array().is_some_and(|a| a.iter().any(|v| v.as_str() == Some(SEE_THROUGH)));
    installed && s.ui["seeThrough"]["enabled"].as_bool().unwrap_or(true)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectsStatus {
    /// This run's windows were created see-through.
    pub active: bool,
    /// What the saved settings ask for (differs from `active` until a restart).
    pub wanted: bool,
    /// Windows build number, when known.
    pub build: Option<u32>,
    pub os: &'static str,
}

#[tauri::command]
pub fn window_effects_status(app: AppHandle) -> EffectsStatus {
    let st = app.state::<AppState>();
    let wanted = see_through_wanted(&st.settings.lock());
    EffectsStatus { active: st.see_through, wanted, build: arcadia_core::registry::windows_build(), os: std::env::consts::OS }
}

/// Apply (or with `None`, clear) the backdrop on the calling window.
/// `color` is the Acrylic/Blur tint, RGBA.
#[tauri::command]
pub async fn window_effect_set(app: AppHandle, window: tauri::Window, effect: Option<String>, color: Option<[u8; 4]>) -> Res<()> {
    if effect.is_some() {
        installed(&app, SEE_THROUGH)?;
    }
    let Some(name) = effect else {
        return window.set_effects(None).map_err(err);
    };
    let e = match name.as_str() {
        "acrylic" => Effect::Acrylic,
        "blur" => Effect::Blur,
        "mica" => Effect::Mica,
        "mica-dark" => Effect::MicaDark,
        "tabbed" => Effect::Tabbed,
        other => return Err(format!("Unknown window effect “{other}”")),
    };
    let mut b = EffectsBuilder::new().effect(e);
    if let Some([r, g, bl, a]) = color {
        b = b.color(Color(r, g, bl, a));
    }
    window.set_effects(b.build()).map_err(err)
}

#[tauri::command]
pub fn app_restart(app: AppHandle) {
    app.restart();
}
