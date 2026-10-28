//! Security: known past incidents, checks against installed mods, a check for
//! leftover fractureiser files, and verifying mods against Modrinth and
//! CurseForge by hash.
//!
//! Sources:
//! * fractureiser (June 2023): fractureiser-investigation/fractureiser and
//!   prismlauncher.org/news/cf-compromised-alert
//! * BleedingPipe (July 2023): blog.mmpa.info/posts/bleeding-pipe and
//!   prismlauncher.org/news/bleedingpipe-alert
//! * Log4Shell (December 2021): minecraft.net "Important message: security
//!   vulnerability in Java Edition"

use super::instance::{self, Instance};
use super::loaders::Loader;
use super::modmeta::normalize;
use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Incident {
    pub id: &'static str,
    pub name: &'static str,
    pub date: &'static str,
    pub kind: &'static str,
    pub summary: &'static str,
    pub what_to_do: &'static str,
    pub links: &'static [&'static str],
}

pub const INCIDENTS: &[Incident] = &[
    Incident {
        id: "fractureiser",
        name: "fractureiser",
        date: "June 2023",
        kind: "Malware in uploaded mods",
        summary: "Malicious code was added to several mods and plugins uploaded to CurseForge and BukkitDev, some of which were in popular modpacks. On Windows and Linux it set itself up to run at startup and downloaded further malware built to steal accounts and saved credentials.",
        what_to_do: "Run Arcadia's leftover-file check below. If anything is found, don't just delete it: run a full antivirus scan, change your passwords from another device, and follow the fractureiser investigation's player guide.",
        links: &["https://github.com/fractureiser-investigation/fractureiser", "https://www.prismlauncher.org/news/cf-compromised-alert/"],
    },
    Incident {
        id: "bleedingpipe",
        name: "BleedingPipe",
        date: "July 2023",
        kind: "Remote code execution in mods",
        summary: "Many mods, mostly for Forge 1.7.10 and 1.12.2, used Java's ObjectInputStream for networking, letting a malicious server or player run code on your computer. Attackers scanned for vulnerable servers. Singleplayer-only play wasn't exposed.",
        what_to_do: "Update the listed mods. On Forge 1.7.2 to 1.12.2, also install PipeBlocker from the Minecraft Malware Prevention Alliance, which blocks the exploit for every mod. Avoid unknown servers with old modpacks.",
        links: &["https://blog.mmpa.info/posts/bleeding-pipe/", "https://www.prismlauncher.org/news/bleedingpipe-alert"],
    },
    Incident {
        id: "log4shell",
        name: "Log4Shell",
        date: "December 2021",
        kind: "Remote code execution in the game itself",
        summary: "A flaw in the Log4j logging library let a chat message run code on clients and servers for Minecraft 1.7 to 1.18. Mojang patched its logging configs; third-party launchers have to apply them.",
        what_to_do: "Nothing: Arcadia always uses Mojang's patched logging config and also turns off Log4j lookups for every version.",
        links: &["https://www.minecraft.net/en-us/article/important-message--security-vulnerability-java-edition"],
    },
];

/// A mod named in an incident. `mc` limits which Minecraft versions it
/// applies to (empty = any); `note` says which mod versions were affected.
struct Affected {
    incident: &'static str,
    name: &'static str,
    mc: &'static [&'static str],
    note: &'static str,
}

