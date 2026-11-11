//! Addons: optional features installed from Arcadia's marketplace.
//!
//! Each addon gets its own sandboxed folder for data and its own slots for
//! secrets. Addons can't read each other's data, and they never see secrets
//! back once stored: the backend uses them on the addon's behalf.
//!
//! ```text
//! %LOCALAPPDATA%\Arcadia\addons\<addon-id>\
//!   <key>.json        one JSON value per key
//! ```
//!
//! Secrets live in Windows Credential Manager ("Arcadia addon <id>"), or in
//! a 0600 file elsewhere.

pub mod discord;

use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Largest single value an addon may store (images are kept as data URLs).
pub const MAX_VALUE_BYTES: usize = 48 * 1024 * 1024;

pub fn root() -> PathBuf {
    crate::paths::root().join("addons")
}

/// Addon ids and keys: lowercase letters, digits, `-` and `_`. This keeps
/// every path inside the addon's own folder.
fn check_name(kind: &str, s: &str) -> Result<()> {
    let ok = !s.is_empty()
        && s.len() <= 64
        && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        && !s.starts_with('-');
    if !ok {
        bail!("Invalid addon {kind} “{s}”: use lowercase letters, digits, - and _ (up to 64)");
    }
    Ok(())
}

fn value_path(root: &Path, addon: &str, key: &str) -> Result<PathBuf> {
    check_name("id", addon)?;
    check_name("key", key)?;
    Ok(root.join(addon).join(format!("{key}.json")))
}

