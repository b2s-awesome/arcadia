//! What Arcadia knows about NVIDIA driver settings: names, groups,
//! descriptions and the meaning of values.
//!
//! Three sources, kept apart so the UI can say where a label came from:
//!
//! 1. **NVIDIA** (`driver_settings.rs`): the settings NVIDIA documents in
//!    `NvApiDriverSettings.h` of its MIT-licensed NVAPI SDK, with the names
//!    of their values.
//! 2. **Profile Inspector** (`data/CustomSettingNames.xml`): the curated
//!    list maintained by the NVIDIA Profile Inspector project (MIT, © 2016
//!    Orbmu2k; licence text next to the file). It gives friendly names,
//!    groups and descriptions, and it also names settings NVIDIA does *not*
//!    document. Those are flagged as undocumented wherever they are shown.
//! 3. **The installed driver**, asked at run time through
//!    `NvAPI_DRS_EnumAvailableSettingIds` and friends (see `mod.rs`).
//!
//! Profile Inspector's much larger `Reference.xml` is not shipped with
//! Arcadia. If the user points Arcadia at their own Profile Inspector
//! folder, [`Meta::load_reference`] reads it from there.

use super::driver_settings::SETTINGS as OFFICIAL;
use super::types::parse_number;
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;
use std::collections::HashMap;

