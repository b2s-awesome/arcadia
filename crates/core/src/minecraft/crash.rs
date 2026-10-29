//! Reads a crash report or log and says, in plain words, what went wrong
//! and how to fix it. Also lists an instance's logs and crash reports.

use super::instance::game_dir;
use anyhow::{bail, Result};
use regex::Regex;
use serde::Serialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Diagnosis {
    /// A short id for the kind of problem ("memory", "missing-dependency"…).
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub fix: String,
    /// Mods it names.
    pub mods: Vec<String>,
}

fn d(kind: &str, title: impl Into<String>, detail: impl Into<String>, fix: impl Into<String>, mods: Vec<String>) -> Diagnosis {
    Diagnosis { kind: kind.into(), title: title.into(), detail: detail.into(), fix: fix.into(), mods }
}

macro_rules! re {
    ($s:expr) => {{
        static R: OnceLock<Regex> = OnceLock::new();
        R.get_or_init(|| Regex::new($s).unwrap())
    }};
}

fn java_for_class(v: u32) -> u32 {
    v.saturating_sub(44)
}

/// What went wrong, most useful first. Empty if nothing is recognised.
pub fn explain(text: &str) -> Vec<Diagnosis> {
    let mut out: Vec<Diagnosis> = Vec::new();
    let mut push = |x: Diagnosis| {
        if !out.iter().any(|o| o.kind == x.kind && o.mods == x.mods) {
            out.push(x);
        }
    };

    // Fabric: missing or wrong-version dependencies.
    for c in re!(r"(?m)Mod '([^']+)' \(([^)]+)\) \S+ requires (?:version (\S+) (?:or later )?of |any version (?:of )?|version )?'?([^',\n]+?)'?(?: \(([^)]+)\))?(?:,| ) ?which is missing").captures_iter(text) {
        let needs = c.get(4).map(|m| m.as_str().trim()).unwrap_or("another mod");
        let ver = c.get(3).map(|m| format!(" {} or newer", m.as_str())).unwrap_or_default();
        push(d(
            "missing-dependency",
            format!("{} needs {}", &c[1], needs),
            format!("{} won't load without {needs}{ver}.", &c[1]),
            format!("Install {needs}{ver} from Add content, or remove {}.", &c[1]),
            vec![c[1].to_string(), needs.to_string()],
        ));
    }
    for c in re!(r"(?m)Mod '([^']+)' \(([^)]+)\) \S+ is incompatible with (?:any version of |version \S+ of )?'?([^',\n]+?)'?(?: \([^)]+\))?(?:,|\n| but)").captures_iter(text) {
        push(d("incompatible", format!("{} and {} don't work together", &c[1], &c[3]), "These two mods say they can't be loaded at the same time.", format!("Turn off {} or {}.", &c[1], &c[3]), vec![c[1].to_string(), c[3].to_string()]));
    }
    // Forge / NeoForge: missing mandatory dependencies.
    for c in re!(r"Mod ID: '([^']+)', Requested by: '([^']+)', Expected range: '([^']+)', Actual version: '([^']+)'").captures_iter(text) {
        let missing = &c[4] == "[MISSING]";
        push(d(
            "missing-dependency",
            if missing { format!("{} needs {}", &c[2], &c[1]) } else { format!("{} needs a different {}", &c[2], &c[1]) },
            if missing { format!("{} won't load without {} ({}).", &c[2], &c[1], &c[3]) } else { format!("{} wants {} {} but {} is installed.", &c[2], &c[1], &c[3], &c[4]) },
            format!("Install a matching {} from Add content, or remove {}.", &c[1], &c[2]),
            vec![c[2].to_string(), c[1].to_string()],
        ));
    }
    // Duplicates.
    for c in re!(r"(?:Mod ID `?'?([\w.-]+)`?'? is supplied by multiple|Found duplicate mods?:?\s*\n?\s*-?\s*'?([\w.-]+))").captures_iter(text) {
        let m = c.get(1).or(c.get(2)).map(|m| m.as_str().to_string()).unwrap_or_default();
        push(d("duplicate", format!("{m} is installed twice"), "Two files provide the same mod, usually an old and a new version.", "Remove the older file from Mods.", vec![m]));
    }
    // Out of memory.
    if text.contains("java.lang.OutOfMemoryError") {
        push(d("memory", "Minecraft ran out of memory", "The game used all the memory it was allowed.", "Raise Memory in the instance's Settings (6–8 GB for big modpacks), or remove some mods.", vec![]));
    }
    if text.contains("Could not reserve enough space for object heap") || text.contains("Invalid maximum heap size") {
        push(d("heap", "Java couldn't get the memory it was given", "The Memory setting is more than this PC can give right now.", "Lower Memory in the instance's Settings, or close other programs.", vec![]));
    }
    // Wrong Java.
    if let Some(c) = re!(r"has been compiled by a more recent version of the Java Runtime \(class file version (\d+)\.\d+\), this version of the Java Runtime only recognizes class file versions up to (\d+)").captures(text) {
        let (need, have) = (java_for_class(c[1].parse().unwrap_or(0)), java_for_class(c[2].parse().unwrap_or(0)));
        push(d("java", format!("A mod needs Java {need}"), format!("Something was built for Java {need}, but this Minecraft version runs on Java {have}."), "That mod is for a newer Minecraft version. Find the version made for this one, or set a newer Java in Settings.", vec![]));
    }
    // Graphics drivers.
    if text.contains("Pixel format not accelerated") || text.contains("WGL: The driver does not appear to support OpenGL") || text.contains("GLFW error 65542") || text.contains("GLFW error 65543") {
        push(d("graphics", "Your graphics driver can't start Minecraft", "Windows is using a driver without OpenGL, often the basic display driver or the wrong GPU on a laptop.", "Update your graphics driver from NVIDIA, AMD or Intel. On laptops, set Java to use the dedicated GPU.", vec![]));
    }
    if text.contains("EXCEPTION_ACCESS_VIOLATION") && re!(r"(?i)(atio6axx|nvoglv|ig\d+icd|amdxx)").is_match(text) {
        push(d("graphics", "The graphics driver crashed", "The crash happened inside the graphics driver.", "Update your graphics driver. Shader packs can trigger this too: try without one.", vec![]));
    }
    // Mixins.
    for c in re!(r"(?:Mixin apply(?:ing)? (?:for|from) mod ([\w.-]+) failed|from mod ([\w.-]+)\] failed injection check|mixins?\.json:[\w.$]+ from mod ([\w.-]+)\])").captures_iter(text) {
        let m = c.get(1).or(c.get(2)).or(c.get(3)).map(|m| m.as_str().to_string()).unwrap_or_default();
        push(d("mixin", format!("{m} doesn't fit this version"), format!("{m} tried to change part of the game that isn't there, usually because it's for another Minecraft or loader version, or another mod changed the same thing."), format!("Update {m}, or turn it off to check."), vec![m]));
    }
    // Forge's loading issue headers and crash report suspects.
    for c in re!(r"-- Mod loading issue for: ([\w.-]+) --").captures_iter(text) {
        push(d("mod-error", format!("{} failed to load", &c[1]), format!("{} threw an error while loading.", &c[1]), format!("Update {} or turn it off.", &c[1]), vec![c[1].to_string()]));
    }
    if let Some(c) = re!(r"Suspected Mods?: ([^\n]+)").captures(text) {
        let names: Vec<String> = c[1].split(',').map(|s| s.split(" (").next().unwrap_or(s).trim().to_string()).filter(|s| !s.is_empty() && s != "NONE" && s != "None" && s != "Minecraft").collect();
        if !names.is_empty() {
            push(d("suspect", format!("Probably caused by {}", names.join(", ")), "The crash report points at these mods.", "Update them, or turn them off one at a time to find the culprit.", names));
        }
    }
    // A library a mod expected isn't there.
    if let Some(c) = re!(r"(?:NoClassDefFoundError|ClassNotFoundException):? ([\w/.$]+)").captures(text) {
        let cls = c[1].replace('/', ".");
        let hint = if cls.starts_with("net.fabricmc.fabric.api") {
            ("Fabric API", "Install Fabric API from Add content.")
        } else if cls.starts_with("net.fabricmc.loader") {
            ("the Fabric loader", "Reinstall the loader: duplicate the instance or change the loader version in Settings.")
        } else if cls.contains("kotlin") {
            ("Fabric Language Kotlin / Kotlin for Forge", "Install Fabric Language Kotlin (Fabric) or Kotlin for Forge.")
        } else if cls.contains("architectury") {
            ("Architectury API", "Install Architectury API from Add content.")
        } else if cls.contains("cloth") || cls.contains("autoconfig") {
            ("Cloth Config", "Install Cloth Config API from Add content.")
        } else {
            ("a library mod", "A mod needs another mod or a different version of one. Check the mods named above.")
        };
        push(d("missing-class", format!("Missing {}", hint.0), format!("The game looked for {cls} and couldn't find it."), hint.1, vec![]));
    }
    out
}

