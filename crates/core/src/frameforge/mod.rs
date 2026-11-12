//! FrameForge: hardware inventory, live performance monitoring, diagnostics,
//! reversible optimisation, GPU driver profiles and benchmarking.
//!
//! Like the rest of `arcadia-core` this module has no GUI dependencies. Every
//! piece that touches the system or a driver sits behind a trait with an
//! in-memory implementation, so the logic is tested without changing the
//! machine that runs the tests.
//!
//! Data lives under `%LOCALAPPDATA%\Arcadia\FrameForge\`:
//!
//! ```text
//! config.json          FrameForge settings (schema-versioned)
//! history\             optimisation sessions and their rollback records
//! nvidia\backups\      .nip snapshots taken before driver changes
//! profiles\            per-game performance profiles
//! presets\             vendor-neutral GPU presets
//! benchmarks\          benchmark captures
//! cache\               disposable monitoring data
//! ```

pub mod config;
pub mod diagnostics;
pub mod ffi;
pub mod gpu;
pub mod hardware;
#[cfg(windows)]
pub(crate) mod hardware_win;
pub mod monitor;
pub mod nvidia;
pub mod optimizer;
pub mod presentmon;
pub mod processes;
pub mod service;
pub mod stats;
pub mod telemetry;
#[cfg(windows)]
pub mod winsys;

pub use service::FrameForge;

use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    crate::paths::root().join("FrameForge")
}
pub fn history_dir() -> PathBuf {
    data_dir().join("history")
}
pub fn nvidia_backups_dir() -> PathBuf {
    data_dir().join("nvidia").join("backups")
}
pub fn profiles_dir() -> PathBuf {
    data_dir().join("profiles")
}
pub fn presets_dir() -> PathBuf {
    data_dir().join("presets")
}
pub fn benchmarks_dir() -> PathBuf {
    data_dir().join("benchmarks")
}
pub fn cache_dir() -> PathBuf {
    data_dir().join("cache")
}

/// Write-then-rename, so a crash never leaves a half-written file.
pub(crate) fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> anyhow::Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// A file name that is safe on Windows and cannot escape its folder.
pub(crate) fn safe_file_stem(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '(' | ')') { c } else { '_' })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').trim();
    let stem: String = trimmed.chars().take(80).collect();
    if stem.is_empty() {
        "unnamed".into()
    } else {
        stem
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_stems_cannot_traverse() {
        assert_eq!(safe_file_stem("../../Windows/System32"), ".._.._Windows_System32".trim_matches('.'));
        assert!(!safe_file_stem("a/b\\c:d").contains(['/', '\\', ':']));
        assert_eq!(safe_file_stem("  "), "unnamed");
        assert_eq!(safe_file_stem(".."), "unnamed");
        assert_eq!(safe_file_stem("Cyberpunk 2077 (DX12)"), "Cyberpunk 2077 (DX12)");
    }
}
