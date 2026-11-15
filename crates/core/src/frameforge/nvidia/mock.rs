//! An in-memory driver profile database with the same behaviour the DRS
//! layer relies on: sessions work on a copy, nothing is stored until
//! `save`, predefined values sit under user values, and settings a profile
//! does not store are inherited from the global profile.
//!
//! Tests use it so they never touch a real driver. It can also be told to
//! fail a named operation, or to "lose" saves, to exercise error paths.

use super::types::*;
use parking_lot::Mutex;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct MockProfile {
    pub name: String,
    pub predefined: bool,
    pub apps: Vec<RawApp>,
    /// NVIDIA's own values for this profile.
    pub predefined_settings: BTreeMap<u32, Value>,
    /// Values the user set on top.
    pub user_settings: BTreeMap<u32, Value>,
}

#[derive(Debug, Clone, Default)]
pub struct MockDb {
    pub profiles: Vec<MockProfile>,
    pub global: String,
    /// Settings the "driver" lists: id → (name, kind, default, allowed values).
    pub available: BTreeMap<u32, (String, AvailableValues)>,
    /// Ids the driver refuses with "setting not found".
    pub unknown_to_driver: HashSet<u32>,
    pub driver: Option<DriverInfo>,
    pub unavailable_reason: Option<String>,
    /// An operation with this name fails (e.g. "set_setting", "save").
    pub fail_on: Option<String>,
    /// `save` reports success but stores nothing.
    pub lose_saves: bool,
    pub saves: u32,
}

impl MockDb {
    fn find(&self, name: &str) -> Option<&MockProfile> {
        let n = if name.is_empty() { self.global.as_str() } else { name };
        self.profiles.iter().find(|p| p.name.eq_ignore_ascii_case(n))
    }
    fn find_mut(&mut self, name: &str) -> Option<&mut MockProfile> {
        let n = if name.is_empty() { self.global.clone() } else { name.to_string() };
        self.profiles.iter_mut().find(|p| p.name.eq_ignore_ascii_case(&n))
    }
}

#[derive(Clone, Default)]
pub struct MockDrs(pub Arc<Mutex<MockDb>>);

pub const VSYNC: u32 = 0x00A879CF;
pub const POWER_MODE: u32 = 0x1057EB71;
pub const PRERENDER: u32 = 0x007BA09E;
pub const TEXT_SETTING: u32 = 0x10AA0001;
pub const BINARY_SETTING: u32 = 0x10AA0002;

impl MockDrs {
    /// A small driver: a global profile, two predefined game profiles and one user-made profile.
    pub fn typical() -> Self {
        let dword = |default: u32, values: &[u32]| AvailableValues { kind: ValueKind::Dword, default: Some(Value::Dword(default)), values: values.iter().map(|v| Value::Dword(*v)).collect() };
        let mut db = MockDb { global: "Base Profile".into(), driver: Some(DriverInfo { version: Some("566.14".into()), branch: Some("r565_00".into()) }), ..Default::default() };
        db.available.insert(VSYNC, ("Vertical Sync".into(), dword(0x60925292, &[0x60925292, 0x08416747, 0x47814940])));
        db.available.insert(POWER_MODE, ("Power management mode".into(), dword(5, &[0, 1, 2, 3, 4, 5])));
        db.available.insert(PRERENDER, ("Maximum pre-rendered frames".into(), dword(0, &[0, 1, 2, 3, 4])));
        db.available.insert(TEXT_SETTING, ("Example text".into(), AvailableValues { kind: ValueKind::Wstring, default: Some(Value::Wstring(String::new())), values: vec![] }));
        db.available.insert(BINARY_SETTING, ("Example binary".into(), AvailableValues { kind: ValueKind::Binary, default: None, values: vec![] }));
        let app = |name: &str, predefined: bool| RawApp { name: name.into(), is_predefined: predefined, ..Default::default() };
        db.profiles = vec![
            MockProfile { name: "Base Profile".into(), predefined: true, predefined_settings: [(VSYNC, Value::Dword(0x60925292)), (POWER_MODE, Value::Dword(5))].into(), ..Default::default() },
            MockProfile {
                name: "Cyberpunk 2077".into(),
                predefined: true,
                apps: vec![app("cyberpunk2077.exe", true)],
                predefined_settings: [(PRERENDER, Value::Dword(1)), (0x00F00BA5, Value::Dword(0xCAFE))].into(),
                ..Default::default()
            },
            MockProfile {
                name: "Counter-Strike 2".into(),
                predefined: true,
                apps: vec![app("cs2.exe", true)],
                predefined_settings: [(PRERENDER, Value::Dword(1))].into(),
                user_settings: [(VSYNC, Value::Dword(0x08416747))].into(),
            },
            MockProfile { name: "My Indie Game".into(), predefined: false, apps: vec![app("indie.exe", false)], user_settings: [(POWER_MODE, Value::Dword(1)), (0xDEADBEEF, Value::Dword(7))].into(), ..Default::default() },
        ];
        Self(Arc::new(Mutex::new(db)))
    }

