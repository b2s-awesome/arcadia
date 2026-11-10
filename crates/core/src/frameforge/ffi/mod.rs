//! Raw vendor interfaces. Everything unsafe about talking to a driver DLL
//! lives under this module; the rest of FrameForge only sees safe wrappers.
//!
//! No vendor binary is shipped with Arcadia. Each DLL is loaded at run time
//! from the installed driver or service, and a missing DLL is an ordinary
//! "unavailable" state, never a crash.

#[allow(non_camel_case_types, non_snake_case, non_upper_case_globals, dead_code, clippy::all)]
pub mod nvapi_types;
#[allow(non_camel_case_types, non_snake_case, non_upper_case_globals, dead_code, clippy::all)]
pub mod presentmon_types;

#[allow(non_camel_case_types, non_snake_case, non_upper_case_globals, dead_code, clippy::all)]
pub mod igcl_types;

#[cfg(all(windows, feature = "adlx"))]
pub mod adlx;
#[cfg(windows)]
pub mod igcl;
#[cfg(windows)]
pub mod igcl_controls;
pub mod nvapi;

/// UTF-16 buffer (NUL-terminated or full) to a Rust string.
pub fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Copy a string into a fixed UTF-16 buffer, always NUL-terminated.
/// Returns false (and writes nothing) when it would not fit.
pub fn string_to_wide(s: &str, buf: &mut [u16]) -> bool {
    let units: Vec<u16> = s.encode_utf16().collect();
    if units.len() + 1 > buf.len() || units.contains(&0) {
        return false;
    }
    buf[..units.len()].copy_from_slice(&units);
    buf[units.len()..].iter_mut().for_each(|c| *c = 0);
    true
}

/// C `char` buffer to a Rust string.
pub fn cchars_to_string(buf: &[std::ffi::c_char]) -> String {
    let bytes: Vec<u8> = buf.iter().take_while(|c| **c != 0).map(|c| *c as u8).collect();
    String::from_utf8_lossy(&bytes).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_round_trip() {
        let mut buf = [0xFFFFu16; 16];
        assert!(string_to_wide("héllo", &mut buf));
        assert_eq!(wide_to_string(&buf), "héllo");
        assert!(buf[5..].iter().all(|c| *c == 0));
    }

    #[test]
    fn wide_rejects_overflow_and_embedded_nul() {
        let mut buf = [0u16; 4];
        assert!(!string_to_wide("abcd", &mut buf)); // no room for the terminator
        assert!(string_to_wide("abc", &mut buf));
        assert!(!string_to_wide("a\0b", &mut [0u16; 8]));
    }
}
