//! Roblox's cached server FastFlags: `%TEMP%\Roblox\cache\flag_cache.dat`.
//!
//! Since Roblox's FastFlag allowlist (Sept 2025), flags in
//! `ClientAppSettings.json` that aren't on the list are ignored. That includes
//! `DFFlagHttpUseEmbeddedCaCerts`, which the asset proxy needs so Roblox
//! trusts `ssl/cacert.pem` (where Arcadia's CA is). Server-delivered flags
//! aren't filtered, and Roblox reads this cache of them at startup, before its
//! first network request, so seeding it makes the very first connection trust
//! the proxy. The proxy then keeps the flag in every ClientSettings response,
//! which Roblox writes back into this file.
//!
//! Layout (the one Fleasion 2.4.1 relies on): u32 little-endian signature
//! length, the signature, one compression byte (0 = none), then the
//! ClientSettings JSON `{"applicationSettings": {...}}`. Arcadia only edits
//! the uncompressed layout and keeps the header as it is.

use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

pub fn path() -> PathBuf {
    std::env::temp_dir().join("Roblox").join("cache").join("flag_cache.dat")
}

/// Which flags Arcadia put there last time, so they can be taken out again
/// when they're no longer wanted.
fn seeded_list_path() -> PathBuf {
    crate::paths::proxy_dir().join("seeded-flags.json")
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Seed {
    /// The file now carries exactly the wanted flags.
    Written { flags: usize, removed: usize },
    /// Nothing to change.
    Unchanged,
    /// Roblox hasn't created the file yet (it does on its first run).
    Missing,
    /// The file isn't in the layout Arcadia knows how to edit.
    Unsupported { reason: String },
}

impl Seed {
    pub fn ok(&self) -> bool {
        matches!(self, Seed::Written { .. } | Seed::Unchanged)
    }
}

/// Roblox stores every flag value as a string.
pub fn flag_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(b) => if *b { "True".into() } else { "False".into() },
        other => other.to_string(),
    }
}

/// Make the cache at `file` carry `wanted`, and drop any of `previous` that
/// aren't wanted any more. Pure file logic, separate for testing.
pub fn seed_file(file: &Path, wanted: &Map<String, Value>, previous: &[String]) -> Seed {
    let raw = match std::fs::read(file) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Seed::Missing,
        Err(e) => return Seed::Unsupported { reason: format!("can't read it: {e}") },
    };
    let unsupported = |reason: &str| Seed::Unsupported { reason: reason.into() };
    if raw.len() < 5 {
        return unsupported("too short");
    }
    let sig_len = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize;
    let Some(comp_at) = sig_len.checked_add(4).filter(|&i| i < raw.len()) else {
        return unsupported("unexpected header");
    };
    if raw[comp_at] != 0 {
        return unsupported("compressed (Roblox changed the format)");
    }
    let body_at = comp_at + 1;
    let Ok(mut doc) = serde_json::from_slice::<Value>(&raw[body_at..]) else {
        return unsupported("settings aren't plain JSON");
    };
    let Some(settings) = doc.get_mut("applicationSettings").and_then(Value::as_object_mut) else {
        return unsupported("no applicationSettings");
    };

    let mut removed = 0;
    for name in previous.iter().filter(|n| !wanted.contains_key(*n)) {
        if settings.remove(name).is_some() {
            removed += 1;
        }
    }
    for (k, v) in wanted {
        settings.insert(k.clone(), Value::String(flag_string(v)));
    }
    let Ok(json) = serde_json::to_vec(&doc) else { return unsupported("couldn't encode") };
    let mut out = raw[..body_at].to_vec();
    out.extend_from_slice(&json);
    if out == raw {
        return Seed::Unchanged;
    }
    // Write beside it and swap in, so Roblox never sees half a file.
    let tmp = file.with_extension(format!("arcadia-{}.tmp", std::process::id()));
    let res = std::fs::write(&tmp, &out).and_then(|_| std::fs::rename(&tmp, file));
    let _ = std::fs::remove_file(&tmp);
    match res {
        Ok(()) => Seed::Written { flags: wanted.len(), removed },
        Err(e) => Seed::Unsupported { reason: format!("can't write it: {e}") },
    }
}