    pub fn without_driver(reason: &str) -> Self {
        Self(Arc::new(Mutex::new(MockDb { unavailable_reason: Some(reason.into()), ..Default::default() })))
    }

    pub fn user_value(&self, profile: &str, id: u32) -> Option<Value> {
        self.0.lock().find(profile).and_then(|p| p.user_settings.get(&id).cloned())
    }
}

impl DrsBackend for MockDrs {
    fn status(&self) -> Result<DriverInfo, String> {
        let db = self.0.lock();
        match &db.unavailable_reason {
            Some(r) => Err(r.clone()),
            None => Ok(db.driver.clone().unwrap_or_default()),
        }
    }
    fn open(&self) -> DrsResult<Box<dyn DrsSession + '_>> {
        let db = self.0.lock();
        if let Some(r) = &db.unavailable_reason {
            return Err(DrsError::unavailable(r.clone()));
        }
        if db.fail_on.as_deref() == Some("open") {
            return Err(DrsError::new(DrsCode::Other, "NvAPI_DRS_CreateSession", "mock failure"));
        }
        Ok(Box::new(MockSession { shared: self.0.clone(), work: db.clone() }))
    }
    fn available_setting_ids(&self) -> Vec<u32> {
        self.0.lock().available.keys().copied().collect()
    }
    fn setting_name(&self, id: u32) -> Option<String> {
        self.0.lock().available.get(&id).map(|x| x.0.clone())
    }
    fn available_values(&self, id: u32) -> Option<AvailableValues> {
        self.0.lock().available.get(&id).map(|x| x.1.clone())
    }
}

struct MockSession {
    shared: Arc<Mutex<MockDb>>,
    work: MockDb,
}

impl MockSession {
    fn fail(&self, op: &'static str) -> DrsResult<()> {
        if self.work.fail_on.as_deref() == Some(op) {
            return Err(DrsError::new(DrsCode::Other, op, "mock failure"));
        }
        Ok(())
    }
    fn need(&self, name: &str, op: &'static str) -> DrsResult<&MockProfile> {
        self.work.find(name).ok_or_else(|| DrsError::new(DrsCode::ProfileNotFound, op, format!("no profile named '{name}'")))
    }
    fn need_mut(&mut self, name: &str, op: &'static str) -> DrsResult<&mut MockProfile> {
        self.work.find_mut(name).ok_or_else(|| DrsError::new(DrsCode::ProfileNotFound, op, format!("no profile named '{name}'")))
    }
    fn raw(&self, p: &MockProfile, id: u32) -> Option<RawSetting> {
        let name = self.work.available.get(&id).map(|x| x.0.clone()).unwrap_or_default();
        let user = p.user_settings.get(&id);
        let pre = p.predefined_settings.get(&id);
        let (user, pre) = match (user, pre) {
            (None, None) => return None,
            x => x,
        };
        let zero = |v: &Value| match v.kind() {
            ValueKind::Dword => Value::Dword(0),
            ValueKind::Qword => Value::Qword(0),
            ValueKind::Wstring => Value::Wstring(String::new()),
            ValueKind::Ansi => Value::Ansi(String::new()),
            ValueKind::Binary => Value::Binary(Vec::new()),
        };
        Some(RawSetting {
            id,
            name,
            location: Location::Current,
            is_current_predefined: user.is_none(),
            is_predefined_valid: pre.is_some(),
            current: user.or(pre).cloned().expect("one of them is set"),
            predefined: pre.cloned().unwrap_or_else(|| zero(user.expect("user is set when pre is not"))),
            unknown_type: None,
        })
    }
}

