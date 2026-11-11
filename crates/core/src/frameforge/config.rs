//! FrameForge's own settings file (`FrameForge\config.json`).
//!
//! Kept apart from Arcadia's `settings.json` so the main settings type does
//! not grow with every FrameForge option. The file carries a schema number;
//! [`migrate`] upgrades older files step by step and is the only place that
//! knows about old layouts.

use serde::{Deserialize, Serialize};

pub const CURRENT_SCHEMA: u32 = 2;

/// Readings the Live Monitor can show, in default order.
pub const MONITOR_TILES: &[&str] = &[
    "fps", "frameTimeGraph", "avgFps", "low1", "low01", "frameTime", "consistency", "cpuBusy", "gpuBusy", "gpuUtil", "cpuUtil", "gpuTemp", "gpuPower", "vram", "ram",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub schema: u32,
    /// PresentMon console executable chosen by the user (fallback capture).
    pub presentmon_cli: Option<String>,
    /// NVIDIA Profile Inspector executable chosen by the user (fallback only).
    pub npi_exe: Option<String>,
    /// Which Live Monitor readings are visible.
    pub monitor_tiles: Vec<String>,
    /// Highest optimisation level shown: 1 safe, 2 advanced, 3 expert.
    pub optimizer_level: u8,
    /// The user has read and accepted the expert-level warning.
    pub expert_acknowledged: bool,
    /// Apply a game's performance profile when the game starts. Opt-in.
    pub auto_profiles: bool,
    /// Start monitoring when a game with a profile starts.
    pub auto_monitor: bool,
    /// Put temporary changes back when the game exits.
    pub restore_on_exit: bool,
    /// NVIDIA setting ids starred in the profile editor.
    pub nvidia_favourites: Vec<u32>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema: CURRENT_SCHEMA,
            presentmon_cli: None,
            npi_exe: None,
            monitor_tiles: MONITOR_TILES.iter().map(|s| s.to_string()).collect(),
            optimizer_level: 1,
            expert_acknowledged: false,
            auto_profiles: false,
            auto_monitor: false,
            restore_on_exit: true,
            nvidia_favourites: Vec::new(),
        }
    }
}

/// Upgrade a stored document to the current schema.
///
/// * schema 0 (no `schema` field): pre-release layout with `presentmonPath`
///   and `tiles`.
/// * schema 1: `monitorTiles` lacked the busy-time readings.
pub fn migrate(mut v: serde_json::Value) -> Config {
    let Some(obj) = v.as_object_mut() else { return Config::default() };
    let mut schema = obj.get("schema").and_then(|s| s.as_u64()).unwrap_or(0) as u32;
    if schema == 0 {
        if let Some(p) = obj.remove("presentmonPath") {
            obj.entry("presentmonCli").or_insert(p);
        }
        if let Some(t) = obj.remove("tiles") {
            obj.entry("monitorTiles").or_insert(t);
        }
        schema = 1;
    }
    if schema == 1 {
        if let Some(tiles) = obj.get_mut("monitorTiles").and_then(|t| t.as_array_mut()) {
            for extra in ["cpuBusy", "gpuBusy"] {
                if !tiles.iter().any(|t| t.as_str() == Some(extra)) {
                    tiles.push(extra.into());
                }
            }
        }
        schema = 2;
    }
    // A file from a newer Arcadia: keep what this version understands.
    let _ = schema;
    obj.insert("schema".into(), CURRENT_SCHEMA.into());
    let mut c: Config = serde_json::from_value(v).unwrap_or_default();
    c.sanitize();
    c
}

impl Config {
    fn sanitize(&mut self) {
        self.schema = CURRENT_SCHEMA;
        self.optimizer_level = self.optimizer_level.clamp(1, 3);
        if self.optimizer_level == 3 && !self.expert_acknowledged {
            self.optimizer_level = 2;
        }
        self.monitor_tiles.retain(|t| MONITOR_TILES.contains(&t.as_str()));
        self.monitor_tiles.dedup();
        self.nvidia_favourites.sort_unstable();
        self.nvidia_favourites.dedup();
        for p in [&mut self.presentmon_cli, &mut self.npi_exe] {
            if p.as_deref().map(|s| s.trim().is_empty()).unwrap_or(false) {
                *p = None;
            }
        }
    }

    pub fn path() -> std::path::PathBuf {
        super::data_dir().join("config.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .map(migrate)
            .unwrap_or_default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let mut c = self.clone();
        c.sanitize();
        super::write_atomic(&Self::path(), &serde_json::to_vec_pretty(&c)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_schema_0_and_1() {
        let old = serde_json::json!({ "presentmonPath": "C:\\Tools\\PresentMon.exe", "tiles": ["fps", "ram", "bogus"] });
        let c = migrate(old);
        assert_eq!(c.schema, CURRENT_SCHEMA);
        assert_eq!(c.presentmon_cli.as_deref(), Some("C:\\Tools\\PresentMon.exe"));
        assert_eq!(c.monitor_tiles, vec!["fps", "ram", "cpuBusy", "gpuBusy"]);

        let v1 = serde_json::json!({ "schema": 1, "monitorTiles": ["fps"], "autoProfiles": true });
        let c = migrate(v1);
        assert!(c.auto_profiles);
        assert!(c.monitor_tiles.contains(&"gpuBusy".to_string()));
    }

    #[test]
    fn newer_or_broken_files_do_not_break_loading() {
        let future = serde_json::json!({ "schema": 99, "optimizerLevel": 2, "somethingNew": { "x": 1 } });
        let c = migrate(future);
        assert_eq!((c.schema, c.optimizer_level), (CURRENT_SCHEMA, 2));
        assert_eq!(migrate(serde_json::json!("not an object")), Config::default());
        assert_eq!(migrate(serde_json::json!({ "schema": 2, "optimizerLevel": "high" })), Config::default());
    }

    #[test]
    fn expert_level_needs_acknowledgement_and_values_are_clamped() {
        let c = migrate(serde_json::json!({ "schema": 2, "optimizerLevel": 3 }));
        assert_eq!(c.optimizer_level, 2);
        let c = migrate(serde_json::json!({ "schema": 2, "optimizerLevel": 3, "expertAcknowledged": true }));
        assert_eq!(c.optimizer_level, 3);
        let c = migrate(serde_json::json!({ "schema": 2, "optimizerLevel": 0, "presentmonCli": "  " }));
        assert_eq!(c.optimizer_level, 1);
        assert!(c.presentmon_cli.is_none());
    }
}
