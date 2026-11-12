//! NVIDIA Profile Inspector `.nip` files: read, write and compare.
//!
//! A `.nip` file is what Profile Inspector's `XmlSerializer` produces for a
//! `List<Profile>` (source: `Common/Import/*.cs` and `Common/Helper/XMLHelper.cs`
//! in <https://github.com/Orbmu2k/nvidiaProfileInspector>, MIT):
//!
//! ```xml
//! <?xml version="1.0" encoding="utf-16"?>
//! <ArrayOfProfile>
//!   <Profile>
//!     <ProfileName>Base Profile</ProfileName>
//!     <Executeables>
//!       <string>game.exe</string>
//!     </Executeables>
//!     <Settings>
//!       <ProfileSetting>
//!         <SettingNameInfo>Vertical Sync</SettingNameInfo>
//!         <SettingID>11041231</SettingID>
//!         <SettingValue>138504007</SettingValue>
//!         <ValueType>Dword</ValueType>
//!       </ProfileSetting>
//!     </Settings>
//!     <ExecutableFindFiles>
//!       <ExecutableFindFile Executable="game.exe" FindFile="steam_api64.dll" />
//!     </ExecutableFindFiles>
//!   </Profile>
//! </ArrayOfProfile>
//! ```
//!
//! Facts the reader and writer rely on, each checked against files written by
//! Profile Inspector itself:
//! * The file is **UTF-8 with a byte-order mark** although the declaration
//!   says `utf-16` (the tool serialises to UTF-16 in memory, then writes the
//!   string as UTF-8). Older builds wrote real UTF-16. The reader goes by the
//!   byte-order mark and ignores the declaration; the writer produces the
//!   current form.
//! * Lines end in CR LF and are indented by two spaces.
//! * `ValueType` is one of `Dword`, `AnsiString`, `String`, `Binary`, `Qword`.
//!   When the element is missing it is `Dword`; a missing `SettingValue` is `0`.
//! * `ExecutableFindFiles` was added later. Files without it are valid.
//!
//! Compatibility rules:
//! * Every setting is kept by its id whether or not Arcadia knows it.
//! * A `ValueType` this version does not know is kept as text and written
//!   back unchanged; it is reported and never sent to the driver.
//! * Elements inside `<Profile>` that this version does not know are kept as
//!   raw XML and written back unchanged.
//!
//! A `.nip` file is *not* the binary/text database backup that
//! `NvAPI_DRS_SaveSettingsToFile` writes, and it is not an Arcadia preset.

use super::types::{parse_number, Value, ValueKind, MAX_STRING_UNITS};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};

/// Largest file the reader accepts. Real exports are a few kilobytes to a few megabytes.
pub const MAX_FILE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_PROFILES: usize = 20_000;
pub const MAX_SETTINGS_PER_PROFILE: usize = 4096;
pub const MAX_EXECUTABLES_PER_PROFILE: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NipSetting {
    pub id: u32,
    /// `SettingNameInfo`: informational only. `None` when the element is absent.
    pub name_info: Option<String>,
    /// `ValueType` exactly as written (`Dword` when the element is absent).
    pub value_type: String,
    /// `SettingValue` exactly as written.
    pub raw_value: String,
    /// The file had no `ValueType` element for this setting.
    #[serde(default)]
    pub type_absent: bool,
}

impl NipSetting {
    pub fn new(id: u32, name_info: &str, value: &Value) -> Self {
        Self { id, name_info: Some(name_info.to_string()), value_type: value.kind().nip_name().to_string(), raw_value: value.to_text(), type_absent: false }
    }

    pub fn kind(&self) -> Option<ValueKind> {
        ValueKind::from_nip_name(&self.value_type)
    }

    /// The typed value, or why it can't be used.
    pub fn value(&self) -> Result<Value, String> {
        let kind = self.kind().ok_or_else(|| format!("value type '{}' is not known to this version of Arcadia", self.value_type))?;
        match kind {
            // Profile Inspector reads these with uint.Parse / ulong.Parse: decimal only.
            ValueKind::Dword | ValueKind::Qword if !is_plain_decimal(&self.raw_value) => Err(format!("'{}' is not a decimal number", self.raw_value.chars().take(40).collect::<String>())),
            _ => Value::parse(kind, &self.raw_value),
        }
    }

    pub fn set_value(&mut self, value: &Value) {
        self.value_type = value.kind().nip_name().to_string();
        self.raw_value = value.to_text();
        self.type_absent = false;
    }
}

