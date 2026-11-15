//! Arcadia's own graphics presets: a vendor-neutral JSON container for
//! vendor-specific settings.
//!
//! A preset says which vendor it is for and lists setting ids with values:
//!
//! * NVIDIA: `drs.0x00A879CF` with a typed driver value (applied to a driver
//!   profile through the DRS layer, with a snapshot first);
//! * AMD: `adlx.*` control ids with numbers (applied through ADLX);
//! * Intel: `igcl.*` control ids with numbers (applied through IGCL).
//!
//! The container is shared; the contents are not interchangeable. A preset
//! is only ever applied to a GPU of its own vendor, and every setting is
//! checked against what that GPU's driver reports before anything is
//! written. This format is **not** a `.nip` file and has nothing to do with
//! NVIDIA's database dump.
//!
//! Preset files are untrusted input: size, schema version, types, ids and
//! the executable name are validated, and nothing in a preset is ever run.

use super::gpu::{self, ControlKind, GpuDevice, Vendor};
use super::nvidia::{self, Change, ProfileKind};
use super::optimizer::{ApplyRequest, SessionStatus};
use super::service::FrameForge;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const PRESET_SCHEMA: u32 = 1;
pub const MAX_PRESET_BYTES: usize = 1024 * 1024;
pub const MAX_SETTINGS: usize = 1024;

/// A number for AMD/Intel controls, a typed driver value for NVIDIA.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PresetValue {
    Number(f64),
    Drs(nvidia::Value),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetSetting {
    pub id: String,
    pub value: PresetValue,
    /// Display name at the time of capture. Informational only.
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub schema: u32,
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub vendor: Vendor,
    /// GPU the preset was made on. A different model only produces a warning.
    #[serde(default)]
    pub gpu_model: Option<String>,
    /// Driver the preset was captured on.
    #[serde(default)]
    pub driver_version: Option<String>,
    /// Oldest driver the author says it works with. Enforced when set.
    #[serde(default)]
    pub min_driver: Option<String>,
    /// Game executable (file name only) the preset is meant for.
    #[serde(default)]
    pub game_exe: Option<String>,
    pub settings: Vec<PresetSetting>,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub created_with: String,
    #[serde(default)]
    pub notes: String,
}

fn drs_id(id: &str) -> Option<u32> {
    let hex = id.strip_prefix("drs.0x")?;
    (hex.len() == 8).then(|| u32::from_str_radix(hex, 16).ok()).flatten()
}

fn vendor_prefix(v: Vendor) -> Option<&'static str> {
    match v {
        Vendor::Nvidia => Some("drs."),
        Vendor::Amd => Some("adlx."),
        Vendor::Intel => Some("igcl."),
        _ => None,
    }
}

/// Compare dotted driver versions numerically ("566.14" < "572.16", "32.0.101.6078").
fn version_lt(a: &str, b: &str) -> Option<bool> {
    let parse = |s: &str| s.split('.').map(|p| p.trim().parse::<u64>().ok()).collect::<Option<Vec<u64>>>();
    let (a, b) = (parse(a)?, parse(b)?);
    Some(a < b)
}

