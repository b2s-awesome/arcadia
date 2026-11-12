//! NVIDIA driver profiles: browse, edit, import and export, back up and restore.
//!
//! Layers, bottom up:
//! * [`types`]: values and the [`DrsBackend`] trait. `drs.rs` implements it
//!   with NVAPI's driver-settings (DRS) functions; `mock.rs` implements it in
//!   memory for tests.
//! * [`nip`]: NVIDIA Profile Inspector `.nip` files.
//! * [`meta`]: what the settings are called and what their values mean.
//! * [`store`]: snapshots taken before every change.
//! * this file: the rules. Every change to the driver goes through
//!   [`NvidiaProfiles::apply`] or [`NvidiaProfiles::import`], which always:
//!
//!   1. read the live profile,
//!   2. save a snapshot of it,
//!   3. work out the differences (also available on their own as a preview),
//!   4. apply them in a DRS session,
//!   5. save the session to the driver,
//!   6. open a new session and read every changed value back,
//!   7. record the result next to the snapshot.
//!
//!   A DRS session works on its own copy of the database, so when a step
//!   fails before the save nothing has reached the driver.
//!
//! Three file formats exist here and they are not interchangeable:
//! `.nip` (Profile Inspector's XML, [`nip`]), NVIDIA's own database dump
//! ([`NvidiaProfiles::save_driver_database`]) and Arcadia's vendor-neutral
//! JSON presets (`frameforge::presets`).

pub mod driver_settings;
#[cfg(windows)]
pub mod drs;
pub mod meta;
pub mod mock;
pub mod nip;
pub mod npi;
pub mod store;
pub mod types;

use meta::{Meta, NameSource, ValueLabel};
use nip::{NipFile, NipProfile, NipSetting};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;
use store::{BackupEntry, BackupReason, BackupStore, DiffLine};
use types::*;

pub use types::{Value, ValueKind};

