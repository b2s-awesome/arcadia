//! Mod loaders.
//!
//! * Fabric: the profile JSON from meta.fabricmc.net is merged over vanilla.
//! * NeoForge and Forge: their official installers are run in client mode
//!   against Arcadia's Minecraft folder (same layout as `.minecraft`), which
//!   is the supported way to get their patched jars.

use super::version::{self, VersionJson};
use super::{progress, OnProgress};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    Vanilla,
    Fabric,
    Neoforge,
    Forge,
}

impl Loader {
    pub fn label(self) -> &'static str {
        match self {
            Loader::Vanilla => "Vanilla",
            Loader::Fabric => "Fabric",
            Loader::Neoforge => "NeoForge",
            Loader::Forge => "Forge",
        }
    }
    /// Modrinth's loader tag.
    pub fn modrinth(self) -> Option<&'static str> {
        match self {
            Loader::Vanilla => None,
            Loader::Fabric => Some("fabric"),
            Loader::Neoforge => Some("neoforge"),
            Loader::Forge => Some("forge"),
        }
    }
    /// CurseForge's modLoaderType number.
    pub fn curseforge(self) -> Option<u32> {
        match self {
            Loader::Vanilla => None,
            Loader::Forge => Some(1),
            Loader::Fabric => Some(4),
            Loader::Neoforge => Some(6),
        }
    }
}

/// Compare dotted versions numerically ("47.10.0" > "47.9.2").
pub fn cmp_versions(a: &str, b: &str) -> Ordering {
    let nums = |s: &str| -> Vec<u64> { s.split(|c: char| !c.is_ascii_digit()).filter(|t| !t.is_empty()).filter_map(|t| t.parse().ok()).collect() };
    nums(a).cmp(&nums(b))
}

#[derive(Debug, Clone, Serialize)]
pub struct LoaderVersion {
    pub version: String,
    pub stable: bool,
}

// ───────────────────────── Fabric ─────────────────────────

#[derive(Deserialize)]
struct FabricEntry {
    loader: FabricLoader,
}
#[derive(Deserialize)]
struct FabricLoader {
    version: String,
    #[serde(default)]
    stable: bool,
}

pub async fn fabric_versions(mc: &str) -> Result<Vec<LoaderVersion>> {
    let url = format!("{}/v2/versions/loader/{mc}", super::urls::fabric_meta());
    let v: Vec<FabricEntry> = super::http().get(url).send().await?.error_for_status()?.json().await?;
    if v.is_empty() {
        bail!("Fabric doesn't support Minecraft {mc} yet");
    }
    Ok(v.into_iter().map(|e| LoaderVersion { version: e.loader.version, stable: e.loader.stable }).collect())
}

pub async fn install_fabric(mc: &str, loader: &str) -> Result<String> {
    let url = format!("{}/v2/versions/loader/{mc}/{loader}/profile/json", super::urls::fabric_meta());
    let v: VersionJson = super::http()
        .get(url)
        .send()
        .await?
        .error_for_status()
        .context("Fabric doesn't offer that loader version for this Minecraft version")?
        .json()
        .await?;
    version::save_local(&v)?;
    Ok(v.id)
}

// ───────────────────────── NeoForge ─────────────────────────

/// NeoForge numbers follow Minecraft: 1.21.1 → 21.1.x, 1.21 → 21.0.x.
pub fn neoforge_prefix(mc: &str) -> Option<String> {
    let p: Vec<&str> = mc.split('.').collect();
    match p.as_slice() {
        ["1", minor] => Some(format!("{minor}.0.")),
        ["1", minor, patch] => Some(format!("{minor}.{patch}.")),
        // Newer year-based Minecraft versions (e.g. 26.1) map directly.
        [major, minor] if *major != "1" => Some(format!("{major}.{minor}.")),
        [major, minor, patch] if *major != "1" => Some(format!("{major}.{minor}.{patch}.")),
        _ => None,
    }
}

#[derive(Deserialize)]
struct MavenVersions {
    versions: Vec<String>,
}

pub async fn neoforge_versions(mc: &str) -> Result<Vec<LoaderVersion>> {
    if mc == "1.20.1" {
        bail!("NeoForge for 1.20.1 was a Forge fork; use Forge for 1.20.1");
    }
    let prefix = neoforge_prefix(mc).ok_or_else(|| anyhow!("NeoForge doesn't support Minecraft {mc}"))?;
    let url = format!("{}/api/maven/versions/releases/net/neoforged/neoforge", super::urls::neoforge_maven());
    let all: MavenVersions = super::http().get(url).send().await?.error_for_status()?.json().await?;
    let mut v: Vec<LoaderVersion> = all
        .versions
        .into_iter()
        .filter(|v| v.starts_with(&prefix))
        .map(|v| LoaderVersion { stable: !v.contains("beta") && !v.contains("alpha"), version: v })
        .collect();
    if v.is_empty() {
        bail!("NeoForge doesn't support Minecraft {mc} yet");
    }
    v.sort_by(|a, b| cmp_versions(&b.version, &a.version));
    Ok(v)
}

// ───────────────────────── Forge ─────────────────────────

pub fn parse_forge_metadata(xml: &str, mc: &str) -> Vec<LoaderVersion> {
    let prefix = format!("{mc}-");
    let re = regex::Regex::new(r"<version>([^<]+)</version>").unwrap();
    let mut v: Vec<LoaderVersion> = re
        .captures_iter(xml)
        .map(|c| c[1].to_string())
        .filter(|v| v.starts_with(&prefix))
        .map(|v| LoaderVersion { version: v[prefix.len()..].to_string(), stable: true })
        .collect();
    v.sort_by(|a, b| cmp_versions(&b.version, &a.version));
    v
}