/// Seed Roblox's real flag cache with `wanted`, remembering what was seeded.
pub fn seed(wanted: &Map<String, Value>) -> Seed {
    let list = seeded_list_path();
    let previous: Vec<String> = std::fs::read(&list)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let r = seed_file(&path(), wanted, &previous);
    if r.ok() {
        let names: Vec<&String> = wanted.keys().collect();
        let _ = crate::paths::ensure_all();
        let _ = std::fs::write(&list, serde_json::to_vec(&names).unwrap_or_default());
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cache_bytes(sig: &[u8], comp: u8, json: &str) -> Vec<u8> {
        let mut v = (sig.len() as u32).to_le_bytes().to_vec();
        v.extend_from_slice(sig);
        v.push(comp);
        v.extend_from_slice(json.as_bytes());
        v
    }

    fn flags_in(file: &Path) -> Map<String, Value> {
        let raw = std::fs::read(file).unwrap();
        let n = u32::from_le_bytes(raw[..4].try_into().unwrap()) as usize;
        let v: Value = serde_json::from_slice(&raw[n + 5..]).unwrap();
        v["applicationSettings"].as_object().unwrap().clone()
    }

    #[test]
    fn seeds_and_cleans_up_keeping_the_header() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("flag_cache.dat");
        let sig = b"SIGNATURE-BYTES";
        std::fs::write(&f, cache_bytes(sig, 0, r#"{"applicationSettings":{"FFlagServer":"True","DFIntOld":"5"}}"#)).unwrap();

        let mut want = Map::new();
        want.insert("DFFlagHttpUseEmbeddedCaCerts".into(), json!(false));
        want.insert("DFIntTaskSchedulerTargetFps".into(), json!(240));
        let r = seed_file(&f, &want, &["DFIntOld".into()]);
        assert_eq!(r, Seed::Written { flags: 2, removed: 1 });

        let raw = std::fs::read(&f).unwrap();
        assert_eq!(&raw[4..4 + sig.len()], sig);
        let got = flags_in(&f);
        assert_eq!(got["DFFlagHttpUseEmbeddedCaCerts"], "False");
        assert_eq!(got["DFIntTaskSchedulerTargetFps"], "240");
        assert_eq!(got["FFlagServer"], "True");
        assert!(!got.contains_key("DFIntOld"));

        assert_eq!(seed_file(&f, &want, &[]), Seed::Unchanged);

        // Turning everything off removes only what Arcadia added.
        let prev: Vec<String> = want.keys().cloned().collect();
        assert_eq!(seed_file(&f, &Map::new(), &prev), Seed::Written { flags: 0, removed: 2 });
        assert_eq!(flags_in(&f).len(), 1);
    }

    #[test]
    fn refuses_what_it_doesnt_understand() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("flag_cache.dat");
        assert_eq!(seed_file(&f, &Map::new(), &[]), Seed::Missing);

        std::fs::write(&f, cache_bytes(b"sig", 1, "(zstd bytes)")).unwrap();
        assert!(matches!(seed_file(&f, &Map::new(), &[]), Seed::Unsupported { .. }));

        std::fs::write(&f, [0xFF, 0xFF, 0xFF, 0x7F, 0]).unwrap();
        assert!(matches!(seed_file(&f, &Map::new(), &[]), Seed::Unsupported { .. }));

        let before = cache_bytes(b"sig", 0, "not json");
        std::fs::write(&f, &before).unwrap();
        assert!(matches!(seed_file(&f, &Map::new(), &[]), Seed::Unsupported { .. }));
        assert_eq!(std::fs::read(&f).unwrap(), before);
    }
}