fn is_plain_decimal(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) && parse_number::<u64>(s).is_some()
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindFile {
    pub executable: Option<String>,
    pub find_file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NipProfile {
    pub name: String,
    pub executables: Vec<String>,
    pub settings: Vec<NipSetting>,
    pub find_files: Vec<FindFile>,
    /// Elements Arcadia does not know, as raw XML, written back unchanged.
    #[serde(default)]
    pub extra: Vec<String>,
}

impl NipProfile {
    pub fn find_file_for(&self, exe: &str) -> Option<&str> {
        self.find_files.iter().find(|f| f.executable.as_deref().map(|e| e.eq_ignore_ascii_case(exe)).unwrap_or(false)).and_then(|f| f.find_file.as_deref()).filter(|f| !f.is_empty())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceEncoding {
    /// UTF-8 with a byte-order mark: what current Profile Inspector writes.
    Utf8Bom,
    Utf8,
    Utf16Le,
    Utf16Be,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NipFile {
    pub profiles: Vec<NipProfile>,
    pub encoding: SourceEncoding,
    /// The file has `ExecutableFindFiles` elements (Profile Inspector 2.4.0.20 and newer).
    pub has_find_files: bool,
    /// Things the reader kept but could not interpret.
    pub warnings: Vec<String>,
}

impl NipFile {
    pub fn new(profiles: Vec<NipProfile>) -> Self {
        Self { profiles, encoding: SourceEncoding::Utf8Bom, has_find_files: true, warnings: Vec::new() }
    }
}

// ── reading ─────────────────────────────────────────────────────────────

fn decode(bytes: &[u8]) -> Result<(String, SourceEncoding), String> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(format!("The file is larger than {} MB, which is not a plausible profile export", MAX_FILE_BYTES / (1024 * 1024)));
    }
    let utf16 = |data: &[u8], be: bool| -> Result<String, String> {
        if data.len() % 2 != 0 {
            return Err("The file claims to be UTF-16 but has an odd number of bytes".to_string());
        }
        let units: Vec<u16> = data.chunks_exact(2).map(|c| if be { u16::from_be_bytes([c[0], c[1]]) } else { u16::from_le_bytes([c[0], c[1]]) }).collect();
        String::from_utf16(&units).map_err(|_| "The file is not valid UTF-16".to_string())
    };
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return Ok((String::from_utf8(rest.to_vec()).map_err(|_| "The file is not valid UTF-8".to_string())?, SourceEncoding::Utf8Bom));
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return Ok((utf16(rest, false)?, SourceEncoding::Utf16Le));
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return Ok((utf16(rest, true)?, SourceEncoding::Utf16Be));
    }
    // No byte-order mark: "<\0" is UTF-16 LE text, otherwise UTF-8.
    if bytes.len() >= 4 && bytes[0] == b'<' && bytes[1] == 0 {
        return Ok((utf16(bytes, false)?, SourceEncoding::Utf16Le));
    }
    Ok((String::from_utf8(bytes.to_vec()).map_err(|_| "The file is not valid UTF-8".to_string())?, SourceEncoding::Utf8))
}

fn xml_err(e: impl std::fmt::Display) -> String {
    format!("The file is not well-formed XML: {e}")
}

/// Text content of a leaf element whose start tag was just read.
fn leaf_text(reader: &mut Reader<&[u8]>, start: &quick_xml::events::BytesStart) -> Result<String, String> {
    let name = start.name().as_ref().to_string();
    let raw = reader.read_text(start.name()).map_err(xml_err)?;
    if raw.contains('<') {
        return Err(format!("<{name}> should hold text only"));
    }
    quick_xml::escape::unescape(&raw).map(|c| c.into_owned()).map_err(|e| format!("<{name}> has an invalid character reference: {e}"))
}

fn check_text(what: &str, s: &str) -> Result<(), String> {
    if s.encode_utf16().count() > MAX_STRING_UNITS {
        return Err(format!("{what} is longer than the {MAX_STRING_UNITS} characters the driver allows"));
    }
    if s.chars().any(|c| c == '\0' || (c.is_control() && !matches!(c, '\t' | '\r' | '\n'))) {
        return Err(format!("{what} contains control characters"));
    }
    Ok(())
}

/// Executable names are stored in the driver as text and never opened or run
/// by Arcadia; they still must look like a file name or a path suffix.
pub fn check_executable(name: &str) -> Result<(), String> {
    check_text("An executable name", name)?;
    if name.trim().is_empty() {
        return Err("An executable name is empty".into());
    }
    if name.split(['\\', '/']).any(|seg| seg == "..") {
        return Err(format!("The executable name '{}' contains '..'", name.chars().take(60).collect::<String>()));
    }
    if name.chars().any(|c| matches!(c, '<' | '>' | '"' | '|' | '?' | '*' | '\r' | '\n' | '\t')) {
        return Err(format!("The executable name '{}' contains characters a file name cannot have", name.chars().take(60).collect::<String>()));
    }
    Ok(())
}

/// Parse a `.nip` file. Input is untrusted: size, structure, types and
/// lengths are all checked, DTDs are refused, and nothing in the file is
/// ever executed or opened as a path.
pub fn parse(bytes: &[u8]) -> Result<NipFile, String> {
    let (text, encoding) = decode(bytes)?;
    let xml = text.as_str();
    let mut reader = Reader::from_str(xml);
    let mut file = NipFile { profiles: Vec::new(), encoding, has_find_files: false, warnings: Vec::new() };
    let mut saw_root = false;

    loop {
        match reader.read_event().map_err(xml_err)? {
            Event::Decl(_) | Event::Comment(_) | Event::PI(_) => {}
            Event::Text(t) if t.chars().all(|c| c.is_ascii_whitespace()) => {}
            Event::DocType(_) => return Err("The file contains a DOCTYPE, which profile files never have. It was not read.".into()),
            Event::Start(e) if !saw_root => {
                if e.name().as_ref() != "ArrayOfProfile" {
                    return Err(format!("This is not a Profile Inspector .nip file (root element is <{}>, expected <ArrayOfProfile>)", e.name().as_ref()));
                }
                saw_root = true;
            }
            Event::Empty(e) if !saw_root => {
                if e.name().as_ref() != "ArrayOfProfile" {
                    return Err(format!("This is not a Profile Inspector .nip file (root element is <{}>, expected <ArrayOfProfile>)", e.name().as_ref()));
                }
                return Ok(file);
            }
            Event::Start(e) if e.name().as_ref() == "Profile" => {
                if file.profiles.len() >= MAX_PROFILES {
                    return Err(format!("The file has more than {MAX_PROFILES} profiles"));
                }
                let p = parse_profile(&mut reader, xml, &mut file)?;
                if file.profiles.iter().any(|x| x.name.eq_ignore_ascii_case(&p.name)) {
                    file.warnings.push(format!("The profile '{}' appears more than once; the later entry is merged over the earlier one on import.", p.name));
                }
                file.profiles.push(p);
            }
            Event::Empty(e) if e.name().as_ref() == "Profile" => return Err("The file has a profile with no name".into()),
            Event::Start(e) => {
                let name = e.name().as_ref().to_string();
                reader.read_to_end(e.name()).map_err(xml_err)?;
                file.warnings.push(format!("Skipped an unknown top-level element <{name}>."));
            }
            Event::End(_) => {}
            Event::Eof => break,
            Event::Empty(_) => {}
            _ => return Err("The file has text where a profile was expected".into()),
        }
    }
    if !saw_root {
        return Err("This is not a Profile Inspector .nip file (no <ArrayOfProfile> element)".into());
    }
    Ok(file)
}

fn parse_profile(reader: &mut Reader<&[u8]>, xml: &str, file: &mut NipFile) -> Result<NipProfile, String> {
    let mut p = NipProfile::default();
    let mut named = false;
    loop {
        let at = reader.buffer_position() as usize;
        match reader.read_event().map_err(xml_err)? {
            Event::Text(t) if t.chars().all(|c| c.is_ascii_whitespace()) => {}
            Event::Comment(_) => {}
            Event::End(e) if e.name().as_ref() == "Profile" => break,
            Event::Start(e) => match e.name().as_ref() {
                "ProfileName" => {
                    p.name = leaf_text(reader, &e)?;
                    named = true;
                }
                "Executeables" => loop {
                    match reader.read_event().map_err(xml_err)? {
                        Event::Text(t) if t.chars().all(|c| c.is_ascii_whitespace()) => {}
                        Event::Start(s) if s.name().as_ref() == "string" => {
                            if p.executables.len() >= MAX_EXECUTABLES_PER_PROFILE {
                                return Err(format!("A profile lists more than {MAX_EXECUTABLES_PER_PROFILE} executables"));
                            }
                            let exe = leaf_text(reader, &s)?;
                            check_executable(&exe)?;
                            p.executables.push(exe);
                        }
                        Event::Empty(s) if s.name().as_ref() == "string" => return Err("An executable name is empty".into()),
                        Event::End(x) if x.name().as_ref() == "Executeables" => break,
                        Event::Eof => return Err("The file ends inside <Executeables>".into()),
                        _ => return Err("<Executeables> should only hold <string> entries".into()),
                    }
                },
                "Settings" => loop {
                    match reader.read_event().map_err(xml_err)? {
                        Event::Text(t) if t.chars().all(|c| c.is_ascii_whitespace()) => {}
                        Event::Start(s) if s.name().as_ref() == "ProfileSetting" => {
                            if p.settings.len() >= MAX_SETTINGS_PER_PROFILE {
                                return Err(format!("A profile has more than {MAX_SETTINGS_PER_PROFILE} settings"));
                            }
                            p.settings.push(parse_setting(reader)?);
                        }
                        Event::Empty(s) if s.name().as_ref() == "ProfileSetting" => return Err("A setting has no id".into()),
                        Event::End(x) if x.name().as_ref() == "Settings" => break,
                        Event::Eof => return Err("The file ends inside <Settings>".into()),
                        _ => return Err("<Settings> should only hold <ProfileSetting> entries".into()),
                    }
                },
                "ExecutableFindFiles" => {
                    file.has_find_files = true;
                    loop {
                        match reader.read_event().map_err(xml_err)? {
                            Event::Text(t) if t.chars().all(|c| c.is_ascii_whitespace()) => {}
                            Event::Empty(s) if s.name().as_ref() == "ExecutableFindFile" => p.find_files.push(parse_find_file(&s)?),
                            Event::Start(s) if s.name().as_ref() == "ExecutableFindFile" => {
                                let f = parse_find_file(&s)?;
                                reader.read_to_end(s.name()).map_err(xml_err)?;
                                p.find_files.push(f);
                            }
                            Event::End(x) if x.name().as_ref() == "ExecutableFindFiles" => break,
                            Event::Eof => return Err("The file ends inside <ExecutableFindFiles>".into()),
                            _ => return Err("<ExecutableFindFiles> should only hold <ExecutableFindFile> entries".into()),
                        }
                    }
                }
                _ => {
                    let name = e.name().as_ref().to_string();
                    reader.read_to_end(e.name()).map_err(xml_err)?;
                    let end = reader.buffer_position() as usize;
                    p.extra.push(xml[at..end].to_string());
                    file.warnings.push(format!("Kept an element this version of Arcadia does not know: <{name}> in profile '{}'.", p.name));
                }
            },
            Event::Empty(e) => match e.name().as_ref() {
                "ProfileName" => named = true,
                "Executeables" | "Settings" => {}
                "ExecutableFindFiles" => file.has_find_files = true,
                _ => {
                    let end = reader.buffer_position() as usize;
                    p.extra.push(xml[at..end].to_string());
                    file.warnings.push(format!("Kept an element this version of Arcadia does not know: <{}> in profile '{}'.", e.name().as_ref(), p.name));
                }
            },
            Event::Eof => return Err("The file ends inside a <Profile>".into()),
            _ => return Err("A <Profile> has text where an element was expected".into()),
        }
    }
    if !named || p.name.trim().is_empty() {
        return Err("The file has a profile with no name".into());
    }
    check_text("A profile name", &p.name)?;
    if p.extra.iter().map(|x| x.len()).sum::<usize>() > 1024 * 1024 {
        return Err(format!("The profile '{}' carries more than 1 MB of unknown data", p.name));
    }
    Ok(p)
}

// `unescape_value` keeps the attribute text exactly as written, which is what a round trip needs.
#[allow(deprecated)]
fn parse_find_file(e: &quick_xml::events::BytesStart) -> Result<FindFile, String> {
    let mut f = FindFile::default();
    for a in e.attributes() {
        let a = a.map_err(xml_err)?;
        let v = a.unescape_value().map_err(xml_err)?.into_owned();
        match a.key.as_ref() {
            "Executable" => {
                check_executable(&v)?;
                f.executable = Some(v);
            }
            "FindFile" => {
                if !v.is_empty() {
                    // May hold several file names separated by ':' (DRS fileInFolder syntax).
                    for part in v.split(':') {
                        check_executable(part).map_err(|e| e.replace("executable name", "find-file name"))?;
                    }
                }
                f.find_file = Some(v);
            }
            _ => {}
        }
    }
    Ok(f)
}

fn parse_setting(reader: &mut Reader<&[u8]>) -> Result<NipSetting, String> {
    let mut s = NipSetting { id: 0, name_info: None, value_type: "Dword".into(), raw_value: "0".into(), type_absent: true };
    let mut has_id = false;
    loop {
        match reader.read_event().map_err(xml_err)? {
            Event::Text(t) if t.chars().all(|c| c.is_ascii_whitespace()) => {}
            Event::Start(e) => match e.name().as_ref() {
                "SettingNameInfo" => {
                    let t = leaf_text(reader, &e)?;
                    // Informational only; cut rather than reject an overlong label.
                    s.name_info = Some(t.chars().filter(|c| !c.is_control()).take(512).collect());
                }
                "SettingID" => {
                    let t = leaf_text(reader, &e)?;
                    s.id = t.trim().parse::<u32>().map_err(|_| format!("'{}' is not a valid setting id", t.chars().take(40).collect::<String>()))?;
                    has_id = true;
                }
                "SettingValue" => s.raw_value = leaf_text(reader, &e)?,
                "ValueType" => {
                    let t = leaf_text(reader, &e)?;
                    let t = t.trim();
                    if t.is_empty() || t.len() > 64 || !t.chars().all(|c| c.is_ascii_alphanumeric()) {
                        return Err(format!("'{}' is not a valid value type", t.chars().take(40).collect::<String>()));
                    }
                    s.value_type = t.to_string();
                    s.type_absent = false;
                }
                _ => {
                    reader.read_to_end(e.name()).map_err(xml_err)?;
                }
            },
            Event::Empty(e) => match e.name().as_ref() {
                "SettingNameInfo" => s.name_info = Some(String::new()),
                "SettingValue" => s.raw_value = String::new(),
                _ => {}
            },
            Event::End(e) if e.name().as_ref() == "ProfileSetting" => break,
            Event::Eof => return Err("The file ends inside a <ProfileSetting>".into()),
            _ => return Err("A <ProfileSetting> has text where an element was expected".into()),
        }
    }
    if !has_id {
        return Err("A setting has no <SettingID>".into());
    }
    if s.raw_value.len() > 8 * 1024 {
        return Err(format!("The value of setting 0x{:08X} is too long to be a driver setting", s.id));
    }
    Ok(s)
}

// ── writing ─────────────────────────────────────────────────────────────

fn esc_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

fn esc_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#xA;"),
            '\r' => out.push_str("&#xD;"),
            '\t' => out.push_str("&#x9;"),
            _ => out.push(c),
        }
    }
    out
}