impl Preset {
    /// Check everything that can be checked without hardware.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema == 0 {
            return Err("The preset has no schema version".into());
        }
        if self.schema > PRESET_SCHEMA {
            return Err(format!("This preset was made by a newer version of Arcadia (schema {}; this version reads up to {PRESET_SCHEMA})", self.schema));
        }
        let name = self.name.trim();
        if name.is_empty() || name.chars().count() > 120 || name.chars().any(|c| c.is_control()) {
            return Err("The preset needs a name of at most 120 characters".into());
        }
        let Some(prefix) = vendor_prefix(self.vendor) else { return Err("The preset does not name a supported GPU vendor (NVIDIA, AMD or Intel)".into()) };
        if let Some(exe) = &self.game_exe {
            nvidia::nip::check_executable(exe)?;
            if exe.contains(['\\', '/', ':']) {
                return Err("The game executable must be a file name, not a path".into());
            }
        }
        for text in [&self.gpu_model, &self.driver_version, &self.min_driver].into_iter().flatten() {
            if text.chars().count() > 200 || text.chars().any(|c| c.is_control()) {
                return Err("The preset has an invalid hardware or driver field".into());
            }
        }
        if self.notes.chars().count() > 4000 {
            return Err("The preset's notes are too long".into());
        }
        if self.settings.is_empty() {
            return Err("The preset has no settings".into());
        }
        if self.settings.len() > MAX_SETTINGS {
            return Err(format!("The preset has more than {MAX_SETTINGS} settings"));
        }
        let mut seen = std::collections::HashSet::new();
        for s in &self.settings {
            if !seen.insert(s.id.as_str()) {
                return Err(format!("The setting '{}' appears twice", s.id));
            }
            if s.id.len() > 80 || !s.id.starts_with(prefix) || !s.id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_')) {
                return Err(format!("'{}' is not a {} setting id", s.id.chars().take(40).collect::<String>(), self.vendor.label()));
            }
            if s.name.chars().count() > 200 || s.name.chars().any(|c| c.is_control()) {
                return Err(format!("The setting '{}' has an invalid name", s.id));
            }
            match (&s.value, self.vendor) {
                (PresetValue::Drs(v), Vendor::Nvidia) => {
                    if drs_id(&s.id).filter(|id| *id != 0).is_none() {
                        return Err(format!("'{}' is not an NVIDIA setting id (expected drs.0x followed by 8 hex digits)", s.id));
                    }
                    v.check().map_err(|e| format!("{}: {e}", s.id))?;
                }
                (PresetValue::Number(n), Vendor::Amd | Vendor::Intel) => {
                    if !n.is_finite() {
                        return Err(format!("The value of '{}' is not a number", s.id));
                    }
                }
                (PresetValue::Number(_), _) => return Err(format!("'{}' needs a typed NVIDIA driver value", s.id)),
                (PresetValue::Drs(_), _) => return Err(format!("'{}' holds an NVIDIA driver value in a {} preset", s.id, self.vendor.label())),
            }
        }
        Ok(())
    }

    /// Parse and validate a preset file.
    pub fn parse(bytes: &[u8]) -> Result<Preset, String> {
        if bytes.len() > MAX_PRESET_BYTES {
            return Err("The file is too large to be an Arcadia preset".into());
        }
        let text = std::str::from_utf8(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes)).map_err(|_| "The file is not UTF-8 text".to_string())?;
        if text.trim_start().starts_with('<') {
            return Err("This looks like an XML file. NVIDIA .nip files are imported on the NVIDIA Profiles page, not as presets.".into());
        }
        let preset: Preset = serde_json::from_str(text).map_err(|e| format!("The file is not an Arcadia preset: {e}"))?;
        preset.validate()?;
        Ok(preset)
    }

    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).expect("presets serialise")
    }
}

/// Presets saved on this PC (`FrameForge\presets\<id>.json`).
pub struct PresetStore {
    dir: PathBuf,
}

impl PresetStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn path(&self, id: &str) -> Option<PathBuf> {
        // Ids are generated here; anything else is refused rather than used as a file name.
        (!id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')).then(|| self.dir.join(format!("{id}.json")))
    }