/// The last thing that went wrong, for when nothing is recognised.
pub fn last_error(text: &str) -> Option<String> {
    let line = text.lines().rev().find(|l| l.starts_with("Caused by:")).or_else(|| text.lines().find(|l| re!(r"^(?:[\w.$]+(?:Exception|Error))(?::|$)").is_match(l.trim())))?;
    Some(line.trim().chars().take(300).collect())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFile {
    /// Path inside the game folder, e.g. "logs/latest.log".
    pub path: String,
    pub kind: String,
    pub modified: u64,
    pub size: u64,
}

pub fn logs(id: &str) -> Vec<LogFile> {
    let gd = game_dir(id);
    let mut out = Vec::new();
    let mut add = |rel: String, kind: &str| {
        if let Ok(m) = std::fs::metadata(gd.join(&rel)) {
            let modified = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            out.push(LogFile { path: rel, kind: kind.into(), modified, size: m.len() });
        }
    };
    for f in ["logs/latest.log", "logs/debug.log"] {
        add(f.into(), "log");
    }
    for (dir, kind, ext) in [("crash-reports", "crash", ".txt"), ("", "jvm", ".log")] {
        if let Ok(rd) = std::fs::read_dir(gd.join(dir)) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if n.ends_with(ext) && (kind != "jvm" || n.starts_with("hs_err_pid")) {
                    add(if dir.is_empty() { n } else { format!("{dir}/{n}") }, kind);
                }
            }
        }
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out
}

