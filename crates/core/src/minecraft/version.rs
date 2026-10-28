//! Mojang version JSON: types, OS rules, `inheritsFrom` merging and
//! launch-argument expansion.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub latest: Latest,
    pub versions: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub release_time: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherits_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minecraft_arguments: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Arguments>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloads: Option<HashMap<String, Artifact>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java_version: Option<JavaVersion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logging: Option<Logging>,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Arg>,
    #[serde(default)]
    pub jvm: Vec<Arg>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Arg {
    Plain(String),
    Ruled { rules: Vec<Rule>, value: StrOrVec },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StrOrVec {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rule {
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<OsRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OsRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Library {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloads: Option<LibDownloads>,
    /// Maven repository base, for loader libraries without `downloads`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<Rule>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LibDownloads {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<Artifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Artifact {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default)]
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    #[serde(default)]
    pub component: Option<String>,
    #[serde(default)]
    pub major_version: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Logging {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<LoggingClient>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingClient {
    pub argument: String,
    pub file: LoggingFile,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingFile {
    pub id: String,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

// ───────────────────────── platform & rules ─────────────────────────

pub fn os_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

pub fn arch_name() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "x86_64"
    }
}

fn rule_matches(r: &Rule, features: &HashSet<&str>) -> bool {
    if let Some(os) = &r.os {
        if let Some(n) = &os.name {
            if n != os_name() {
                return false;
            }
        }
        if let Some(a) = &os.arch {
            if a != arch_name() {
                return false;
            }
        }
    }
    if let Some(f) = &r.features {
        for (k, want) in f {
            if features.contains(k.as_str()) != *want {
                return false;
            }
        }
    }
    true
}

/// Mojang's rule semantics: no rules means allowed; otherwise start
/// disallowed and let the last matching rule decide.
pub fn allowed(rules: Option<&[Rule]>, features: &HashSet<&str>) -> bool {
    let Some(rules) = rules.filter(|r| !r.is_empty()) else { return true };
    let mut ok = false;
    for r in rules {
        if rule_matches(r, features) {
            ok = r.action == "allow";
        }
    }
    ok
}

// ───────────────────────── maven ─────────────────────────

/// `group:artifact:version[:classifier][@ext]` → repository-relative path.
pub fn maven_path(name: &str) -> Option<String> {
    let (coords, ext) = match name.split_once('@') {
        Some((c, e)) => (c, e),
        None => (name, "jar"),
    };
    let parts: Vec<&str> = coords.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let (g, a, v) = (parts[0], parts[1], parts[2]);
    let file = match parts.get(3) {
        Some(c) => format!("{a}-{v}-{c}.{ext}"),
        None => format!("{a}-{v}.{ext}"),
    };
    Some(format!("{}/{a}/{v}/{file}", g.replace('.', "/")))
}

/// Identity of a library ignoring its version, for de-duplication.
pub fn library_key(name: &str) -> String {
    let coords = name.split('@').next().unwrap_or(name);
    let p: Vec<&str> = coords.split(':').collect();
    match p.len() {
        0..=2 => coords.to_string(),
        3 => format!("{}:{}", p[0], p[1]),
        _ => format!("{}:{}:{}", p[0], p[1], p[3]),
    }
}

// ───────────────────────── merging ─────────────────────────

/// Combine a loader profile with the version it inherits from.
pub fn merge(child: VersionJson, parent: VersionJson) -> VersionJson {
    let mut seen = HashSet::new();
    let mut libraries = Vec::new();
    for l in child.libraries.into_iter().chain(parent.libraries) {
        if seen.insert(library_key(&l.name)) {
            libraries.push(l);
        }
    }
    let arguments = match (parent.arguments, child.arguments) {
        (Some(mut p), Some(c)) => {
            p.game.extend(c.game);
            p.jvm.extend(c.jvm);
            Some(p)
        }
        (p, c) => c.or(p),
    };
    VersionJson {
        id: child.id,
        inherits_from: None,
        main_class: child.main_class.or(parent.main_class),
        minecraft_arguments: child.minecraft_arguments.or(parent.minecraft_arguments),
        arguments,
        libraries,
        asset_index: child.asset_index.or(parent.asset_index),
        assets: child.assets.or(parent.assets),
        downloads: child.downloads.or(parent.downloads),
        java_version: child.java_version.or(parent.java_version),
        logging: child.logging.or(parent.logging),
        kind: child.kind.or(parent.kind),
    }
}

/// Load `versions/<id>/<id>.json`, following `inheritsFrom`.
pub fn load_resolved(id: &str) -> Result<VersionJson> {
    let mut v = load_local(id)?;
    let mut depth = 0;
    while let Some(parent) = v.inherits_from.clone() {
        depth += 1;
        if depth > 8 {
            anyhow::bail!("version inheritance is too deep for {id}");
        }
        v = merge(v, load_local(&parent)?);
    }
    Ok(v)
}

pub fn local_path(id: &str) -> std::path::PathBuf {
    super::versions_dir().join(id).join(format!("{id}.json"))
}

pub fn load_local(id: &str) -> Result<VersionJson> {
    let p = local_path(id);
    let s = std::fs::read_to_string(&p).with_context(|| format!("version {id} isn't installed"))?;
    serde_json::from_str(&s).with_context(|| format!("reading {}", p.display()))
}

pub fn save_local(v: &VersionJson) -> Result<()> {
    let p = local_path(&v.id);
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(p, serde_json::to_vec_pretty(v)?)?;
    Ok(())
}

// ───────────────────────── arguments ─────────────────────────

pub fn expand_args(args: &[Arg], features: &HashSet<&str>) -> Vec<String> {
    let mut out = Vec::new();
    for a in args {
        match a {
            Arg::Plain(s) => out.push(s.clone()),
            Arg::Ruled { rules, value } => {
                if allowed(Some(rules), features) {
                    match value {
                        StrOrVec::One(s) => out.push(s.clone()),
                        StrOrVec::Many(v) => out.extend(v.iter().cloned()),
                    }
                }
            }
        }
    }
    out
}

/// Replace `${name}` placeholders.
pub fn substitute(arg: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(arg.len());
    let mut rest = arg;
    while let Some(i) = rest.find("${") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        match after.find('}') {
            Some(j) => {
                let key = &after[..j];
                match vars.get(key) {
                    Some(v) => out.push_str(v),
                    None => out.push_str(&rest[i..i + 3 + j]),
                }
                rest = &after[j + 1..];
            }
            None => {
                out.push_str(&rest[i..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn v(j: serde_json::Value) -> VersionJson {
        serde_json::from_value(j).unwrap()
    }

    #[test]
    fn maven_paths() {
        assert_eq!(maven_path("net.fabricmc:fabric-loader:0.16.9").unwrap(), "net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar");
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3:natives-windows").unwrap(),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar"
        );
        assert_eq!(maven_path("de.oceanlabs.mcp:mcp_config:1.20.1@zip").unwrap(), "de/oceanlabs/mcp/mcp_config/1.20.1/mcp_config-1.20.1.zip");
        assert_eq!(library_key("org.ow2.asm:asm:9.6"), "org.ow2.asm:asm");
        assert_eq!(library_key("org.lwjgl:lwjgl:3.3.3:natives-windows"), "org.lwjgl:lwjgl:natives-windows");
    }

    #[test]
    fn rules_follow_mojang_semantics() {
        let f = HashSet::new();
        let other_os = if os_name() == "windows" { "osx" } else { "windows" };
        let r: Vec<Rule> = serde_json::from_value(json!([{"action":"allow"},{"action":"disallow","os":{"name": other_os}}])).unwrap();
        assert!(allowed(Some(&r), &f));
        let r: Vec<Rule> = serde_json::from_value(json!([{"action":"allow","os":{"name": other_os}}])).unwrap();
        assert!(!allowed(Some(&r), &f));
        let r: Vec<Rule> = serde_json::from_value(json!([{"action":"allow","features":{"is_demo_user":true}}])).unwrap();
        assert!(!allowed(Some(&r), &f));
        assert!(allowed(None, &f));
    }

    #[test]
    fn merges_loader_profile_over_vanilla() {
        let parent = v(json!({
            "id": "1.21.1", "mainClass": "net.minecraft.client.main.Main",
            "arguments": {"game": ["--username", "${auth_player_name}"], "jvm": ["-cp", "${classpath}"]},
            "libraries": [{"name": "org.ow2.asm:asm:9.3"}, {"name": "com.mojang:brigadier:1.3.10"}],
            "assetIndex": {"id": "17", "url": "https://x/17.json"}
        }));
        let child = v(json!({
            "id": "fabric-loader-0.16.9-1.21.1", "inheritsFrom": "1.21.1",
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "arguments": {"game": [], "jvm": ["-DFabricMcEmu= net.minecraft.client.main.Main "]},
            "libraries": [{"name": "org.ow2.asm:asm:9.7.1", "url": "https://maven.fabricmc.net/"}]
        }));
        let m = merge(child, parent);
        assert_eq!(m.main_class.as_deref(), Some("net.fabricmc.loader.impl.launch.knot.KnotClient"));
        let names: Vec<_> = m.libraries.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, vec!["org.ow2.asm:asm:9.7.1", "com.mojang:brigadier:1.3.10"], "loader's newer ASM wins");
        assert_eq!(m.arguments.unwrap().jvm.len(), 3);
        assert_eq!(m.asset_index.unwrap().id, "17");
    }

    #[test]
    fn expands_ruled_args_and_substitutes() {
        let args: Vec<Arg> = serde_json::from_value(json!([
            "--username", "${auth_player_name}",
            {"rules": [{"action": "allow", "features": {"has_custom_resolution": true}}], "value": ["--width", "${resolution_width}"]},
            {"rules": [{"action": "allow"}], "value": "-Dx=${unknown}"}
        ]))
        .unwrap();
        let out = expand_args(&args, &HashSet::new());
        assert_eq!(out, vec!["--username", "${auth_player_name}", "-Dx=${unknown}"]);
        let mut vars = HashMap::new();
        vars.insert("auth_player_name", "Steve".to_string());
        assert_eq!(substitute("${auth_player_name}", &vars), "Steve");
        assert_eq!(substitute("-Dx=${unknown}", &vars), "-Dx=${unknown}");
        assert_eq!(substitute("a${b", &vars), "a${b");
    }
}
