//! Games the user adds by hand (any .exe or launcher link).

use super::{Game, LaunchTarget, Source};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomGame {
    pub id: String,
    pub name: String,
    pub launch: LaunchTarget,
    #[serde(default)]
    pub cover: Option<String>,
}

fn load() -> Vec<CustomGame> {
    std::fs::read_to_string(crate::paths::custom_library_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save(list: &[CustomGame]) -> anyhow::Result<()> {
    crate::paths::ensure_all()?;
    crate::paths::write_atomic(&crate::paths::custom_library_file(), &serde_json::to_vec_pretty(list)?)?;
    Ok(())
}

pub fn add_custom(name: String, launch: LaunchTarget, cover: Option<String>) -> anyhow::Result<CustomGame> {
    let mut list = load();
    let g = CustomGame { id: uuid::Uuid::new_v4().to_string(), name, launch, cover };
    list.push(g.clone());
    save(&list)?;
    Ok(g)
}

pub fn remove_custom(id: &str) -> anyhow::Result<()> {
    let mut list = load();
    let id = id.strip_prefix("custom:").unwrap_or(id);
    list.retain(|g| g.id != id);
    save(&list)
}

pub fn scan() -> anyhow::Result<Option<Vec<Game>>> {
    let list = load();
    if list.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        list.into_iter()
            .map(|c| Game {
                id: format!("custom:{}", c.id),
                name: c.name,
                source: Source::Custom,
                install_dir: match &c.launch {
                    LaunchTarget::Exe { path, .. } => std::path::Path::new(path)
                        .parent()
                        .map(|p| p.to_string_lossy().to_string()),
                    _ => None,
                },
                launch: c.launch,
                cover: c.cover,
                hero: None,
            })
            .collect(),
    ))
}
