//! Read Fleasion replacement configs, so people can bring their existing
//! setups over unchanged.
//!
//! A Fleasion config is a JSON file in `%LOCALAPPDATA%\FleasionNT\configs`.
//! It is either `{"replacement_rules": [...]}` or, in older versions, a bare
//! list of rules. Each rule looks like:
//!
//! ```json
//! {"name": "Sky", "enabled": true, "replace_ids": [123, "456"],
//!  "mode": "id" | "remove" | "cdn" | "local",
//!  "with_id": 789, "cdn_url": "https://…", "local_path": "C:\\…" or "/Folder/file.png"}
//! ```
//!
//! Very old rules use `"remove": true` instead of `mode`. Rules can be nested
//! in `{"type": "group", "name": …, "children": [...]}` folders.
//!
//! This mirrors how Fleasion itself reads the format (`config/manager.py`,
//! `get_all_replacements`); it is a fresh implementation, not a port.

use super::rules::{Action, Profile, Rule};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Roblox's asset-type numbers run up to here. Fleasion treats bare numbers in
/// this range as mistakes rather than asset ids, and so do we.
const RESERVED_TYPE_ID_MAX: u64 = 80;

/// Fleasion's data folder (`%LOCALAPPDATA%\FleasionNT`).
pub fn data_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")).join("FleasionNT")
}

/// Where Fleasion keeps its configs, and what `/Folder/file` paths are relative to.
pub fn configs_dir() -> PathBuf {
    data_dir().join("configs")
}

