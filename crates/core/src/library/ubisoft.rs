use super::{Game, LaunchTarget, Source};
use crate::registry::{read_string, subkeys, uninstall_entries, Hive};

const KEY: &str = r"SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs";

pub fn scan() -> anyhow::Result<Option<Vec<Game>>> {
    let ids = subkeys(Hive::LocalMachine, KEY);
    if ids.is_empty() {
        return Ok(None);
    }
    let uninstall = uninstall_entries();
    let mut out = Vec::new();
    for id in ids {
        let dir = read_string(Hive::LocalMachine, &format!(r"{KEY}\{id}"), "InstallDir");
        // Ubisoft keeps the display name in "Uplay Install <id>".
        let name = uninstall
            .iter()
            .find(|e| e.key == format!("Uplay Install {id}"))
            .map(|e| e.display_name.clone())
            .or_else(|| {
                dir.as_ref().and_then(|d| {
                    std::path::Path::new(d.trim_end_matches(['/', '\\']))
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                })
            })
            .unwrap_or_else(|| format!("Ubisoft game {id}"));
        out.push(Game {
            id: format!("ubisoft:{id}"),
            name,
            source: Source::Ubisoft,
            install_dir: dir,
            launch: LaunchTarget::Uri { uri: format!("uplay://launch/{id}/0") },
            cover: None,
            hero: None,
        });
    }
    Ok(Some(out))
}