/// Is the driver's profile API usable, and which driver is it?
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NvStatus {
    pub available: bool,
    /// Why not, when unavailable.
    pub reason: Option<String>,
    pub driver: DriverInfo,
    pub known_settings: usize,
    /// A user-supplied Reference.xml is loaded.
    pub reference_loaded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileKind {
    /// Applies to everything without its own profile.
    Global,
    /// Shipped by NVIDIA.
    Predefined,
    /// Made by the user (or a tool).
    User,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub name: String,
    pub kind: ProfileKind,
    pub executables: Vec<String>,
    pub setting_count: u32,
    /// Settings the user changed in this profile.
    pub customised_count: u32,
    /// Executables the user added to an NVIDIA profile.
    pub user_executables: u32,
}

impl ProfileSummary {
    pub fn is_customised(&self) -> bool {
        self.kind == ProfileKind::User || self.customised_count > 0 || self.user_executables > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingState {
    /// Changed by the user in this profile.
    User,
    /// NVIDIA's own value for this profile.
    Nvidia,
    /// Not stored in this profile; taken from the global profile.
    Inherited,
    /// Not stored anywhere; the driver default.
    Default,
    /// The driver returned nothing for this setting.
    Unset,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingView {
    pub id: u32,
    pub id_hex: String,
    pub name: String,
    pub name_source: NameSource,
    pub alternate_names: Vec<String>,
    pub group: String,
    pub description: String,
    pub kind: ValueKind,
    pub kind_label: String,
    /// The value in effect. `None`: the driver has no value for it.
    pub value: Option<Value>,
    /// Friendly name of the value, when one is known.
    pub value_label: Option<String>,
    /// The real value, always shown next to any label.
    pub raw: String,
    /// What "restore" would bring back.
    pub default: Option<Value>,
    pub default_label: Option<String>,
    pub state: SettingState,
    pub modified: bool,
    pub options: Vec<ValueLabel>,
    /// NVIDIA documents this setting in its SDK.
    pub documented: bool,
    /// The installed driver lists this setting as available.
    pub exposed: bool,
    pub common: bool,
    pub hidden: bool,
    /// NVIDIA stores this predefined value encoded; it is shown raw and cannot be edited.
    pub internal: bool,
    pub editable: bool,
    pub read_only_reason: Option<String>,
    /// e.g. "Needs driver 572.16 or newer".
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDetail {
    pub summary: ProfileSummary,
    pub apps: Vec<RawApp>,
    pub settings: Vec<SettingView>,
    /// Group names in display order.
    pub groups: Vec<String>,
    /// Whether encoded ("internal") NVIDIA values could be identified on this driver.
    pub internal_scan: bool,
}

/// One requested change to a profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Change {
    Set { id: u32, value: Value },
    /// Remove the user's value so the setting goes back to NVIDIA's value or what it inherits.
    Restore { id: u32 },
    /// Undo every user change to the profile's settings.
    RestoreProfile,
    #[serde(rename_all = "camelCase")]
    AddApp { exe: String, #[serde(default)] find_file: Option<String> },
    RemoveApp { exe: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOutcome {
    pub backup_id: String,
    pub changes: Vec<DiffLine>,
    /// Every change was read back from the driver and matched.
    pub verified: bool,
    /// What did not match, when `verified` is false.
    pub mismatches: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportMode {
    /// Make the profile match the file: user settings that are not in the file are removed.
    Replace,
    /// Add the file's settings on top of what is there.
    Merge,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewSetting {
    pub id: u32,
    pub id_hex: String,
    pub name: String,
    pub name_source: NameSource,
    pub documented: bool,
    pub value_type: String,
    pub raw_value: String,
    pub label: Option<String>,
    /// Why this entry cannot be applied (kept in the file regardless).
    pub problem: Option<String>,
    /// The live value in the driver now, when the profile exists.
    pub live: Option<String>,
    pub same_as_live: bool,
    pub options: Vec<ValueLabel>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewProfile {
    pub name: String,
    pub exists: bool,
    pub kind: Option<ProfileKind>,
    pub executables: Vec<String>,
    /// Executables that already belong to another profile: `(exe, owner)`.
    pub conflicts: Vec<(String, String)>,
    pub settings: Vec<PreviewSetting>,
    /// User values in the live profile that the file does not have (removed by a replace import).
    pub live_only: Vec<PreviewSetting>,
    pub unknown_elements: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub file: NipFile,
    pub profiles: Vec<PreviewProfile>,
    pub warnings: Vec<String>,
    /// The driver could be read, so live values are shown.
    pub driver_available: bool,
    pub driver_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedSetting {
    pub id: u32,
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub profile: String,
    pub created: bool,
    pub ok: bool,
    pub error: Option<String>,
    pub applied: usize,
    pub skipped: Vec<SkippedSetting>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOutcome {
    pub backup_id: String,
    pub profiles: Vec<ImportResult>,
    pub verified: bool,
    pub mismatches: Vec<String>,
}

fn drs(e: DrsError) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

fn hex(id: u32) -> String {
    format!("0x{id:08X}")
}

fn text_of(meta: &meta::SettingMeta, v: &Value) -> String {
    match v.as_u64().and_then(|n| meta.label_for(n)) {
        Some(label) => format!("{label} ({})", v.raw_text()),
        None => v.raw_text(),
    }
}

/// What a verification pass expects to find.
enum Expect {
    Value(u32, Value),
    NotUser(u32),
    NoUserSettings,
    App(String, bool),
}

pub struct NvidiaProfiles {
    backend: Arc<dyn DrsBackend>,
    store: BackupStore,
    /// One driver change at a time.
    lock: Mutex<()>,
    cache: Mutex<Option<Vec<ProfileSummary>>>,
    reference: Mutex<Option<Arc<Meta>>>,
    /// `Some(None)`: scanned, the driver's dump could not be interpreted.
    internal: Mutex<Option<Option<HashSet<(String, u32)>>>>,
    exposed: std::sync::OnceLock<HashSet<u32>>,
}

impl NvidiaProfiles {
    pub fn new(backend: Arc<dyn DrsBackend>, backups_dir: std::path::PathBuf) -> Self {
        Self { backend, store: BackupStore::new(backups_dir), lock: Mutex::new(()), cache: Mutex::new(None), reference: Mutex::new(None), internal: Mutex::new(None), exposed: std::sync::OnceLock::new() }
    }

    /// The real driver on Windows; elsewhere a backend that explains why not.
    pub fn system() -> Self {
        #[cfg(windows)]
        let backend: Arc<dyn DrsBackend> = Arc::new(drs::NvapiDrs::new());
        #[cfg(not(windows))]
        let backend: Arc<dyn DrsBackend> = Arc::new(mock::MockDrs::without_driver("NVIDIA driver profiles are only available on Windows"));
        Self::new(backend, crate::frameforge::nvidia_backups_dir())
    }

    fn with_meta<T>(&self, f: impl FnOnce(&Meta) -> T) -> T {
        let custom = self.reference.lock().clone();
        match custom {
            Some(m) => f(&m),
            None => f(Meta::builtin()),
        }
    }

    /// Load names from the user's own Profile Inspector `Reference.xml`.
    pub fn load_reference(&self, xml: &[u8]) -> Result<usize, String> {
        let m = Meta::with_reference(xml)?;
        let n = m.len();
        *self.reference.lock() = Some(Arc::new(m));
        Ok(n)
    }

    pub fn status(&self) -> NvStatus {
        let (known, reference_loaded) = (self.with_meta(|m| m.len()), self.reference.lock().is_some());
        match self.backend.status() {
            Ok(driver) => NvStatus { available: true, reason: None, driver, known_settings: known, reference_loaded },
            Err(reason) => NvStatus { available: false, reason: Some(reason), driver: DriverInfo::default(), known_settings: known, reference_loaded },
        }
    }

    fn exposed(&self) -> &HashSet<u32> {
        self.exposed.get_or_init(|| self.backend.available_setting_ids().into_iter().collect())
    }

    fn invalidate(&self) {
        *self.cache.lock() = None;
    }

    fn summary(session: &mut dyn DrsSession, p: &RawProfile, global: &str) -> DrsResult<ProfileSummary> {
        let apps = if p.app_count > 0 { session.apps(&p.name)? } else { Vec::new() };
        let customised = if p.setting_count > 0 { session.settings(&p.name)?.iter().filter(|s| s.is_user_value()).count() as u32 } else { 0 };
        let kind = if p.name.eq_ignore_ascii_case(global) {
            ProfileKind::Global
        } else if p.is_predefined {
            ProfileKind::Predefined
        } else {
            ProfileKind::User
        };
        Ok(ProfileSummary {
            name: p.name.clone(),
            kind,
            user_executables: if p.is_predefined { apps.iter().filter(|a| !a.is_predefined).count() as u32 } else { 0 },
            executables: apps.into_iter().map(|a| a.name).collect(),
            setting_count: p.setting_count,
            customised_count: customised,
        })
    }

    /// Every driver profile. The first call reads the whole database (a few
    /// thousand profiles on a real system); the result is cached until
    /// something changes or `refresh` is set.
    pub fn list(&self, refresh: bool) -> anyhow::Result<Vec<ProfileSummary>> {
        if !refresh {
            if let Some(c) = self.cache.lock().clone() {
                return Ok(c);
            }
        }
        let mut s = self.backend.open().map_err(drs)?;
        let global = s.global_profile_name().map_err(drs)?;
        let mut out = Vec::new();
        for p in s.profiles().map_err(drs)? {
            // One unreadable profile must not hide the rest.
            match Self::summary(s.as_mut(), &p, &global) {
                Ok(x) => out.push(x),
                Err(e) => log::warn!("NVIDIA profile '{}' could not be read: {e}", p.name),
            }
        }
        out.sort_by(|a, b| (a.kind != ProfileKind::Global).cmp(&(b.kind != ProfileKind::Global)).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        *self.cache.lock() = Some(out.clone());
        Ok(out)
    }

    /// `(profile name, setting id)` pairs whose NVIDIA value is stored
    /// encoded. Found by asking the driver to dump its database and reading
    /// the `InternalSettingFlag` markers; `None` when that is not possible.
    fn internal_settings(&self, session: &mut dyn DrsSession) -> Option<HashSet<(String, u32)>> {
        let mut slot = self.internal.lock();
        if let Some(known) = slot.as_ref() {
            return known.clone();
        }
        let path = crate::frameforge::cache_dir().join(format!("nvdrs-{}.txt", uuid::Uuid::new_v4().simple()));
        let _ = std::fs::create_dir_all(crate::frameforge::cache_dir());
        let result = session.save_to_file(&path).ok().and_then(|_| std::fs::read(&path).ok()).and_then(|bytes| parse_internal_flags(&bytes));
        let _ = std::fs::remove_file(&path);
        *slot = Some(result.clone());
        result
    }

    fn view(&self, m: &Meta, id: u32, raw: Option<&RawSetting>, default: Option<Value>, internal: bool, driver: &DriverInfo) -> SettingView {
        let mut info = m.describe(id);
        let exposed = self.exposed().contains(&id);
        if info.name.is_empty() {
            let from_driver = raw.map(|r| r.name.clone()).filter(|n| !n.trim().is_empty()).or_else(|| self.backend.setting_name(id));
            if let Some(n) = from_driver {
                info.name = n;
                info.name_source = NameSource::Driver;
                if info.group == meta::UNKNOWN_GROUP {
                    info.group = meta::OFFICIAL_GROUP.into();
                }
            }
        }
        let available = if exposed { self.backend.available_values(id) } else { None };
        let kind = raw
            .map(|r| r.current.kind())
            .or(available.as_ref().map(|a| a.kind))
            .unwrap_or(if info.options.iter().any(|o| o.value > u32::MAX as u64) { ValueKind::Qword } else { ValueKind::Dword });
        // Values the driver allows that nothing names still show up as choices.
        let mut options = info.options.clone();
        for v in available.iter().flat_map(|a| a.values.iter()).filter_map(|v| v.as_u64()) {
            if !options.iter().any(|o| o.value == v) {
                options.push(ValueLabel { value: v, label: if kind == ValueKind::Qword { format!("0x{v:016X}") } else { format!("0x{v:08X}") } });
            }
        }
        let value = raw.map(|r| r.effective().clone());
        let state = match raw {
            None => SettingState::Unset,
            Some(r) if r.location != Location::Current => {
                if r.location == Location::Default {
                    SettingState::Default
                } else {
                    SettingState::Inherited
                }
            }
            Some(r) if r.is_user_value() => SettingState::User,
            Some(_) => SettingState::Nvidia,
        };
        let default = default.or_else(|| available.as_ref().and_then(|a| a.default.clone()));
        let unknown_type = raw.and_then(|r| r.unknown_type);
        let read_only_reason = if let Some(code) = unknown_type {
            Some(format!("The driver reports a value type Arcadia does not know (type {code}). It is shown as raw bytes and cannot be edited."))
        } else if internal && state == SettingState::Nvidia {
            Some("NVIDIA stores this value encoded. It is shown exactly as the driver returns it and is not edited by Arcadia.".into())
        } else {
            None
        };
        let version: Option<f64> = driver.version.as_deref().and_then(|v| v.parse().ok());
        let note = match (version, info.min_driver, info.max_driver) {
            (Some(v), Some(min), _) if v < min => Some(format!("Needs driver {min} or newer (installed: {v})")),
            (Some(v), _, Some(max)) if v > max => Some(format!("Removed after driver {max} (installed: {v})")),
            _ => None,
        };
        SettingView {
            id,
            id_hex: hex(id),
            value_label: value.as_ref().and_then(|v| v.as_u64()).and_then(|n| options.iter().find(|o| o.value == n)).map(|o| o.label.clone()),
            raw: value.as_ref().map(|v| v.raw_text()).unwrap_or_default(),
            default_label: default.as_ref().and_then(|v| v.as_u64()).and_then(|n| options.iter().find(|o| o.value == n)).map(|o| o.label.clone()),
            default,
            kind,
            kind_label: kind.label().into(),
            value,
            state,
            modified: state == SettingState::User,
            options,
            documented: info.documented || exposed,
            exposed,
            common: info.common,
            hidden: info.hidden,
            internal: internal && state == SettingState::Nvidia,
            editable: read_only_reason.is_none(),
            read_only_reason,
            note,
            name: info.name,
            name_source: info.name_source,
            alternate_names: info.alternate_names,
            group: info.group,
            description: info.description,
        }
    }

    /// One profile with every setting worth showing: what the profile
    /// stores, the curated everyday list, and what the driver lists.
    pub fn detail(&self, profile: &str) -> anyhow::Result<ProfileDetail> {
        let driver = self.backend.status().unwrap_or_default();
        let mut s = self.backend.open().map_err(drs)?;
        let global = s.global_profile_name().map_err(drs)?;
        let raw_profile = s.profile(profile).map_err(drs)?.ok_or_else(|| anyhow::anyhow!("The NVIDIA driver has no profile named '{profile}'"))?;
        let name = raw_profile.name.clone();
        let summary = Self::summary(s.as_mut(), &raw_profile, &global).map_err(drs)?;
        let apps = s.apps(&name).map_err(drs)?;
        let stored = s.settings(&name).map_err(drs)?;
        let internal = self.internal_settings(s.as_mut());
        let is_internal = |id: u32| internal.as_ref().map(|set| set.contains(&(name.to_lowercase(), id))).unwrap_or(false);
        let is_global = summary.kind == ProfileKind::Global;

        let settings = self.with_meta(|m| -> DrsResult<Vec<SettingView>> {
            let mut ids: BTreeSet<u32> = stored.iter().map(|r| r.id).collect();
            ids.extend(m.ids().filter(|id| m.get(*id).map(|x| x.common).unwrap_or(false)));
            ids.extend(self.exposed().iter().copied());
            ids.remove(&0);
            let mut out = Vec::with_capacity(ids.len());
            for id in ids {
                let raw = match stored.iter().find(|r| r.id == id) {
                    Some(r) => Some(r.clone()),
                    None => s.setting(&name, id)?,
                };
                // What restoring a user value brings back: NVIDIA's value for
                // this profile if it has one, else whatever the global profile says.
                let default = match &raw {
                    Some(r) if r.is_user_value() && r.is_predefined_valid => Some(r.predefined.clone()),
                    Some(r) if r.is_user_value() && !is_global => s.setting(&global, id)?.map(|g| g.effective().clone()),
                    Some(r) if !r.is_user_value() => Some(r.effective().clone()),
                    _ => None,
                };
                out.push(self.view(m, id, raw.as_ref(), default, is_internal(id), &driver));
            }
            Ok(out)
        });
        let mut settings = settings.map_err(drs)?;
        settings.sort_by(|a, b| a.group.cmp(&b.group).then_with(|| (a.name.is_empty()).cmp(&b.name.is_empty())).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())).then_with(|| a.id.cmp(&b.id)));
        let mut groups: Vec<String> = Vec::new();
        for g in settings.iter().map(|x| &x.group) {
            if !groups.contains(g) {
                groups.push(g.clone());
            }
        }
        Ok(ProfileDetail { summary, apps, settings, groups, internal_scan: internal.is_some() })
    }

    /// The profile as a `.nip` profile: every executable, and the settings the user changed.
    fn snapshot(&self, s: &mut dyn DrsSession, name: &str) -> DrsResult<NipProfile> {
        let apps = s.apps(name)?;
        let stored = s.settings(name)?;
        let settings = self.with_meta(|m| {
            stored
                .iter()
                .filter(|r| r.is_user_value() && r.unknown_type.is_none())
                .map(|r| {
                    let label = if r.name.trim().is_empty() { m.get(r.id).map(|x| x.name.clone()).unwrap_or_default() } else { r.name.clone() };
                    NipSetting::new(r.id, &label, &r.current)
                })
                .collect()
        });
        Ok(NipProfile {
            name: name.to_string(),
            find_files: apps.iter().filter(|a| !a.file_in_folder.is_empty()).map(|a| nip::FindFile { executable: Some(a.name.clone()), find_file: Some(a.file_in_folder.clone()) }).collect(),
            executables: apps.into_iter().map(|a| a.name).collect(),
            settings,
            extra: Vec::new(),
        })
    }

    fn check_change(&self, change: &Change) -> anyhow::Result<()> {
        match change {
            Change::Set { id, value } => {
                if *id == 0 {
                    anyhow::bail!("Setting id 0 is not a driver setting");
                }
                value.check().map_err(|e| anyhow::anyhow!("{}: {e}", hex(*id)))?;
            }
            Change::AddApp { exe, find_file } => {
                nip::check_executable(exe).map_err(|e| anyhow::anyhow!(e))?;
                if let Some(f) = find_file.as_deref().filter(|f| !f.is_empty()) {
                    for part in f.split(':') {
                        nip::check_executable(part).map_err(|e| anyhow::anyhow!(e.replace("executable name", "find-file name")))?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Work out what `changes` would do to the live profile, and what to check afterwards.
    fn plan_in(&self, s: &mut dyn DrsSession, profile: &RawProfile, global: &str, changes: &[Change]) -> anyhow::Result<(Vec<DiffLine>, Vec<Expect>)> {
        let name = &profile.name;
        let stored = s.settings(name).map_err(drs)?;
        let apps = s.apps(name).map_err(drs)?;
        let internal = self.internal_settings(s);
        let mut lines = Vec::new();
        let mut expect = Vec::new();
        let mut seen: HashSet<u32> = HashSet::new();
        for c in changes {
            self.check_change(c)?;
            match c {
                Change::Set { id, value } => {
                    if !seen.insert(*id) {
                        anyhow::bail!("{} is changed twice in one request", hex(*id));
                    }
                    let live = match stored.iter().find(|r| r.id == *id) {
                        Some(r) => Some(r.clone()),
                        None => s.setting(name, *id).map_err(drs)?,
                    };
                    let info = self.with_meta(|m| m.describe(*id));
                    if let Some(r) = &live {
                        if r.unknown_type.is_some() {
                            anyhow::bail!("{} has a value type Arcadia does not know and cannot be edited", hex(*id));
                        }
                        if r.current.kind() != value.kind() {
                            anyhow::bail!("{} holds a {} value; a {} value cannot be written to it", hex(*id), r.current.kind().label(), value.kind().label());
                        }
                        if !r.is_user_value() && r.location == Location::Current && internal.as_ref().map(|set| set.contains(&(name.to_lowercase(), *id))).unwrap_or(false) {
                            anyhow::bail!("{} is stored encoded by NVIDIA in this profile and is not edited by Arcadia", hex(*id));
                        }
                    } else if let Some(av) = self.backend.available_values(*id) {
                        if av.kind != value.kind() {
                            anyhow::bail!("{} takes a {} value", hex(*id), av.kind.label());
                        }
                    }
                    let exposed = self.exposed().contains(id);
                    let note = if !info.documented && !exposed {
                        Some("Not documented by NVIDIA. Its effect can differ between driver versions.".to_string())
                    } else {
                        None
                    };
                    lines.push(DiffLine {
                        profile: name.clone(),
                        setting_id: Some(*id),
                        what: if info.name.is_empty() { live.as_ref().map(|r| r.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| "Unnamed setting".into()) } else { info.name.clone() },
                        before: live.as_ref().map(|r| text_of(&info, r.effective())).unwrap_or_else(|| "not set".into()),
                        after: text_of(&info, value),
                        note,
                    });
                    expect.push(Expect::Value(*id, value.clone()));
                }
                Change::Restore { id } => {
                    if !seen.insert(*id) {
                        anyhow::bail!("{} is changed twice in one request", hex(*id));
                    }
                    let Some(r) = stored.iter().find(|r| r.id == *id && r.is_user_value()) else {
                        anyhow::bail!("{} has no user value in '{name}' to restore", hex(*id));
                    };
                    let info = self.with_meta(|m| m.describe(*id));
                    let after = if r.is_predefined_valid {
                        format!("{} (NVIDIA's value)", text_of(&info, &r.predefined))
                    } else if name.eq_ignore_ascii_case(global) {
                        "driver default".to_string()
                    } else {
                        match s.setting(global, *id).map_err(drs)? {
                            Some(g) => format!("{} (from the global profile)", text_of(&info, g.effective())),
                            None => "driver default".to_string(),
                        }
                    };
                    lines.push(DiffLine { profile: name.clone(), setting_id: Some(*id), what: if info.name.is_empty() { r.name.clone() } else { info.name.clone() }, before: text_of(&info, &r.current), after, note: None });
                    expect.push(Expect::NotUser(*id));
                }
                Change::RestoreProfile => {
                    let n = stored.iter().filter(|r| r.is_user_value()).count();
                    lines.push(DiffLine { profile: name.clone(), setting_id: None, what: "All settings".into(), before: format!("{n} changed by you"), after: if profile.is_predefined { "NVIDIA's values".into() } else { "no settings (profile kept)".into() }, note: None });
                    expect.push(Expect::NoUserSettings);
                }
                Change::AddApp { exe, .. } => {
                    if apps.iter().any(|a| a.name.eq_ignore_ascii_case(exe)) {
                        anyhow::bail!("'{exe}' is already part of this profile");
                    }
                    if let Some(owner) = s.find_app_profile(exe).map_err(drs)? {
                        if !owner.eq_ignore_ascii_case(name) {
                            anyhow::bail!("'{exe}' already belongs to the profile '{owner}'. Remove it there first.");
                        }
                    }
                    lines.push(DiffLine { profile: name.clone(), setting_id: None, what: "Executable".into(), before: "not in this profile".into(), after: exe.clone(), note: None });
                    expect.push(Expect::App(exe.clone(), true));
                }
                Change::RemoveApp { exe } => {
                    let Some(a) = apps.iter().find(|a| a.name.eq_ignore_ascii_case(exe)) else { anyhow::bail!("'{exe}' is not part of this profile") };
                    lines.push(DiffLine {
                        profile: name.clone(),
                        setting_id: None,
                        what: "Executable".into(),
                        before: a.name.clone(),
                        after: "removed from this profile".into(),
                        note: a.is_predefined.then(|| "NVIDIA assigned this executable to the profile. Restoring the profile's defaults brings it back.".to_string()),
                    });
                    expect.push(Expect::App(exe.clone(), false));
                }
            }
        }
        Ok((lines, expect))
    }

    /// Preview: what `changes` would do. Nothing is written.
    pub fn plan(&self, profile: &str, changes: &[Change]) -> anyhow::Result<Vec<DiffLine>> {
        let mut s = self.backend.open().map_err(drs)?;
        let global = s.global_profile_name().map_err(drs)?;
        let p = s.profile(profile).map_err(drs)?.ok_or_else(|| anyhow::anyhow!("The NVIDIA driver has no profile named '{profile}'"))?;
        Ok(self.plan_in(s.as_mut(), &p, &global, changes)?.0)
    }

    fn run_changes(s: &mut dyn DrsSession, profile: &RawProfile, changes: &[Change]) -> DrsResult<()> {
        let name = &profile.name;
        for c in changes {
            match c {
                Change::Set { id, value } => s.set_setting(name, *id, value)?,
                Change::Restore { id } => {
                    let has_nvidia_value = s.setting(name, *id)?.map(|r| r.is_predefined_valid && r.location == Location::Current).unwrap_or(false);
                    if has_nvidia_value {
                        s.restore_setting_default(name, *id)?;
                    } else {
                        s.delete_setting(name, *id)?;
                    }
                }
                Change::RestoreProfile => Self::reset_profile(s, profile)?,
                Change::AddApp { exe, find_file } => s.add_app(name, exe, find_file.as_deref().unwrap_or(""))?,
                Change::RemoveApp { exe } => {
                    if let Some(a) = s.apps(name)?.into_iter().find(|a| a.name.eq_ignore_ascii_case(exe)) {
                        s.remove_app(name, &a)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Undo the user's setting changes: NVIDIA's defaults for a predefined
    /// profile, no settings for a user-made one.
    fn reset_profile(s: &mut dyn DrsSession, profile: &RawProfile) -> DrsResult<()> {
        if profile.is_predefined {
            s.restore_profile_default(&profile.name)
        } else {
            for r in s.settings(&profile.name)?.into_iter().filter(|r| r.is_user_value()) {
                s.delete_setting(&profile.name, r.id)?;
            }
            Ok(())
        }
    }

    fn verify(&self, profile: &str, expect: &[Expect]) -> Vec<String> {
        let mut bad = Vec::new();
        let mut s = match self.backend.open() {
            Ok(s) => s,
            Err(e) => return vec![format!("The driver could not be re-read to check the change: {e}")],
        };
        let (stored, apps) = match (s.settings(profile), s.apps(profile)) {
            (Ok(a), Ok(b)) => (a, b),
            (Err(e), _) | (_, Err(e)) => return vec![format!("The profile could not be re-read to check the change: {e}")],
        };
        for e in expect {
            match e {
                Expect::Value(id, want) => match stored.iter().find(|r| r.id == *id) {
                    Some(r) if r.effective() == want => {}
                    Some(r) => bad.push(format!("{} reads back as {} instead of {}", hex(*id), r.effective().raw_text(), want.raw_text())),
                    None => bad.push(format!("{} is not in the profile after saving", hex(*id))),
                },
                Expect::NotUser(id) => {
                    if stored.iter().any(|r| r.id == *id && r.is_user_value()) {
                        bad.push(format!("{} still has a user value", hex(*id)));
                    }
                }
                Expect::NoUserSettings => {
                    let n = stored.iter().filter(|r| r.is_user_value()).count();
                    if n > 0 {
                        bad.push(format!("{n} user value(s) remain in the profile"));
                    }
                }
                Expect::App(exe, present) => {
                    if apps.iter().any(|a| a.name.eq_ignore_ascii_case(exe)) != *present {
                        bad.push(format!("'{exe}' is {} the profile after saving", if *present { "missing from" } else { "still in" }));
                    }
                }
            }
        }
        bad
    }

    /// Apply changes to one profile: snapshot, apply, save, read back, record.
    pub fn apply(&self, profile: &str, changes: &[Change], label: Option<&str>) -> anyhow::Result<ApplyOutcome> {
        if changes.is_empty() {
            anyhow::bail!("There is nothing to apply");
        }
        let _guard = self.lock.lock();
        let driver = self.backend.status().unwrap_or_default();
        let mut s = self.backend.open().map_err(drs)?;
        let global = s.global_profile_name().map_err(drs)?;
        let p = s.profile(profile).map_err(drs)?.ok_or_else(|| anyhow::anyhow!("The NVIDIA driver has no profile named '{profile}'"))?;
        let (lines, expect) = self.plan_in(s.as_mut(), &p, &global, changes)?;
        let before = self.snapshot(s.as_mut(), &p.name).map_err(drs)?;
        let entry = self.store.create(label.unwrap_or(&format!("Before editing {}", p.name)), BackupReason::Edit, vec![before], Vec::new(), lines.clone(), driver.version.clone())?;

        let result = Self::run_changes(s.as_mut(), &p, changes).and_then(|_| s.save());
        drop(s);
        if let Err(e) = result {
            // Nothing was saved to the driver; the session is discarded.
            let _ = self.store.update(&entry.id, |x| x.note = Some(format!("Not applied: {e}")));
            log::warn!("NVIDIA profile '{}' was not changed: {e}", p.name);
            anyhow::bail!("{e}. Nothing was changed in the driver.");
        }
        self.invalidate();
        let mismatches = self.verify(&p.name, &expect);
        let verified = mismatches.is_empty();
        let _ = self.store.update(&entry.id, |x| {
            x.verified = Some(verified);
            if !verified {
                x.note = Some(mismatches.join("; "));
            }
        });
        log::info!("NVIDIA profile '{}': {} change(s) saved, verified={verified}, snapshot {}", p.name, lines.len(), entry.id);
        Ok(ApplyOutcome { backup_id: entry.id, changes: lines, verified, mismatches })
    }

    /// Create an empty profile, optionally with one executable.
    pub fn create_profile(&self, name: &str, exe: Option<&str>) -> anyhow::Result<ApplyOutcome> {
        let name = name.trim();
        if name.is_empty() {
            anyhow::bail!("Give the profile a name");
        }
        if name.encode_utf16().count() > MAX_STRING_UNITS || name.chars().any(|c| c.is_control()) {
            anyhow::bail!("That profile name is not usable");
        }
        let exe = exe.map(str::trim).filter(|e| !e.is_empty());
        if let Some(e) = exe {
            nip::check_executable(e).map_err(|e| anyhow::anyhow!(e))?;
        }
        let _guard = self.lock.lock();
        let mut s = self.backend.open().map_err(drs)?;
        if s.profile(name).map_err(drs)?.is_some() {
            anyhow::bail!("A profile named '{name}' already exists");
        }
        if let Some(e) = exe {
            if let Some(owner) = s.find_app_profile(e).map_err(drs)? {
                anyhow::bail!("'{e}' already belongs to the profile '{owner}'");
            }
        }
        let mut lines = vec![DiffLine { profile: name.into(), setting_id: None, what: "Profile".into(), before: "does not exist".into(), after: "created".into(), note: None }];
        if let Some(e) = exe {
            lines.push(DiffLine { profile: name.into(), setting_id: None, what: "Executable".into(), before: "not in this profile".into(), after: e.into(), note: None });
        }
        let entry = self.store.create(&format!("Before creating {name}"), BackupReason::Edit, Vec::new(), vec![name.to_string()], lines.clone(), self.backend.status().ok().and_then(|d| d.version))?;
        let result = s.create_profile(name).and_then(|_| match exe {
            Some(e) => s.add_app(name, e, ""),
            None => Ok(()),
        });
        let result = result.and_then(|_| s.save());
        drop(s);
        if let Err(e) = result {
            let _ = self.store.delete(&entry.id);
            anyhow::bail!("{e}. Nothing was changed in the driver.");
        }
        self.invalidate();
        let mut mismatches = Vec::new();
        match self.backend.open().and_then(|mut s| s.profile(name)) {
            Ok(Some(_)) => {}
            _ => mismatches.push(format!("The profile '{name}' is not in the driver after saving")),
        }
        if let Some(e) = exe {
            mismatches.extend(self.verify(name, &[Expect::App(e.to_string(), true)]));
        }
        let verified = mismatches.is_empty();
        let _ = self.store.update(&entry.id, |x| x.verified = Some(verified));
        Ok(ApplyOutcome { backup_id: entry.id, changes: lines, verified, mismatches })
    }

    /// Delete a profile the user made. NVIDIA's own profiles are never deleted.
    pub fn delete_profile(&self, name: &str) -> anyhow::Result<ApplyOutcome> {
        let _guard = self.lock.lock();
        let mut s = self.backend.open().map_err(drs)?;
        let global = s.global_profile_name().map_err(drs)?;
        let p = s.profile(name).map_err(drs)?.ok_or_else(|| anyhow::anyhow!("The NVIDIA driver has no profile named '{name}'"))?;
        if p.is_predefined || p.name.eq_ignore_ascii_case(&global) {
            anyhow::bail!("'{}' is one of NVIDIA's own profiles. It can be restored to its defaults, not deleted.", p.name);
        }
        let before = self.snapshot(s.as_mut(), &p.name).map_err(drs)?;
        let lines = vec![DiffLine { profile: p.name.clone(), setting_id: None, what: "Profile".into(), before: format!("{} setting(s), {} executable(s)", before.settings.len(), before.executables.len()), after: "deleted".into(), note: None }];
        let entry = self.store.create(&format!("Before deleting {}", p.name), BackupReason::Delete, vec![before], Vec::new(), lines.clone(), self.backend.status().ok().and_then(|d| d.version))?;
        let result = s.delete_profile(&p.name).and_then(|_| s.save());
        drop(s);
        if let Err(e) = result {
            let _ = self.store.update(&entry.id, |x| x.note = Some(format!("Not applied: {e}")));
            anyhow::bail!("{e}. Nothing was changed in the driver.");
        }
        self.invalidate();
        let gone = matches!(self.backend.open().and_then(|mut s| s.profile(&p.name)), Ok(None));
        let _ = self.store.update(&entry.id, |x| x.verified = Some(gone));
        Ok(ApplyOutcome { backup_id: entry.id, changes: lines, verified: gone, mismatches: if gone { Vec::new() } else { vec![format!("'{}' is still in the driver", p.name)] } })
    }

    // ── export ───────────────────────────────────────────────────────────

    /// The named profiles as `.nip` bytes (executables plus the settings the user changed).
    pub fn export(&self, names: &[String]) -> anyhow::Result<Vec<u8>> {
        if names.is_empty() {
            anyhow::bail!("Choose at least one profile to export");
        }
        let mut s = self.backend.open().map_err(drs)?;
        let mut profiles = Vec::new();
        for n in names {
            let p = s.profile(n).map_err(drs)?.ok_or_else(|| anyhow::anyhow!("The NVIDIA driver has no profile named '{n}'"))?;
            profiles.push(self.snapshot(s.as_mut(), &p.name).map_err(drs)?);
        }
        Ok(nip::write(&NipFile::new(profiles)))
    }

    /// Names of every profile the user changed (what Profile Inspector's "export customized" covers).
    pub fn customised_names(&self) -> anyhow::Result<Vec<String>> {
        Ok(self.list(false)?.into_iter().filter(|p| p.is_customised()).map(|p| p.name).collect())
    }

    pub fn export_to(&self, path: &std::path::Path, names: &[String]) -> anyhow::Result<usize> {
        check_output_path(path, "nip")?;
        let bytes = self.export(names)?;
        crate::frameforge::write_atomic(path, &bytes)?;
        log::info!("Exported {} NVIDIA profile(s) to a .nip file", names.len());
        Ok(names.len())
    }

    /// Write NVIDIA's own dump of the whole profile database. This is the
    /// driver's backup format, not a `.nip` file.
    pub fn save_driver_database(&self, path: &std::path::Path) -> anyhow::Result<()> {
        check_output_path(path, "nvdrs")?;
        let mut s = self.backend.open().map_err(drs)?;
        s.save_to_file(path).map_err(drs)
    }

    // ── import ───────────────────────────────────────────────────────────

    fn preview_setting(&self, m: &Meta, x: &NipSetting, live: Option<&RawSetting>) -> PreviewSetting {
        let info = m.describe(x.id);
        let parsed = x.value();
        let mut problem = parsed.as_ref().err().cloned();
        if x.id == 0 {
            problem = Some("setting id 0 is not a driver setting".into());
        }
        if let (Ok(v), Some(r)) = (&parsed, live) {
            if r.current.kind() != v.kind() {
                problem = Some(format!("the driver holds a {} value for this setting; the file has a {} value", r.current.kind().label(), v.kind().label()));
            }
        }
        let name = if !info.name.is_empty() {
            info.name.clone()
        } else {
            x.name_info.clone().filter(|n| !n.trim().is_empty()).or_else(|| self.backend.setting_name(x.id)).unwrap_or_default()
        };
        PreviewSetting {
            id: x.id,
            id_hex: hex(x.id),
            name_source: if !info.name.is_empty() { info.name_source } else { NameSource::None },
            name,
            documented: info.documented || self.exposed().contains(&x.id),
            value_type: x.value_type.clone(),
            raw_value: x.raw_value.clone(),
            label: parsed.as_ref().ok().and_then(|v| v.as_u64()).and_then(|n| info.label_for(n)).map(String::from),
            same_as_live: matches!((&parsed, live), (Ok(v), Some(r)) if r.effective() == v),
            live: live.map(|r| text_of(&info, r.effective())),
            problem,
            options: info.options,
        }
    }

    /// Read a `.nip` file and show what it holds next to the live driver
    /// state. Works without an NVIDIA driver (then there is nothing live to compare with).
    pub fn import_preview(&self, bytes: &[u8]) -> anyhow::Result<ImportPreview> {
        let file = nip::parse(bytes).map_err(|e| anyhow::anyhow!(e))?;
        Ok(self.preview(file))
    }

    pub fn preview(&self, file: NipFile) -> ImportPreview {
        let merged = nip::merge_duplicates(&file.profiles);
        let mut session = self.backend.open();
        let driver_reason = session.as_ref().err().map(|e| e.to_string());
        let global = session.as_mut().ok().and_then(|s| s.global_profile_name().ok()).unwrap_or_default();
        let mut warnings = file.warnings.clone();
        let mut profiles = Vec::new();
        for p in &merged {
            let mut live_settings: Vec<RawSetting> = Vec::new();
            let mut exists = false;
            let mut kind = None;
            let mut conflicts = Vec::new();
            if let Ok(s) = session.as_mut() {
                if let Ok(Some(raw)) = s.profile(&p.name) {
                    exists = true;
                    kind = Some(if raw.name.eq_ignore_ascii_case(&global) {
                        ProfileKind::Global
                    } else if raw.is_predefined {
                        ProfileKind::Predefined
                    } else {
                        ProfileKind::User
                    });
                    live_settings = s.settings(&raw.name).unwrap_or_default();
                }
                for exe in &p.executables {
                    if let Ok(Some(owner)) = s.find_app_profile(exe) {
                        if !owner.eq_ignore_ascii_case(&p.name) {
                            conflicts.push((exe.clone(), owner));
                        }
                    }
                }
            }
            let (settings, live_only) = self.with_meta(|m| {
                let settings: Vec<PreviewSetting> = p.settings.iter().map(|x| self.preview_setting(m, x, live_settings.iter().find(|r| r.id == x.id))).collect();
                let live_only: Vec<PreviewSetting> = live_settings
                    .iter()
                    .filter(|r| r.is_user_value() && !p.settings.iter().any(|x| x.id == r.id))
                    .map(|r| self.preview_setting(m, &NipSetting::new(r.id, &r.name, &r.current), Some(r)))
                    .collect();
                (settings, live_only)
            });
            if settings.iter().any(|x| x.problem.is_some()) {
                warnings.push(format!("'{}' has entries that cannot be applied. They stay in the file and are skipped on import.", p.name));
            }
            profiles.push(PreviewProfile { name: p.name.clone(), exists, kind, executables: p.executables.clone(), conflicts, settings, live_only, unknown_elements: p.extra.len() });
        }
        ImportPreview { file, profiles, warnings, driver_available: session.is_ok(), driver_reason }
    }

    /// Make one live profile match (replace) or include (merge) a `.nip` profile.
    fn import_one(&self, s: &mut dyn DrsSession, target: &NipProfile, mode: ImportMode, result: &mut ImportResult, expect: &mut Vec<Expect>) -> DrsResult<()> {
        let existing = s.profile(&target.name)?;
        let profile = match existing {
            Some(p) => {
                if mode == ImportMode::Replace {
                    Self::reset_profile(s, &p)?;
                }
                p
            }
            None => {
                s.create_profile(&target.name)?;
                result.created = true;
                RawProfile { name: target.name.clone(), ..Default::default() }
            }
        };
        let name = profile.name.clone();

        // Executables. NVIDIA's own assignments are never removed by an import.
        let apps = s.apps(&name)?;
        if mode == ImportMode::Replace {
            for a in apps.iter().filter(|a| !a.is_predefined && !target.executables.iter().any(|e| e.eq_ignore_ascii_case(&a.name))) {
                s.remove_app(&name, a)?;
                expect.push(Expect::App(a.name.clone(), false));
            }
        }
        for exe in &target.executables {
            if apps.iter().any(|a| a.name.eq_ignore_ascii_case(exe)) {
                continue;
            }
            if let Some(owner) = s.find_app_profile(exe)? {
                if !owner.eq_ignore_ascii_case(&name) {
                    return Err(DrsError::new(DrsCode::ExecutableInUse, "NvAPI_DRS_CreateApplication", format!("'{exe}' already belongs to the profile '{owner}'")));
                }
                continue;
            }
            s.add_app(&name, exe, target.find_file_for(exe).unwrap_or(""))?;
            expect.push(Expect::App(exe.clone(), true));
        }

        // Settings.
        let stored = s.settings(&name)?;
        for x in &target.settings {
            let label = || self.with_meta(|m| m.get(x.id).map(|i| i.name.clone())).filter(|n| !n.is_empty()).or_else(|| x.name_info.clone()).unwrap_or_default();
            let value = match x.value() {
                Ok(v) if x.id != 0 => v,
                Ok(_) => {
                    result.skipped.push(SkippedSetting { id: x.id, name: label(), reason: "setting id 0 is not a driver setting".into() });
                    continue;
                }
                Err(reason) => {
                    result.skipped.push(SkippedSetting { id: x.id, name: label(), reason });
                    continue;
                }
            };
            let live = stored.iter().find(|r| r.id == x.id);
            if let Some(r) = live {
                if r.current.kind() != value.kind() {
                    result.skipped.push(SkippedSetting { id: x.id, name: label(), reason: format!("the driver holds a {} value for this setting", r.current.kind().label()) });
                    continue;
                }
                // Already NVIDIA's value: writing it again would only mark it as user-changed.
                if !r.is_user_value() && r.effective() == &value {
                    result.applied += 1;
                    expect.push(Expect::Value(x.id, value));
                    continue;
                }
            }
            match s.set_setting(&name, x.id, &value) {
                Ok(()) => {
                    result.applied += 1;
                    expect.push(Expect::Value(x.id, value));
                }
                // Same rule as Profile Inspector: a setting this driver does not know is skipped, not fatal.
                Err(e) if e.code == DrsCode::SettingNotFound => result.skipped.push(SkippedSetting { id: x.id, name: label(), reason: "this driver version does not know the setting".into() }),
                Err(e) if e.code == DrsCode::InvalidValue => result.skipped.push(SkippedSetting { id: x.id, name: label(), reason: e.detail.clone() }),
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Import profiles from a parsed (and possibly edited) `.nip` file.
    /// `only` limits the import to the named profiles; empty means all.
    pub fn import(&self, file: &NipFile, only: &[String], mode: ImportMode, label: &str) -> anyhow::Result<ImportOutcome> {
        // The file may have been edited in the UI: run it through the writer
        // and the reader again so every limit is enforced on what is applied.
        let checked = nip::parse(&nip::write(file)).map_err(|e| anyhow::anyhow!(e))?;
        let targets: Vec<NipProfile> = nip::merge_duplicates(&checked.profiles).into_iter().filter(|p| only.is_empty() || only.iter().any(|n| n.eq_ignore_ascii_case(&p.name))).collect();
        if targets.is_empty() {
            anyhow::bail!("None of the chosen profiles are in the file");
        }
        self.import_inner(targets, &[], mode, BackupReason::Import, label)
    }

    fn import_inner(&self, targets: Vec<NipProfile>, delete: &[String], mode: ImportMode, reason: BackupReason, label: &str) -> anyhow::Result<ImportOutcome> {
        let _guard = self.lock.lock();
        let driver = self.backend.status().unwrap_or_default();

        // 1. Snapshot the live state of everything that is about to change.
        let mut before = Vec::new();
        let mut created = Vec::new();
        let mut lines = Vec::new();
        {
            let mut s = self.backend.open().map_err(drs)?;
            for t in &targets {
                match s.profile(&t.name).map_err(drs)? {
                    Some(p) => {
                        let snap = self.snapshot(s.as_mut(), &p.name).map_err(drs)?;
                        let stored = s.settings(&p.name).map_err(drs)?;
                        self.with_meta(|m| {
                            for x in &t.settings {
                                let info = m.describe(x.id);
                                let live = stored.iter().find(|r| r.id == x.id);
                                if let Ok(v) = x.value() {
                                    if live.map(|r| r.effective() != &v).unwrap_or(true) {
                                        lines.push(DiffLine { profile: p.name.clone(), setting_id: Some(x.id), what: if info.name.is_empty() { x.name_info.clone().unwrap_or_default() } else { info.name.clone() }, before: live.map(|r| text_of(&info, r.effective())).unwrap_or_else(|| "not set".into()), after: text_of(&info, &v), note: None });
                                    }
                                }
                            }
                            if mode == ImportMode::Replace {
                                for r in stored.iter().filter(|r| r.is_user_value() && !t.settings.iter().any(|x| x.id == r.id)) {
                                    let info = m.describe(r.id);
                                    lines.push(DiffLine { profile: p.name.clone(), setting_id: Some(r.id), what: if info.name.is_empty() { r.name.clone() } else { info.name.clone() }, before: text_of(&info, &r.current), after: "restored (not in the file)".into(), note: None });
                                }
                            }
                        });
                        before.push(snap);
                    }
                    None => {
                        created.push(t.name.clone());
                        lines.push(DiffLine { profile: t.name.clone(), setting_id: None, what: "Profile".into(), before: "does not exist".into(), after: format!("created with {} setting(s)", t.settings.len()), note: None });
                    }
                }
            }
            for name in delete {
                if let Some(p) = s.profile(name).map_err(drs)? {
                    if !p.is_predefined {
                        before.push(self.snapshot(s.as_mut(), &p.name).map_err(drs)?);
                        lines.push(DiffLine { profile: p.name.clone(), setting_id: None, what: "Profile".into(), before: "exists".into(), after: "deleted".into(), note: None });
                    }
                }
            }
        }
        // 2. The snapshot is on disk before the driver is touched.
        let entry = self.store.create(label, reason, before, created, lines, driver.version.clone())?;

        // 3. One session per profile: a profile either lands whole or not at all.
        let mut results = Vec::new();
        let mut checks: Vec<(String, Vec<Expect>)> = Vec::new();
        for t in &targets {
            let mut r = ImportResult { profile: t.name.clone(), created: false, ok: false, error: None, applied: 0, skipped: Vec::new() };
            let mut expect = Vec::new();
            let outcome = self.backend.open().and_then(|mut s| {
                self.import_one(s.as_mut(), t, mode, &mut r, &mut expect)?;
                s.save()
            });
            match outcome {
                Ok(()) => {
                    r.ok = true;
                    checks.push((t.name.clone(), expect));
                }
                Err(e) => {
                    r.created = false;
                    r.applied = 0;
                    r.error = Some(format!("{e}. This profile was left as it was."));
                    log::warn!("NVIDIA profile '{}' was not imported: {e}", t.name);
                }
            }
            results.push(r);
        }
        let mut mismatches = Vec::new();
        for name in delete {
            let outcome = self.backend.open().and_then(|mut s| match s.profile(name)? {
                Some(p) if !p.is_predefined => {
                    s.delete_profile(&p.name)?;
                    s.save()
                }
                _ => Ok(()),
            });
            if let Err(e) = outcome {
                mismatches.push(format!("'{name}' could not be removed: {e}"));
            }
        }
        self.invalidate();

        // 4. Read everything back.
        for (name, expect) in &checks {
            mismatches.extend(self.verify(name, expect).into_iter().map(|m| format!("{name}: {m}")));
        }
        let all_ok = results.iter().all(|r| r.ok);
        let verified = mismatches.is_empty() && all_ok;
        let _ = self.store.update(&entry.id, |x| {
            x.verified = Some(verified);
            let mut notes: Vec<String> = results.iter().filter_map(|r| r.error.as_ref().map(|e| format!("{}: {e}", r.profile))).collect();
            notes.extend(mismatches.iter().cloned());
            if !notes.is_empty() {
                x.note = Some(notes.join("; "));
            }
        });
        log::info!("NVIDIA import '{label}': {} of {} profile(s) applied, verified={verified}, snapshot {}", results.iter().filter(|r| r.ok).count(), results.len(), entry.id);
        Ok(ImportOutcome { backup_id: entry.id, profiles: results, verified, mismatches })
    }

    // ── backups ──────────────────────────────────────────────────────────

    pub fn backups(&self) -> Vec<BackupEntry> {
        self.store.list()
    }

    pub fn backup_contents(&self, id: &str) -> anyhow::Result<ImportPreview> {
        let (_, file) = self.store.read(id)?;
        Ok(self.preview(file))
    }

    pub fn backup_path(&self, id: &str) -> Option<std::path::PathBuf> {
        self.store.path_of(id)
    }

    /// Save a named snapshot of profiles without changing anything.
    pub fn backup_now(&self, names: &[String], label: &str) -> anyhow::Result<BackupEntry> {
        if names.is_empty() {
            anyhow::bail!("Choose at least one profile to back up");
        }
        let label = label.trim();
        let mut s = self.backend.open().map_err(drs)?;
        let mut profiles = Vec::new();
        for n in names {
            let p = s.profile(n).map_err(drs)?.ok_or_else(|| anyhow::anyhow!("The NVIDIA driver has no profile named '{n}'"))?;
            profiles.push(self.snapshot(s.as_mut(), &p.name).map_err(drs)?);
        }
        let entry = self.store.create(if label.is_empty() { "Manual backup" } else { label }, BackupReason::Manual, profiles, Vec::new(), Vec::new(), self.backend.status().ok().and_then(|d| d.version))?;
        Ok(entry)
    }

    /// Put profiles back the way a snapshot recorded them. The current state
    /// is snapshotted first, so a restore can itself be undone.
    pub fn restore_backup(&self, id: &str) -> anyhow::Result<ImportOutcome> {
        let (entry, file) = self.store.read(id)?;
        if file.profiles.is_empty() && entry.created_profiles.is_empty() {
            anyhow::bail!("That backup holds no profiles");
        }
        self.import_inner(nip::merge_duplicates(&file.profiles), &entry.created_profiles, ImportMode::Replace, BackupReason::Restore, &format!("Before restoring \"{}\"", entry.label))
    }

    pub fn delete_backup(&self, id: &str) -> anyhow::Result<()> {
        self.store.delete(id)
    }

    /// Compare two `.nip` files, naming settings where a name is known.
    pub fn compare_files(&self, a: &[u8], b: &[u8]) -> anyhow::Result<nip::NipDiff> {
        let fa = nip::parse(a).map_err(|e| anyhow::anyhow!("First file: {e}"))?;
        let fb = nip::parse(b).map_err(|e| anyhow::anyhow!("Second file: {e}"))?;
        let mut diff = nip::compare(&fa.profiles, &fb.profiles);
        self.with_meta(|m| {
            for s in diff.profiles.iter_mut().flat_map(|p| p.settings.iter_mut()) {
                if let Some(info) = m.get(s.id).filter(|i| !i.name.is_empty()) {
                    s.name = info.name.clone();
                }
            }
        });
        Ok(diff)
    }
}

/// An output file chosen by the user: absolute, with the expected extension.
fn check_output_path(path: &std::path::Path, extension: &str) -> anyhow::Result<()> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        anyhow::bail!("Choose a full path for the file");
    }
    if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case(extension)) != Some(true) {
        anyhow::bail!("The file name must end in .{extension}");
    }
    Ok(())
}

/// Read `InternalSettingFlag=V0` markers from NVIDIA's text dump of the
/// profile database. Returns `None` when the dump is not text (some drivers
/// write a binary database), in which case encoded values cannot be told apart.
fn parse_internal_flags(bytes: &[u8]) -> Option<HashSet<(String, u32)>> {
    let text = if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        String::from_utf16_lossy(&rest.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>())
    } else {
        String::from_utf8(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes).to_vec()).ok()?
    };
    if !text.contains("Profile \"") {
        return None;
    }
    let mut out = HashSet::new();
    let mut profile = String::new();
    for line in text.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("Profile \"") {
            profile = rest.split('"').next().unwrap_or_default().to_lowercase();
        } else if t.contains("InternalSettingFlag=V0") {
            if let Some(idx) = t.find("ID_0x") {
                let hex_digits: String = t[idx + 5..].chars().take_while(|c| c.is_ascii_hexdigit()).collect();
                if let Ok(id) = u32::from_str_radix(&hex_digits, 16) {
                    out.insert((profile.clone(), id));
                }
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests;
