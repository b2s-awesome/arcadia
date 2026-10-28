//! Asset replacement rules, grouped into profiles that can be toggled and
//! shared (import/export as JSON).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    /// Serve a different Roblox asset instead.
    #[serde(rename_all = "camelCase")]
    Replace { with_id: u64 },
    /// Serve a file from this computer instead.
    File { path: String },
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
        std::fs::write(crate::paths::proxy_dir().join("rules.json"), serde_json::to_vec_pretty(self)?)?;
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

    /// Merge profiles exported from another Arcadia install.
    pub fn import(&mut self, json: &str) -> anyhow::Result<usize> {
        let other: RuleConfig = serde_json::from_str(json)
            .or_else(|_| serde_json::from_str::<Profile>(json).map(|p| RuleConfig { profiles: vec![p] }))?;
        let n = other.profiles.len();
        for mut p in other.profiles {
            let mut name = p.name.clone();
            let mut i = 2;
            while self.profiles.iter().any(|q| q.name == name) {
                name = format!("{} ({i})", p.name);
                i += 1;
            }
            p.name = name;
            self.profiles.push(p);
        }
        Ok(n)
    }
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
    fn action_json_shape() {
        let j = serde_json::to_string(&Action::Replace { with_id: 7 }).unwrap();
        assert_eq!(j, r#"{"type":"replace","withId":7}"#);
    }
}
