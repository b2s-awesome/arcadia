//! Arcadia's audio engine.
//!
//! * Native decoding (symphonia): MP3, AAC/M4A, ALAC, FLAC, WAV, Ogg Vorbis.
//! * Gapless: the next track is decoded straight onto the end of the current
//!   one, and MP3/AAC encoder padding is trimmed.
//! * Resampling (rubato) only when a file's rate differs from the device's,
//!   so matching files pass through untouched.
//! * Follows the Windows default output device, and reopens after unplugging.

pub mod engine;
pub mod output;
pub mod pipeline;
pub mod source;

pub use engine::{Engine, Event};
pub use output::{devices, OutputInfo};
pub use source::EXTENSIONS;
