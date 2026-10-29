use super::{vdf, Game, LaunchTarget, Source};
use crate::registry::{read_string, Hive};
use std::path::{Path, PathBuf};

/// App ids that are tools/runtimes rather than games.
const NOT_GAMES: &[&str] = &[
    "228980",  // Steamworks Common Redistributables
    "1070560", // Steam Linux Runtime
    "1391110", // Steam Linux Runtime - Soldier
    "1628350", // Steam Linux Runtime - Sniper
    "1493710", // Proton Experimental
    "2180100", // Proton Hotfix
    "250820",  // SteamVR
];

/// Steam's local artwork cache, if Steam is installed.
pub fn art_cache_dir() -> Option<PathBuf> {
    let d = root()?.join("appcache").join("librarycache");
    d.is_dir().then_some(d)
}

/// Find a cached image for `appid`. Steam has used three layouts:
/// `<appid>_library_600x900.jpg`, `<appid>/library_600x900.jpg`, and
/// `<appid>/<hash>/library_600x900.jpg`.
fn cached_art(cache: &Path, appid: &str, file: &str) -> Option<String> {
    let flat = cache.join(format!("{appid}_{file}"));
    if flat.is_file() {
        return Some(flat.to_string_lossy().into());
    }
    let dir = cache.join(appid);
    let direct = dir.join(file);
    if direct.is_file() {
        return Some(direct.to_string_lossy().into());
    }
    std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|e| e.path().join(file))
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().into())
}

pub(crate) fn root() -> Option<PathBuf> {
    if let Some(p) = read_string(Hive::CurrentUser, r"Software\Valve\Steam", "SteamPath") {
        return Some(PathBuf::from(p));
    }
    if let Some(p) = read_string(Hive::LocalMachine, r"SOFTWARE\WOW6432Node\Valve\Steam", "InstallPath") {
        return Some(PathBuf::from(p));
    }
    let home = dirs::home_dir()?;
    [
        home.join(".steam/steam"),
        home.join(".local/share/Steam"),
        home.join("Library/Application Support/Steam"),
    ]
    .into_iter()
    .find(|p| p.join("steamapps").is_dir())
}

pub fn scan() -> anyhow::Result<Option<Vec<Game>>> {
    let Some(root) = root() else { return Ok(None) };
    Ok(Some(scan_root(&root)))
}

pub(crate) fn scan_root(root: &Path) -> Vec<Game> {
    let mut libs = vec![root.to_path_buf()];
    if let Ok(src) = std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
        let v = vdf::parse(&src);
        if let Some(lf) = v.get("libraryfolders") {
            for (_, e) in lf.entries() {
                if let Some(p) = e.str("path") {
                    let p = PathBuf::from(p);
                    if !libs.contains(&p) {
                        libs.push(p);
                    }
                }
            }
        }
    }

    let cache = root.join("appcache").join("librarycache");
    let mut games = Vec::new();
    for lib in libs {
        let apps = lib.join("steamapps");
        let Ok(rd) = std::fs::read_dir(&apps) else { continue };
        for f in rd.flatten() {
            let name = f.file_name().to_string_lossy().to_string();
            if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                continue;
            }
            let Ok(src) = std::fs::read_to_string(f.path()) else { continue };
            let v = vdf::parse(&src);
            let Some(st) = v.get("AppState") else { continue };
            let (Some(appid), Some(title)) = (st.str("appid"), st.str("name")) else { continue };
            if NOT_GAMES.contains(&appid) || title.starts_with("Proton ") {
                continue;
            }
            let dir = st.str("installdir").map(|d| apps.join("common").join(d).to_string_lossy().to_string());
            let cdn = "https://shared.steamstatic.com/store_item_assets/steam/apps";
            games.push(Game {
                id: format!("steam:{appid}"),
                name: title.to_string(),
                source: Source::Steam,
                install_dir: dir,
                launch: LaunchTarget::Uri { uri: format!("steam://rungameid/{appid}") },
                cover: cached_art(&cache, appid, "library_600x900.jpg")
                    .or_else(|| Some(format!("{cdn}/{appid}/library_600x900.jpg"))),
                hero: cached_art(&cache, appid, "library_hero.jpg")
                    .or_else(|| Some(format!("{cdn}/{appid}/library_hero.jpg"))),
            });
        }
    }
    games
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_fake_steam_install() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("Steam");
        let lib2 = t.path().join("Lib2");
        std::fs::create_dir_all(root.join("steamapps")).unwrap();
        std::fs::create_dir_all(lib2.join("steamapps")).unwrap();
        std::fs::write(
            root.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\" {{ \"0\" {{ \"path\" \"{}\" }} \"1\" {{ \"path\" \"{}\" }} }}",
                root.display().to_string().replace('\\', "\\\\"),
                lib2.display().to_string().replace('\\', "\\\\")
            ),
        )
        .unwrap();
        std::fs::write(
            root.join("steamapps/appmanifest_730.acf"),
            r#""AppState" { "appid" "730" "name" "Counter-Strike 2" "installdir" "cs2" }"#,
        )
        .unwrap();
        std::fs::write(
            root.join("steamapps/appmanifest_228980.acf"),
            r#""AppState" { "appid" "228980" "name" "Steamworks Common Redistributables" }"#,
        )
        .unwrap();
        std::fs::write(
            lib2.join("steamapps/appmanifest_1086940.acf"),
            r#""AppState" { "appid" "1086940" "name" "Baldur's Gate 3" "installdir" "BG3" }"#,
        )
        .unwrap();

        // Cached art in the newer per-app folder layout.
        std::fs::create_dir_all(root.join("appcache/librarycache/730/abc123")).unwrap();
        std::fs::write(root.join("appcache/librarycache/730/abc123/library_600x900.jpg"), b"jpg").unwrap();

        let mut g = scan_root(&root);
        g.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(g.len(), 2);
        assert!(g[1].cover.as_deref().unwrap().ends_with("library_600x900.jpg"));
        assert!(!g[1].cover.as_deref().unwrap().starts_with("https"));
        assert!(g[0].cover.as_deref().unwrap().starts_with("https://shared.steamstatic.com"));
        assert_eq!(g[0].name, "Baldur's Gate 3");
        assert_eq!(g[1].launch, LaunchTarget::Uri { uri: "steam://rungameid/730".into() });
    }
}
