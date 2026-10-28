//! Roblox bootstrapper: install/update, FastFlags, mods, proxy wiring, launch.

pub mod activity;
pub mod cache;
pub mod deploy;
pub mod fflags;
pub mod mods;

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

pub use deploy::Progress;

const CA_BEGIN: &str = "# >>> Arcadia local CA >>>";
const CA_END: &str = "# <<< Arcadia local CA <<<";

/// Register Arcadia as the handler for Roblox's join links.
pub fn register_protocols(arcadia_exe: &str) -> Result<()> {
    crate::registry::register_protocol("roblox-player", "Roblox", arcadia_exe)?;
    crate::registry::register_protocol("roblox", "Roblox", arcadia_exe)?;
    Ok(())
}

pub fn is_protocol_registered(arcadia_exe: &str) -> bool {
    crate::registry::protocol_command("roblox-player")
        .is_some_and(|c| c.to_lowercase().contains(&arcadia_exe.to_lowercase()))
}

/// Is this command-line argument a Roblox launch link?
pub fn is_roblox_uri(arg: &str) -> bool {
    let a = arg.to_ascii_lowercase();
    a.starts_with("roblox-player:") || a.starts_with("roblox:")
}

/// Add (or refresh) Arcadia's CA in the client's `ssl/cacert.pem`, so the
/// client trusts the local proxy's certificates.
pub fn install_ca(version_dir: &Path, ca_pem: &str) -> Result<()> {
    let path = version_dir.join("ssl").join("cacert.pem");
    let cur = std::fs::read_to_string(&path).unwrap_or_default();
    let cleaned = strip_ca(&cur);
    let mut out = cleaned.trim_end().to_string();
    out.push_str(&format!("\n\n{CA_BEGIN}\n{}\n{CA_END}\n", ca_pem.trim()));
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, out)?;
    Ok(())
}

pub fn remove_ca(version_dir: &Path) -> Result<()> {
    let path = version_dir.join("ssl").join("cacert.pem");
    if let Ok(cur) = std::fs::read_to_string(&path) {
        std::fs::write(&path, strip_ca(&cur))?;
    }
    Ok(())
}

fn strip_ca(s: &str) -> String {
    match (s.find(CA_BEGIN), s.find(CA_END)) {
        (Some(a), Some(b)) if b > a => format!("{}{}", &s[..a], &s[b + CA_END.len()..]).trim_end().to_string() + "\n",
        _ => s.to_string(),
    }
}

/// What the proxy needs the launcher to know.
#[derive(Debug, Clone)]
pub struct ProxyHook {
    pub port: u16,
    pub ca_pem: String,
}

pub struct Launched {
    pub child: Child,
    pub version_dir: PathBuf,
    pub version: String,
}

