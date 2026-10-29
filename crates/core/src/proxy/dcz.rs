//! Dictionary-compressed responses (`Content-Encoding: dcz`, from the
//! Compression Dictionary Transport spec).
//!
//! Roblox serves the player's ClientSettings from
//! `/v2/settings-compressed/application/PCDesktopClient/<sha256>.dcz`: a zstd
//! frame compressed against a shared dictionary, published at
//! `clientsettings.roblox.com/v2/compression-dictionaries/<sha256>`. To add
//! flags, the proxy fetches that dictionary, decodes, edits, and re-encodes
//! with the same dictionary so the client can still read it.
//!
//! On the wire a dcz body may start with a 40-byte header: a zstd skippable
//! frame (magic `5E 2A 4D 18`, length 32) holding the dictionary's SHA-256.

use sha2::{Digest, Sha256};
use std::io::{Read, Write};

const HEADER_MAGIC: [u8; 8] = [0x5e, 0x2a, 0x4d, 0x18, 0x20, 0x00, 0x00, 0x00];
const HEADER_LEN: usize = 40;

/// The dictionary hash named in a `.dcz` URL, if any.
pub fn dictionary_hash(path_and_query: &str) -> Option<String> {
    let path = path_and_query.split('?').next()?;
    let name = path.rsplit('/').next()?;
    let hash = name.strip_suffix(".dcz")?;
    (hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())).then(|| hash.to_ascii_lowercase())
}

pub fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// Decode a dcz body with its dictionary (used as raw content, as Roblox
/// publishes it). The 40-byte header is optional.
pub fn decode(body: &[u8], dictionary: &[u8]) -> Option<Vec<u8>> {
    let frame = if body.starts_with(&HEADER_MAGIC) && body.len() >= HEADER_LEN { &body[HEADER_LEN..] } else { body };
    let mut out = Vec::new();
    zstd::stream::read::Decoder::with_ref_prefix(frame, dictionary)
        .ok()?
        .read_to_end(&mut out)
        .ok()?;
    Some(out)
}

/// Encode `plain` against `dictionary`, with the standard header.
pub fn encode(plain: &[u8], dictionary: &[u8]) -> Option<Vec<u8>> {
    let mut out = HEADER_MAGIC.to_vec();
    out.extend_from_slice(&Sha256::digest(dictionary));
    let mut enc = zstd::stream::write::Encoder::with_ref_prefix(out, 3, dictionary).ok()?;
    enc.write_all(plain).ok()?;
    enc.finish().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dictionary() -> Vec<u8> {
        br#"{"applicationSettings":{"FFlagSomething":"True","DFIntSomething":"100","FStringThing":"abc"}}"#.repeat(20)
    }

    #[test]
    fn round_trips_with_and_without_header() {
        let dict = dictionary();
        let plain = br#"{"applicationSettings":{"FFlagSomething":"False","DFIntSomething":"100"}}"#;
        let enc = encode(plain, &dict).unwrap();
        assert!(enc.starts_with(&HEADER_MAGIC));
        assert_eq!(&enc[8..40], Sha256::digest(&dict).as_slice());
        assert_eq!(decode(&enc, &dict).unwrap(), plain);
        assert_eq!(decode(&enc[HEADER_LEN..], &dict).unwrap(), plain);
        // The frame really refers to the dictionary.
        assert!(decode(&enc, b"").is_none());
    }

    #[test]
    fn reads_the_hash_from_the_url() {
        let h = "57ace1df87f8e37023aebc8d9ce7e0802ffc0ee11dbcc89b34b18ed4e75f1de7";
        assert_eq!(dictionary_hash(&format!("/v2/settings-compressed/application/PCDesktopClient/{h}.dcz")).as_deref(), Some(h));
        assert_eq!(dictionary_hash(&format!("/x/{h}.dcz?a=1")).as_deref(), Some(h));
        assert_eq!(dictionary_hash("/v2/settings/application/PCDesktopClient"), None);
        assert_eq!(dictionary_hash("/x/abc.dcz"), None);
    }
}