fn leaf(out: &mut String, indent: &str, name: &str, text: &str) {
    if text.is_empty() {
        out.push_str(&format!("{indent}<{name} />\r\n"));
    } else {
        out.push_str(&format!("{indent}<{name}>{}</{name}>\r\n", esc_text(text)));
    }
}

/// Serialise to the bytes current Profile Inspector writes: UTF-8 with a
/// byte-order mark, a `utf-16` declaration, CR LF line ends, two-space indent.
pub fn write(file: &NipFile) -> Vec<u8> {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"utf-16\"?>\r\n");
    if file.profiles.is_empty() {
        s.push_str("<ArrayOfProfile />");
    } else {
        s.push_str("<ArrayOfProfile>\r\n");
        for p in &file.profiles {
            s.push_str("  <Profile>\r\n");
            leaf(&mut s, "    ", "ProfileName", &p.name);
            if p.executables.is_empty() {
                s.push_str("    <Executeables />\r\n");
            } else {
                s.push_str("    <Executeables>\r\n");
                for e in &p.executables {
                    leaf(&mut s, "      ", "string", e);
                }
                s.push_str("    </Executeables>\r\n");
            }
            if p.settings.is_empty() {
                s.push_str("    <Settings />\r\n");
            } else {
                s.push_str("    <Settings>\r\n");
                for x in &p.settings {
                    s.push_str("      <ProfileSetting>\r\n");
                    if let Some(n) = &x.name_info {
                        leaf(&mut s, "        ", "SettingNameInfo", n);
                    }
                    s.push_str(&format!("        <SettingID>{}</SettingID>\r\n", x.id));
                    leaf(&mut s, "        ", "SettingValue", &x.raw_value);
                    s.push_str(&format!("        <ValueType>{}</ValueType>\r\n", esc_text(&x.value_type)));
                    s.push_str("      </ProfileSetting>\r\n");
                }
                s.push_str("    </Settings>\r\n");
            }
            if file.has_find_files || !p.find_files.is_empty() {
                if p.find_files.is_empty() {
                    s.push_str("    <ExecutableFindFiles />\r\n");
                } else {
                    s.push_str("    <ExecutableFindFiles>\r\n");
                    for f in &p.find_files {
                        s.push_str("      <ExecutableFindFile");
                        if let Some(e) = &f.executable {
                            s.push_str(&format!(" Executable=\"{}\"", esc_attr(e)));
                        }
                        if let Some(e) = &f.find_file {
                            s.push_str(&format!(" FindFile=\"{}\"", esc_attr(e)));
                        }
                        s.push_str(" />\r\n");
                    }
                    s.push_str("    </ExecutableFindFiles>\r\n");
                }
            }
            for raw in &p.extra {
                s.push_str("    ");
                s.push_str(raw);
                s.push_str("\r\n");
            }
            s.push_str("  </Profile>\r\n");
        }
        s.push_str("</ArrayOfProfile>");
    }
    let mut out = vec![0xEF, 0xBB, 0xBF];
    out.extend_from_slice(s.as_bytes());
    out
}

