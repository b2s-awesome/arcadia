//! The real driver-settings backend: NVAPI's documented `NvAPI_DRS_*`
//! functions, resolved at run time from the installed driver.
//!
//! Reference: <https://docs.nvidia.com/nvapi/group__drsapi.html> and
//! `nvapi.h` in NVIDIA's SDK. Functions used:
//! `CreateSession`, `LoadSettings`, `SaveSettings`, `SaveSettingsToFile`,
//! `DestroySession`, `EnumProfiles`, `GetProfileInfo`, `FindProfileByName`,
//! `GetCurrentGlobalProfile`, `CreateProfile`, `DeleteProfile`,
//! `EnumApplications`, `FindApplicationByName`, `CreateApplication`,
//! `DeleteApplicationEx`, `EnumSettings`, `GetSetting`, `SetSetting`,
//! `DeleteProfileSetting`, `RestoreProfileDefault`,
//! `RestoreProfileDefaultSetting`, `EnumAvailableSettingIds`,
//! `EnumAvailableSettingValues`, `GetSettingNameFromId`.
//!
//! All `unsafe` for DRS lives in this file. The conversions between NVAPI's
//! structures and [`Value`] are plain functions with tests that run on any
//! platform; the calls themselves can only run on Windows with an NVIDIA
//! driver and are not exercised by the automated tests.

use super::types::*;
use crate::frameforge::ffi::nvapi::{self as nv, shared, NvApi, Status, NVDRS_APPLICATION_VER_V4, NVDRS_PROFILE_VER, NVDRS_SETTING_VALUES_VER, NVDRS_SETTING_VER};
use crate::frameforge::ffi::nvapi_types::*;
use crate::frameforge::ffi::{string_to_wide, wide_to_string};
use std::sync::Arc;

const NVAPI_EXECUTABLE_NOT_FOUND: Status = -166;
const NVAPI_DATATYPE_MISMATCH: Status = -168;
const NVAPI_SETTING_SIZE_TOO_LARGE: Status = -161;
/// Upper bound for `EnumAvailableSettingIds` (the driver lists a few hundred).
const MAX_SETTING_IDS: usize = 2048;
/// Upper bound on profiles read in one enumeration, as a guard against a driver that never ends it.
const MAX_PROFILES: u32 = 100_000;

/// Size of the value unions in `NVDRS_SETTING` (a 4-byte length plus 4096 bytes).
const UNION_BYTES: usize = std::mem::size_of::<NVDRS_BINARY_SETTING>();

fn zeroed<T: Copy>() -> T {
    // SAFETY: only used for NVAPI's plain-data C structures, for which all-zero is a valid value.
    unsafe { std::mem::zeroed() }
}

/// View one of the value unions as bytes.
fn union_bytes<U>(u: &U) -> &[u8] {
    debug_assert!(std::mem::size_of::<U>() >= UNION_BYTES);
    // SAFETY: the unions are plain data of at least UNION_BYTES bytes, fully
    // initialised (they come from zeroed structures filled in by the driver).
    unsafe { std::slice::from_raw_parts(u as *const U as *const u8, UNION_BYTES) }
}

fn union_bytes_mut<U>(u: &mut U) -> &mut [u8] {
    debug_assert!(std::mem::size_of::<U>() >= UNION_BYTES);
    // SAFETY: as above; any byte pattern is valid for these unions.
    unsafe { std::slice::from_raw_parts_mut(u as *mut U as *mut u8, UNION_BYTES) }
}