const AFFECTED: &[Affected] = &[
    // fractureiser: specific malicious uploads of these projects (not every version).
    Affected { incident: "fractureiser", name: "Dungeons Arise", mc: &[], note: "Some uploads in spring 2023 were infected; current files are clean." },
    Affected { incident: "fractureiser", name: "Sky Villages", mc: &[], note: "Some uploads in spring 2023 were infected." },
    Affected { incident: "fractureiser", name: "Dungeonz", mc: &[], note: "Some uploads in spring 2023 were infected." },
    Affected { incident: "fractureiser", name: "Skyblock Core", mc: &[], note: "Some uploads in spring 2023 were infected." },
    Affected { incident: "fractureiser", name: "Vault Integrations", mc: &[], note: "Some uploads in spring 2023 were infected." },
    Affected { incident: "fractureiser", name: "AutoBroadcast", mc: &[], note: "Some uploads in spring 2023 were infected." },
    Affected { incident: "fractureiser", name: "Museum Curator Advanced", mc: &[], note: "Some uploads in spring 2023 were infected." },
    Affected { incident: "fractureiser", name: "Create Infernal Expansion Plus", mc: &[], note: "Removed from CurseForge; delete it." },
    // BleedingPipe (per MMPA and Prism Launcher).
    Affected { incident: "bleedingpipe", name: "AetherCraft", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "Advent of Ascension", mc: &["1.12.2"], note: "1.12.2 versions." },
    Affected { incident: "bleedingpipe", name: "Nevermine", mc: &["1.12.2"], note: "1.12.2 versions." },
    Affected { incident: "bleedingpipe", name: "Arrows Plus", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "Astral Sorcery", mc: &[], note: "Versions 1.9.1 and earlier." },
    Affected { incident: "bleedingpipe", name: "BDLib", mc: &["1.7.10", "1.8.9", "1.10.2", "1.11.2", "1.12", "1.12.1", "1.12.2"], note: "Fixed in builds from August 2023 (original and GTNH fork)." },
    Affected { incident: "bleedingpipe", name: "Brazier", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "Carbonization", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "Custom Friends Capes", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "CustomOreGen", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "DankNull", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "Energy Manipulation", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "EnderCore", mc: &["1.9", "1.9.4", "1.10.2", "1.11.2", "1.12", "1.12.1", "1.12.2", "1.13.2"], note: "Versions for 1.9 to 1.13; update EnderCore and EnderIO." },
    Affected { incident: "bleedingpipe", name: "Enderman Evolution", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "Extrafirma", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "Gadomancy", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "Giacomo's Bookshelf", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "Immersive Armors", mc: &["1.17.1", "1.18.1", "1.19", "1.19.1"], note: "Only for 1.17.1, 1.18.1, 1.19.0 and 1.19.1." },
    Affected { incident: "bleedingpipe", name: "Immersive Aircraft", mc: &[], note: "Listed as affected; update to the latest version." },
    Affected { incident: "bleedingpipe", name: "Immersive Paintings", mc: &[], note: "Listed as affected; update to the latest version." },
    Affected { incident: "bleedingpipe", name: "JourneyMap", mc: &["1.16.5"], note: "1.16.5-5.7.1 only; fixed in 1.16.5-5.7.2." },
    Affected { incident: "bleedingpipe", name: "LanteaCraft", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "SGCraft", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "Logistics Pipes", mc: &["1.4.7", "1.5.2", "1.6.4", "1.7.10"], note: "1.4.7 to 1.7.10; fixed in 0.10.0.71 and later (original) and the GTNH fork." },
    Affected { incident: "bleedingpipe", name: "Minecraft Comes Alive", mc: &["1.5.2", "1.6.4"], note: "1.5.2 to 1.6.4 only." },
    Affected { incident: "bleedingpipe", name: "MattDahEpic Core", mc: &["1.8.8", "1.8.9", "1.9.4", "1.10.2", "1.11.2", "1.12.2"], note: "1.8.8 to 1.12.2." },
    Affected { incident: "bleedingpipe", name: "mxTune", mc: &["1.12.2", "1.14.4", "1.15.2", "1.16.5"], note: "1.12 to 1.16.5." },
    Affected { incident: "bleedingpipe", name: "p455w0rd's Things", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "Project Blue", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "RadixCore", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "RebornCore", mc: &[], note: "Versions 3.13.8 up to (not including) 4.7.3." },
    Affected { incident: "bleedingpipe", name: "SimpleAchievements", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "Smart Moving", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "Strange", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "SuperMartijn642's Config Lib", mc: &[], note: "Versions before 1.0.9." },
    Affected { incident: "bleedingpipe", name: "Thaumic Tinkerer", mc: &["1.6.4", "1.7.2"], note: "Fixed in 2.3-138 for 1.7.2; 1.6.x stays affected." },
    Affected { incident: "bleedingpipe", name: "Tough Expansion", mc: &[], note: "All versions." },
    Affected { incident: "bleedingpipe", name: "ttCore", mc: &["1.7.10"], note: "1.7.10 only." },
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub incident: String,
    pub instance_id: String,
    pub instance_name: String,
    pub mod_name: String,
    pub file_name: String,
    pub note: String,
}