/// Combine profiles that share a name, later entries winning per setting and
/// per executable (what Profile Inspector does when it loads import files).
pub fn merge_duplicates(profiles: &[NipProfile]) -> Vec<NipProfile> {
    let mut out: Vec<NipProfile> = Vec::new();
    for p in profiles {
        match out.iter_mut().find(|x| x.name.eq_ignore_ascii_case(&p.name)) {
            None => out.push(p.clone()),
            Some(t) => {
                for e in &p.executables {
                    match t.executables.iter_mut().find(|x| x.eq_ignore_ascii_case(e)) {
                        Some(x) => *x = e.clone(),
                        None => t.executables.push(e.clone()),
                    }
                }
                for f in &p.find_files {
                    match t.find_files.iter_mut().find(|x| x.executable.as_deref().unwrap_or("").eq_ignore_ascii_case(f.executable.as_deref().unwrap_or(""))) {
                        Some(x) => x.find_file = f.find_file.clone(),
                        None => t.find_files.push(f.clone()),
                    }
                }
                for s in &p.settings {
                    match t.settings.iter_mut().find(|x| x.id == s.id) {
                        Some(x) => *x = s.clone(),
                        None => t.settings.push(s.clone()),
                    }
                }
                t.extra.extend(p.extra.iter().cloned());
            }
        }
    }
    out
}