/// Names of the configs switched on in Fleasion's settings.
pub fn enabled_config_names(data_dir: &Path) -> Vec<String> {
    let Ok(raw) = std::fs::read(data_dir.join("settings.json")) else { return vec![] };
    let Ok(v) = serde_json::from_str::<Value>(&decode_text(&raw)) else { return vec![] };
    v.get("enabled_configs")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

/// Decode a JSON file the way Fleasion tolerates them: UTF-8 with or without
/// a BOM, or UTF-16 (Notepad's "Unicode").
pub fn decode_text(raw: &[u8]) -> String {
    let utf16 = |bytes: &[u8], le: bool| {
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match raw {
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        _ => String::from_utf8_lossy(raw).into_owned(),
    }
}

/// Is this JSON shaped like a Fleasion config?
pub fn looks_like_config(v: &Value) -> bool {
    match v {
        Value::Object(m) => m.get("replacement_rules").is_some_and(Value::is_array),
        Value::Array(a) => a.iter().any(|r| r.get("replace_ids").is_some() || r.get("type").and_then(Value::as_str) == Some("group")),
        _ => false,
    }
}

/// What came across, for the "imported N rules" message.
#[derive(Debug, Default, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Converted {
    pub rules: usize,
    /// Entries Arcadia can't target yet: asset-type wildcards ("Image",
    /// "TexturePack:2") and texture-pack slot keys ("123:1").
    pub skipped_targets: usize,
    /// Rules dropped entirely (nothing usable left in them).
    pub skipped_rules: usize,
    /// Local files the config points at that aren't on this PC.
    pub missing_files: Vec<String>,
}

/// Turn one Fleasion config into an Arcadia profile.
pub fn convert(v: &Value, profile_name: &str, configs_dir: &Path) -> (Profile, Converted) {
    let entries = match v {
        Value::Object(m) => m.get("replacement_rules").and_then(Value::as_array).cloned().unwrap_or_default(),
        Value::Array(a) => a.clone(),
        _ => vec![],
    };
    let mut out = Converted::default();
    let mut rules = Vec::new();
    walk(&entries, "", true, configs_dir, &mut rules, &mut out);
    out.rules = rules.len();
    (Profile { name: profile_name.to_string(), enabled: true, rules }, out)
}

fn walk(entries: &[Value], prefix: &str, parent_on: bool, configs_dir: &Path, rules: &mut Vec<Rule>, out: &mut Converted) {
    for entry in entries {
        let Some(obj) = entry.as_object() else { continue };
        let on = parent_on && obj.get("enabled").and_then(Value::as_bool).unwrap_or(true);
        let name = obj.get("name").and_then(Value::as_str).unwrap_or("").trim();
        if obj.get("type").and_then(Value::as_str) == Some("group") {
            let children = obj.get("children").and_then(Value::as_array).cloned().unwrap_or_default();
            walk(&children, &join_name(prefix, name), on, configs_dir, rules, out);
            continue;
        }
        match convert_rule(obj, configs_dir, out) {
            Some((asset_ids, action)) => {
                let base = if name.is_empty() { format!("Rule {}", rules.len() + 1) } else { name.to_string() };
                rules.push(Rule {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: join_name(prefix, &base),
                    enabled: on,
                    asset_ids,
                    action,
                });
            }
            None => out.skipped_rules += 1,
        }
    }
}

fn join_name(prefix: &str, name: &str) -> String {
    match (prefix.is_empty(), name.is_empty()) {
        (true, _) => name.to_string(),
        (false, true) => prefix.to_string(),
        (false, false) => format!("{prefix} / {name}"),
    }
}

/// A whole-number asset id, as Fleasion accepts them (numbers or digit strings, never floats or bools).
fn parse_id(v: &Value) -> Option<u64> {
    match v {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => {
            let s = s.trim();
            (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().ok()).flatten()
        }
        _ => None,
    }
}

fn convert_rule(obj: &serde_json::Map<String, Value>, configs_dir: &Path, out: &mut Converted) -> Option<(Vec<u64>, Action)> {
    let mut ids = Vec::new();
    for v in obj.get("replace_ids").and_then(Value::as_array).into_iter().flatten() {
        match parse_id(v) {
            Some(id) if id > RESERVED_TYPE_ID_MAX => {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            // Type names, "TexturePack:N", "parent:slot" keys, tiny numbers.
            _ => out.skipped_targets += 1,
        }
    }
    if ids.is_empty() {
        return None;
    }

    let legacy_remove = obj.get("remove").and_then(Value::as_bool);
    let mode = match obj.get("mode").and_then(Value::as_str) {
        Some(m) => m.to_ascii_lowercase(),
        None if legacy_remove == Some(true) => "remove".into(),
        None => "id".into(),
    };
    let text = |k: &str| obj.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());

    let action = match mode.as_str() {
        "remove" => Action::Remove,
        // An empty link or path means "remove" in Fleasion.
        "cdn" => match text("cdn_url") {
            Some(url) => Action::Link { url: url.to_string() },
            None => Action::Remove,
        },
        "local" => match text("local_path") {
            Some(p) => {
                let path = resolve_local_path(p, configs_dir);
                if !path.is_file() {
                    out.missing_files.push(path.to_string_lossy().into_owned());
                }
                Action::File { path: path.to_string_lossy().into_owned() }
            }
            None => Action::Remove,
        },
        "id" => match obj.get("with_id") {
            None | Some(Value::Null) => Action::Remove,
            Some(v) => match parse_id(v) {
                // 0 and 1 are Fleasion's dummy placeholders: the rule does nothing.
                Some(t) if t > RESERVED_TYPE_ID_MAX => Action::Replace { with_id: t },
                _ => return None,
            },
        },
        _ => return None,
    };
    Some((ids, action))
}

/// Fleasion's portable paths: `/Folder/file.png` means a file under its
/// configs folder (at least one folder deep). Anything else is a normal path.
pub fn resolve_local_path(p: &str, configs_dir: &Path) -> PathBuf {
    let portable = p.starts_with('/') && !p.starts_with("//") && !p.contains('\\');
    if portable {
        let parts: Vec<&str> = p[1..].split('/').collect();
        let ok = parts.len() >= 2 && parts.iter().all(|s| !s.is_empty() && *s != "." && *s != "..");
        if ok {
            let candidate = parts.iter().fold(configs_dir.to_path_buf(), |acc, s| acc.join(s));
            if candidate.is_file() || !Path::new(p).is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn converts_every_mode() {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(t.path().join("Skies")).unwrap();
        std::fs::write(t.path().join("Skies/up.png"), b"png").unwrap();

        let cfg = json!({"replacement_rules": [
            {"name": "Swap", "replace_ids": [1000001, "1000002"], "mode": "id", "with_id": "2000000"},
            {"name": "Gone", "replace_ids": [1000003], "mode": "remove"},
            {"name": "Link", "replace_ids": [1000004], "mode": "cdn", "cdn_url": "https://tr.rbxcdn.com/abc"},
            {"name": "File", "replace_ids": [1000005], "mode": "local", "local_path": "/Skies/up.png", "enabled": false},
            {"name": "Old", "replace_ids": [1000006], "remove": true},
            {"name": "Dummy", "replace_ids": [1000007], "mode": "id", "with_id": 1},
            {"name": "Types", "replace_ids": ["Image", "TexturePack:2", "123456:1", 5], "mode": "remove"},
        ]});
        assert!(looks_like_config(&cfg));
        let (p, r) = convert(&cfg, "My config", t.path());
        assert_eq!(p.name, "My config");
        assert_eq!(r.rules, 5);
        assert_eq!(r.skipped_rules, 2); // Dummy (no-op) and Types (nothing targetable)
        assert_eq!(r.skipped_targets, 4);
        assert!(r.missing_files.is_empty());

        assert_eq!(p.rules[0].asset_ids, vec![1000001, 1000002]);
        assert_eq!(p.rules[0].action, Action::Replace { with_id: 2000000 });
        assert_eq!(p.rules[1].action, Action::Remove);
        assert_eq!(p.rules[2].action, Action::Link { url: "https://tr.rbxcdn.com/abc".into() });
        assert_eq!(p.rules[3].action, Action::File { path: t.path().join("Skies").join("up.png").to_string_lossy().into() });
        assert!(!p.rules[3].enabled);
        assert_eq!(p.rules[4].action, Action::Remove);
    }

    #[test]
    fn flattens_groups_and_legacy_lists() {
        let cfg = json!([
            {"type": "group", "name": "Weapons", "enabled": false, "children": [
                {"name": "AK", "replace_ids": [3000001], "mode": "cdn", "cdn_url": ""},
                {"type": "group", "name": "Sounds", "children": [
                    {"replace_ids": [3000002], "mode": "local", "local_path": "C:\\nope\\shot.ogg"}
                ]}
            ]},
            {"name": "Loose", "replace_ids": [3000003], "mode": "id", "with_id": 3000004}
        ]);
        assert!(looks_like_config(&cfg));
        let (p, r) = convert(&cfg, "Legacy", Path::new("/nowhere"));
        let names: Vec<_> = p.rules.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Weapons / AK", "Weapons / Sounds / Rule 2", "Loose"]);
        assert_eq!(p.rules[0].action, Action::Remove); // empty link = remove
        assert!(!p.rules[0].enabled && !p.rules[1].enabled && p.rules[2].enabled);
        assert_eq!(r.missing_files, vec!["C:\\nope\\shot.ogg".to_string()]);
    }

    #[test]
    fn decodes_bom_and_utf16() {
        assert_eq!(decode_text(b"\xEF\xBB\xBF{}"), "{}");
        assert_eq!(decode_text(&[0xFF, 0xFE, b'{', 0, b'}', 0]), "{}");
        assert!(!looks_like_config(&json!({"profiles": []})));
    }
}
