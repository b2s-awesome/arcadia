//! Shared NVIDIA profile types and the driver-settings (DRS) backend trait.
//!
//! Everything above this trait is safe Rust and is tested against the
//! in-memory backend in `mock.rs`. The real backend (`drs.rs`) is the only
//! place that calls NVAPI.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Longest string DRS accepts (`NVAPI_UNICODE_STRING_MAX` is 2048 UTF-16 units including the terminator).
pub const MAX_STRING_UNITS: usize = 2047;
/// Largest binary setting DRS accepts (`NVAPI_BINARY_DATA_MAX`).
pub const MAX_BINARY_BYTES: usize = 4096;

/// A driver setting value. The five kinds are the five `NVDRS_SETTING_TYPE`s.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ValueJson", into = "ValueJson")]
pub enum Value {
    Dword(u32),
    Qword(u64),
    /// `NVDRS_WSTRING_TYPE`.
    Wstring(String),
    /// `NVDRS_STRING_TYPE` (8-bit string).
    Ansi(String),
    Binary(Vec<u8>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValueKind {
    Dword,
    Qword,
    Wstring,
    Ansi,
    Binary,
}

impl ValueKind {
    /// The name NVIDIA Profile Inspector uses in `.nip` files.
    pub fn nip_name(self) -> &'static str {
        match self {
            ValueKind::Dword => "Dword",
            ValueKind::Qword => "Qword",
            ValueKind::Wstring => "String",
            ValueKind::Ansi => "AnsiString",
            ValueKind::Binary => "Binary",
        }
    }
    pub fn from_nip_name(s: &str) -> Option<Self> {
        Some(match s {
            "Dword" => ValueKind::Dword,
            "Qword" => ValueKind::Qword,
            "String" => ValueKind::Wstring,
            "AnsiString" => ValueKind::Ansi,
            "Binary" => ValueKind::Binary,
            _ => return None,
        })
    }
    pub fn label(self) -> &'static str {
        match self {
            ValueKind::Dword => "32-bit number",
            ValueKind::Qword => "64-bit number",
            ValueKind::Wstring => "Text",
            ValueKind::Ansi => "Text (8-bit)",
            ValueKind::Binary => "Binary",
        }
    }
}

/// Wire form: the value as text, so 64-bit numbers survive JavaScript and
/// binary travels as base64. This is also exactly the text a `.nip` file holds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueJson {
    #[serde(rename = "type")]
    pub kind: ValueKind,
    pub value: String,
}

impl From<Value> for ValueJson {
    fn from(v: Value) -> Self {
        ValueJson { kind: v.kind(), value: v.to_text() }
    }
}

impl TryFrom<ValueJson> for Value {
    type Error = String;
    fn try_from(j: ValueJson) -> Result<Self, String> {
        Value::parse(j.kind, &j.value)
    }
}

impl Value {
    pub fn kind(&self) -> ValueKind {
        match self {
            Value::Dword(_) => ValueKind::Dword,
            Value::Qword(_) => ValueKind::Qword,
            Value::Wstring(_) => ValueKind::Wstring,
            Value::Ansi(_) => ValueKind::Ansi,
            Value::Binary(_) => ValueKind::Binary,
        }
    }

    /// Text form used in `.nip` files: decimal numbers, plain strings, base64 binary.
    pub fn to_text(&self) -> String {
        use base64::Engine;
        match self {
            Value::Dword(v) => v.to_string(),
            Value::Qword(v) => v.to_string(),
            Value::Wstring(s) | Value::Ansi(s) => s.clone(),
            Value::Binary(b) => base64::engine::general_purpose::STANDARD.encode(b),
        }
    }

    /// Parse the `.nip` text form, enforcing the limits DRS has.
    pub fn parse(kind: ValueKind, text: &str) -> Result<Self, String> {
        use base64::Engine;
        let v = match kind {
            ValueKind::Dword => Value::Dword(parse_number::<u32>(text).ok_or_else(|| format!("'{}' is not a 32-bit number", clip(text)))?),
            ValueKind::Qword => Value::Qword(parse_number::<u64>(text).ok_or_else(|| format!("'{}' is not a 64-bit number", clip(text)))?),
            ValueKind::Wstring => Value::Wstring(text.to_string()),
            ValueKind::Ansi => Value::Ansi(text.to_string()),
            ValueKind::Binary => Value::Binary(
                base64::engine::general_purpose::STANDARD.decode(text.trim()).map_err(|_| "the binary value is not valid base64".to_string())?,
            ),
        };
        v.check()?;
        Ok(v)
    }