// ── comparing ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingDiff {
    pub id: u32,
    pub name: String,
    /// Value in the first file (`None`: not present there).
    pub a: Option<String>,
    pub b: Option<String>,
    pub a_type: Option<String>,
    pub b_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDiff {
    pub name: String,
    /// "onlyA", "onlyB", "changed" or "same".
    pub status: String,
    pub settings: Vec<SettingDiff>,
    pub executables_only_a: Vec<String>,
    pub executables_only_b: Vec<String>,
    pub settings_same: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NipDiff {
    pub profiles: Vec<ProfileDiff>,
    pub identical: bool,
}

/// Compare two sets of profiles by profile name and setting id. Values are
/// compared as typed values when both parse (so `01` equals `1`), otherwise as text.
pub fn compare(a: &[NipProfile], b: &[NipProfile]) -> NipDiff {
    let a = merge_duplicates(a);
    let b = merge_duplicates(b);
    let mut out = Vec::new();
    let same_value = |x: &NipSetting, y: &NipSetting| match (x.value(), y.value()) {
        (Ok(v), Ok(w)) => v == w,
        _ => x.value_type == y.value_type && x.raw_value == y.raw_value,
    };
    let label = |s: &NipSetting| s.name_info.clone().filter(|n| !n.trim().is_empty()).unwrap_or_default();
    for pa in &a {
        match b.iter().find(|x| x.name.eq_ignore_ascii_case(&pa.name)) {
            None => out.push(ProfileDiff {
                name: pa.name.clone(),
                status: "onlyA".into(),
                settings: pa.settings.iter().map(|s| SettingDiff { id: s.id, name: label(s), a: Some(s.raw_value.clone()), b: None, a_type: Some(s.value_type.clone()), b_type: None }).collect(),
                executables_only_a: pa.executables.clone(),
                executables_only_b: Vec::new(),
                settings_same: 0,
            }),
            Some(pb) => {
                let mut settings = Vec::new();
                let mut same = 0;
                for s in &pa.settings {
                    match pb.settings.iter().find(|x| x.id == s.id) {
                        Some(t) if same_value(s, t) => same += 1,
                        Some(t) => settings.push(SettingDiff { id: s.id, name: label(s), a: Some(s.raw_value.clone()), b: Some(t.raw_value.clone()), a_type: Some(s.value_type.clone()), b_type: Some(t.value_type.clone()) }),
                        None => settings.push(SettingDiff { id: s.id, name: label(s), a: Some(s.raw_value.clone()), b: None, a_type: Some(s.value_type.clone()), b_type: None }),
                    }
                }
                for t in pb.settings.iter().filter(|t| !pa.settings.iter().any(|s| s.id == t.id)) {
                    settings.push(SettingDiff { id: t.id, name: label(t), a: None, b: Some(t.raw_value.clone()), a_type: None, b_type: Some(t.value_type.clone()) });
                }
                settings.sort_by_key(|s| s.id);
                let only = |x: &[String], y: &[String]| x.iter().filter(|e| !y.iter().any(|f| f.eq_ignore_ascii_case(e))).cloned().collect::<Vec<_>>();
                let ea = only(&pa.executables, &pb.executables);
                let eb = only(&pb.executables, &pa.executables);
                let status = if settings.is_empty() && ea.is_empty() && eb.is_empty() { "same" } else { "changed" };
                out.push(ProfileDiff { name: pa.name.clone(), status: status.into(), settings, executables_only_a: ea, executables_only_b: eb, settings_same: same });
            }
        }
    }
    for pb in b.iter().filter(|p| !a.iter().any(|x| x.name.eq_ignore_ascii_case(&p.name))) {
        out.push(ProfileDiff {
            name: pb.name.clone(),
            status: "onlyB".into(),
            settings: pb.settings.iter().map(|s| SettingDiff { id: s.id, name: label(s), a: None, b: Some(s.raw_value.clone()), a_type: None, b_type: Some(s.value_type.clone()) }).collect(),
            executables_only_a: Vec::new(),
            executables_only_b: pb.executables.clone(),
            settings_same: 0,
        });
    }
    let identical = out.iter().all(|p| p.status == "same");
    NipDiff { profiles: out, identical }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Written the way current Profile Inspector writes (UTF-8 BOM, `utf-16`
    /// declaration, CR LF, no final newline).
    const CURRENT: &[u8] = include_bytes!("fixtures/npi_current.nip");
    /// Real UTF-16 LE, no `ExecutableFindFiles`, no `ValueType` on one setting (older builds).
    const LEGACY: &[u8] = include_bytes!("fixtures/npi_legacy_utf16.nip");
    /// A file from a hypothetical newer build: unknown element and unknown value type.
    const FUTURE: &[u8] = include_bytes!("fixtures/npi_future.nip");

    #[test]
    fn reads_the_current_format() {
        let f = parse(CURRENT).unwrap();
        assert_eq!(f.encoding, SourceEncoding::Utf8Bom);
        assert!(f.has_find_files);
        assert!(f.warnings.is_empty(), "{:?}", f.warnings);
        assert_eq!(f.profiles.len(), 2);
        let base = &f.profiles[0];
        assert_eq!(base.name, "Base Profile");
        assert!(base.executables.is_empty());
        assert_eq!(base.settings.len(), 5);
        // Empty and whitespace-only names are kept exactly.
        assert_eq!(base.settings[0].name_info.as_deref(), Some(""));
        assert_eq!(base.settings[1].name_info.as_deref(), Some(" "));
        assert_eq!(base.settings[0].value().unwrap(), Value::Dword(929060917));
        assert_eq!(base.settings[2].value().unwrap(), Value::Wstring("Arcadia <test> & more".into()));
        assert_eq!(base.settings[3].value().unwrap(), Value::Binary(vec![1, 2, 3, 4, 255]));
        assert_eq!(base.settings[4].value().unwrap(), Value::Qword(18446744073709551615));
        let game = &f.profiles[1];
        assert_eq!(game.name, "Test Game & Co");
        assert_eq!(game.executables, vec!["testgame.exe", "bin\\testgame-dx12.exe"]);
        assert_eq!(game.find_file_for("TESTGAME.EXE"), Some("steam_api64.dll"));
        assert_eq!(game.find_file_for("bin\\testgame-dx12.exe"), None);
    }

    #[test]
    fn writing_reproduces_profile_inspector_output_byte_for_byte() {
        let f = parse(CURRENT).unwrap();
        assert_eq!(write(&f), CURRENT);
    }

    #[test]
    fn reads_legacy_utf16_and_defaults_missing_fields() {
        let f = parse(LEGACY).unwrap();
        assert_eq!(f.encoding, SourceEncoding::Utf16Le);
        assert!(!f.has_find_files);
        let p = &f.profiles[0];
        assert_eq!(p.name, "Ünïcode Gäme");
        assert_eq!(p.settings.len(), 2);
        // No <ValueType>: Dword. No <SettingValue>: 0.
        assert!(p.settings[1].type_absent);
        assert_eq!(p.settings[1].value().unwrap(), Value::Dword(0));
        // Re-reading what we write gives the same profiles, and the
        // ExecutableFindFiles element is not invented for a file that had none.
        let again = parse(&write(&f)).unwrap();
        assert_eq!(again.profiles, {
            let mut expected = f.profiles.clone();
            expected[0].settings[1].type_absent = false;
            expected
        });
        assert!(!String::from_utf8_lossy(&write(&f)).contains("ExecutableFindFiles"));
    }

    #[test]
    fn unknown_entries_survive_a_round_trip() {
        let f = parse(FUTURE).unwrap();
        let p = &f.profiles[0];
        assert_eq!(p.extra.len(), 2);
        assert!(p.extra[0].starts_with("<GpuSelection>") && p.extra[0].ends_with("</GpuSelection>"));
        assert_eq!(p.extra[1], "<Pinned />");
        let odd = p.settings.iter().find(|s| s.id == 555).unwrap();
        assert_eq!(odd.value_type, "Float");
        assert!(odd.value().unwrap_err().contains("not known"));
        // A setting id Arcadia has never heard of is an ordinary setting.
        assert_eq!(p.settings.iter().find(|s| s.id == 0xDEADBEEF).unwrap().value().unwrap(), Value::Dword(7));
        assert_eq!(f.warnings.len(), 2);
        let again = parse(&write(&f)).unwrap();
        assert_eq!(again.profiles, f.profiles);
    }

    #[test]
    fn edits_change_only_what_was_edited() {
        let mut f = parse(CURRENT).unwrap();
        f.profiles[0].settings[0].set_value(&Value::Dword(1));
        let text = String::from_utf8(write(&f)[3..].to_vec()).unwrap();
        let original = String::from_utf8(CURRENT[3..].to_vec()).unwrap();
        let changed: Vec<(&str, &str)> = original.lines().zip(text.lines()).filter(|(a, b)| a != b).collect();
        assert_eq!(changed, vec![("        <SettingValue>929060917</SettingValue>", "        <SettingValue>1</SettingValue>")]);
    }

    #[test]
    fn rejects_hostile_or_wrong_input() {
        let wrap = |inner: &str| format!("<?xml version=\"1.0\"?><ArrayOfProfile><Profile><ProfileName>P</ProfileName>{inner}</Profile></ArrayOfProfile>").into_bytes();
        assert!(parse(b"").unwrap_err().contains("not a Profile Inspector"));
        assert!(parse(b"{\"json\": true}").is_err());
        assert!(parse(b"<Presets><Preset/></Presets>").unwrap_err().contains("root element"));
        assert!(parse(b"<!DOCTYPE x [<!ENTITY a \"b\">]><ArrayOfProfile/>").unwrap_err().contains("DOCTYPE"));
        assert!(parse(b"<ArrayOfProfile><Profile><ProfileName>x</ProfileName>").is_err());
        assert!(parse(b"<ArrayOfProfile><Profile></Profile></ArrayOfProfile>").unwrap_err().contains("no name"));
        assert!(parse(&wrap("<Settings><ProfileSetting><SettingID>abc</SettingID></ProfileSetting></Settings>")).unwrap_err().contains("setting id"));
        assert!(parse(&wrap("<Settings><ProfileSetting><SettingValue>1</SettingValue></ProfileSetting></Settings>")).unwrap_err().contains("SettingID"));
        assert!(parse(&wrap("<Executeables><string>..\\..\\evil.exe</string></Executeables>")).unwrap_err().contains(".."));
        assert!(parse(&wrap("<Executeables><string>a|b.exe</string></Executeables>")).is_err());
        assert!(parse(&wrap(&format!("<Executeables><string>{}</string></Executeables>", "a".repeat(3000)))).unwrap_err().contains("longer"));
        assert!(parse(&vec![b' '; MAX_FILE_BYTES + 1]).unwrap_err().contains("larger"));
        // Out-of-range and non-decimal numbers are kept in the file but refuse to become values.
        let f = parse(&wrap("<Settings><ProfileSetting><SettingID>1</SettingID><SettingValue>4294967296</SettingValue><ValueType>Dword</ValueType></ProfileSetting><ProfileSetting><SettingID>2</SettingID><SettingValue>0x10</SettingValue><ValueType>Dword</ValueType></ProfileSetting></Settings>")).unwrap();
        assert!(f.profiles[0].settings[0].value().is_err());
        assert!(f.profiles[0].settings[1].value().is_err());
        // Nothing in a profile file is ever treated as a command.
        let f = parse(&wrap("<Executeables><string>cmd.exe /c calc</string></Executeables>")).unwrap();
        assert_eq!(f.profiles[0].executables, vec!["cmd.exe /c calc"]);
    }

    #[test]
    fn compare_reports_differences_both_ways() {
        let a = parse(CURRENT).unwrap();
        let mut b = a.clone();
        assert!(compare(&a.profiles, &b.profiles).identical);
        b.profiles[0].settings[0].set_value(&Value::Dword(5));
        b.profiles[0].settings.remove(4);
        b.profiles[0].settings.push(NipSetting::new(42, "New one", &Value::Dword(1)));
        b.profiles[1].executables.push("extra.exe".into());
        b.profiles.push(NipProfile { name: "Only In B".into(), ..Default::default() });
        let d = compare(&a.profiles, &b.profiles);
        assert!(!d.identical);
        let base = &d.profiles[0];
        assert_eq!(base.status, "changed");
        assert_eq!(base.settings.len(), 3);
        assert_eq!(base.settings_same, 3);
        assert!(base.settings.iter().any(|s| s.id == 42 && s.a.is_none() && s.b.as_deref() == Some("1")));
        assert!(base.settings.iter().any(|s| s.a.as_deref() == Some("929060917") && s.b.as_deref() == Some("5")));
        assert!(base.settings.iter().any(|s| s.b.is_none()));
        assert_eq!(d.profiles[1].executables_only_b, vec!["extra.exe"]);
        assert_eq!(d.profiles[2].status, "onlyB");
        assert_eq!(compare(&b.profiles, &a.profiles).profiles.iter().find(|p| p.name == "Only In B").unwrap().status, "onlyA");
    }

    #[test]
    fn duplicate_profiles_merge_with_later_values_winning() {
        let p1 = NipProfile { name: "Game".into(), executables: vec!["a.exe".into()], settings: vec![NipSetting::new(1, "", &Value::Dword(1)), NipSetting::new(2, "", &Value::Dword(2))], ..Default::default() };
        let p2 = NipProfile { name: "game".into(), executables: vec!["A.EXE".into(), "b.exe".into()], settings: vec![NipSetting::new(2, "", &Value::Dword(20))], ..Default::default() };
        let m = merge_duplicates(&[p1, p2]);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].executables, vec!["A.EXE", "b.exe"]);
        assert_eq!(m[0].settings.iter().map(|s| s.raw_value.as_str()).collect::<Vec<_>>(), vec!["1", "20"]);
    }

    /// Optional: round-trip real files exported by Profile Inspector.
    /// `ARCADIA_NIP_SAMPLES=<folder> cargo test -p arcadia-core nip_samples -- --nocapture`
    #[test]
    fn nip_samples_from_disk_round_trip() {
        let Ok(dir) = std::env::var("ARCADIA_NIP_SAMPLES") else { return };
        let mut n = 0;
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("nip")) != Some(true) {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            let f = parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let written = write(&f);
            // Some tools and editors add a final line break; Profile Inspector does not.
            let trimmed: &[u8] = bytes.strip_suffix(b"\r\n").or_else(|| bytes.strip_suffix(b"\n")).unwrap_or(&bytes);
            if f.encoding == SourceEncoding::Utf8Bom && f.profiles.iter().all(|p| p.settings.iter().all(|s| !s.type_absent)) {
                assert!(written == trimmed, "{}: not byte-identical after a round trip", path.display());
            }
            assert_eq!(parse(&written).unwrap().profiles.len(), f.profiles.len());
            for p in &f.profiles {
                for s in &p.settings {
                    s.value().unwrap_or_else(|e| panic!("{}: setting {}: {e}", path.display(), s.id));
                }
            }
            println!("{}: {} profile(s), {} setting(s), byte-identical", path.display(), f.profiles.len(), f.profiles.iter().map(|p| p.settings.len()).sum::<usize>());
            n += 1;
        }
        assert!(n > 0, "no .nip files found");
    }
}