pub async fn forge_versions(mc: &str) -> Result<Vec<LoaderVersion>> {
    let url = format!("{}/net/minecraftforge/forge/maven-metadata.xml", super::urls::forge_maven());
    let xml = super::http().get(url).send().await?.error_for_status()?.text().await?;
    let v = parse_forge_metadata(&xml, mc);
    if v.is_empty() {
        bail!("Forge doesn't support Minecraft {mc}");
    }
    Ok(v)
}

// ───────────────────────── installer-based loaders ─────────────────────────

async fn run_installer(loader: Loader, mc: &str, lv: &str, task: &str, on: &OnProgress) -> Result<String> {
    let (url, expected_id, flags): (String, String, [&str; 2]) = match loader {
        Loader::Neoforge => (
            format!("{0}/releases/net/neoforged/neoforge/{lv}/neoforge-{lv}-installer.jar", super::urls::neoforge_maven()),
            format!("neoforge-{lv}"),
            ["--install-client", "--installClient"],
        ),
        Loader::Forge => (
            format!("{0}/net/minecraftforge/forge/{mc}-{lv}/forge-{mc}-{lv}-installer.jar", super::urls::forge_maven()),
            format!("{mc}-forge-{lv}"),
            ["--installClient", "--install-client"],
        ),
        _ => unreachable!(),
    };
    if version::local_path(&expected_id).exists() {
        return Ok(expected_id);
    }

    // The installer needs vanilla in place and a launcher_profiles.json.
    let vanilla = version::load_resolved(mc)?;
    super::install::ensure_files(&vanilla, mc, task, on).await?;
    let root = super::dir();
    let profiles = root.join("launcher_profiles.json");
    if !profiles.exists() {
        std::fs::write(&profiles, r#"{"profiles":{},"selectedProfile":null}"#)?;
    }
    let component = super::java::component_for(&vanilla);
    super::java::ensure(&component, task, on).await?;
    let java = super::java::java_console_exe(&component);

    progress(on, task, format!("Downloading the {} installer", loader.label()), 0, 1);
    let tmp = root.join("installers");
    std::fs::create_dir_all(&tmp)?;
    let jar = tmp.join(format!("{}-{lv}-installer.jar", loader.label().to_lowercase()));
    super::download::fetch(&super::http(), &super::download::Job { url, dest: jar.clone(), sha1: None, sha512: None, size: None })
        .await
        .with_context(|| format!("{} {lv} isn't available for {mc}", loader.label()))?;

    progress(on, task, format!("Running the {} installer (this can take a few minutes)", loader.label()), 0, 1);
    let log_path = tmp.join(format!("{expected_id}.log"));
    let mut last_err = String::new();
    for flag in flags {
        let out = tokio::process::Command::new(&java)
            .arg("-Djava.awt.headless=true")
            .arg("-jar")
            .arg(&jar)
            .arg(flag)
            .arg(&root)
            .current_dir(&tmp)
            .output()
            .await
            .context("starting Java for the installer")?;
        let mut log = out.stdout.clone();
        log.extend_from_slice(&out.stderr);
        let _ = std::fs::write(&log_path, &log);
        if out.status.success() && version::local_path(&expected_id).exists() {
            let _ = std::fs::remove_file(&jar);
            return Ok(expected_id);
        }
        last_err = String::from_utf8_lossy(&log).lines().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
    }
    bail!("the {} installer didn't finish. Its log is at {}.\n{last_err}", loader.label(), log_path.display())
}

/// List loader versions for a Minecraft version, newest first.
pub async fn versions(loader: Loader, mc: &str) -> Result<Vec<LoaderVersion>> {
    match loader {
        Loader::Vanilla => Ok(vec![]),
        Loader::Fabric => fabric_versions(mc).await,
        Loader::Neoforge => neoforge_versions(mc).await,
        Loader::Forge => forge_versions(mc).await,
    }
}

/// Install a loader (vanilla must already have its JSON). Returns the
/// version id to launch.
pub async fn install(loader: Loader, mc: &str, lv: &str, task: &str, on: &OnProgress) -> Result<String> {
    super::install::ensure_vanilla_json(mc).await?;
    match loader {
        Loader::Vanilla => Ok(mc.to_string()),
        Loader::Fabric => install_fabric(mc, lv).await,
        Loader::Neoforge | Loader::Forge => run_installer(loader, mc, lv, task, on).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neoforge_mapping() {
        assert_eq!(neoforge_prefix("1.21.1").as_deref(), Some("21.1."));
        assert_eq!(neoforge_prefix("1.21").as_deref(), Some("21.0."));
        assert_eq!(neoforge_prefix("1.20.4").as_deref(), Some("20.4."));
        assert_eq!(neoforge_prefix("26.1").as_deref(), Some("26.1."));
    }

    #[test]
    fn forge_metadata_parsing_and_order() {
        let xml = "<metadata><versioning><versions><version>1.20.1-47.2.0</version><version>1.20.1-47.10.1</version>\
                   <version>1.19.2-43.3.0</version><version>1.20.1-47.3.0</version></versions></versioning></metadata>";
        let v = parse_forge_metadata(xml, "1.20.1");
        let names: Vec<_> = v.iter().map(|v| v.version.as_str()).collect();
        assert_eq!(names, vec!["47.10.1", "47.3.0", "47.2.0"]);
    }

    #[test]
    fn version_compare() {
        assert_eq!(cmp_versions("21.1.77", "21.1.9"), Ordering::Greater);
        assert_eq!(cmp_versions("0.16.9", "0.16.10"), Ordering::Less);
    }
}
