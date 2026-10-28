use super::{Game, LaunchTarget, Source};
use crate::registry::{read_string, subkeys, Hive};

const KEY: &str = r"SOFTWARE\WOW6432Node\GOG.com\Games";

pub fn scan() -> anyhow::Result<Option<Vec<Game>>> {
    let ids = subkeys(Hive::LocalMachine, KEY);
    if ids.is_empty() {
        return Ok(None);
    }
    let mut out = Vec::new();
    for id in ids {
        let k = format!(r"{KEY}\{id}");
        let get = |v: &str| read_string(Hive::LocalMachine, &k, v);
        let Some(name) = get("gameName") else { continue };
        // GOG writes DLC under the same key with a dependsOn value.
        if get("dependsOn").is_some_and(|d| !d.is_empty()) {
            continue;
        }
        let path = get("path");
        let launch = match get("exe") {
            Some(exe) => LaunchTarget::Exe {
                path: exe,
                args: get("launchParam")
                    .filter(|p| !p.is_empty())
                    .map(|p| p.split_whitespace().map(String::from).collect())
                    .unwrap_or_default(),
                cwd: get("workingDir").or_else(|| path.clone()),
            },
            None => LaunchTarget::Uri { uri: format!("goggalaxy://openGameView/{id}") },
        };
        out.push(Game {
            id: format!("gog:{id}"),
            name,
            source: Source::Gog,
            install_dir: path,
            launch,
            cover: None,
            hero: None,
        });
    }
    Ok(Some(out))
}
