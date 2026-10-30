//! Asset replacement rules, grouped into profiles that can be toggled and
//! shared (import/export as JSON).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    /// Serve a different Roblox asset instead.
    #[serde(rename_all = "camelCase")]
    Replace { with_id: u64 },
    /// Serve a file from this computer instead.
    File { path: String },
    /// Load the asset from this web address instead (Fleasion's "CDN" mode).
    Link { url: String },
    /// Strip the asset from the request so it never loads.
    Remove,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    pub asset_ids: Vec<u64>,
    pub action: Action,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleConfig {
    pub profiles: Vec<Profile>,
}

impl RuleConfig {
    pub fn load() -> Self {
        std::fs::read_to_string(crate::paths::proxy_dir().join("rules.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| RuleConfig { profiles: vec![Profile { name: "Default".into(), enabled: true, rules: vec![] }] })
    }

    pub fn save(&self) -> anyhow::Result<()> {
        crate::paths::ensure_all()?;
        crate::paths::write_atomic(&crate::paths::proxy_dir().join("rules.json"), &serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    /// Flatten enabled rules into a lookup. Later profiles win on conflicts.
    pub fn compile(&self) -> HashMap<u64, Action> {
        let mut m = HashMap::new();
        for p in self.profiles.iter().filter(|p| p.enabled) {
            for r in p.rules.iter().filter(|r| r.enabled) {
                for id in &r.asset_ids {
                    m.insert(*id, r.action.clone());
                }
            }
        }
        m
    }

    /// Merge profiles from pasted or opened JSON: Arcadia's own export (all
    /// profiles or one), or a Fleasion config. `name_hint` names a Fleasion
    /// import (usually the file name).
    pub fn import(&mut self, json: &str, name_hint: Option<&str>) -> anyhow::Result<ImportReport> {
        let json = json.trim_start_matches('\u{feff}');
        let v: serde_json::Value = serde_json::from_str(json).map_err(|e| anyhow::anyhow!("not valid JSON ({e})"))?;
        let mut report = ImportReport::default();
        if super::fleasion::looks_like_config(&v) {
            let name = name_hint.map(str::trim).filter(|n| !n.is_empty()).unwrap_or("Fleasion import");
            let (p, c) = super::fleasion::convert(&v, name, &super::fleasion::configs_dir());
            report.add_fleasion(c);
            report.profiles.push(self.add_profile(p));
            return Ok(report);
        }
        let other: RuleConfig = serde_json::from_value(v.clone())
            .or_else(|_| serde_json::from_value::<Profile>(v).map(|p| RuleConfig { profiles: vec![p] }))
            .map_err(|_| anyhow::anyhow!("this isn't an Arcadia profile or a Fleasion config"))?;
        for p in other.profiles {
            report.rules += p.rules.len();
            report.profiles.push(self.add_profile(p));
        }
        Ok(report)
    }

    /// Import every config in a Fleasion install's configs folder, keeping
    /// which ones were switched on.
    pub fn import_fleasion_folder(&mut self, data_dir: &Path) -> anyhow::Result<ImportReport> {
        let configs = data_dir.join("configs");
        let mut files: Vec<_> = std::fs::read_dir(&configs)
            .map_err(|_| anyhow::anyhow!("no Fleasion configs found in {}", configs.display()))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("json")))
            .collect();
        files.sort();
        let enabled = super::fleasion::enabled_config_names(data_dir);
        let mut report = ImportReport::default();
        for f in files {
            let stem = f.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let Ok(raw) = std::fs::read(&f) else { continue };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&super::fleasion::decode_text(&raw)) else { continue };
            if !super::fleasion::looks_like_config(&v) {
                continue;
            }
            let (mut p, c) = super::fleasion::convert(&v, &stem, &configs);
            p.enabled = enabled.is_empty() || enabled.iter().any(|n| n == &stem);
            report.add_fleasion(c);
            report.profiles.push(self.add_profile(p));
        }
        if report.profiles.is_empty() {
            anyhow::bail!("no Fleasion configs found in {}", configs.display());
        }
        Ok(report)
    }

    /// Append a profile, renaming it if the name is taken. Returns the final name.
    fn add_profile(&mut self, mut p: Profile) -> String {
        let mut name = p.name.clone();
        let mut i = 2;
        while self.profiles.iter().any(|q| q.name == name) {
            name = format!("{} ({i})", p.name);
            i += 1;
        }
        p.name = name.clone();
        self.profiles.push(p);
        name
    }

    /// Copy every replacement file the rules point at into Arcadia's own
    /// folder, so imported rules keep working if the originals move.
    /// Missing files are left as they are.
    pub fn adopt_files(&mut self, profiles: &[String]) {
        for p in self.profiles.iter_mut().filter(|p| profiles.contains(&p.name)) {
            for r in &mut p.rules {
                if let Action::File { path } = &mut r.action {
                    if let Ok(new) = adopt_file(Path::new(path)) {
                        *path = new;
                    }
                }
            }
        }
    }
}

/// What an import brought in.
#[derive(Debug, Default, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub profiles: Vec<String>,
    pub rules: usize,
    pub skipped_targets: usize,
    pub skipped_rules: usize,
    pub missing_files: Vec<String>,
}

