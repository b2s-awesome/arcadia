//! Tauri commands for marketplace addons.
//!
//! Every command names the addon it acts for, and is refused unless that
//! addon is installed. Addons only ever reach their own storage.

use crate::{err, AppState, Res};
use arcadia_core::addons::{self, discord, secrets};
use serde_json::Value;
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
