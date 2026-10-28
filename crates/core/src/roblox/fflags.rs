//! FastFlags written to `ClientSettings/ClientAppSettings.json`.

use serde::Serialize;
use serde_json::{json, Map, Value};
use std::path::Path;

/// Named presets shown as toggles in the UI.
#[derive(Debug, Clone, Serialize)]
pub struct Preset {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub flags: Value,
}

pub fn presets() -> Vec<Preset> {
    vec![
        Preset {
            id: "dx11",
            label: "Force Direct3D 11",
            description: "Use the D3D11 renderer.",
            flags: json!({"FFlagDebugGraphicsPreferD3D11": "True"}),
        },
        Preset {
            id: "vulkan",
            label: "Force Vulkan",
            description: "Use the Vulkan renderer.",
            flags: json!({"FFlagDebugGraphicsPreferVulkan": "True"}),
        },
        Preset {
            id: "opengl",
            label: "Force OpenGL",
            description: "Use the OpenGL renderer.",
            flags: json!({"FFlagDebugGraphicsPreferOpenGL": "True"}),
        },
        Preset {
            id: "no-grass",
            label: "Disable terrain grass",
            description: "Hides grass decoration for clarity and FPS.",
            flags: json!({"FIntFRMMinGrassDistance": "0", "FIntFRMMaxGrassDistance": "0"}),
        },
        Preset {
            id: "msaa-off",
            label: "Disable MSAA",
            description: "Turns off multisample anti-aliasing.",
            flags: json!({"FIntDebugForceMSAASamples": "0"}),
        },
    ]
}

/// Build the final flag set for this launch.
///
/// The frame rate is no longer set here: Roblox ignores the old FastFlag and
/// has its own "Maximum Frame Rate" setting in-game.
pub fn compose(user: &std::collections::BTreeMap<String, Value>, proxy_enabled: bool) -> Map<String, Value> {
    let mut m: Map<String, Value> = user.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    m.remove("DFIntTaskSchedulerTargetFps");
    if proxy_enabled {
        // Same as Fleasion: make Roblox trust ssl/cacert.pem (which contains
        // Arcadia's local CA) instead of its embedded certificate bundle.
        m.insert("DFFlagHttpUseEmbeddedCaCerts".into(), Value::String("False".into()));
    }
    m
}

pub fn write(version_dir: &Path, flags: &Map<String, Value>) -> std::io::Result<()> {
    let d = version_dir.join("ClientSettings");
    std::fs::create_dir_all(&d)?;
    std::fs::write(
        d.join("ClientAppSettings.json"),
        serde_json::to_vec_pretty(flags).unwrap_or_default(),
    )
}

/// Accept a pasted JSON object of flags (from another bootstrapper or a site).
pub fn parse_import(src: &str) -> anyhow::Result<Map<String, Value>> {
    let v: Value = serde_json::from_str(src.trim())?;
    match v {
        Value::Object(m) => Ok(m),
        _ => anyhow::bail!("expected a JSON object of flag names to values"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compose_adds_proxy_flag_and_drops_old_fps_flag() {
        let mut u = std::collections::BTreeMap::new();
        u.insert("FFlagX".to_string(), json!("True"));
        u.insert("DFIntTaskSchedulerTargetFps".to_string(), json!("240"));
        let m = compose(&u, true);
        assert_eq!(m["DFFlagHttpUseEmbeddedCaCerts"], "False");
        assert_eq!(m["FFlagX"], "True");
        assert!(!m.contains_key("DFIntTaskSchedulerTargetFps"));
    }
}