/// The end of a log (the last `max` bytes), with the access token never
/// present since Minecraft doesn't write it.
pub fn read_log(id: &str, rel: &str, max: usize) -> Result<String> {
    if !logs(id).iter().any(|l| l.path == rel) {
        bail!("That isn't one of this instance's logs");
    }
    let b = std::fs::read(game_dir(id).join(rel))?;
    let start = b.len().saturating_sub(max);
    Ok(String::from_utf8_lossy(&b[start..]).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_common_crashes() {
        let fabric = "net.fabricmc.loader.impl.FormattedException: Mod resolution encountered an incompatible mod set!\nA potential solution has been determined:\n\t - Install fabric-api, any version.\nUnmet dependency listing:\n\t - Mod 'Sodium Extra' (sodium-extra) 0.6.0 requires version 0.6.0 or later of 'sodium', which is missing!\n";
        let r = explain(fabric);
        assert_eq!(r[0].kind, "missing-dependency");
        assert_eq!(r[0].mods, ["Sodium Extra", "sodium"]);
        assert!(r[0].fix.contains("0.6.0 or newer"));

        let forge = "Missing or unsupported mandatory dependencies:\n\tMod ID: 'geckolib', Requested by: 'alexsmobs', Expected range: '[4.4,)', Actual version: '[MISSING]'";
        assert_eq!(explain(forge)[0].title, "alexsmobs needs geckolib");

        assert_eq!(explain("Exception in thread \"main\" java.lang.OutOfMemoryError: Java heap space")[0].kind, "memory");
        let java = "java.lang.UnsupportedClassVersionError: x has been compiled by a more recent version of the Java Runtime (class file version 65.0), this version of the Java Runtime only recognizes class file versions up to 61.0";
        assert_eq!(explain(java)[0].title, "A mod needs Java 21");
        assert_eq!(explain("org.lwjgl.LWJGLException: Pixel format not accelerated")[0].kind, "graphics");
        let mixin = "Mixin apply for mod create failed create.mixins.json:SomeMixin from mod create -> net.minecraft.X";
        assert_eq!(explain(mixin)[0].mods, ["create"]);
        let report = "---- Minecraft Crash Report ----\nSuspected Mods: Iris (iris), Sodium (sodium)\n";
        assert_eq!(explain(report)[0].mods, ["Iris", "Sodium"]);
        assert_eq!(explain("java.lang.NoClassDefFoundError: net/fabricmc/fabric/api/event/Event")[0].title, "Missing Fabric API");
        assert!(explain("all fine").is_empty());
        assert_eq!(last_error("a\nCaused by: java.lang.IllegalStateException: boom\n\tat x").as_deref(), Some("Caused by: java.lang.IllegalStateException: boom"));
    }
}
