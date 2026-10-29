//! FastFlags: the allowlisted ones written to
//! `ClientSettings/ClientAppSettings.json`, and the server-delivered set
//! (see [`super::flagcache`]).

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

/// Makes Roblox trust `ssl/cacert.pem` (which holds Arcadia's CA) instead of
/// its built-in certificate list. Not on the allowlist, so it can only arrive
/// as a server flag (the flag cache and proxied ClientSettings responses).
pub const EMBEDDED_CA_FLAG: &str = "DFFlagHttpUseEmbeddedCaCerts";
/// How often Roblox re-fetches server flags. One second lets live FastFlag
/// edits reach a running client almost at once.
pub const RELOAD_INTERVAL_FLAG: &str = "DFIntSecondsBetweenDynamicVariableReloading";
pub const FPS_FLAG: &str = "DFIntTaskSchedulerTargetFps";

/// Flags for `ClientAppSettings.json`. Roblox ignores anything there that's
/// not on its allowlist, so the FPS cap and the proxy's certificate flag go
/// through [`server_flags`] instead.
pub fn compose(user: &std::collections::BTreeMap<String, Value>) -> Map<String, Value> {
    let mut m: Map<String, Value> = user.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    m.remove(FPS_FLAG);
    m.remove(EMBEDDED_CA_FLAG);
    m
}

/// Flags delivered the way Roblox's own servers deliver them: seeded into
/// its flag cache before launch, and merged into ClientSettings responses by
/// the proxy. These skip the allowlist.
///
/// - With the proxy on, the certificate flag is always included; without it
///   the proxy can't work at all.
/// - The FPS cap and live FastFlags are included only after the person has
///   accepted the ban risk (Roblox has said non-allowlisted flags can lead
///   to bans).
pub fn server_flags(settings: &crate::Settings, proxy: bool) -> Map<String, Value> {
    let mut m = Map::new();
    if settings.proxy_fflags_risk_accepted {
        for (k, v) in &settings.proxy_fflag_overrides {
            m.insert(k.clone(), Value::String(super::flagcache::flag_string(v)));
        }
        if settings.roblox_fps_cap > 0 {
            m.insert(FPS_FLAG.into(), Value::String(settings.roblox_fps_cap.to_string()));
        }
        if proxy {
            m.insert(RELOAD_INTERVAL_FLAG.into(), Value::String("1".into()));
        }
    }
    if proxy {
        m.insert(EMBEDDED_CA_FLAG.into(), Value::String("False".into()));
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
    fn local_file_keeps_only_allowlist_candidates() {
        let mut u = std::collections::BTreeMap::new();
        u.insert("FFlagX".to_string(), json!("True"));
        u.insert(FPS_FLAG.to_string(), json!("240"));
        u.insert(EMBEDDED_CA_FLAG.to_string(), json!("False"));
        let m = compose(&u);
        assert_eq!(m.len(), 1);
        assert_eq!(m["FFlagX"], "True");
    }

    #[test]
    fn server_flags_follow_proxy_and_risk_choice() {
        let mut s = crate::Settings::default();
        s.roblox_fps_cap = 240;
        s.proxy_fflag_overrides.insert("FIntY".into(), json!(3));
        assert!(server_flags(&s, false).is_empty());
        assert_eq!(server_flags(&s, true).keys().collect::<Vec<_>>(), [EMBEDDED_CA_FLAG]);

        s.proxy_fflags_risk_accepted = true;
        let m = server_flags(&s, true);
        assert_eq!(m[EMBEDDED_CA_FLAG], "False");
        assert_eq!(m[FPS_FLAG], "240");
        assert_eq!(m["FIntY"], "3");
        assert_eq!(m[RELOAD_INTERVAL_FLAG], "1");
        let m = server_flags(&s, false);
        assert!(!m.contains_key(EMBEDDED_CA_FLAG) && !m.contains_key(RELOAD_INTERVAL_FLAG));
        assert_eq!(m[FPS_FLAG], "240");
    }
}
