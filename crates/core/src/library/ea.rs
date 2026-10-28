//! EA app. EA doesn't expose a stable per-game launch link without an offer
//! id, so games found in "Add or remove programs" launch their own exe, which
//! hands off to the EA app for entitlement checks as normal.

use super::{clean_exe_path, Game, LaunchTarget, Source};
use crate::registry::uninstall_entries;

pub fn scan() -> anyhow::Result<Option<Vec<Game>>> {
    let all = uninstall_entries();
    let has_launcher = all
        .iter()
        .any(|e| e.display_name == "EA app" || e.display_name == "Origin");
    let games: Vec<Game> = all
        .into_iter()
        .filter(|e| {
            e.publisher.starts_with("Electronic Arts")
                && !matches!(e.display_name.as_str(), "EA app" | "Origin" | "EA Desktop")
                && e.display_icon.to_ascii_lowercase().contains(".exe")
        })
        .map(|e| Game {
            id: format!("ea:{}", e.key),
            name: e.display_name,
            source: Source::Ea,
            install_dir: (!e.install_location.is_empty()).then_some(e.install_location),
            launch: LaunchTarget::Exe { path: clean_exe_path(&e.display_icon), args: vec![], cwd: None },
            cover: None,
            hero: None,
        })
        .collect();
    Ok((has_launcher || !games.is_empty()).then_some(games))
}