/// Decode a value union for the given `NVDRS_SETTING_TYPE`. An unknown type
/// code comes back as the first raw bytes plus the code.
pub(crate) fn decode_value(kind: i32, bytes: &[u8]) -> (Value, Option<i32>) {
    match kind {
        NVDRS_DWORD_TYPE => (Value::Dword(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])), None),
        NVDRS_QWORD_TYPE => (Value::Qword(u64::from_le_bytes(bytes[..8].try_into().expect("8 bytes"))), None),
        NVDRS_BINARY_TYPE => {
            let len = (u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize).min(MAX_BINARY_BYTES).min(bytes.len() - 4);
            (Value::Binary(bytes[4..4 + len].to_vec()), None)
        }
        NVDRS_WSTRING_TYPE => {
            let units: Vec<u16> = bytes[..4096].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|u| *u != 0).collect();
            (Value::Wstring(String::from_utf16_lossy(&units)), None)
        }
        NVDRS_STRING_TYPE => {
            let end = bytes[..4096].iter().position(|b| *b == 0).unwrap_or(4096);
            (Value::Ansi(bytes[..end].iter().map(|b| if b.is_ascii() { *b as char } else { '?' }).collect()), None)
        }
        other => (Value::Binary(bytes[..64].to_vec()), Some(other)),
    }
}

/// Encode a value into a zeroed value union. Returns the `NVDRS_SETTING_TYPE`.
pub(crate) fn encode_value(value: &Value, bytes: &mut [u8]) -> Result<i32, String> {
    value.check()?;
    bytes.fill(0);
    Ok(match value {
        Value::Dword(v) => {
            bytes[..4].copy_from_slice(&v.to_le_bytes());
            NVDRS_DWORD_TYPE
        }
        Value::Qword(v) => {
            bytes[..8].copy_from_slice(&v.to_le_bytes());
            NVDRS_QWORD_TYPE
        }
        Value::Binary(b) => {
            bytes[..4].copy_from_slice(&(b.len() as u32).to_le_bytes());
            bytes[4..4 + b.len()].copy_from_slice(b);
            NVDRS_BINARY_TYPE
        }
        Value::Wstring(s) => {
            for (i, u) in s.encode_utf16().enumerate() {
                bytes[i * 2..i * 2 + 2].copy_from_slice(&u.to_le_bytes());
            }
            NVDRS_WSTRING_TYPE
        }
        Value::Ansi(s) => {
            bytes[..s.len()].copy_from_slice(s.as_bytes());
            NVDRS_STRING_TYPE
        }
    })
}

pub(crate) fn raw_setting(s: &NVDRS_SETTING_V1) -> RawSetting {
    let kind = s.settingType;
    let (current, unknown) = decode_value(kind, union_bytes(&s.__bindgen_anon_2));
    let (predefined, _) = decode_value(kind, union_bytes(&s.__bindgen_anon_1));
    RawSetting {
        id: s.settingId,
        name: wide_to_string(&s.settingName),
        location: match s.settingLocation {
            NVDRS_CURRENT_PROFILE_LOCATION => Location::Current,
            NVDRS_GLOBAL_PROFILE_LOCATION => Location::Global,
            NVDRS_BASE_PROFILE_LOCATION => Location::Base,
            _ => Location::Default,
        },
        is_current_predefined: s.isCurrentPredefined != 0,
        is_predefined_valid: s.isPredefinedValid != 0,
        current,
        predefined,
        unknown_type: unknown,
    }
}

pub(crate) fn new_setting(id: u32, value: &Value) -> Result<Box<NVDRS_SETTING_V1>, String> {
    let mut s: Box<NVDRS_SETTING_V1> = Box::new(zeroed());
    s.version = NVDRS_SETTING_VER;
    s.settingId = id;
    s.settingType = encode_value(value, union_bytes_mut(&mut s.__bindgen_anon_2))?;
    Ok(s)
}

fn wide(text: &str, op: &'static str) -> DrsResult<Box<NvAPI_UnicodeString>> {
    let mut buf: Box<NvAPI_UnicodeString> = Box::new([0u16; 2048]);
    if !string_to_wide(text, &mut buf[..]) {
        return Err(DrsError::new(DrsCode::InvalidValue, op, "the text is longer than the driver allows"));
    }
    Ok(buf)
}

