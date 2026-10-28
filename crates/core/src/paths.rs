//! Where Arcadia keeps its files.
//!
//! ```text
//! %LOCALAPPDATA%\Arcadia\
//!   settings.json
//!   library-custom.json        user-added games
//!   proxy\ca.pem, ca.key.pem   local certificate authority
//!   proxy\rules.json           asset replacement rules
//!   proxy\files\               local files served as replacements
//!   Roblox\Versions\version-*  Roblox installs managed by Arcadia
//!   Roblox\Downloads\          package cache
//!   Modifications\             user mods overlaid onto Roblox on launch
//! ```

use std::path::PathBuf;

/// Root data directory. Overridable with `ARCADIA_HOME` (used by tests).
pub fn root() -> PathBuf {
    if let Ok(p) = std::env::var("ARCADIA_HOME") {
        return PathBuf::from(p);
    }
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Arcadia")
}

pub fn settings_file() -> PathBuf {
    root().join("settings.json")
}
pub fn custom_library_file() -> PathBuf {
    root().join("library-custom.json")
}
pub fn proxy_dir() -> PathBuf {
    root().join("proxy")
}
pub fn proxy_files_dir() -> PathBuf {
    proxy_dir().join("files")
}
pub fn roblox_dir() -> PathBuf {
    root().join("Roblox")
}
pub fn roblox_versions_dir() -> PathBuf {
    roblox_dir().join("Versions")
}
pub fn roblox_downloads_dir() -> PathBuf {
    roblox_dir().join("Downloads")
}
pub fn mods_dir() -> PathBuf {
    root().join("Modifications")
}
pub fn wallpapers_dir() -> PathBuf {
    root().join("wallpapers")
}
pub fn minecraft_dir() -> PathBuf {
    root().join("Minecraft")
}
pub fn logs_dir() -> PathBuf {
    root().join("Logs")
}

/// Roblox's own per-user folder (logs, cache database, settings).
pub fn roblox_local_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Roblox")
}

pub fn ensure_all() -> std::io::Result<()> {
    for d in [
        root(),
        proxy_dir(),
        proxy_files_dir(),
        roblox_versions_dir(),
        roblox_downloads_dir(),
        mods_dir(),
        logs_dir(),
    ] {
        std::fs::create_dir_all(d)?;
    }
    Ok(())
}
