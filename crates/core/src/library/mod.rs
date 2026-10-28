//! Universal game library: one list of installed games from every launcher.

mod battlenet;
mod custom;
mod ea;
mod epic;
mod gog;
mod steam;
mod ubisoft;
pub mod vdf;

pub use custom::{add_custom, remove_custom, CustomGame};
pub use steam::art_cache_dir as steam_art_dir;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Roblox,
    Minecraft,
    Steam,
    Epic,
    Battlenet,
    Gog,
    Ea,
    Ubisoft,
    Custom,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Roblox => "Roblox",
            Source::Minecraft => "Minecraft",
            Source::Steam => "Steam",
            Source::Epic => "Epic Games",
            Source::Battlenet => "Battle.net",
            Source::Gog => "GOG",
            Source::Ea => "EA app",
            Source::Ubisoft => "Ubisoft Connect",
            Source::Custom => "Custom",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum LaunchTarget {
    /// A launcher protocol link, e.g. `steam://rungameid/730`.
    Uri { uri: String },
    /// Run an executable directly.
    Exe { path: String, args: Vec<String>, cwd: Option<String> },
    /// Handled by Arcadia's own Roblox bootstrapper.
    Roblox,
    /// Opens Arcadia's Minecraft launcher.
    Minecraft,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    /// Stable id: `<source>:<launcher id>`.
    pub id: String,
    pub name: String,
    pub source: Source,
    pub install_dir: Option<String>,
    pub launch: LaunchTarget,
    /// Portrait cover art URL, if the launcher has a public CDN for it.
    pub cover: Option<String>,
    /// Wide header/hero art URL.
    pub hero: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanReport {
    pub games: Vec<Game>,
    /// Launchers that were detected on this machine.
    pub launchers: Vec<Source>,
    /// Non-fatal problems, shown in the UI rather than failing the scan.
    pub warnings: Vec<String>,
}

/// Scan every launcher. Never fails; problems become warnings.
pub fn scan() -> ScanReport {
    let mut games = vec![roblox_entry(), minecraft_entry()];
    let mut launchers = vec![Source::Roblox, Source::Minecraft];
    let mut warnings = Vec::new();

    type Scanner = fn() -> anyhow::Result<Option<Vec<Game>>>;
    let scanners: [(Source, Scanner); 7] = [
        (Source::Steam, steam::scan),
        (Source::Epic, epic::scan),
        (Source::Battlenet, battlenet::scan),
        (Source::Gog, gog::scan),
        (Source::Ea, ea::scan),
        (Source::Ubisoft, ubisoft::scan),
        (Source::Custom, custom::scan),
    ];
    for (src, f) in scanners {
        match f() {
            Ok(Some(mut g)) => {
                launchers.push(src);
                games.append(&mut g);
            }
            Ok(None) => {}
            Err(e) => warnings.push(format!("{}: {e:#}", src.label())),
        }
    }
    // Same game reported twice (e.g. two Steam libraries) – keep the first.
    let mut seen = std::collections::HashSet::new();
    games.retain(|g| seen.insert(g.id.clone()));
    games[2..].sort_by_key(|g| g.name.to_lowercase());
    ScanReport { games, launchers, warnings }
}

fn minecraft_entry() -> Game {
    Game {
        id: "minecraft:launcher".into(),
        name: "Minecraft".into(),
        source: Source::Minecraft,
        install_dir: Some(crate::paths::minecraft_dir().to_string_lossy().into()),
        launch: LaunchTarget::Minecraft,
        cover: None,
        hero: None,
    }
}

fn roblox_entry() -> Game {
    Game {
        id: "roblox:player".into(),
        name: "Roblox".into(),
        source: Source::Roblox,
        install_dir: Some(crate::paths::roblox_versions_dir().to_string_lossy().into()),
        launch: LaunchTarget::Roblox,
        cover: None,
        hero: None,
    }
}

/// Open a URI with the OS handler.
pub fn open_uri(uri: &str) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        // url.dll handles '&' and spaces in the URI, unlike `cmd /c start`.
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", uri])
            .spawn()?;
    }
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(uri).spawn()?;
    #[cfg(all(unix, not(target_os = "macos")))]
    std::process::Command::new("xdg-open").arg(uri).spawn()?;
    Ok(())
}

/// Launch a non-Roblox game. Roblox launches go through [`crate::roblox`].
pub fn launch(target: &LaunchTarget) -> anyhow::Result<()> {
    match target {
        LaunchTarget::Uri { uri } => open_uri(uri),
        LaunchTarget::Exe { path, args, cwd } => {
            let mut c = std::process::Command::new(path);
            c.args(args);
            let dir = cwd
                .clone()
                .or_else(|| std::path::Path::new(path).parent().map(|p| p.to_string_lossy().into()));
            if let Some(d) = dir {
                c.current_dir(d);
            }
            c.spawn()?;
            Ok(())
        }
        LaunchTarget::Roblox => anyhow::bail!("Roblox launches are handled by the bootstrapper"),
        LaunchTarget::Minecraft => anyhow::bail!("Minecraft opens in Arcadia's own launcher"),
    }
}

/// Strip quotes and a trailing ",0" icon index from a registry path value.
pub(crate) fn clean_exe_path(s: &str) -> String {
    let s = s.trim();
    let s = s.split(",").next().unwrap_or(s);
    s.trim_matches('"').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleans_icon_paths() {
        assert_eq!(clean_exe_path(r#""C:\Games\X\x.exe",0"#), r"C:\Games\X\x.exe");
        assert_eq!(clean_exe_path(r"C:\G\y.exe"), r"C:\G\y.exe");
    }
}