fn code_of(status: Status) -> DrsCode {
    match status {
        nv::NVAPI_SETTING_NOT_FOUND => DrsCode::SettingNotFound,
        nv::NVAPI_PROFILE_NOT_FOUND => DrsCode::ProfileNotFound,
        nv::NVAPI_PROFILE_NAME_IN_USE => DrsCode::ProfileNameInUse,
        nv::NVAPI_EXECUTABLE_ALREADY_IN_USE => DrsCode::ExecutableInUse,
        nv::NVAPI_ACCESS_DENIED | nv::NVAPI_INVALID_USER_PRIVILEGE => DrsCode::AccessDenied,
        nv::NVAPI_NOT_SUPPORTED | nv::NVAPI_NO_IMPLEMENTATION => DrsCode::NotSupported,
        NVAPI_DATATYPE_MISMATCH | NVAPI_SETTING_SIZE_TOO_LARGE => DrsCode::InvalidValue,
        _ => DrsCode::Other,
    }
}

/// The NVAPI-backed [`DrsBackend`]. NVAPI is loaded on first use.
#[derive(Default)]
pub struct NvapiDrs;

impl NvapiDrs {
    pub fn new() -> Self {
        Self
    }
    fn api(&self) -> Result<Arc<NvApi>, String> {
        shared()
    }
}

fn check(api: &NvApi, op: &'static str, status: Status) -> DrsResult<()> {
    if status == nv::NVAPI_OK {
        Ok(())
    } else {
        Err(DrsError::new(code_of(status), op, api.error_message(status)))
    }
}

/// Resolve an optional NVAPI function or fail with "not implemented by this driver".
macro_rules! func {
    ($api:expr, $name:ident) => {
        $api.$name.ok_or_else(|| DrsError::new(DrsCode::NotSupported, concat!("NvAPI_", stringify!($name)), "this NVIDIA driver does not implement the function"))
    };
}

impl DrsBackend for NvapiDrs {
    fn status(&self) -> Result<DriverInfo, String> {
        let api = self.api()?;
        if api.DRS_CreateSession.is_none() || api.DRS_LoadSettings.is_none() || api.DRS_EnumProfiles.is_none() {
            return Err("This NVIDIA driver does not expose the driver-settings (DRS) functions".into());
        }
        let mut info = DriverInfo::default();
        if let Some(f) = api.SYS_GetDriverAndBranchVersion {
            let mut version = 0u32;
            let mut branch: NvAPI_ShortString = [0; 64];
            // SAFETY: both out-pointers are valid for the call; the branch buffer is the 64-char string NVAPI expects.
            if unsafe { f(&mut version, &mut branch) } == nv::NVAPI_OK {
                info.version = Some(format!("{}.{:02}", version / 100, version % 100));
                info.branch = Some(crate::frameforge::ffi::cchars_to_string(&branch)).filter(|b| !b.is_empty());
            }
        }
        Ok(info)
    }