impl ImportReport {
    fn add_fleasion(&mut self, c: super::fleasion::Converted) {
        self.rules += c.rules;
        self.skipped_targets += c.skipped_targets;
        self.skipped_rules += c.skipped_rules;
        self.missing_files.extend(c.missing_files);
    }
}

/// Copy a replacement file into Arcadia's files folder and return the new
/// path. A file already in that folder is left where it is.
pub fn adopt_file(src: &Path) -> anyhow::Result<String> {
    let dir = crate::paths::proxy_files_dir();
    if src.parent() == Some(dir.as_path()) {
        return Ok(src.to_string_lossy().into());
    }
    let name = src.file_name().ok_or_else(|| anyhow::anyhow!("pick a file, not a folder"))?.to_string_lossy().to_string();
    std::fs::create_dir_all(&dir)?;
    let mut dest = dir.join(&name);
    let mut i = 2;
    while dest.exists() {
        dest = dir.join(format!("{i}-{name}"));
        i += 1;
    }
    std::fs::copy(src, &dest)?;
    Ok(dest.to_string_lossy().into())
}

/// Parse "123, 456 789\n1011" into ids (forgiving of separators and URLs).
pub fn parse_ids(s: &str) -> Vec<u64> {
    let mut out: Vec<u64> = s
        .split(|c: char| !c.is_ascii_digit())
        .filter(|t| t.len() >= 3)
        .filter_map(|t| t.parse().ok())
        .collect();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compile_respects_toggles_and_order() {
        let c = RuleConfig {
            profiles: vec![
                Profile {
                    name: "A".into(),
                    enabled: true,
                    rules: vec![Rule { id: "1".into(), name: "".into(), enabled: true, asset_ids: vec![10, 11], action: Action::Remove }],
                },
                Profile {
                    name: "B".into(),
                    enabled: true,
                    rules: vec![Rule { id: "2".into(), name: "".into(), enabled: true, asset_ids: vec![11], action: Action::Replace { with_id: 99 } }],
                },
                Profile {
                    name: "C".into(),
                    enabled: false,
                    rules: vec![Rule { id: "3".into(), name: "".into(), enabled: true, asset_ids: vec![10], action: Action::Replace { with_id: 5 } }],
                },
            ],
        };
        let m = c.compile();
        assert_eq!(m[&10], Action::Remove);
        assert_eq!(m[&11], Action::Replace { with_id: 99 });
    }

    #[test]
    fn parses_id_lists() {
        assert_eq!(parse_ids("123, 4567\nhttps://www.roblox.com/catalog/8901/x"), vec![123, 4567, 8901]);
    }

    #[test]
    fn import_detects_both_formats() {
        let mut c = RuleConfig { profiles: vec![Profile { name: "Default".into(), enabled: true, rules: vec![] }] };

        let arcadia = r#"{"name":"Default","rules":[{"id":"a","assetIds":[123456],"action":{"type":"remove"}}]}"#;
        let r = c.import(arcadia, None).unwrap();
        assert_eq!(r.profiles, vec!["Default (2)"]);
        assert_eq!(r.rules, 1);

        let fleasion = "\u{feff}{\"replacement_rules\":[{\"name\":\"Sky\",\"replace_ids\":[\"7000001\"],\"mode\":\"cdn\",\"cdn_url\":\"https://x.test/s.png\"}]}";
        let r = c.import(fleasion, Some("skies")).unwrap();
        assert_eq!(r.profiles, vec!["skies"]);
        let p = c.profiles.last().unwrap();
        assert_eq!(p.rules[0].action, Action::Link { url: "https://x.test/s.png".into() });
        assert_eq!(c.compile()[&7000001], Action::Link { url: "https://x.test/s.png".into() });

        let err = c.import(r#"{"hello": 1}"#, None).unwrap_err().to_string();
        assert!(err.contains("Fleasion"), "{err}");
    }

    #[test]
    fn imports_a_fleasion_folder_with_its_toggles() {
        let t = tempfile::tempdir().unwrap();
        let cfgs = t.path().join("configs");
        std::fs::create_dir_all(&cfgs).unwrap();
        std::fs::write(cfgs.join("On.json"), r#"{"replacement_rules":[{"replace_ids":[8000001],"mode":"remove"}]}"#).unwrap();
        std::fs::write(cfgs.join("Off.json"), r#"[{"replace_ids":[8000002],"mode":"id","with_id":8000003}]"#).unwrap();
        std::fs::write(cfgs.join("notes.txt"), "ignore me").unwrap();
        std::fs::write(t.path().join("settings.json"), r#"{"enabled_configs":["On"]}"#).unwrap();

        let mut c = RuleConfig::default();
        let r = c.import_fleasion_folder(t.path()).unwrap();
        assert_eq!(r.profiles, vec!["Off", "On"]);
        assert_eq!(r.rules, 2);
        assert!(!c.profiles[0].enabled && c.profiles[1].enabled);
        assert!(c.import_fleasion_folder(&t.path().join("missing")).is_err());
    }

    #[test]
    fn action_json_shape() {
        let j = serde_json::to_string(&Action::Replace { with_id: 7 }).unwrap();
        assert_eq!(j, r#"{"type":"replace","withId":7}"#);
    }
}
