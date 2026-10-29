//! Persistent app settings (one JSON file).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    // ── General ──────────────────────────────────────────────
    pub accent: String,
    pub run_on_boot: bool,
    pub minimize_to_tray: bool,

    // ── Roblox bootstrapper ──────────────────────────────────
    /// Deployment channel; "production" = live.
    pub roblox_channel: String,
    /// Keep Roblox on a specific version-hash instead of auto-updating.
    pub roblox_pinned_version: Option<String>,
    pub roblox_auto_update: bool,
    pub roblox_multi_instance: bool,
    /// FPS cap written as DFIntTaskSchedulerTargetFps; 0 = leave Roblox default.
    pub roblox_fps_cap: u32,
    /// "Normal" | "AboveNormal" | "High" | "BelowNormal"
    pub roblox_priority: String,
    pub roblox_custom_font: Option<String>,
    pub roblox_apply_mods: bool,
    /// FastFlags written to ClientAppSettings.json (local file route).
    pub fast_flags: BTreeMap<String, serde_json::Value>,
    /// Saved FastFlag sets. Picking one copies it into `fast_flags`.
    pub roblox_flag_profiles: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
    /// The profile `fast_flags` came from ("" = none).
    pub roblox_flag_profile: String,
    /// Join again after being kicked for idling or losing connection.
    pub roblox_auto_rejoin: bool,
    /// Seconds to wait (with a chance to cancel) before rejoining.
    pub roblox_rejoin_delay: u32,
    /// Rejoin the same server instead of any server of the game.
    pub roblox_rejoin_same_server: bool,
    pub roblox_mod_studio: crate::roblox::mods::ModStudio,
    pub roblox_mod_presets: BTreeMap<String, crate::roblox::mods::ModStudio>,

    // ── Asset proxy (Fleasion-style) ─────────────────────────
    pub proxy_enabled: bool,
    pub proxy_port: u16,
    pub proxy_clear_cache_on_launch: bool,
    pub proxy_clear_cache_on_exit: bool,
    /// Live FastFlag overrides delivered through the proxy. These bypass
    /// Roblox's local allowlist and can get an account banned.
    pub proxy_fflag_overrides: BTreeMap<String, serde_json::Value>,
    pub proxy_fflags_risk_accepted: bool,
    pub proxy_inspector_enabled: bool,

    // ── Appearance ───────────────────────────────────────────
    /// Look, wallpaper, top bar and saved looks. Owned by the UI; the
    /// backend only stores it.
    pub ui: serde_json::Value,

    // ── Behaviour ────────────────────────────────────────────
    /// Hide Arcadia to the tray when a game starts.
    pub hide_on_game_launch: bool,

    // ── Minecraft ────────────────────────────────────────────
    /// "page" opens Minecraft inside Arcadia; "window" opens the standalone launcher.
    pub minecraft_open_mode: String,
    /// Azure app (client) ID approved for Minecraft sign-in.
    pub minecraft_client_id: String,
    pub curseforge_api_key: String,
    pub minecraft_default_memory_mb: u32,

    /// One-time settings migrations already applied.
    pub migrations: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            accent: "#7c9cff".into(),
            run_on_boot: false,
            minimize_to_tray: true,
            roblox_channel: "production".into(),
            roblox_pinned_version: None,
            roblox_auto_update: true,
            roblox_multi_instance: false,
            roblox_fps_cap: 0,
            roblox_priority: "Normal".into(),
            roblox_custom_font: None,
            roblox_apply_mods: true,
            fast_flags: BTreeMap::new(),
            roblox_flag_profiles: BTreeMap::new(),
            roblox_flag_profile: String::new(),
            roblox_auto_rejoin: false,
            roblox_rejoin_delay: 10,
            roblox_rejoin_same_server: true,
            roblox_mod_studio: Default::default(),
            roblox_mod_presets: BTreeMap::new(),
            proxy_enabled: false,
            proxy_port: 58443,
            proxy_clear_cache_on_launch: true,
            proxy_clear_cache_on_exit: true,
            proxy_fflag_overrides: BTreeMap::new(),
            proxy_fflags_risk_accepted: false,
            proxy_inspector_enabled: true,
            ui: serde_json::Value::Null,
            hide_on_game_launch: false,
            minecraft_open_mode: "page".into(),
            minecraft_client_id: String::new(),
            curseforge_api_key: String::new(),
            minecraft_default_memory_mb: 4096,
            migrations: CURRENT_MIGRATION,
        }
    }
}

const CURRENT_MIGRATION: u32 = 1;

impl Settings {
    pub fn load() -> Self {
        let mut s: Settings = std::fs::read_to_string(crate::paths::settings_file())
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .map(|mut v| {
                // Files from before migrations existed count as migration 0.
                if v.get("migrations").is_none() {
                    v["migrations"] = 0.into();
                }
                serde_json::from_value(v).unwrap_or_default()
            })
            .unwrap_or_default();
        if s.migrations < 1 {
            // Roblox now rejects the asset proxy; turn it off for existing installs.
            s.proxy_enabled = false;
            s.proxy_fflags_risk_accepted = false;
            s.proxy_fflag_overrides.clear();
            s.migrations = 1;
            let _ = s.save();
        }
        s
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = crate::paths::settings_file();
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        // Write-then-rename so a crash never leaves a half-written file.
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(tmp, path)?;
        Ok(())
    }
}