    /// Does the value fit what the driver accepts?
    pub fn check(&self) -> Result<(), String> {
        match self {
            Value::Wstring(s) => {
                if s.encode_utf16().count() > MAX_STRING_UNITS {
                    return Err(format!("text values are limited to {MAX_STRING_UNITS} characters"));
                }
                if s.contains('\0') {
                    return Err("text values cannot contain a NUL character".into());
                }
            }
            Value::Ansi(s) => {
                if s.len() > MAX_STRING_UNITS || !s.is_ascii() || s.contains('\0') {
                    return Err(format!("8-bit text values must be plain ASCII and at most {MAX_STRING_UNITS} characters"));
                }
            }
            Value::Binary(b) if b.len() > MAX_BINARY_BYTES => return Err(format!("binary values are limited to {MAX_BINARY_BYTES} bytes")),
            _ => {}
        }
        Ok(())
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::Dword(v) => Some(*v as u64),
            Value::Qword(v) => Some(*v),
            _ => None,
        }
    }

    /// How the raw value is shown next to any friendly label: hex for
    /// numbers, the text itself for strings, hex bytes for binary.
    pub fn raw_text(&self) -> String {
        match self {
            Value::Dword(v) => format!("0x{v:08X}"),
            Value::Qword(v) => format!("0x{v:016X}"),
            Value::Wstring(s) | Value::Ansi(s) => s.clone(),
            Value::Binary(b) => {
                let shown: Vec<String> = b.iter().take(32).map(|x| format!("{x:02X}")).collect();
                format!("{}{} ({} bytes)", shown.join(" "), if b.len() > 32 { " …" } else { "" }, b.len())
            }
        }
    }
}

fn clip(s: &str) -> String {
    s.chars().take(40).collect()
}

/// Decimal, or hex with a `0x` prefix. No signs, no whitespace inside.
pub fn parse_number<T: TryFrom<u64>>(text: &str) -> Option<T> {
    let t = text.trim();
    let v = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(h, 16).ok()?
    } else if !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()) {
        t.parse::<u64>().ok()?
    } else {
        return None;
    };
    T::try_from(v).ok()
}

/// Where the driver took a setting's value from (`NVDRS_SETTING_LOCATION`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Location {
    /// Stored in the profile that was asked.
    Current,
    Global,
    Base,
    Default,
}

/// One setting as the driver reports it for a profile.
#[derive(Debug, Clone, PartialEq)]
pub struct RawSetting {
    pub id: u32,
    /// Name supplied by the driver; empty for settings it does not name.
    pub name: String,
    pub location: Location,
    /// The current value is NVIDIA's predefined one (not changed by the user).
    pub is_current_predefined: bool,
    pub is_predefined_valid: bool,
    pub current: Value,
    pub predefined: Value,
    /// The driver reported a setting type this version does not know (the
    /// raw type code). The values then hold raw bytes and are read-only.
    pub unknown_type: Option<i32>,
}

