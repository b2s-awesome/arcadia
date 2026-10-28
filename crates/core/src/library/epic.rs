use super::{Game, LaunchTarget, Source};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Manifest {
    display_name: String,
    app_name: String,
    #[serde(default)]
    catalog_namespace: String,
    #[serde(default)]
    catalog_item_id: String,
    #[serde(default)]
    install_location: String,
    #[serde(default, rename = "bIsIncompleteInstall")]
    incomplete: bool,
    #[serde(default)]
    app_categories: Vec<String>,
}

fn manifests_dir() -> Option<PathBuf> {
    // %PROGRAMDATA%\Epic\EpicGamesLauncher\Data\Manifests
    let base = std::env::var_os("PROGRAMDATA").map(PathBuf::from)?;
    let d = base.join(r"Epic\EpicGamesLauncher\Data\Manifests");
    d.is_dir().then_some(d)
}

pub fn scan() -> anyhow::Result<Option<Vec<Game>>> {
    let Some(dir) = manifests_dir() else { return Ok(None) };
    Ok(Some(scan_dir(&dir)))
}

pub(crate) fn scan_dir(dir: &Path) -> Vec<Game> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for f in rd.flatten() {
        if f.path().extension().and_then(|e| e.to_str()) != Some("item") {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(f.path()) else { continue };
        let Ok(m) = serde_json::from_str::<Manifest>(&src) else { continue };
        // DLC and plugins ship their own manifests; keep only games.
        if m.incomplete || (!m.app_categories.is_empty() && !m.app_categories.iter().any(|c| c == "games")) {
            continue;
        }
        let uri = if m.catalog_namespace.is_empty() {
            format!("com.epicgames.launcher://apps/{}?action=launch&silent=true", m.app_name)
        } else {
            format!(
                "com.epicgames.launcher://apps/{}%3A{}%3A{}?action=launch&silent=true",
                m.catalog_namespace, m.catalog_item_id, m.app_name
            )
        };
        out.push(Game {
            id: format!("epic:{}", m.app_name),
            name: m.display_name,
            source: Source::Epic,
            install_dir: (!m.install_location.is_empty()).then_some(m.install_location),
            launch: LaunchTarget::Uri { uri },
            cover: None,
            hero: None,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_epic_manifests_and_skips_dlc() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(
            t.path().join("A.item"),
            r#"{"DisplayName":"Fortnite","AppName":"Fortnite","CatalogNamespace":"fn","CatalogItemId":"4fe7","InstallLocation":"C:\\FN","AppCategories":["public","games","applications"]}"#,
        ).unwrap();
        std::fs::write(
            t.path().join("B.item"),
            r#"{"DisplayName":"Some DLC","AppName":"dlc1","AppCategories":["addons"]}"#,
        ).unwrap();
        let g = scan_dir(t.path());
        assert_eq!(g.len(), 1);
        assert_eq!(
            g[0].launch,
            LaunchTarget::Uri { uri: "com.epicgames.launcher://apps/fn%3A4fe7%3AFortnite?action=launch&silent=true".into() }
        );
    }
}