    fn open(&self) -> DrsResult<Box<dyn DrsSession + '_>> {
        let api = self.api().map_err(DrsError::unavailable)?;
        let create = func!(api, DRS_CreateSession)?;
        let load = func!(api, DRS_LoadSettings)?;
        let mut handle: NvDRSSessionHandle = std::ptr::null_mut();
        // SAFETY: out-pointer is valid; on success the handle is owned by the Session below and destroyed in Drop.
        check(&api, "NvAPI_DRS_CreateSession", unsafe { create(&mut handle) })?;
        let session = Session { api, handle };
        // SAFETY: the handle was just created.
        check(&session.api, "NvAPI_DRS_LoadSettings", unsafe { load(session.handle) })?;
        Ok(Box::new(session))
    }

    fn available_setting_ids(&self) -> Vec<u32> {
        let Ok(api) = self.api() else { return Vec::new() };
        let Some(f) = api.DRS_EnumAvailableSettingIds else { return Vec::new() };
        let mut ids = vec![0u32; MAX_SETTING_IDS];
        let mut count = MAX_SETTING_IDS as u32;
        // SAFETY: `ids` has room for `count` entries; the driver writes at most that many and updates `count`.
        if unsafe { f(ids.as_mut_ptr(), &mut count) } != nv::NVAPI_OK {
            return Vec::new();
        }
        ids.truncate((count as usize).min(MAX_SETTING_IDS));
        ids
    }

    fn setting_name(&self, id: u32) -> Option<String> {
        let api = self.api().ok()?;
        let f = api.DRS_GetSettingNameFromId?;
        let mut buf: Box<NvAPI_UnicodeString> = Box::new([0u16; 2048]);
        // SAFETY: the buffer is the 2048-unit NvAPI_UnicodeString the function fills.
        (unsafe { f(id, &mut *buf) } == nv::NVAPI_OK).then(|| wide_to_string(&buf[..])).filter(|n| !n.trim().is_empty())
    }

    fn available_values(&self, id: u32) -> Option<AvailableValues> {
        // Driver 571.96 overflows on this id range (found by the Profile Inspector project); never ask for it.
        if (id & 0xFFFF_F000) == 0x10C7_D000 {
            return None;
        }
        let api = self.api().ok()?;
        let f = api.DRS_EnumAvailableSettingValues?;
        // 414 KB: keep it off the stack.
        // SAFETY: all-zero is a valid NVDRS_SETTING_VALUES.
        let mut values: Box<NVDRS_SETTING_VALUES> = unsafe { Box::new_zeroed().assume_init() };
        values.version = NVDRS_SETTING_VALUES_VER;
        let mut max = values.settingValues.len() as u32;
        // SAFETY: `values` is a correctly versioned structure with room for `max` entries.
        if unsafe { f(id, &mut max, &mut *values) } != nv::NVAPI_OK {
            return None;
        }
        let kind = values.settingType;
        let (default, unknown) = decode_value(kind, union_bytes(&values.__bindgen_anon_1));
        if unknown.is_some() {
            return None;
        }
        let n = (values.numSettingValues as usize).min(values.settingValues.len());
        let list = values.settingValues[..n].iter().map(|v| decode_value(kind, union_bytes(v)).0).collect();
        Some(AvailableValues { kind: default.kind(), default: Some(default), values: list })
    }
}

struct Session {
    api: Arc<NvApi>,
    handle: NvDRSSessionHandle,
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(destroy) = self.api.DRS_DestroySession {
            // SAFETY: the handle came from CreateSession and is destroyed exactly once.
            unsafe { destroy(self.handle) };
        }
    }
}

impl Session {
    /// Profile handle by name; `""` is the current global profile.
    fn handle_of(&self, name: &str, op: &'static str) -> DrsResult<NvDRSProfileHandle> {
        let mut h: NvDRSProfileHandle = std::ptr::null_mut();
        if name.is_empty() {
            let f = func!(self.api, DRS_GetCurrentGlobalProfile)?;
            // SAFETY: valid session handle and out-pointer.
            check(&self.api, "NvAPI_DRS_GetCurrentGlobalProfile", unsafe { f(self.handle, &mut h) })?;
        } else {
            let f = func!(self.api, DRS_FindProfileByName)?;
            let mut w = wide(name, op)?;
            // SAFETY: `w` is a NUL-terminated NvAPI_UnicodeString; out-pointer is valid.
            let st = unsafe { f(self.handle, &mut *w, &mut h) };
            if st != nv::NVAPI_OK {
                return Err(DrsError::new(code_of(st), op, if st == nv::NVAPI_PROFILE_NOT_FOUND { format!("no profile named '{name}'") } else { self.api.error_message(st) }));
            }
        }
        Ok(h)
    }

    fn info(&self, h: NvDRSProfileHandle) -> DrsResult<Box<NVDRS_PROFILE_V1>> {
        let f = func!(self.api, DRS_GetProfileInfo)?;
        let mut p: Box<NVDRS_PROFILE_V1> = Box::new(zeroed());
        p.version = NVDRS_PROFILE_VER;
        // SAFETY: `p` is a correctly versioned NVDRS_PROFILE the driver fills.
        check(&self.api, "NvAPI_DRS_GetProfileInfo", unsafe { f(self.handle, h, &mut *p) })?;
        Ok(p)
    }

    fn raw_profile(p: &NVDRS_PROFILE_V1) -> RawProfile {
        RawProfile { name: wide_to_string(&p.profileName), is_predefined: p.isPredefined != 0, app_count: p.numOfApps, setting_count: p.numOfSettings }
    }