impl RawSetting {
    /// Set by the user in this profile (not inherited, not NVIDIA's own value).
    pub fn is_user_value(&self) -> bool {
        self.location == Location::Current && !self.is_current_predefined
    }
    /// The value in effect.
    pub fn effective(&self) -> &Value {
        if self.is_current_predefined && self.is_predefined_valid {
            &self.predefined
        } else {
            &self.current
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawApp {
    pub name: String,
    pub friendly_name: String,
    pub launcher: String,
    /// Extra file that must sit next to the executable for the profile to match.
    pub file_in_folder: String,
    pub command_line: String,
    pub is_predefined: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RawProfile {
    pub name: String,
    pub is_predefined: bool,
    pub app_count: u32,
    pub setting_count: u32,
}

/// What the driver says a setting may be set to (`NvAPI_DRS_EnumAvailableSettingValues`).
#[derive(Debug, Clone, PartialEq)]
pub struct AvailableValues {
    pub kind: ValueKind,
    pub default: Option<Value>,
    pub values: Vec<Value>,
}

/// The failures callers react to; everything else is `Other` with NVAPI's own text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrsCode {
    /// No NVIDIA driver, or it has no DRS support.
    Unavailable,
    SettingNotFound,
    ProfileNotFound,
    ProfileNameInUse,
    ExecutableInUse,
    AccessDenied,
    NotSupported,
    InvalidValue,
    Other,
}

#[derive(Debug, Clone)]
pub struct DrsError {
    pub code: DrsCode,
    /// The DRS operation that failed, e.g. `NvAPI_DRS_SetSetting`.
    pub op: &'static str,
    pub detail: String,
}

impl DrsError {
    pub fn new(code: DrsCode, op: &'static str, detail: impl Into<String>) -> Self {
        Self { code, op, detail: detail.into() }
    }
    pub fn unavailable(detail: impl Into<String>) -> Self {
        Self::new(DrsCode::Unavailable, "NVAPI", detail)
    }
}

impl fmt::Display for DrsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.code {
            DrsCode::Unavailable => write!(f, "{}", self.detail),
            DrsCode::AccessDenied => write!(f, "The NVIDIA driver refused the change ({}: {}). Some settings need Arcadia to run as administrator.", self.op, self.detail),
            _ => write!(f, "{} failed: {}", self.op, self.detail),
        }
    }
}

impl std::error::Error for DrsError {}

pub type DrsResult<T> = Result<T, DrsError>;

/// The driver's profile database.
pub trait DrsBackend: Send + Sync {
    /// Is the driver's profile API usable? `Err` carries the reason for the UI.
    fn status(&self) -> Result<DriverInfo, String>;
    /// Open a session and load the current settings into it.
    fn open(&self) -> DrsResult<Box<dyn DrsSession + '_>>;
    /// Setting ids the driver itself lists as available (its documented set).
    fn available_setting_ids(&self) -> Vec<u32>;
    fn setting_name(&self, id: u32) -> Option<String>;
    fn available_values(&self, id: u32) -> Option<AvailableValues>;
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriverInfo {
    /// e.g. "566.14".
    pub version: Option<String>,
    pub branch: Option<String>,
}

/// One loaded copy of the profile database. Changes stay in the session
/// until [`DrsSession::save`]; dropping it without saving discards them.
/// Profiles are addressed by name; `""` is the current global profile.
pub trait DrsSession {
    fn profiles(&mut self) -> DrsResult<Vec<RawProfile>>;
    /// Name of the profile that applies to everything without its own profile.
    fn global_profile_name(&mut self) -> DrsResult<String>;
    fn profile(&mut self, name: &str) -> DrsResult<Option<RawProfile>>;
    fn apps(&mut self, profile: &str) -> DrsResult<Vec<RawApp>>;
    /// Settings stored in or inherited by the profile, as the driver enumerates them.
    fn settings(&mut self, profile: &str) -> DrsResult<Vec<RawSetting>>;
    fn setting(&mut self, profile: &str, id: u32) -> DrsResult<Option<RawSetting>>;
    fn set_setting(&mut self, profile: &str, id: u32, value: &Value) -> DrsResult<()>;
    /// Remove a user value so the setting falls back to what it inherits.
    fn delete_setting(&mut self, profile: &str, id: u32) -> DrsResult<()>;
    fn restore_setting_default(&mut self, profile: &str, id: u32) -> DrsResult<()>;
    fn restore_profile_default(&mut self, profile: &str) -> DrsResult<()>;
    fn create_profile(&mut self, name: &str) -> DrsResult<()>;
    fn delete_profile(&mut self, name: &str) -> DrsResult<()>;
    fn add_app(&mut self, profile: &str, exe: &str, file_in_folder: &str) -> DrsResult<()>;
    fn remove_app(&mut self, profile: &str, app: &RawApp) -> DrsResult<()>;
    /// Profile that already owns an executable, if any.
    fn find_app_profile(&mut self, exe: &str) -> DrsResult<Option<String>>;
    /// Write the session's changes to the driver.
    fn save(&mut self) -> DrsResult<()>;
    /// Write the whole driver database to a file in NVIDIA's own backup
    /// format (`NvAPI_DRS_SaveSettingsToFile`). This is not a `.nip` file.
    fn save_to_file(&mut self, path: &std::path::Path) -> DrsResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip_through_text_and_json() {
        for v in [Value::Dword(0), Value::Dword(u32::MAX), Value::Qword(u64::MAX), Value::Wstring("héllo <&>".into()), Value::Ansi("abc".into()), Value::Binary(vec![0, 1, 254, 255])] {
            assert_eq!(Value::parse(v.kind(), &v.to_text()).unwrap(), v);
            let json = serde_json::to_string(&v).unwrap();
            assert_eq!(serde_json::from_str::<Value>(&json).unwrap(), v);
        }
        // 64-bit numbers travel as text, never as a JSON number.
        assert_eq!(serde_json::to_string(&Value::Qword(u64::MAX)).unwrap(), r#"{"type":"qword","value":"18446744073709551615"}"#);
    }

    #[test]
    fn invalid_values_are_rejected() {
        assert!(Value::parse(ValueKind::Dword, "4294967296").is_err());
        assert!(Value::parse(ValueKind::Dword, "-1").is_err());
        assert!(Value::parse(ValueKind::Dword, "12abc").is_err());
        assert!(Value::parse(ValueKind::Dword, "").is_err());
        assert_eq!(Value::parse(ValueKind::Dword, "0x10").unwrap(), Value::Dword(16));
        assert!(Value::parse(ValueKind::Binary, "not base64!").is_err());
        assert!(Value::parse(ValueKind::Binary, &Value::Binary(vec![0; MAX_BINARY_BYTES + 1]).to_text()).is_err());
        assert!(Value::parse(ValueKind::Wstring, &"x".repeat(MAX_STRING_UNITS + 1)).is_err());
        assert!(Value::parse(ValueKind::Ansi, "ünï").is_err());
        assert!(serde_json::from_str::<Value>(r#"{"type":"dword","value":"nope"}"#).is_err());
    }

    #[test]
    fn raw_text_shows_the_real_value() {
        assert_eq!(Value::Dword(0x10).raw_text(), "0x00000010");
        assert_eq!(Value::Binary(vec![0xAB, 0x01]).raw_text(), "AB 01 (2 bytes)");
    }
}
