//! Music: the library (folders, tags, covers, playlists) and the play queue.
//! Playback itself is the `arcadia-audio` crate.

pub mod library;
pub mod queue;

pub use library::{Library, Playlist, ScanProgress, ScanReport, Track};
pub use queue::{Queue, Repeat};

pub fn dir() -> std::path::PathBuf {
    crate::paths::root().join("Music")
}

/// The queue and player settings, kept between runs.
pub fn session_file() -> std::path::PathBuf {
    dir().join("session.json")
}
