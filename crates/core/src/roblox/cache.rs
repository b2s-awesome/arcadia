//! Roblox's asset cache. After changing asset replacements the cache must be
//! cleared so the client re-downloads through the proxy (as with Fleasion).

use std::path::PathBuf;

fn targets() -> Vec<PathBuf> {
    let local = crate::paths::roblox_local_dir();
    let mut v = vec![
        local.join("rbx-storage.db"),
        local.join("rbx-storage.db-wal"),
        local.join("rbx-storage.db-shm"),
        local.join("rbx-storage"),
        local.join("http"),
    ];
    v.push(std::env::temp_dir().join("Roblox").join("http"));
    // Microsoft Store / GDK installs keep their own copy under Packages.
    if let Some(pk) = dirs::data_local_dir().map(|d| d.join("Packages")) {
        if let Ok(rd) = std::fs::read_dir(&pk) {
            for e in rd.flatten() {
                if e.file_name().to_string_lossy().starts_with("ROBLOXCORPORATION.ROBLOX") {
                    let lc = e.path().join("LocalState");
                    v.push(lc.join("rbx-storage.db"));
                    v.push(lc.join("rbx-storage"));
                }
            }
        }
    }
    v
}

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct ClearReport {
    pub removed: usize,
    pub locked: usize,
}

/// Delete cache files. Files held open by a running Roblox are counted as
/// `locked` instead of failing.
pub fn clear() -> ClearReport {
    let mut r = ClearReport::default();
    for t in targets() {
        if !t.exists() {
            continue;
        }
        let res = if t.is_dir() { std::fs::remove_dir_all(&t) } else { std::fs::remove_file(&t) };
        match res {
            Ok(()) => r.removed += 1,
            Err(_) => r.locked += 1,
        }
    }
    r
}