/// Compare installed mods against the incident lists (by mod name or id).
pub fn scan_instance(inst: &Instance) -> Vec<Finding> {
    let mods = instance::mods(&inst.id).unwrap_or_default();
    let mut out = Vec::new();
    for m in &mods {
        let keys: Vec<String> = [
            m.meta.as_ref().map(|x| normalize(&x.name)),
            m.meta.as_ref().map(|x| normalize(&x.id)),
            Some(normalize(m.file_name.split(['-', '_', '+']).next().unwrap_or(&m.file_name).trim_end_matches(".jar"))),
        ]
        .into_iter()
        .flatten()
        .filter(|k| k.len() >= 4)
        .collect();
        for a in AFFECTED {
            if !a.mc.is_empty() && !a.mc.contains(&inst.mc_version.as_str()) {
                continue;
            }
            let n = normalize(a.name);
            let dup = out.iter().any(|f: &Finding| f.incident == a.incident && f.file_name == m.file_name);
            if !dup && keys.iter().any(|k| *k == n) {
                out.push(Finding {
                    incident: a.incident.into(),
                    instance_id: inst.id.clone(),
                    instance_name: inst.name.clone(),
                    mod_name: m.meta.as_ref().map(|x| x.name.clone()).unwrap_or_else(|| m.file_name.clone()),
                    file_name: m.file_name.clone(),
                    note: a.note.into(),
                });
            }
        }
    }
    out
}

/// Whether PipeBlocker is worth suggesting for an instance.
pub fn pipeblocker_advised(inst: &Instance) -> bool {
    let old_forge = matches!(inst.loader, Loader::Forge) && {
        let parts: Vec<u32> = inst.mc_version.split('.').filter_map(|p| p.parse().ok()).collect();
        matches!(parts.as_slice(), [1, minor, ..] if (7..=12).contains(minor))
    };
    old_forge && !instance::mods(&inst.id).unwrap_or_default().iter().any(|m| normalize(&m.file_name).contains("pipeblocker"))
}

// ───────────────────────── fractureiser leftovers ─────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leftover {
    pub path: String,
    pub what: String,
}

pub fn leftover_paths() -> Vec<(PathBuf, &'static str)> {
    let mut v = Vec::new();
    if cfg!(windows) {
        if let Some(local) = dirs::data_local_dir() {
            // Note the space: "Microsoft Edge", not the real "Microsoft\Edge".
            let d = local.join("Microsoft Edge");
            for f in ["libWebGL64.jar", ".ref", "client.jar", "lib.dll", "run.bat"] {
                v.push((d.join(f), "fractureiser stage 2/3 file"));
            }
        }
        if let Some(roaming) = dirs::data_dir() {
            v.push((roaming.join(r"Microsoft\Windows\Start Menu\Programs\Startup\run.bat"), "fractureiser startup script"));
        }
    } else if let Some(home) = dirs::home_dir() {
        v.push((home.join(".config/systemd/user/systemd-utility.service"), "fractureiser persistence service"));
        v.push((PathBuf::from("/etc/systemd/system/systemd-utility.service"), "fractureiser persistence service"));
        let d = home.join(".config/.data");
        for f in ["lib.jar", ".ref", "client.jar", "lib.dll", "libWebGL64.jar", "run.bat"] {
            v.push((d.join(f), "fractureiser stage 2/3 file"));
        }
    }
    v
}