    fn raw_apps(&self, h: NvDRSProfileHandle) -> DrsResult<Vec<NVDRS_APPLICATION_V4>> {
        let n = self.info(h)?.numOfApps as usize;
        if n == 0 {
            return Ok(Vec::new());
        }
        let f = func!(self.api, DRS_EnumApplications)?;
        let mut first: NVDRS_APPLICATION_V4 = zeroed();
        first.version = NVDRS_APPLICATION_VER_V4;
        let mut apps = vec![first; n];
        let mut count = n as u32;
        // SAFETY: `apps` holds `count` versioned structures; the driver fills at most that many.
        let st = unsafe { f(self.handle, h, 0, &mut count, apps.as_mut_ptr()) };
        if st == nv::NVAPI_END_ENUMERATION {
            return Ok(Vec::new());
        }
        check(&self.api, "NvAPI_DRS_EnumApplications", st)?;
        apps.truncate((count as usize).min(n));
        Ok(apps)
    }
}

fn raw_app(a: &NVDRS_APPLICATION_V4) -> RawApp {
    RawApp {
        name: wide_to_string(&a.appName),
        friendly_name: wide_to_string(&a.userFriendlyName),
        launcher: wide_to_string(&a.launcher),
        file_in_folder: wide_to_string(&a.fileInFolder),
        command_line: wide_to_string(&a.commandLine),
        is_predefined: a.isPredefined != 0,
    }
}

impl DrsSession for Session {
    fn profiles(&mut self) -> DrsResult<Vec<RawProfile>> {
        let f = func!(self.api, DRS_EnumProfiles)?;
        let mut out = Vec::new();
        for index in 0..MAX_PROFILES {
            let mut h: NvDRSProfileHandle = std::ptr::null_mut();
            // SAFETY: valid session handle and out-pointer.
            let st = unsafe { f(self.handle, index, &mut h) };
            if st == nv::NVAPI_END_ENUMERATION {
                break;
            }
            check(&self.api, "NvAPI_DRS_EnumProfiles", st)?;
            out.push(Self::raw_profile(&*self.info(h)?));
        }
        Ok(out)
    }

    fn global_profile_name(&mut self) -> DrsResult<String> {
        let h = self.handle_of("", "NvAPI_DRS_GetCurrentGlobalProfile")?;
        Ok(wide_to_string(&self.info(h)?.profileName))
    }