impl DrsSession for MockSession {
    fn profiles(&mut self) -> DrsResult<Vec<RawProfile>> {
        self.fail("profiles")?;
        Ok(self.work.profiles.iter().map(|p| RawProfile { name: p.name.clone(), is_predefined: p.predefined, app_count: p.apps.len() as u32, setting_count: p.predefined_settings.keys().chain(p.user_settings.keys()).collect::<HashSet<_>>().len() as u32 }).collect())
    }
    fn global_profile_name(&mut self) -> DrsResult<String> {
        Ok(self.work.global.clone())
    }
    fn profile(&mut self, name: &str) -> DrsResult<Option<RawProfile>> {
        let n = if name.is_empty() { self.work.global.clone() } else { name.to_string() };
        Ok(self.profiles()?.into_iter().find(|p| p.name.eq_ignore_ascii_case(&n)))
    }
    fn apps(&mut self, profile: &str) -> DrsResult<Vec<RawApp>> {
        Ok(self.need(profile, "NvAPI_DRS_EnumApplications")?.apps.clone())
    }
    fn settings(&mut self, profile: &str) -> DrsResult<Vec<RawSetting>> {
        self.fail("settings")?;
        let p = self.need(profile, "NvAPI_DRS_EnumSettings")?;
        let ids: std::collections::BTreeSet<u32> = p.predefined_settings.keys().chain(p.user_settings.keys()).copied().collect();
        Ok(ids.into_iter().filter_map(|id| self.raw(p, id)).collect())
    }
    fn setting(&mut self, profile: &str, id: u32) -> DrsResult<Option<RawSetting>> {
        let p = self.need(profile, "NvAPI_DRS_GetSetting")?;
        if let Some(s) = self.raw(p, id) {
            return Ok(Some(s));
        }
        // Not stored in this profile: inherited from the global profile, then the driver default.
        if let Some(g) = self.work.find("").filter(|g| !g.name.eq_ignore_ascii_case(&p.name)).and_then(|g| self.raw(g, id)) {
            return Ok(Some(RawSetting { location: Location::Global, ..g }));
        }
        Ok(self.work.available.get(&id).and_then(|(name, av)| {
            av.default.clone().map(|d| RawSetting { id, name: name.clone(), location: Location::Default, is_current_predefined: true, is_predefined_valid: true, current: d.clone(), predefined: d, unknown_type: None })
        }))
    }
    fn set_setting(&mut self, profile: &str, id: u32, value: &Value) -> DrsResult<()> {
        const OP: &str = "NvAPI_DRS_SetSetting";
        self.fail("set_setting")?;
        if self.work.unknown_to_driver.contains(&id) {
            return Err(DrsError::new(DrsCode::SettingNotFound, OP, "the driver does not know this setting"));
        }
        if let Some((_, av)) = self.work.available.get(&id) {
            if av.kind != value.kind() {
                return Err(DrsError::new(DrsCode::InvalidValue, OP, "the value type does not match the setting"));
            }
        }
        value.check().map_err(|e| DrsError::new(DrsCode::InvalidValue, OP, e))?;
        self.need_mut(profile, OP)?.user_settings.insert(id, value.clone());
        Ok(())
    }
    fn delete_setting(&mut self, profile: &str, id: u32) -> DrsResult<()> {
        const OP: &str = "NvAPI_DRS_DeleteProfileSetting";
        self.fail("delete_setting")?;
        match self.need_mut(profile, OP)?.user_settings.remove(&id) {
            Some(_) => Ok(()),
            None => Err(DrsError::new(DrsCode::SettingNotFound, OP, "the profile has no user value for this setting")),
        }
    }
    fn restore_setting_default(&mut self, profile: &str, id: u32) -> DrsResult<()> {
        const OP: &str = "NvAPI_DRS_RestoreProfileDefaultSetting";
        self.fail("restore_setting_default")?;
        let p = self.need_mut(profile, OP)?;
        if !p.predefined_settings.contains_key(&id) && !p.user_settings.contains_key(&id) {
            return Err(DrsError::new(DrsCode::SettingNotFound, OP, "the profile does not hold this setting"));
        }
        p.user_settings.remove(&id);
        Ok(())
    }
    fn restore_profile_default(&mut self, profile: &str) -> DrsResult<()> {
        const OP: &str = "NvAPI_DRS_RestoreProfileDefault";
        self.fail("restore_profile_default")?;
        let p = self.need_mut(profile, OP)?;
        if !p.predefined {
            return Err(DrsError::new(DrsCode::Other, OP, "only predefined profiles have defaults to restore"));
        }
        p.user_settings.clear();
        p.apps.retain(|a| a.is_predefined);
        Ok(())
    }
    fn create_profile(&mut self, name: &str) -> DrsResult<()> {
        const OP: &str = "NvAPI_DRS_CreateProfile";
        self.fail("create_profile")?;
        if self.work.find(name).is_some() {
            return Err(DrsError::new(DrsCode::ProfileNameInUse, OP, format!("a profile named '{name}' already exists")));
        }
        self.work.profiles.push(MockProfile { name: name.into(), ..Default::default() });
        Ok(())
    }
    fn delete_profile(&mut self, name: &str) -> DrsResult<()> {
        const OP: &str = "NvAPI_DRS_DeleteProfile";
        self.fail("delete_profile")?;
        self.need(name, OP)?;
        self.work.profiles.retain(|p| !p.name.eq_ignore_ascii_case(name));
        Ok(())
    }
    fn add_app(&mut self, profile: &str, exe: &str, file_in_folder: &str) -> DrsResult<()> {
        const OP: &str = "NvAPI_DRS_CreateApplication";
        self.fail("add_app")?;
        if let Some(owner) = self.work.profiles.iter().find(|p| p.apps.iter().any(|a| a.name.eq_ignore_ascii_case(exe))) {
            return Err(DrsError::new(DrsCode::ExecutableInUse, OP, format!("'{exe}' already belongs to the profile '{}'", owner.name)));
        }
        self.need_mut(profile, OP)?.apps.push(RawApp { name: exe.to_lowercase(), file_in_folder: file_in_folder.into(), ..Default::default() });
        Ok(())
    }
    fn remove_app(&mut self, profile: &str, app: &RawApp) -> DrsResult<()> {
        const OP: &str = "NvAPI_DRS_DeleteApplicationEx";
        self.fail("remove_app")?;
        let p = self.need_mut(profile, OP)?;
        let before = p.apps.len();
        p.apps.retain(|a| !a.name.eq_ignore_ascii_case(&app.name));
        if p.apps.len() == before {
            return Err(DrsError::new(DrsCode::Other, OP, format!("'{}' is not part of that profile", app.name)));
        }
        Ok(())
    }
    fn find_app_profile(&mut self, exe: &str) -> DrsResult<Option<String>> {
        Ok(self.work.profiles.iter().find(|p| p.apps.iter().any(|a| a.name.eq_ignore_ascii_case(exe))).map(|p| p.name.clone()))
    }
    fn save(&mut self) -> DrsResult<()> {
        self.fail("save")?;
        let mut shared = self.shared.lock();
        shared.saves += 1;
        if !shared.lose_saves {
            let (saves, fail_on, lose) = (shared.saves, shared.fail_on.clone(), shared.lose_saves);
            *shared = self.work.clone();
            shared.saves = saves;
            shared.fail_on = fail_on;
            shared.lose_saves = lose;
        }
        Ok(())
    }
    fn save_to_file(&mut self, path: &std::path::Path) -> DrsResult<()> {
        self.fail("save_to_file")?;
        // The same shape as NVIDIA's text dump, enough for the internal-setting scan.
        let mut out = String::new();
        for p in &self.work.profiles {
            out.push_str(&format!("Profile \"{}\"\n", p.name));
            for (id, v) in &p.predefined_settings {
                let internal = if *id == 0x00F00BA5 { " InternalSettingFlag=V0" } else { "" };
                out.push_str(&format!("    Setting ID_0x{id:08x} = {}{internal}\n", v.raw_text()));
            }
            out.push_str("EndProfile\n");
        }
        std::fs::write(path, out).map_err(|e| DrsError::new(DrsCode::Other, "NvAPI_DRS_SaveSettingsToFile", e.to_string()))
    }
}