/// Install/update, prepare the version folder, then start RobloxPlayerBeta.
///
/// `uri` is the `roblox-player:` link from the browser, or `None` to open the
/// Roblox app home screen.
pub async fn launch(
    settings: &crate::Settings,
    uri: Option<&str>,
    proxy: Option<ProxyHook>,
    on: &(dyn Fn(Progress) + Send + Sync),
) -> Result<Launched> {
    let dir = deploy::ensure(settings, on).await?;
    let version = dir.file_name().unwrap().to_string_lossy().to_string();
    let exe = dir.join("RobloxPlayerBeta.exe");

    // FastFlags (local route).
    let flags = fflags::compose(&settings.fast_flags, proxy.is_some());
    fflags::write(&dir, &flags).context("writing FastFlags")?;

    // Mods + custom font.
    let mut extra = Vec::new();
    if let Some(font) = settings.roblox_custom_font.as_deref().filter(|f| !f.is_empty()) {
        extra = mods::custom_font_files(&dir, Path::new(font)).context("preparing custom font")?;
    }
    if settings.roblox_apply_mods || !extra.is_empty() {
        let mods_dir = crate::paths::mods_dir();
        let mods_dir = settings.roblox_apply_mods.then_some(mods_dir.as_path());
        mods::apply(&dir, mods_dir, extra).context("applying mods")?;
    } else {
        mods::revert(&dir)?;
    }

    // Proxy wiring.
    match &proxy {
        Some(p) => {
            install_ca(&dir, &p.ca_pem)?;
            if settings.proxy_clear_cache_on_launch {
                cache::clear();
            }
        }
        None => remove_ca(&dir)?,
    }

    if settings.roblox_multi_instance {
        multi_instance::hold();
    }

    let mut cmd = Command::new(&exe);
    cmd.current_dir(&dir);
    match uri {
        Some(u) => cmd.arg(u),
        None => cmd.arg("--app"),
    };
    if let Some(p) = &proxy {
        let url = format!("http://127.0.0.1:{}", p.port);
        cmd.env("HTTPS_PROXY", &url).env("HTTP_PROXY", &url).env("https_proxy", &url).env("http_proxy", &url);
        cmd.env_remove("NO_PROXY").env_remove("no_proxy");
    }
    set_priority(&mut cmd, &settings.roblox_priority);
    let child = cmd.spawn().with_context(|| format!("starting {}", exe.display()))?;
    Ok(Launched { child, version_dir: dir, version })
}

#[cfg(windows)]
fn set_priority(cmd: &mut Command, p: &str) {
    use std::os::windows::process::CommandExt;
    let flag = match p {
        "High" => 0x0000_0080,
        "AboveNormal" => 0x0000_8000,
        "BelowNormal" => 0x0000_4000,
        _ => 0x0000_0020,
    };
    cmd.creation_flags(flag);
}
#[cfg(not(windows))]
fn set_priority(_: &mut Command, _: &str) {}

/// Roblox refuses to start a second client while `ROBLOX_singletonMutex`
/// exists and is owned by someone else; owning it ourselves lifts the limit.
pub mod multi_instance {
    #[cfg(windows)]
    pub fn hold() {
        use std::sync::OnceLock;
        use windows_sys::Win32::System::Threading::CreateMutexW;
        static HELD: OnceLock<usize> = OnceLock::new();
        HELD.get_or_init(|| {
            let name: Vec<u16> = "ROBLOX_singletonMutex".encode_utf16().chain(Some(0)).collect();
            // SAFETY: valid null-terminated wide string; handle is kept for the
            // life of the process on purpose.
            unsafe { CreateMutexW(std::ptr::null(), 1, name.as_ptr()) as usize }
        });
    }
    #[cfg(not(windows))]
    pub fn hold() {}
}

/// Kill every running Roblox Player (used before clearing a locked cache).
pub fn kill_all() {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "RobloxPlayerBeta.exe"])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ca_block_is_idempotent() {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(t.path().join("ssl")).unwrap();
        std::fs::write(t.path().join("ssl/cacert.pem"), "ORIGINAL\n").unwrap();
        install_ca(t.path(), "PEM-A").unwrap();
        install_ca(t.path(), "PEM-B").unwrap();
        let s = std::fs::read_to_string(t.path().join("ssl/cacert.pem")).unwrap();
        assert!(s.starts_with("ORIGINAL"));
        assert!(s.contains("PEM-B") && !s.contains("PEM-A"));
        assert_eq!(s.matches(CA_BEGIN).count(), 1);
        remove_ca(t.path()).unwrap();
        assert_eq!(std::fs::read_to_string(t.path().join("ssl/cacert.pem")).unwrap(), "ORIGINAL\n");
    }

    #[test]
    fn detects_roblox_links() {
        assert!(is_roblox_uri("roblox-player:1+launchmode:play+gameinfo:abc"));
        assert!(is_roblox_uri("roblox://experiences/start?placeId=1"));
        assert!(!is_roblox_uri("steam://rungameid/1"));
    }
}