pub fn get_in(root: &Path, addon: &str, key: &str) -> Result<Option<Value>> {
    let p = value_path(root, addon, key)?;
    match std::fs::read(&p) {
        Ok(b) => Ok(Some(serde_json::from_slice(&b).with_context(|| format!("{} is damaged", p.display()))?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Store a value, or delete it with `None`.
pub fn set_in(root: &Path, addon: &str, key: &str, value: Option<&Value>) -> Result<()> {
    let p = value_path(root, addon, key)?;
    let Some(v) = value else {
        match std::fs::remove_file(&p) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
            _ => return Ok(()),
        }
    };
    let bytes = serde_json::to_vec(v)?;
    if bytes.len() > MAX_VALUE_BYTES {
        bail!("That's too big to save ({} MB; the limit is {} MB)", bytes.len() / 1_048_576, MAX_VALUE_BYTES / 1_048_576);
    }
    std::fs::create_dir_all(p.parent().unwrap())?;
    // Write-then-rename so a crash never leaves half a file.
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, &p)?;
    Ok(())
}

/// Keys the addon has stored, sorted.
pub fn keys_in(root: &Path, addon: &str) -> Result<Vec<String>> {
    check_name("id", addon)?;
    let mut out: Vec<String> = match std::fs::read_dir(root.join(addon)) {
        Ok(rd) => rd
            .flatten()
            .filter_map(|e| e.file_name().to_str()?.strip_suffix(".json").map(str::to_string))
            .filter(|k| check_name("key", k).is_ok())
            .collect(),
        Err(_) => Vec::new(),
    };
    out.sort();
    Ok(out)
}

/// Remove everything an addon stored (on uninstall, if the user asks).
pub fn clear_in(root: &Path, addon: &str) -> Result<()> {
    check_name("id", addon)?;
    match std::fs::remove_dir_all(root.join(addon)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

pub fn get(addon: &str, key: &str) -> Result<Option<Value>> {
    get_in(&root(), addon, key)
}
pub fn set(addon: &str, key: &str, value: Option<&Value>) -> Result<()> {
    set_in(&root(), addon, key, value)
}
pub fn keys(addon: &str) -> Result<Vec<String>> {
    keys_in(&root(), addon)
}
pub fn clear(addon: &str) -> Result<()> {
    clear_in(&root(), addon)?;
    secrets::clear(addon);
    Ok(())
}

/// Per-addon secrets (API tokens). Write-only from the UI's point of view.
pub mod secrets {
    use super::check_name;
    use anyhow::Result;

    #[cfg(windows)]
    fn service(addon: &str) -> String {
        format!("Arcadia addon {addon}")
    }

    #[cfg(windows)]
    pub fn set(addon: &str, name: &str, value: Option<&str>) -> Result<()> {
        check_name("id", addon)?;
        check_name("secret", name)?;
        let e = keyring::Entry::new(&service(addon), name)?;
        match value {
            Some(v) => e.set_password(v)?,
            None => {
                let _ = e.delete_credential();
            }
        }
        remember(addon, name, value.is_some())
    }
    #[cfg(windows)]
    pub fn get(addon: &str, name: &str) -> Option<String> {
        check_name("id", addon).ok()?;
        keyring::Entry::new(&service(addon), name).ok()?.get_password().ok()
    }

    #[cfg(not(windows))]
    pub fn set(addon: &str, name: &str, value: Option<&str>) -> Result<()> {
        check_name("id", addon)?;
        check_name("secret", name)?;
        let mut all = file_load(addon);
        match value {
            Some(v) => {
                all.insert(name.to_string(), v.to_string());
            }
            None => {
                all.remove(name);
            }
        }
        file_save(addon, &all)?;
        remember(addon, name, value.is_some())
    }
    #[cfg(not(windows))]
    pub fn get(addon: &str, name: &str) -> Option<String> {
        check_name("id", addon).ok()?;
        file_load(addon).get(name).cloned()
    }
    #[cfg(not(windows))]
    fn file_path(addon: &str) -> std::path::PathBuf {
        super::root().join(addon).join(".secrets")
    }
    #[cfg(not(windows))]
    fn file_load(addon: &str) -> std::collections::BTreeMap<String, String> {
        std::fs::read(file_path(addon)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }
    #[cfg(not(windows))]
    fn file_save(addon: &str, all: &std::collections::BTreeMap<String, String>) -> Result<()> {
        let p = file_path(addon);
        std::fs::create_dir_all(p.parent().unwrap())?;
        std::fs::write(&p, serde_json::to_vec(all)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    /// Which secrets are set is ordinary data, so the UI can show "saved".
    fn remember(addon: &str, name: &str, present: bool) -> Result<()> {
        let mut names: Vec<String> = super::get(addon, "_secrets")?
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        names.retain(|n| n != name);
        if present {
            names.push(name.to_string());
        }
        super::set(addon, "_secrets", Some(&serde_json::json!(names)))
    }

    pub fn present(addon: &str) -> Vec<String> {
        super::get(addon, "_secrets").ok().flatten().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
    }

    pub(super) fn clear(addon: &str) {
        #[cfg(windows)]
        for n in present(addon) {
            if let Ok(e) = keyring::Entry::new(&service(addon), &n) {
                let _ = e.delete_credential();
            }
        }
        let _ = addon;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn store_roundtrip_and_isolation() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        assert_eq!(get_in(r, "discord-profile", "profiles").unwrap(), None);
        set_in(r, "discord-profile", "profiles", Some(&json!([{"a": 1}]))).unwrap();
        set_in(r, "discord-profile", "config", Some(&json!({"x": true}))).unwrap();
        assert_eq!(get_in(r, "discord-profile", "profiles").unwrap(), Some(json!([{"a": 1}])));
        assert_eq!(keys_in(r, "discord-profile").unwrap(), vec!["config", "profiles"]);
        // Another addon sees nothing.
        assert_eq!(get_in(r, "other", "profiles").unwrap(), None);
        set_in(r, "discord-profile", "config", None).unwrap();
        assert_eq!(keys_in(r, "discord-profile").unwrap(), vec!["profiles"]);
        clear_in(r, "discord-profile").unwrap();
        assert!(keys_in(r, "discord-profile").unwrap().is_empty());
    }

    #[test]
    fn names_cannot_escape_the_folder() {
        let t = tempfile::tempdir().unwrap();
        for bad in ["../x", "a/b", "A", "", "-x", "a.b", "..", "a\\b"] {
            assert!(set_in(t.path(), "ok", bad, Some(&json!(1))).is_err(), "key {bad:?}");
            assert!(set_in(t.path(), bad, "ok", Some(&json!(1))).is_err(), "id {bad:?}");
        }
    }
}
