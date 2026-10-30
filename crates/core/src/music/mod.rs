//! Music: the library (folders, tags, covers, playlists) and the play queue.
//! Playback itself is the `arcadia-audio` crate.

pub mod covers;
pub mod dupes;
pub mod library;
pub mod lyrics;
pub mod queue;
pub mod stats;

pub use library::{Library, Playlist, ScanProgress, ScanReport, Track};
pub use queue::{Queue, Repeat, ShuffleInfo, ShuffleMode};

pub fn dir() -> std::path::PathBuf {
    crate::paths::root().join("Music")
}

/// The queue and player settings, kept between runs.
pub fn session_file() -> std::path::PathBuf {
    dir().join("session.json")
}