pub fn check_leftovers() -> Vec<Leftover> {
    let mut out: Vec<Leftover> = leftover_paths()
        .into_iter()
        .filter(|(p, _)| p.exists())
        .map(|(p, w)| Leftover { path: p.to_string_lossy().into(), what: w.into() })
        .collect();
    if let Some(v) = crate::registry::read_string(crate::registry::Hive::CurrentUser, r"Software\Microsoft\Windows\CurrentVersion\Run", "t") {
        out.push(Leftover { path: format!(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run → t = {v}"), what: "fractureiser autostart entry".into() });
    }
    out
}

// ───────────────────────── verify against Modrinth / CurseForge ─────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Verified {
    pub file_name: String,
    /// "modrinth", "curseforge" or "unknown"
    pub source: String,
    pub project: Option<String>,
}

/// Look up every mod jar by hash. Files neither site knows are "unknown":
/// not necessarily bad (dev builds, private mods), but worth a second look.
pub async fn verify_instance(id: &str, cf_key: &str) -> Result<Vec<Verified>> {
    let dir = instance::game_dir(id).join("mods");
    let files: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| (e.file_name().to_string_lossy().to_string(), e.path()))
        .filter(|(n, _)| n.ends_with(".jar") || n.ends_with(".jar.disabled"))
        .collect();
    let mut sha: HashMap<String, String> = HashMap::new();
    let mut fps: HashMap<u32, String> = HashMap::new();
    for (name, path) in &files {
        if let Ok(h) = super::download::sha1_file(path) {
            sha.insert(h, name.clone());
        }
        if !cf_key.trim().is_empty() {
            if let Ok(bytes) = std::fs::read(path) {
                fps.insert(super::curseforge::fingerprint(&bytes), name.clone());
            }
        }
    }
    let mut result: HashMap<String, Verified> = files
        .iter()
        .map(|(n, _)| (n.clone(), Verified { file_name: n.clone(), source: "unknown".into(), project: None }))
        .collect();

    if let Ok(found) = super::modrinth::versions_by_sha1(&sha.keys().cloned().collect::<Vec<_>>()).await {
        let ids: Vec<String> = found.values().map(|v| v.project_id.clone()).collect();
        let names: HashMap<String, String> = super::modrinth::projects(&ids).await.unwrap_or_default().into_iter().map(|p| (p.id, p.title)).collect();
        for (h, v) in found {
            if let Some(file) = sha.get(&h) {
                result.insert(file.clone(), Verified { file_name: file.clone(), source: "modrinth".into(), project: names.get(&v.project_id).cloned().or(Some(v.name)) });
            }
        }
    }
    let unknown_fps: Vec<u32> = fps.iter().filter(|(_, n)| result.get(*n).map(|r| r.source == "unknown").unwrap_or(false)).map(|(f, _)| *f).collect();
    if !unknown_fps.is_empty() {
        if let Ok(found) = super::curseforge::match_fingerprints(cf_key, &unknown_fps).await {
            for (fp, f) in found {
                if let Some(file) = fps.get(&fp) {
                    result.insert(file.clone(), Verified { file_name: file.clone(), source: "curseforge".into(), project: Some(f.display_name) });
                }
            }
        }
    }
    let mut v: Vec<Verified> = result.into_values().collect();
    v.sort_by(|a, b| (a.source != "unknown").cmp(&(b.source != "unknown")).then(a.file_name.cmp(&b.file_name)));
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_named_mods_only_on_matching_versions() {
        let (_g, _t) = super::super::test_home();
        let old = instance::create("GTNH-ish", "1.7.10", Loader::Forge, Some("10.13.4.1614".into()), 4096).unwrap();
        let mods = instance::game_dir(&old.id).join("mods");
        super::super::modmeta::tests::make_jar(&mods.join("logisticspipes-0.9.3.132.jar"), &[("mcmod.info", r#"[{"modid":"LogisticsPipes","name":"Logistics Pipes","version":"0.9.3.132"}]"#)]);
        super::super::modmeta::tests::make_jar(&mods.join("journeymap-1.7.10.jar"), &[("mcmod.info", r#"[{"modid":"journeymap","name":"JourneyMap","version":"5.1.4"}]"#)]);
        let f = scan_instance(&old);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].incident, "bleedingpipe");
        assert!(pipeblocker_advised(&old));

        let new = instance::create("Modern", "1.21.1", Loader::Neoforge, Some("21.1.77".into()), 4096).unwrap();
        super::super::modmeta::tests::make_jar(&instance::game_dir(&new.id).join("mods/logisticspipes.jar"), &[("mcmod.info", r#"[{"modid":"logisticspipes","name":"Logistics Pipes"}]"#)]);
        assert!(scan_instance(&new).is_empty(), "1.21.1 isn't an affected version");
        assert!(!pipeblocker_advised(&new));
    }

    #[test]
    fn leftover_paths_use_the_spaced_edge_folder() {
        let p = leftover_paths();
        assert!(!p.is_empty());
        if cfg!(windows) {
            assert!(p.iter().any(|(x, _)| x.to_string_lossy().contains("Microsoft Edge")));
        } else {
            assert!(p.iter().any(|(x, _)| x.ends_with("systemd-utility.service")));
        }
    }
}