const COMMUNITY_XML: &str = include_str!("data/CustomSettingNames.xml");

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueLabel {
    /// Numeric value (DWORD and QWORD settings).
    pub value: u64,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NameSource {
    /// NVIDIA's own header.
    Nvidia,
    /// NVIDIA Profile Inspector's curated list.
    ProfileInspector,
    /// Profile Inspector's Reference.xml, loaded from the user's own copy.
    Reference,
    /// The installed driver named it.
    Driver,
    None,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingMeta {
    pub id: u32,
    pub name: String,
    pub name_source: NameSource,
    /// Other names people search for.
    pub alternate_names: Vec<String>,
    pub group: String,
    pub description: String,
    pub options: Vec<ValueLabel>,
    /// NVIDIA documents this setting in its SDK.
    pub documented: bool,
    /// Part of the curated everyday list.
    pub common: bool,
    /// Marked "hidden" by the curated list (rarely useful, or risky).
    pub hidden: bool,
    pub min_driver: Option<f64>,
    pub max_driver: Option<f64>,
    /// NVIDIA's constant name, when documented (e.g. `VSYNCMODE`).
    pub constant: Option<String>,
    pub official_default: Option<u32>,
}

impl SettingMeta {
    fn unknown(id: u32) -> Self {
        Self {
            id,
            name: String::new(),
            name_source: NameSource::None,
            alternate_names: Vec::new(),
            group: UNKNOWN_GROUP.into(),
            description: String::new(),
            options: Vec::new(),
            documented: false,
            common: false,
            hidden: false,
            min_driver: None,
            max_driver: None,
            constant: None,
            official_default: None,
        }
    }

    /// The label for a numeric value, if one is known.
    pub fn label_for(&self, value: u64) -> Option<&str> {
        self.options.iter().find(|o| o.value == value).map(|o| o.label.as_str())
    }
}

pub const UNKNOWN_GROUP: &str = "Unidentified";
pub const OFFICIAL_GROUP: &str = "Other NVIDIA settings";

pub struct Meta {
    by_id: HashMap<u32, SettingMeta>,
}

/// `VSYNCMODE_FORCEON` → "Forceon"; `PREFER_MAX` → "Prefer max".
fn pretty_constant(c: &str) -> String {
    let lower = c.replace('_', " ").to_lowercase();
    let mut chars = lower.chars();
    match chars.next() {
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Tidy text from the curated list: literal `\r\n` sequences become line
/// breaks and `${…}` placeholders (filled in by Profile Inspector at run
/// time with installed DLSS versions) are removed.
fn tidy(text: &str) -> String {
    let mut s = text.replace("\\r\\n", "\n").replace("\\n", "\n");
    while let Some(start) = s.find("${") {
        match s[start..].find('}') {
            Some(end) => {
                let (mut a, mut b) = (start, start + end + 1);
                // Drop the brackets around a placeholder that stood alone: "(${X})".
                if a > 0 && s.as_bytes()[a - 1] == b'(' && s.as_bytes().get(b) == Some(&b')') {
                    a -= 1;
                    b += 1;
                }
                s.replace_range(a..b, "");
            }
            None => break,
        }
    }
    let lines: Vec<String> = s.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).collect();
    lines.join("\n").trim().to_string()
}

/// "05 - Upscaling and Frame Generation" → "Upscaling and Frame Generation".
/// The numeric prefix is only there to order groups.
pub fn group_label(group: &str) -> &str {
    match group.split_once(" - ") {
        Some((n, rest)) if n.trim().chars().all(|c| c.is_ascii_digit()) => rest,
        _ => group,
    }
}

#[derive(Default)]
struct XmlSetting {
    name: String,
    id: Option<u32>,
    group: String,
    description: String,
    alternate: String,
    hidden: bool,
    min: Option<f64>,
    max: Option<f64>,
    values: Vec<ValueLabel>,
}

/// Parse a Profile Inspector `CustomSettingNames`-format document.
fn parse_names_xml(xml: &str) -> Result<Vec<XmlSetting>, String> {
    let mut reader = Reader::from_str(xml);
    let mut out = Vec::new();
    let mut cur: Option<XmlSetting> = None;
    let mut value: Option<(String, Option<u64>)> = None;
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) => {
                let name = e.name().as_ref().to_string();
                match name.as_str() {
                    "CustomSetting" => cur = Some(XmlSetting::default()),
                    "CustomSettingValue" => value = Some((String::new(), None)),
                    "CustomSettingNames" | "Settings" | "SettingValues" | "SettingMasks" => {}
                    _ => {
                        let raw = reader.read_text(e.name()).map_err(|e| e.to_string())?;
                        let text = quick_xml::escape::unescape(&raw).map(|c| c.into_owned()).unwrap_or_else(|_| raw.to_string());
                        let Some(s) = cur.as_mut() else { continue };
                        if let Some(v) = value.as_mut() {
                            match name.as_str() {
                                "UserfriendlyName" => v.0 = tidy(&text),
                                "HexValue" => v.1 = parse_number::<u64>(&text),
                                _ => {}
                            }
                            continue;
                        }
                        match name.as_str() {
                            "UserfriendlyName" => s.name = tidy(&text),
                            "HexSettingID" => s.id = parse_number::<u32>(&text),
                            "GroupName" => s.group = text.trim().to_string(),
                            "Description" => s.description = tidy(&text),
                            "AlternateNames" => s.alternate = text.trim().to_string(),
                            "Hidden" => s.hidden = text.trim().eq_ignore_ascii_case("true"),
                            "MinRequiredDriverVersion" => s.min = text.trim().parse::<f64>().ok().filter(|v| *v > 0.0),
                            "MaxRequiredDriverVersion" => s.max = text.trim().parse::<f64>().ok().filter(|v| *v > 0.0),
                            _ => {}
                        }
                    }
                }
            }
            Event::End(e) => match e.name().as_ref() {
                "CustomSettingValue" => {
                    if let (Some(s), Some((label, Some(v)))) = (cur.as_mut(), value.take()) {
                        if !label.is_empty() {
                            s.values.push(ValueLabel { value: v, label });
                        }
                    }
                    value = None;
                }
                "CustomSetting" => {
                    if let Some(s) = cur.take() {
                        if s.id.is_some() {
                            out.push(s);
                        }
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

impl Meta {
    /// NVIDIA's documented settings plus the curated list. Built once.
    pub fn builtin() -> &'static Meta {
        static META: std::sync::OnceLock<Meta> = std::sync::OnceLock::new();
        META.get_or_init(|| {
            let mut m = Meta { by_id: HashMap::new() };
            for o in OFFICIAL {
                let mut options: Vec<ValueLabel> = Vec::new();
                for (c, v) in o.values {
                    // The header lists range markers (MIN, MAX) beside real values; keep the first name per value.
                    if !options.iter().any(|x| x.value == *v as u64) {
                        options.push(ValueLabel { value: *v as u64, label: pretty_constant(c) });
                    }
                }
                m.by_id.insert(
                    o.id,
                    SettingMeta {
                        id: o.id,
                        name: o.name.to_string(),
                        name_source: NameSource::Nvidia,
                        group: OFFICIAL_GROUP.into(),
                        options,
                        documented: true,
                        constant: Some(o.constant.to_string()),
                        official_default: o.default,
                        ..SettingMeta::unknown(o.id)
                    },
                );
            }
            // The embedded file is checked by a test; a parse failure only loses friendly names.
            m.merge(parse_names_xml(COMMUNITY_XML).unwrap_or_default(), NameSource::ProfileInspector, true);
            m
        })
    }

    fn merge(&mut self, list: Vec<XmlSetting>, source: NameSource, curated: bool) {
        for x in list {
            let Some(id) = x.id else { continue };
            let e = self.by_id.entry(id).or_insert_with(|| SettingMeta::unknown(id));
            // The curated list wins over the header; Reference.xml only fills gaps.
            let fill_only = !curated;
            if !x.name.is_empty() && (!fill_only || e.name.is_empty()) {
                if !e.name.is_empty() && !e.name.eq_ignore_ascii_case(&x.name) && !e.alternate_names.contains(&e.name) {
                    e.alternate_names.push(e.name.clone());
                }
                e.name = x.name;
                e.name_source = source;
            }
            if !x.group.is_empty() && (!fill_only || e.group == UNKNOWN_GROUP) {
                e.group = x.group;
            }
            if !x.description.is_empty() && (!fill_only || e.description.is_empty()) {
                e.description = x.description;
            }
            for a in x.alternate.split(',').map(str::trim).filter(|a| !a.is_empty()) {
                if !e.alternate_names.iter().any(|n| n == a) {
                    e.alternate_names.push(a.to_string());
                }
            }
            if !x.values.is_empty() && (!fill_only || e.options.is_empty()) {
                e.options = x.values;
            }
            if curated {
                e.common = !x.hidden;
                e.hidden = x.hidden;
                e.min_driver = x.min;
                e.max_driver = x.max;
            }
        }
    }

    /// A copy of the built-in data with a user's own `Reference.xml` added
    /// (names for settings nothing else names). The file is untrusted input:
    /// size-limited, and only names, groups, descriptions and value labels are read.
    pub fn with_reference(xml_bytes: &[u8]) -> Result<Meta, String> {
        if xml_bytes.len() > 16 * 1024 * 1024 {
            return Err("Reference.xml is larger than 16 MB".into());
        }
        let text = std::str::from_utf8(xml_bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(xml_bytes)).map_err(|_| "Reference.xml is not UTF-8".to_string())?;
        if text.contains("<!DOCTYPE") {
            return Err("Reference.xml contains a DOCTYPE and was not read".into());
        }
        let list = parse_names_xml(text).map_err(|e| format!("Reference.xml could not be read: {e}"))?;
        if list.is_empty() {
            return Err("Reference.xml has no settings in it".into());
        }
        let mut m = Meta { by_id: Self::builtin().by_id.clone() };
        m.merge(list, NameSource::Reference, false);
        Ok(m)
    }

    pub fn get(&self, id: u32) -> Option<&SettingMeta> {
        self.by_id.get(&id)
    }

    /// Metadata for any id; unknown ids get an empty entry, never a guess.
    pub fn describe(&self, id: u32) -> SettingMeta {
        self.by_id.get(&id).cloned().unwrap_or_else(|| SettingMeta::unknown(id))
    }

    pub fn ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.by_id.keys().copied()
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_list_parses_and_merges_with_the_official_table() {
        let listed = parse_names_xml(COMMUNITY_XML).unwrap();
        assert!(listed.len() > 100, "{}", listed.len());
        let m = Meta::builtin();
        // Vertical Sync: documented by NVIDIA, friendly labels from the curated list.
        let vsync = m.get(0x00A879CF).unwrap();
        assert!(vsync.documented && vsync.common);
        assert_eq!(vsync.constant.as_deref(), Some("VSYNCMODE"));
        assert!(vsync.name.to_lowercase().contains("vertical sync"), "{}", vsync.name);
        assert!(vsync.label_for(0x08416747).is_some());
        assert!(!vsync.group.is_empty() && vsync.group != UNKNOWN_GROUP);
        // Settings only the curated list knows are flagged as not documented by NVIDIA.
        let community_only: Vec<&SettingMeta> = m.by_id.values().filter(|x| x.common && !x.documented).collect();
        assert!(community_only.len() > 20, "{}", community_only.len());
        assert!(community_only.iter().all(|x| x.name_source == NameSource::ProfileInspector && x.constant.is_none()));
        // Placeholders are gone and escaped line breaks are real ones.
        assert!(COMMUNITY_XML.contains("${") && COMMUNITY_XML.contains("\\r\\n"));
        assert!(m.by_id.values().any(|x| x.description.contains('\n')));
        for x in m.by_id.values() {
            assert!(!x.description.contains("${") && !x.description.contains("\\r\\n"), "{}", x.name);
            assert!(x.options.iter().all(|o| !o.label.contains("${") && !o.label.contains("()")), "{:?}", x.options);
        }
        // Every documented setting is present even when the curated list skips it.
        assert!(OFFICIAL.iter().all(|o| m.get(o.id).map(|x| x.documented).unwrap_or(false)));
    }

    #[test]
    fn unknown_ids_get_no_invented_name() {
        let x = Meta::builtin().describe(0xDEADBEEF);
        assert_eq!(x.name, "");
        assert_eq!(x.name_source, NameSource::None);
        assert_eq!(x.group, UNKNOWN_GROUP);
        assert!(!x.documented && !x.common);
    }

    #[test]
    fn tidy_and_labels() {
        assert_eq!(tidy("On - overridden by latest (${DlssVersion})"), "On - overridden by latest");
        assert_eq!(tidy("a\\r\\nb   c"), "a\nb c");
        assert_eq!(pretty_constant("PREFER_MAX"), "Prefer max");
        assert_eq!(group_label("05 - Upscaling and Frame Generation"), "Upscaling and Frame Generation");
        assert_eq!(group_label("Unidentified"), "Unidentified");
    }

    #[test]
    fn a_user_reference_file_only_fills_gaps() {
        let xml = r#"<?xml version="1.0"?><CustomSettingNames><Settings>
            <CustomSetting><UserfriendlyName>Wrong Name</UserfriendlyName><HexSettingID>0x00A879CF</HexSettingID><GroupName>X</GroupName></CustomSetting>
            <CustomSetting><UserfriendlyName>Mystery Toggle</UserfriendlyName><HexSettingID>0xDEADBEEF</HexSettingID><GroupName>01 - Compatibility</GroupName>
              <SettingValues><CustomSettingValue><UserfriendlyName>Off</UserfriendlyName><HexValue>0x00000000</HexValue></CustomSettingValue></SettingValues></CustomSetting>
        </Settings></CustomSettingNames>"#;
        let m = Meta::with_reference(xml.as_bytes()).unwrap();
        assert_ne!(m.get(0x00A879CF).unwrap().name, "Wrong Name");
        let x = m.get(0xDEADBEEF).unwrap();
        assert_eq!((x.name.as_str(), x.name_source), ("Mystery Toggle", NameSource::Reference));
        assert!(!x.common && !x.documented);
        assert_eq!(x.label_for(0), Some("Off"));
        assert!(Meta::with_reference(b"<!DOCTYPE a><CustomSettingNames/>").is_err());
        assert!(Meta::with_reference(b"<nothing/>").is_err());
    }
}
