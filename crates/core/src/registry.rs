//! Thin Windows registry helpers. On other platforms every read returns
//! nothing, so scanners simply find no registry-based games.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hive {
    CurrentUser,
    LocalMachine,
}

/// One entry from the "Add or remove programs" list.
#[derive(Debug, Clone, Default)]
pub struct UninstallEntry {
    pub key: String,
    pub display_name: String,
    pub publisher: String,
    pub install_location: String,
    pub display_icon: String,
    pub uninstall_string: String,
}

#[cfg(windows)]
mod imp {
    use super::{Hive, UninstallEntry};
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};
    use winreg::RegKey;

    fn hive(h: Hive) -> RegKey {
        match h {
            Hive::CurrentUser => RegKey::predef(HKEY_CURRENT_USER),
            Hive::LocalMachine => RegKey::predef(HKEY_LOCAL_MACHINE),
        }
    }

    pub fn read_string(h: Hive, path: &str, name: &str) -> Option<String> {
        hive(h).open_subkey(path).ok()?.get_value::<String, _>(name).ok()
    }

    pub fn subkeys(h: Hive, path: &str) -> Vec<String> {
        match hive(h).open_subkey(path) {
            Ok(k) => k.enum_keys().filter_map(Result::ok).collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn uninstall_entries() -> Vec<UninstallEntry> {
        const PATH: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
        let mut out = Vec::new();
        let views = [
            (HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY),
            (HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY),
            (HKEY_CURRENT_USER, 0),
        ];
        for (root, view) in views {
            let Ok(base) = RegKey::predef(root).open_subkey_with_flags(PATH, KEY_READ | view) else {
                continue;
            };
            for name in base.enum_keys().filter_map(Result::ok) {
                let Ok(k) = base.open_subkey_with_flags(&name, KEY_READ | view) else { continue };
                let get = |v: &str| k.get_value::<String, _>(v).unwrap_or_default();
                let display_name = get("DisplayName");
                if display_name.is_empty() {
                    continue;
                }
                out.push(UninstallEntry {
                    key: name.clone(),
                    display_name,
                    publisher: get("Publisher"),
                    install_location: get("InstallLocation"),
                    display_icon: get("DisplayIcon"),
                    uninstall_string: get("UninstallString"),
                });
            }
        }
        out
    }

    /// Register a URL protocol (e.g. `roblox-player`) to open `exe "%1"`.
    pub fn register_protocol(scheme: &str, friendly: &str, exe: &str) -> std::io::Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (k, _) = hkcu.create_subkey(format!(r"Software\Classes\{scheme}"))?;
        k.set_value("", &format!("URL: {friendly} Protocol"))?;
        k.set_value("URL Protocol", &"")?;
        let (icon, _) = k.create_subkey("DefaultIcon")?;
        icon.set_value("", &format!("\"{exe}\",0"))?;
        let (cmd, _) = k.create_subkey(r"shell\open\command")?;
        cmd.set_value("", &format!("\"{exe}\" \"%1\""))?;
        Ok(())
    }

    pub fn protocol_command(scheme: &str) -> Option<String> {
        read_string(Hive::CurrentUser, &format!(r"Software\Classes\{scheme}\shell\open\command"), "")
    }

    pub fn set_run_on_boot(name: &str, command: Option<&str>) -> std::io::Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (k, _) = hkcu.create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")?;
        match command {
            Some(c) => k.set_value(name, &c.to_string()),
            None => match k.delete_value(name) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            },
        }
    }
    // ── FrameForge helpers ────────────────────────────────────────────────

    pub fn read_u32(h: Hive, path: &str, name: &str) -> Option<u32> {
        hive(h).open_subkey(path).ok()?.get_value::<u32, _>(name).ok()
    }

    /// Every string value under a key, as `(name, data)`.
    pub fn string_values(h: Hive, path: &str) -> Vec<(String, String)> {
        let Ok(k) = hive(h).open_subkey(path) else { return Vec::new() };
        k.enum_values()
            .filter_map(Result::ok)
            .filter_map(|(name, _)| k.get_value::<String, _>(&name).ok().map(|v| (name, v)))
            .collect()
    }

    /// Write a DWORD under HKEY_CURRENT_USER, creating the key if needed.
    pub fn write_user_u32(path: &str, name: &str, value: u32) -> std::io::Result<()> {
        let (k, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(path)?;
        k.set_value(name, &value)
    }

    /// Write a string under HKEY_CURRENT_USER, creating the key if needed.
    pub fn write_user_string(path: &str, name: &str, value: &str) -> std::io::Result<()> {
        let (k, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(path)?;
        k.set_value(name, &value.to_string())
    }

    /// Delete a value under HKEY_CURRENT_USER. A value that is already gone is not an error.
    pub fn delete_user_value(path: &str, name: &str) -> std::io::Result<()> {
        let Ok(k) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(path, winreg::enums::KEY_SET_VALUE) else {
            return Ok(());
        };
        match k.delete_value(name) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{Hive, UninstallEntry};
    pub fn read_string(_: Hive, _: &str, _: &str) -> Option<String> {
        None
    }
    pub fn subkeys(_: Hive, _: &str) -> Vec<String> {
        Vec::new()
    }
    pub fn uninstall_entries() -> Vec<UninstallEntry> {
        Vec::new()
    }
    pub fn register_protocol(_: &str, _: &str, _: &str) -> std::io::Result<()> {
        Ok(())
    }
    pub fn protocol_command(_: &str) -> Option<String> {
        None
    }
    pub fn set_run_on_boot(_: &str, _: Option<&str>) -> std::io::Result<()> {
        Ok(())
    }
    pub fn read_u32(_: Hive, _: &str, _: &str) -> Option<u32> {
        None
    }
    pub fn string_values(_: Hive, _: &str) -> Vec<(String, String)> {
        Vec::new()
    }
    pub fn write_user_u32(_: &str, _: &str, _: u32) -> std::io::Result<()> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "the registry only exists on Windows"))
    }
    pub fn write_user_string(_: &str, _: &str, _: &str) -> std::io::Result<()> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "the registry only exists on Windows"))
    }
    pub fn delete_user_value(_: &str, _: &str) -> std::io::Result<()> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "the registry only exists on Windows"))
    }
}

pub use imp::*;
