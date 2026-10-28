//! Minecraft: Java Edition multi-launcher (Prism/MultiMC-style).
//!
//! ```text
//! %LOCALAPPDATA%\Arcadia\Minecraft\
//!   libraries\  assets\  versions\  runtimes\     shared by all instances
//!   instances\<id>\instance.json                  instance settings
//!   instances\<id>\minecraft\                     the game folder (mods, saves…)
//!   accounts.json                                 signed-in accounts (tokens in the OS keychain on Windows)
//! ```

pub mod auth;
pub mod curseforge;
pub mod download;
pub mod import;
pub mod install;
pub mod instance;
pub mod java;
pub mod launch;
pub mod loaders;
pub mod modmeta;
pub mod modrinth;
pub mod security;
pub mod version;

use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;

pub fn dir() -> PathBuf {
    crate::paths::minecraft_dir()
}
pub fn libraries_dir() -> PathBuf {
    dir().join("libraries")
}
pub fn assets_dir() -> PathBuf {
    dir().join("assets")
}
pub fn versions_dir() -> PathBuf {
    dir().join("versions")
}
pub fn runtimes_dir() -> PathBuf {
    dir().join("runtimes")
}
pub fn instances_dir() -> PathBuf {
    dir().join("instances")
}

pub const USER_AGENT: &str = concat!("arcadia-launcher/arcadia/", env!("CARGO_PKG_VERSION"), " (github.com/arcadia-launcher)");

/// Shared HTTP client. Every service we call (Mojang, Modrinth, CurseForge,
/// Fabric, NeoForge) asks for an identifying User-Agent.
pub fn http() -> reqwest::Client {
    static C: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(std::time::Duration::from_secs(20))
            .build()
            .expect("http client")
    })
    .clone()
}

/// Progress reported to the UI while installing or launching.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// What this progress belongs to (an instance id, or "import").
    pub task: String,
    pub message: String,
    pub done: u64,
    pub total: u64,
}

pub type OnProgress = Arc<dyn Fn(Progress) + Send + Sync>;

pub fn progress(on: &OnProgress, task: &str, message: impl Into<String>, done: u64, total: u64) {
    on(Progress { task: task.into(), message: message.into(), done, total });
}

/// Base URLs, overridable so tests can point everything at a local server.
pub mod urls {
    fn var(name: &str, default: &str) -> String {
        std::env::var(name).unwrap_or_else(|_| default.to_string())
    }
    pub fn version_manifest() -> String {
        var("ARCADIA_MC_MANIFEST", "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
    }
    pub fn resources() -> String {
        var("ARCADIA_MC_RESOURCES", "https://resources.download.minecraft.net")
    }
    pub fn libraries() -> String {
        var("ARCADIA_MC_LIBRARIES", "https://libraries.minecraft.net")
    }
    pub fn java_runtimes() -> String {
        var(
            "ARCADIA_MC_JAVA",
            "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json",
        )
    }
    pub fn fabric_meta() -> String {
        var("ARCADIA_FABRIC_META", "https://meta.fabricmc.net")
    }
    pub fn neoforge_maven() -> String {
        var("ARCADIA_NEOFORGE_MAVEN", "https://maven.neoforged.net")
    }
    pub fn forge_maven() -> String {
        var("ARCADIA_FORGE_MAVEN", "https://maven.minecraftforge.net")
    }
    pub fn modrinth() -> String {
        var("ARCADIA_MODRINTH", "https://api.modrinth.com")
    }
    pub fn curseforge() -> String {
        var("ARCADIA_CURSEFORGE", "https://api.curseforge.com")
    }
}

/// Reject archive/pack paths that escape their folder.
pub fn safe_rel_path(p: &str) -> Option<PathBuf> {
    let p = p.replace('\\', "/");
    if p.starts_with('/') || p.contains(':') {
        return None;
    }
    let rel = PathBuf::from(&p);
    if rel.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        return None;
    }
    Some(rel)
}

/// Tests that change ARCADIA_HOME must hold this so they don't race.
#[cfg(test)]
pub(crate) fn test_home() -> (std::sync::MutexGuard<'static, ()>, tempfile::TempDir) {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let t = tempfile::tempdir().unwrap();
    std::env::set_var("ARCADIA_HOME", t.path());
    (g, t)
}

#[cfg(test)]
mod tests {
    #[test]
    fn rejects_escaping_paths() {
        use super::safe_rel_path as s;
        assert!(s("mods/a.jar").is_some());
        assert!(s("../a.jar").is_none());
        assert!(s("/etc/passwd").is_none());
        assert!(s("C:/Windows/x").is_none());
        assert!(s("config\\..\\..\\x").is_none());
    }
}