    pub fn list(&self) -> Vec<Preset> {
        let mut out: Vec<Preset> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
            .filter_map(|e| std::fs::read(e.path()).ok())
            .filter_map(|b| Preset::parse(&b).ok())
            .filter(|p| !p.id.is_empty())
            .collect();
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.id.cmp(&b.id)));
        out
    }

    pub fn get(&self, id: &str) -> anyhow::Result<Preset> {
        let path = self.path(id).ok_or_else(|| anyhow::anyhow!("That preset does not exist"))?;
        let bytes = std::fs::read(path).map_err(|_| anyhow::anyhow!("That preset does not exist"))?;
        Preset::parse(&bytes).map_err(|e| anyhow::anyhow!(e))
    }

    /// Save under a new id (imports and captures never overwrite).
    pub fn add(&self, mut preset: Preset) -> anyhow::Result<Preset> {
        preset.validate().map_err(|e| anyhow::anyhow!(e))?;
        preset.id = uuid::Uuid::new_v4().simple().to_string();
        preset.name = preset.name.trim().to_string();
        super::write_atomic(&self.path(&preset.id).expect("generated id is valid"), &preset.to_json())?;
        Ok(preset)
    }

    pub fn delete(&self, id: &str) -> anyhow::Result<()> {
        let path = self.path(id).ok_or_else(|| anyhow::anyhow!("That preset does not exist"))?;
        std::fs::remove_file(path).map_err(|_| anyhow::anyhow!("That preset does not exist"))
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingCheck {
    pub id: String,
    pub name: String,
    pub current: Option<String>,
    pub target: String,
    /// "change", "same" or "skip".
    pub status: String,
    pub reason: Option<String>,
}

/// Whether and how a preset fits this PC.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetCheck {
    pub usable: bool,
    /// Reasons the preset cannot be applied at all.
    pub problems: Vec<String>,
    pub warnings: Vec<String>,
    /// What it would be applied to (a GPU, or an NVIDIA driver profile).
    pub target: String,
    pub settings: Vec<SettingCheck>,
    /// Optimiser level the changes need (AMD/Intel controls).
    pub level: u8,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetOutcome {
    pub applied: usize,
    pub skipped: Vec<SettingCheck>,
    pub verified: bool,
    pub message: String,
    /// Where to undo it: an optimiser session (AMD/Intel)…
    pub session_id: Option<String>,
    /// …or an NVIDIA profile snapshot.
    pub backup_id: Option<String>,
}

fn control_text(c: &gpu::GpuControl, v: f64) -> String {
    match &c.kind {
        ControlKind::Toggle => if v != 0.0 { "On" } else { "Off" }.to_string(),
        ControlKind::Choice { options } => options.iter().find(|o| o.value == v).map(|o| o.label.clone()).unwrap_or_else(|| v.to_string()),
        ControlKind::Range { unit, .. } => format!("{v} {unit}").trim().to_string(),
        ControlKind::ReadOnly => v.to_string(),
    }
}

impl FrameForge {
    pub fn presets(&self) -> PresetStore {
        PresetStore::new(super::presets_dir())
    }

    fn preset_gpu(&self, vendor: Vendor) -> Option<GpuDevice> {
        self.inventory(false).gpus.into_iter().find(|g| g.vendor == vendor)
    }

    /// The NVIDIA driver profile a preset targets: the one that owns its
    /// executable, the global profile when it names none, or `None` when a
    /// profile would have to be created.
    fn preset_nv_profile(&self, preset: &Preset) -> anyhow::Result<Option<String>> {
        let list = self.nvidia().list(false)?;
        Ok(match &preset.game_exe {
            None => list.iter().find(|p| p.kind == ProfileKind::Global).map(|p| p.name.clone()),
            Some(exe) => list.iter().find(|p| p.executables.iter().any(|e| e.eq_ignore_ascii_case(exe))).map(|p| p.name.clone()),
        })
    }

    /// Capture the current settings of one GPU as a preset (not saved yet).
    /// AMD/Intel: every supported, editable control below Expert level.
    /// NVIDIA: the settings the user changed in the profile that owns
    /// `game_exe` (the global profile when no executable is given).
    pub fn preset_capture(&self, gpu_id: &str, name: &str, game_exe: Option<&str>, notes: &str) -> anyhow::Result<Preset> {
        let gpu = self.inventory(false).gpus.into_iter().find(|g| g.id == gpu_id).ok_or_else(|| anyhow::anyhow!("That graphics adapter is no longer present"))?;
        let game_exe = game_exe.map(str::trim).filter(|e| !e.is_empty()).map(String::from);
        let settings: Vec<PresetSetting> = match gpu.vendor {
            Vendor::Nvidia => {
                let probe = Preset { schema: PRESET_SCHEMA, id: String::new(), name: "x".into(), vendor: Vendor::Nvidia, gpu_model: None, driver_version: None, min_driver: None, game_exe: game_exe.clone(), settings: Vec::new(), created_at: 0, created_with: String::new(), notes: String::new() };
                let profile = self.preset_nv_profile(&probe)?.ok_or_else(|| anyhow::anyhow!("No NVIDIA driver profile contains {}", game_exe.clone().unwrap_or_default()))?;
                self.nvidia().detail(&profile)?.settings.into_iter().filter(|s| s.modified).filter_map(|s| Some(PresetSetting { id: format!("drs.0x{:08X}", s.id), value: PresetValue::Drs(s.value?), name: s.name })).collect()
            }
            Vendor::Amd | Vendor::Intel => self
                .gpus()
                .controls(&gpu)
                .into_iter()
                .filter(|c| c.supported && c.level < 3 && c.kind != ControlKind::ReadOnly)
                .filter_map(|c| Some(PresetSetting { id: c.id, value: PresetValue::Number(c.value?), name: c.name }))
                .collect(),
            _ => anyhow::bail!("Presets are available for NVIDIA, AMD and Intel graphics"),
        };
        if settings.is_empty() {
            anyhow::bail!(if gpu.vendor == Vendor::Nvidia { "That driver profile has no settings changed by you, so there is nothing to save" } else { "The driver reports no adjustable settings for this adapter, so there is nothing to save" });
        }
        let preset = Preset {
            schema: PRESET_SCHEMA,
            id: String::new(),
            name: name.trim().to_string(),
            vendor: gpu.vendor,
            gpu_model: Some(gpu.name.clone()),
            driver_version: gpu.driver_version_vendor.clone().or(gpu.driver_version.clone()),
            min_driver: None,
            game_exe,
            settings,
            created_at: super::hardware::now_secs(),
            created_with: format!("Arcadia {}", env!("CARGO_PKG_VERSION")),
            notes: notes.trim().to_string(),
        };
        preset.validate().map_err(|e| anyhow::anyhow!(e))?;
        Ok(preset)
    }

    /// Compare a preset with this PC. Nothing is written.
    pub fn preset_check(&self, preset: &Preset) -> PresetCheck {
        let mut check = PresetCheck { usable: false, problems: Vec::new(), warnings: Vec::new(), target: String::new(), settings: Vec::new(), level: 1 };
        if let Err(e) = preset.validate() {
            check.problems.push(e);
            return check;
        }
        let Some(gpu) = self.preset_gpu(preset.vendor) else {
            check.problems.push(format!("This preset is for {} graphics. This PC has no {} GPU, and settings from one vendor cannot be applied to another.", preset.vendor.label(), preset.vendor.label()));
            return check;
        };
        check.target = gpu.name.clone();
        if let Some(model) = preset.gpu_model.as_deref().filter(|m| !m.eq_ignore_ascii_case(&gpu.name)) {
            check.warnings.push(format!("Made on a {model}; this PC has a {}. Settings the driver does not offer here are skipped.", gpu.name));
        }
        let installed = gpu.driver_version_vendor.clone().or(gpu.driver_version.clone());
        if let Some(min) = &preset.min_driver {
            match installed.as_deref().and_then(|i| version_lt(i, min)) {
                Some(true) => check.problems.push(format!("Needs graphics driver {min} or newer (installed: {}).", installed.clone().unwrap_or_default())),
                Some(false) => {}
                None => check.warnings.push(format!("The preset asks for driver {min} or newer; the installed driver version could not be compared.")),
            }
        }
        match preset.vendor {
            Vendor::Nvidia => self.check_nvidia(preset, &mut check),
            _ => self.check_controls(preset, &gpu, &mut check),
        }
        if check.problems.is_empty() && !check.settings.iter().any(|s| s.status == "change") {
            if check.settings.iter().any(|s| s.status == "same") {
                check.warnings.push("Everything in this preset is already set.".into());
            } else {
                check.problems.push("None of the preset's settings can be applied on this PC.".into());
            }
        }
        check.usable = check.problems.is_empty() && check.settings.iter().any(|s| s.status == "change");
        check
    }

    fn check_controls(&self, preset: &Preset, gpu: &GpuDevice, check: &mut PresetCheck) {
        let status = self.gpus().adapter_for(gpu.vendor).map(|a| a.status());
        if let Some(gpu::ApiStatus::Unavailable { reason, .. }) = &status {
            check.problems.push(format!("{} is not available: {reason}", gpu.vendor.api_name()));
            return;
        }
        let controls = self.gpus().controls(gpu);
        for s in &preset.settings {
            let PresetValue::Number(v) = s.value else { continue };
            let entry = match controls.iter().find(|c| c.id == s.id) {
                None => SettingCheck { id: s.id.clone(), name: s.name.clone(), current: None, target: v.to_string(), status: "skip".into(), reason: Some("The driver does not offer this setting on this adapter.".into()) },
                Some(c) => match gpu::validate_control(c, gpu.vendor, v) {
                    Err(e) => SettingCheck { id: s.id.clone(), name: c.name.clone(), current: c.value.map(|x| control_text(c, x)), target: v.to_string(), status: "skip".into(), reason: Some(e.to_string()) },
                    Ok(()) => {
                        let same = c.value.map(|x| (x - v).abs() < 1e-6).unwrap_or(false);
                        if !same {
                            check.level = check.level.max(c.level);
                        }
                        SettingCheck { id: s.id.clone(), name: c.name.clone(), current: c.value.map(|x| control_text(c, x)), target: control_text(c, v), status: if same { "same" } else { "change" }.into(), reason: None }
                    }
                },
            };
            check.settings.push(entry);
        }
    }

    fn nv_changes(preset: &Preset) -> Vec<(String, Change)> {
        preset.settings.iter().filter_map(|s| match (&s.value, drs_id(&s.id)) {
            (PresetValue::Drs(v), Some(id)) => Some((s.id.clone(), Change::Set { id, value: v.clone() })),
            _ => None,
        }).collect()
    }

    fn check_nvidia(&self, preset: &Preset, check: &mut PresetCheck) {
        let nv = self.nvidia();
        let st = nv.status();
        if !st.available {
            check.problems.push(format!("NVIDIA driver profiles are not available: {}", st.reason.unwrap_or_default()));
            return;
        }
        let profile = match self.preset_nv_profile(preset) {
            Ok(p) => p,
            Err(e) => {
                check.problems.push(e.to_string());
                return;
            }
        };
        check.level = 2;
        let detail = profile.as_ref().and_then(|p| nv.detail(p).ok());
        check.target = match (&profile, &preset.game_exe) {
            (Some(p), _) => format!("NVIDIA driver profile \"{p}\""),
            (None, Some(exe)) => format!("a new NVIDIA driver profile for {exe}"),
            (None, None) => "the NVIDIA global profile".into(),
        };
        for (sid, change) in Self::nv_changes(preset) {
            let Change::Set { id, value } = &change else { continue };
            let live = detail.as_ref().and_then(|d| d.settings.iter().find(|s| s.id == *id));
            let name = live.map(|s| s.name.clone()).filter(|n| !n.is_empty()).or_else(|| preset.settings.iter().find(|s| s.id == sid).map(|s| s.name.clone())).unwrap_or_default();
            let current = live.and_then(|s| s.value.as_ref().map(|v| s.value_label.clone().map(|l| format!("{l} ({})", v.raw_text())).unwrap_or_else(|| v.raw_text())));
            let target = live.and_then(|s| value.as_u64().and_then(|n| s.options.iter().find(|o| o.value == n)).map(|o| format!("{} ({})", o.label, value.raw_text()))).unwrap_or_else(|| value.raw_text());
            // An existing profile is asked directly whether the change is acceptable.
            let problem = match (&profile, live) {
                (Some(p), _) => nv.plan(p, std::slice::from_ref(&change)).err().map(|e| e.to_string()),
                _ => None,
            };
            let same = live.and_then(|s| s.value.as_ref()).map(|v| v == value).unwrap_or(false) && live.map(|s| s.modified).unwrap_or(false);
            check.settings.push(SettingCheck { id: sid, name, current, target, status: if problem.is_some() { "skip" } else if same { "same" } else { "change" }.into(), reason: problem });
        }
    }

    /// Apply a preset. `approved_level` is what the user confirmed (needed
    /// for AMD/Intel controls, which go through the optimiser and its history).
    pub fn preset_apply(&self, preset: &Preset, approved_level: u8) -> anyhow::Result<PresetOutcome> {
        let check = self.preset_check(preset);
        if !check.usable {
            anyhow::bail!("{}", check.problems.first().cloned().unwrap_or_else(|| "Everything in this preset is already set.".into()));
        }
        let wanted: Vec<&SettingCheck> = check.settings.iter().filter(|s| s.status == "change").collect();
        let skipped: Vec<SettingCheck> = check.settings.iter().filter(|s| s.status == "skip").cloned().collect();
        if preset.vendor == Vendor::Nvidia {
            let nv = self.nvidia();
            let profile = match self.preset_nv_profile(preset)? {
                Some(p) => p,
                None => {
                    let exe = preset.game_exe.clone().ok_or_else(|| anyhow::anyhow!("The NVIDIA global profile was not found"))?;
                    let name = format!("{} (Arcadia)", exe.trim_end_matches(".exe"));
                    nv.create_profile(&name, Some(&exe))?;
                    name
                }
            };
            let changes: Vec<Change> = Self::nv_changes(preset).into_iter().filter(|(id, _)| wanted.iter().any(|w| &w.id == id)).map(|(_, c)| c).collect();
            let out = nv.apply(&profile, &changes, Some(&format!("Before preset \"{}\"", preset.name)))?;
            return Ok(PresetOutcome {
                applied: out.changes.len(),
                skipped,
                verified: out.verified,
                message: if out.verified { format!("Applied to {profile} and verified. Undo it from NVIDIA Profiles, Backups.") } else { format!("Applied to {profile}, but not everything read back as expected: {}", out.mismatches.join("; ")) },
                session_id: None,
                backup_id: Some(out.backup_id),
            });
        }
        let gpu = self.preset_gpu(preset.vendor).ok_or_else(|| anyhow::anyhow!("That graphics adapter is no longer present"))?;
        let requests: Vec<ApplyRequest> = preset
            .settings
            .iter()
            .filter(|s| wanted.iter().any(|w| w.id == s.id))
            .filter_map(|s| match s.value {
                PresetValue::Number(v) => Some(ApplyRequest { id: "gpu.control".into(), params: serde_json::json!({ "gpu": gpu.id, "control": s.id, "value": v }) }),
                _ => None,
            })
            .collect();
        if approved_level < check.level {
            anyhow::bail!("This preset changes level {} controls and was approved for level {approved_level}", check.level);
        }
        let session = self.optimizer_apply(&requests, approved_level, &format!("Preset: {}", preset.name))?;
        let ok = session.status == SessionStatus::Applied;
        Ok(PresetOutcome {
            applied: if ok { session.changes.len() } else { 0 },
            skipped,
            verified: ok,
            message: if ok { format!("Applied to {} and read back. Undo it from Optimizer, History.", gpu.name) } else { session.error.clone().unwrap_or_else(|| "The preset could not be applied".into()) },
            session_id: Some(session.id),
            backup_id: None,
        })
    }

    pub fn preset_export(&self, id: &str, path: &Path) -> anyhow::Result<()> {
        if !path.is_absolute() || path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("json")) != Some(true) {
            anyhow::bail!("Choose a full path ending in .json");
        }
        super::write_atomic(path, &self.presets().get(id)?.to_json())
    }

    pub fn preset_import(&self, bytes: &[u8]) -> anyhow::Result<Preset> {
        let preset = Preset::parse(bytes).map_err(|e| anyhow::anyhow!(e))?;
        self.presets().add(preset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameforge::gpu::mock::{MockBase, MockVendor};
    use crate::frameforge::gpu::{ControlOption, GpuControl, GpuRegistry, Risk};
    use crate::frameforge::nvidia::mock::{MockDrs, PRERENDER, VSYNC};
    use crate::frameforge::nvidia::{NvidiaProfiles, Value};
    use crate::frameforge::optimizer::ops::{MockOps, MockProcs};
    use std::sync::Arc;

    fn control(id: &str, kind: ControlKind, value: f64, level: u8) -> GpuControl {
        GpuControl { id: id.into(), vendor: Vendor::Amd, name: id.into(), description: String::new(), category: "3D".into(), kind, value: Some(value), default: None, supported: true, unsupported_reason: None, risk: Risk::Low, level, requires_restart: false }
    }

    struct World {
        ff: FrameForge,
        amd: Arc<MockVendor>,
        drs: MockDrs,
        _dir: tempfile::TempDir,
        _home: (std::sync::MutexGuard<'static, ()>, tempfile::TempDir),
    }

    /// A PC with an AMD card (ADLX mocked) and an NVIDIA card (DRS mocked).
    fn world() -> World {
        let home = crate::minecraft::test_home();
        let dir = tempfile::tempdir().unwrap();
        let mut amd = MockVendor::available(Vendor::Amd);
        amd.controls = vec![
            control("adlx.antilag", ControlKind::Toggle, 0.0, 2),
            control("adlx.frtc_fps", ControlKind::Range { min: 30.0, max: 300.0, step: 1.0, unit: "FPS".into() }, 60.0, 2),
            control("adlx.vsync", ControlKind::Choice { options: vec![ControlOption { value: 0.0, label: "Always off".into() }, ControlOption { value: 3.0, label: "Always on".into() }] }, 0.0, 2),
            control("adlx.power", ControlKind::Range { min: 0.0, max: 10.0, step: 1.0, unit: "%".into() }, 0.0, 3),
            GpuControl { supported: false, unsupported_reason: Some("not on this card".into()), ..control("adlx.boost", ControlKind::Toggle, 0.0, 2) },
        ];
        let amd = Arc::new(amd);
        let devices = vec![
            GpuDevice { id: "amd0".into(), vendor: Vendor::Amd, name: "Radeon RX 7800 XT".into(), driver_version: Some("31.0.24027.1012".into()), ..Default::default() },
            GpuDevice { id: "nv0".into(), vendor: Vendor::Nvidia, name: "GeForce RTX 4070".into(), driver_version_vendor: Some("566.14".into()), ..Default::default() },
        ];
        let registry = GpuRegistry::new(Arc::new(MockBase::new(devices)), vec![amd.clone(), Arc::new(MockVendor::available(Vendor::Nvidia)), Arc::new(MockVendor::missing(Vendor::Intel, "no Intel GPU"))]);
        let drs = MockDrs::typical();
        let ff = FrameForge::with_parts(registry, Arc::new(MockOps::typical()), Arc::new(MockProcs::with(&[]))).with_nvidia(NvidiaProfiles::new(Arc::new(drs.clone()), dir.path().join("nvbackups")));
        World { ff, amd, drs, _dir: dir, _home: home }
    }

    #[test]
    fn capture_serialise_and_reload() {
        let w = world();
        let p = w.ff.preset_capture("amd0", "  Competitive  ", Some("game.exe"), "").unwrap();
        assert_eq!((p.schema, p.vendor, p.name.as_str()), (PRESET_SCHEMA, Vendor::Amd, "Competitive"));
        assert_eq!(p.gpu_model.as_deref(), Some("Radeon RX 7800 XT"));
        // Supported, editable, below Expert: the power control and the unsupported one are left out.
        assert_eq!(p.settings.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), vec!["adlx.antilag", "adlx.frtc_fps", "adlx.vsync"]);
        assert!(p.created_with.starts_with("Arcadia "));
        let saved = w.ff.presets().add(p.clone()).unwrap();
        assert!(!saved.id.is_empty());
        let again = Preset::parse(&saved.to_json()).unwrap();
        assert_eq!(again, saved);
        assert_eq!(w.ff.presets().list(), vec![saved.clone()]);
        // Export, import as a new copy, delete.
        let file = w._dir.path().join("p.json");
        w.ff.preset_export(&saved.id, &file).unwrap();
        let copy = w.ff.preset_import(&std::fs::read(&file).unwrap()).unwrap();
        assert_ne!(copy.id, saved.id);
        assert_eq!(w.ff.presets().list().len(), 2);
        w.ff.presets().delete(&copy.id).unwrap();
        assert!(w.ff.presets().get(&copy.id).is_err());
        assert!(w.ff.presets().get("../../etc/passwd").is_err());
        assert!(w.ff.preset_export(&saved.id, Path::new("relative.json")).is_err());
    }

    #[test]
    fn amd_preset_applies_through_the_optimiser_and_reverts() {
        let w = world();
        let mut p = w.ff.preset_capture("amd0", "Low latency", None, "").unwrap();
        p.settings = vec![
            PresetSetting { id: "adlx.antilag".into(), value: PresetValue::Number(1.0), name: String::new() },
            PresetSetting { id: "adlx.frtc_fps".into(), value: PresetValue::Number(60.0), name: String::new() },
            PresetSetting { id: "adlx.frtc_fps_typo".into(), value: PresetValue::Number(5.0), name: String::new() },
            PresetSetting { id: "adlx.vsync".into(), value: PresetValue::Number(2.0), name: String::new() },
            PresetSetting { id: "adlx.boost".into(), value: PresetValue::Number(1.0), name: String::new() },
        ];
        let c = w.ff.preset_check(&p);
        assert!(c.usable, "{:?}", c.problems);
        assert_eq!(c.settings.iter().map(|s| s.status.as_str()).collect::<Vec<_>>(), vec!["change", "same", "skip", "skip", "skip"]);
        assert_eq!((c.settings[0].current.as_deref(), c.settings[0].target.as_str()), (Some("Off"), "On"));
        assert_eq!(c.level, 2);
        // Approval below the needed level is refused; nothing reaches the driver.
        assert!(w.ff.preset_apply(&p, 1).is_err());
        assert!(w.amd.applied.lock().is_empty());
        let out = w.ff.preset_apply(&p, 2).unwrap();
        assert!(out.verified);
        assert_eq!((out.applied, out.skipped.len()), (1, 3));
        assert_eq!(*w.amd.applied.lock(), vec![("adlx.antilag".to_string(), 1.0)]);
        // It is an ordinary recorded session: revert puts the old value back.
        w.ff.optimizer_revert_session(out.session_id.as_ref().unwrap()).unwrap();
        assert_eq!(w.amd.applied.lock().last().unwrap(), &("adlx.antilag".to_string(), 0.0));
        // Applying again when everything matches is refused with the reason.
        p.settings.truncate(2);
        p.settings[0].value = PresetValue::Number(0.0);
        assert!(w.ff.preset_apply(&p, 2).unwrap_err().to_string().contains("already set"));
    }

    #[test]
    fn vendor_and_driver_requirements_are_enforced() {
        let w = world();
        let mut p = w.ff.preset_capture("amd0", "X", None, "").unwrap();
        // An Intel preset on a PC without Intel graphics.
        let mut intel = p.clone();
        intel.vendor = Vendor::Intel;
        intel.settings = vec![PresetSetting { id: "igcl.3d.low_latency".into(), value: PresetValue::Number(1.0), name: String::new() }];
        let c = w.ff.preset_check(&intel);
        assert!(!c.usable && c.problems[0].contains("no Intel GPU"), "{:?}", c.problems);
        assert!(w.ff.preset_apply(&intel, 3).is_err());
        // AMD ids inside an NVIDIA preset never validate, so AMD settings cannot be sent to NVIDIA.
        let mut cross = p.clone();
        cross.vendor = Vendor::Nvidia;
        assert!(cross.validate().unwrap_err().contains("NVIDIA setting id"));
        // Driver requirement.
        p.min_driver = Some("32.0.0.0".into());
        p.settings[0].value = PresetValue::Number(1.0);
        assert!(w.ff.preset_check(&p).problems[0].contains("32.0.0.0"));
        p.min_driver = Some("31.0.24027.1012".into());
        assert!(w.ff.preset_check(&p).usable);
        // A different GPU model is a warning, not a refusal.
        p.gpu_model = Some("Radeon RX 6600".into());
        let c = w.ff.preset_check(&p);
        assert!(c.usable && c.warnings.iter().any(|x| x.contains("RX 6600")));
        assert_eq!(version_lt("566.14", "572.16"), Some(true));
        assert_eq!(version_lt("572.16", "566.14"), Some(false));
        assert_eq!(version_lt("abc", "1"), None);
    }

    #[test]
    fn nvidia_preset_goes_through_drs_with_a_snapshot() {
        let w = world();
        // Capture what the user changed in the profile that owns cs2.exe.
        let p = w.ff.preset_capture("nv0", "CS2 tuned", Some("cs2.exe"), "").unwrap();
        assert_eq!(p.settings.len(), 1);
        assert_eq!(p.settings[0].id, format!("drs.0x{VSYNC:08X}"));
        assert_eq!(p.settings[0].value, PresetValue::Drs(Value::Dword(0x08416747)));
        assert!(w.ff.preset_check(&p).warnings.iter().any(|x| x.contains("already set")));

        // Edit it and apply: the driver profile changes, with a snapshot to undo it.
        let mut p2 = p.clone();
        p2.settings.push(PresetSetting { id: format!("drs.0x{PRERENDER:08X}"), value: PresetValue::Drs(Value::Dword(3)), name: String::new() });
        p2.settings.push(PresetSetting { id: format!("drs.0x{PRERENDER:08X}").replace("9E", "9F"), value: PresetValue::Drs(Value::Wstring("x".into())), name: String::new() });
        let c = w.ff.preset_check(&p2);
        assert!(c.target.contains("Counter-Strike 2"), "{}", c.target);
        assert_eq!(c.settings.iter().map(|s| s.status.as_str()).collect::<Vec<_>>(), vec!["same", "change", "change"]);
        let out = w.ff.preset_apply(&p2, 2).unwrap();
        assert!(out.verified, "{}", out.message);
        assert_eq!(w.drs.user_value("Counter-Strike 2", PRERENDER), Some(Value::Dword(3)));
        w.ff.nvidia().restore_backup(out.backup_id.as_ref().unwrap()).unwrap();
        assert_eq!(w.drs.user_value("Counter-Strike 2", PRERENDER), None);

        // A game without a driver profile gets one created for it.
        let mut fresh = p2.clone();
        fresh.game_exe = Some("brandnew.exe".into());
        fresh.settings.truncate(2);
        let c = w.ff.preset_check(&fresh);
        assert!(c.target.contains("new NVIDIA driver profile"), "{}", c.target);
        let out = w.ff.preset_apply(&fresh, 2).unwrap();
        assert!(out.verified, "{}", out.message);
        assert_eq!(w.drs.user_value("brandnew (Arcadia)", PRERENDER), Some(Value::Dword(3)));
        // Nothing went to the AMD card.
        assert!(w.amd.applied.lock().is_empty());
    }

    #[test]
    fn hostile_or_wrong_files_are_rejected() {
        let good = Preset { schema: 1, id: String::new(), name: "P".into(), vendor: Vendor::Amd, gpu_model: None, driver_version: None, min_driver: None, game_exe: Some("game.exe".into()), settings: vec![PresetSetting { id: "adlx.antilag".into(), value: PresetValue::Number(1.0), name: String::new() }], created_at: 0, created_with: String::new(), notes: String::new() };
        assert!(good.validate().is_ok());
        let json = |edit: &dyn Fn(&mut serde_json::Value)| {
            let mut v = serde_json::to_value(&good).unwrap();
            edit(&mut v);
            Preset::parse(&serde_json::to_vec(&v).unwrap())
        };
        assert!(json(&|_| {}).is_ok());
        assert!(json(&|v| v["schema"] = 99.into()).unwrap_err().contains("newer version"));
        assert!(json(&|v| v["schema"] = 0.into()).is_err());
        assert!(json(&|v| v["name"] = "".into()).is_err());
        assert!(json(&|v| v["vendor"] = "matrox".into()).is_err());
        assert!(json(&|v| v["gameExe"] = "..\\..\\evil.exe".into()).is_err());
        assert!(json(&|v| v["gameExe"] = "C:\\Windows\\cmd.exe".into()).unwrap_err().contains("not a path"));
        assert!(json(&|v| v["settings"][0]["id"] = "igcl.3d.vsync".into()).is_err(), "an Intel id in an AMD preset");
        assert!(json(&|v| v["settings"][0]["id"] = "adlx.x; rm -rf".into()).is_err());
        assert!(json(&|v| v["settings"][0]["value"] = "on".into()).is_err());
        assert!(json(&|v| v["settings"][0]["value"] = serde_json::json!({"type": "dword", "value": "1"})).is_err(), "an NVIDIA value in an AMD preset");
        assert!(json(&|v| v["settings"] = serde_json::json!([])).is_err());
        assert!(json(&|v| {
            let s = v["settings"][0].clone();
            v["settings"].as_array_mut().unwrap().push(s);
        })
        .unwrap_err()
        .contains("twice"));
        // Extra fields from a later minor version are ignored, not fatal.
        assert!(json(&|v| v["futureField"] = "x".into()).is_ok());
        assert!(Preset::parse(b"<?xml version=\"1.0\"?><ArrayOfProfile/>").unwrap_err().contains(".nip"));
        assert!(Preset::parse(&vec![b' '; MAX_PRESET_BYTES + 1]).is_err());
        assert!(Preset::parse(b"\xff\xfe").is_err());
    }
}