    fn profile(&mut self, name: &str) -> DrsResult<Option<RawProfile>> {
        match self.handle_of(name, "NvAPI_DRS_FindProfileByName") {
            Ok(h) => Ok(Some(Self::raw_profile(&*self.info(h)?))),
            Err(e) if e.code == DrsCode::ProfileNotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn apps(&mut self, profile: &str) -> DrsResult<Vec<RawApp>> {
        let h = self.handle_of(profile, "NvAPI_DRS_EnumApplications")?;
        Ok(self.raw_apps(h)?.iter().map(raw_app).collect())
    }

    fn settings(&mut self, profile: &str) -> DrsResult<Vec<RawSetting>> {
        const OP: &str = "NvAPI_DRS_EnumSettings";
        let h = self.handle_of(profile, OP)?;
        let n = self.info(h)?.numOfSettings as usize;
        if n == 0 {
            return Ok(Vec::new());
        }
        let f = func!(self.api, DRS_EnumSettings)?;
        let mut first: NVDRS_SETTING_V1 = zeroed();
        first.version = NVDRS_SETTING_VER;
        let mut buf = vec![first; n];
        let mut count = n as u32;
        // SAFETY: `buf` holds `count` versioned structures; the driver fills at most that many.
        let st = unsafe { f(self.handle, h, 0, &mut count, buf.as_mut_ptr()) };
        if st == nv::NVAPI_END_ENUMERATION {
            return Ok(Vec::new());
        }
        check(&self.api, OP, st)?;
        Ok(buf[..(count as usize).min(n)].iter().map(raw_setting).collect())
    }

    fn setting(&mut self, profile: &str, id: u32) -> DrsResult<Option<RawSetting>> {
        const OP: &str = "NvAPI_DRS_GetSetting";
        let h = self.handle_of(profile, OP)?;
        let f = func!(self.api, DRS_GetSetting)?;
        let mut s: Box<NVDRS_SETTING_V1> = Box::new(zeroed());
        s.version = NVDRS_SETTING_VER;
        // SAFETY: `s` is a correctly versioned NVDRS_SETTING the driver fills.
        let st = unsafe { f(self.handle, h, id, &mut *s) };
        if st == nv::NVAPI_SETTING_NOT_FOUND {
            return Ok(None);
        }
        check(&self.api, OP, st)?;
        Ok(Some(raw_setting(&s)))
    }

    fn set_setting(&mut self, profile: &str, id: u32, value: &Value) -> DrsResult<()> {
        const OP: &str = "NvAPI_DRS_SetSetting";
        let h = self.handle_of(profile, OP)?;
        let f = func!(self.api, DRS_SetSetting)?;
        let mut s = new_setting(id, value).map_err(|e| DrsError::new(DrsCode::InvalidValue, OP, e))?;
        // SAFETY: `s` is a fully initialised, versioned NVDRS_SETTING.
        check(&self.api, OP, unsafe { f(self.handle, h, &mut *s) })
    }

    fn delete_setting(&mut self, profile: &str, id: u32) -> DrsResult<()> {
        const OP: &str = "NvAPI_DRS_DeleteProfileSetting";
        let h = self.handle_of(profile, OP)?;
        let f = func!(self.api, DRS_DeleteProfileSetting)?;
        // SAFETY: valid session and profile handles.
        check(&self.api, OP, unsafe { f(self.handle, h, id) })
    }

    fn restore_setting_default(&mut self, profile: &str, id: u32) -> DrsResult<()> {
        const OP: &str = "NvAPI_DRS_RestoreProfileDefaultSetting";
        let h = self.handle_of(profile, OP)?;
        let f = func!(self.api, DRS_RestoreProfileDefaultSetting)?;
        // SAFETY: valid session and profile handles.
        check(&self.api, OP, unsafe { f(self.handle, h, id) })
    }

    fn restore_profile_default(&mut self, profile: &str) -> DrsResult<()> {
        const OP: &str = "NvAPI_DRS_RestoreProfileDefault";
        let h = self.handle_of(profile, OP)?;
        let f = func!(self.api, DRS_RestoreProfileDefault)?;
        // SAFETY: valid session and profile handles.
        check(&self.api, OP, unsafe { f(self.handle, h) })
    }

    fn create_profile(&mut self, name: &str) -> DrsResult<()> {
        const OP: &str = "NvAPI_DRS_CreateProfile";
        let f = func!(self.api, DRS_CreateProfile)?;
        let mut p: Box<NVDRS_PROFILE_V1> = Box::new(zeroed());
        p.version = NVDRS_PROFILE_VER;
        if !string_to_wide(name, &mut p.profileName) {
            return Err(DrsError::new(DrsCode::InvalidValue, OP, "the profile name is longer than the driver allows"));
        }
        // A profile for consumer (GeForce) GPUs, as NVIDIA's own tools create them.
        p.gpuSupport.set_geforce(1);
        let mut h: NvDRSProfileHandle = std::ptr::null_mut();
        // SAFETY: `p` is a fully initialised, versioned NVDRS_PROFILE; out-pointer is valid.
        check(&self.api, OP, unsafe { f(self.handle, &mut *p, &mut h) })
    }

    fn delete_profile(&mut self, name: &str) -> DrsResult<()> {
        const OP: &str = "NvAPI_DRS_DeleteProfile";
        if name.is_empty() {
            return Err(DrsError::new(DrsCode::InvalidValue, OP, "the global profile cannot be deleted"));
        }
        let h = self.handle_of(name, OP)?;
        let f = func!(self.api, DRS_DeleteProfile)?;
        // SAFETY: valid session and profile handles.
        check(&self.api, OP, unsafe { f(self.handle, h) })
    }

    fn add_app(&mut self, profile: &str, exe: &str, file_in_folder: &str) -> DrsResult<()> {
        const OP: &str = "NvAPI_DRS_CreateApplication";
        let h = self.handle_of(profile, OP)?;
        let f = func!(self.api, DRS_CreateApplication)?;
        let mut a: Box<NVDRS_APPLICATION_V4> = Box::new(zeroed());
        a.version = NVDRS_APPLICATION_VER_V4;
        if !string_to_wide(exe, &mut a.appName) || !string_to_wide(file_in_folder, &mut a.fileInFolder) {
            return Err(DrsError::new(DrsCode::InvalidValue, OP, "the executable name is longer than the driver allows"));
        }
        // SAFETY: `a` is a fully initialised, versioned NVDRS_APPLICATION.
        check(&self.api, OP, unsafe { f(self.handle, h, &mut *a) })
    }

    fn remove_app(&mut self, profile: &str, app: &RawApp) -> DrsResult<()> {
        const OP: &str = "NvAPI_DRS_DeleteApplicationEx";
        let h = self.handle_of(profile, OP)?;
        let f = func!(self.api, DRS_DeleteApplicationEx)?;
        // Hand the driver back the exact entry it enumerated.
        let mut apps = self.raw_apps(h)?;
        let Some(entry) = apps.iter_mut().find(|a| wide_to_string(&a.appName).eq_ignore_ascii_case(&app.name) && wide_to_string(&a.fileInFolder).eq_ignore_ascii_case(&app.file_in_folder)) else {
            return Err(DrsError::new(DrsCode::Other, OP, format!("'{}' is not part of that profile", app.name)));
        };
        // SAFETY: `entry` is a structure the driver itself filled in.
        check(&self.api, OP, unsafe { f(self.handle, h, entry) })
    }

    fn find_app_profile(&mut self, exe: &str) -> DrsResult<Option<String>> {
        const OP: &str = "NvAPI_DRS_FindApplicationByName";
        let f = func!(self.api, DRS_FindApplicationByName)?;
        let mut w = wide(exe, OP)?;
        let mut h: NvDRSProfileHandle = std::ptr::null_mut();
        let mut a: Box<NVDRS_APPLICATION_V4> = Box::new(zeroed());
        a.version = NVDRS_APPLICATION_VER_V4;
        // SAFETY: NUL-terminated name, valid out-pointers, versioned structure.
        let st = unsafe { f(self.handle, &mut *w, &mut h, &mut *a) };
        if st == NVAPI_EXECUTABLE_NOT_FOUND || st == nv::NVAPI_PROFILE_NOT_FOUND {
            return Ok(None);
        }
        check(&self.api, OP, st)?;
        Ok(Some(wide_to_string(&self.info(h)?.profileName)))
    }

    fn save(&mut self) -> DrsResult<()> {
        let f = func!(self.api, DRS_SaveSettings)?;
        // SAFETY: valid session handle.
        check(&self.api, "NvAPI_DRS_SaveSettings", unsafe { f(self.handle) })
    }

    fn save_to_file(&mut self, path: &std::path::Path) -> DrsResult<()> {
        const OP: &str = "NvAPI_DRS_SaveSettingsToFile";
        let f = func!(self.api, DRS_SaveSettingsToFile)?;
        let text = path.to_str().ok_or_else(|| DrsError::new(DrsCode::InvalidValue, OP, "the path is not valid Unicode"))?;
        let mut w = wide(text, OP)?;
        // SAFETY: valid session handle and NUL-terminated path.
        check(&self.api, OP, unsafe { f(self.handle, &mut *w) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(v: Value) {
        let s = new_setting(0x1234, &v).unwrap();
        let raw = raw_setting(&s);
        assert_eq!(raw.id, 0x1234);
        assert_eq!(raw.current, v);
        assert_eq!(raw.unknown_type, None);
    }

    #[test]
    fn values_survive_the_native_structure() {
        round_trip(Value::Dword(0));
        round_trip(Value::Dword(0xDEADBEEF));
        round_trip(Value::Qword(u64::MAX));
        round_trip(Value::Wstring("Grüße, 世界".into()));
        round_trip(Value::Wstring("x".repeat(MAX_STRING_UNITS)));
        round_trip(Value::Ansi("plain".into()));
        round_trip(Value::Binary(vec![]));
        round_trip(Value::Binary((0..=255).cycle().take(MAX_BINARY_BYTES).collect()));
    }

    #[test]
    fn oversized_values_never_reach_the_structure() {
        assert!(new_setting(1, &Value::Binary(vec![0; MAX_BINARY_BYTES + 1])).is_err());
        assert!(new_setting(1, &Value::Wstring("x".repeat(MAX_STRING_UNITS + 1))).is_err());
    }

    #[test]
    fn native_fields_map_to_the_model() {
        let mut s: Box<NVDRS_SETTING_V1> = Box::new(zeroed());
        s.settingId = 7;
        s.settingType = NVDRS_DWORD_TYPE;
        s.settingLocation = NVDRS_GLOBAL_PROFILE_LOCATION;
        s.isCurrentPredefined = 1;
        s.isPredefinedValid = 1;
        union_bytes_mut(&mut s.__bindgen_anon_1)[..4].copy_from_slice(&5u32.to_le_bytes());
        union_bytes_mut(&mut s.__bindgen_anon_2)[..4].copy_from_slice(&9u32.to_le_bytes());
        assert!(string_to_wide("Some setting", &mut s.settingName));
        let r = raw_setting(&s);
        assert_eq!((r.name.as_str(), r.location, r.is_current_predefined, r.is_predefined_valid), ("Some setting", Location::Global, true, true));
        assert_eq!((r.current.clone(), r.predefined.clone()), (Value::Dword(9), Value::Dword(5)));
        assert_eq!(r.effective(), &Value::Dword(5));
        assert!(!r.is_user_value());
    }

    #[test]
    fn a_lying_length_or_unknown_type_cannot_overrun() {
        let mut bytes = vec![0xAAu8; UNION_BYTES];
        bytes[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        let (v, unknown) = decode_value(NVDRS_BINARY_TYPE, &bytes);
        assert_eq!((v, unknown), (Value::Binary(vec![0xAA; MAX_BINARY_BYTES]), None));
        // No terminator anywhere: strings stop at the end of the field.
        let (v, _) = decode_value(NVDRS_WSTRING_TYPE, &bytes);
        assert!(matches!(v, Value::Wstring(s) if s.encode_utf16().count() == 2048));
        let (v, unknown) = decode_value(99, &bytes);
        assert_eq!(unknown, Some(99));
        assert!(matches!(v, Value::Binary(b) if b.len() == 64));
    }

    #[test]
    fn status_codes_map_to_the_errors_callers_handle() {
        assert_eq!(code_of(nv::NVAPI_SETTING_NOT_FOUND), DrsCode::SettingNotFound);
        assert_eq!(code_of(nv::NVAPI_EXECUTABLE_ALREADY_IN_USE), DrsCode::ExecutableInUse);
        assert_eq!(code_of(nv::NVAPI_INVALID_USER_PRIVILEGE), DrsCode::AccessDenied);
        assert_eq!(code_of(nv::NVAPI_PROFILE_NAME_IN_USE), DrsCode::ProfileNameInUse);
        assert_eq!(code_of(nv::NVAPI_ERROR), DrsCode::Other);
    }

    /// Without the NVIDIA driver (every CI and non-Windows machine) the
    /// backend reports why instead of failing later.
    #[test]
    fn no_driver_means_unavailable_with_a_reason() {
        let b = NvapiDrs::new();
        if let Err(reason) = b.status() {
            assert!(!reason.is_empty());
            assert!(matches!(b.open(), Err(e) if e.code == DrsCode::Unavailable));
            assert!(b.available_setting_ids().is_empty());
            assert!(b.setting_name(1).is_none() && b.available_values(1).is_none());
        }
    }
}
