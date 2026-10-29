//! Lyrics: LRC parsing (with word timings when present) and finding them.
//!
//! Sources, first match wins: a `.lrc` next to the song, the song's own
//! tags, Arcadia's cache, then LRCLIB (lrclib.net, a free open database).

use super::{Library, Track};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Lyrics {
    /// Every line has a time.
    pub synced: bool,
    /// Some lines have word-by-word times.
    pub words: bool,
    /// "file", "tags", "lrclib" or "yours" (saved from the timing editor).
    pub source: String,
    pub lines: Vec<Line>,
    /// The song has no words.
    pub instrumental: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// Seconds in; `None` for unsynced lyrics.
    pub t: Option<f64>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub words: Vec<Word>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Word {
    pub t: f64,
    pub text: String,
}

/// `mm:ss`, `mm:ss.xx` or `mm:ss:xx` to seconds.
fn stamp(s: &str) -> Option<f64> {
    let (m, rest) = s.split_once(':')?;
    let m: f64 = m.trim().parse().ok()?;
    let rest = rest.replacen(':', ".", 1);
    let sec: f64 = rest.trim().parse().ok()?;
    (m >= 0.0 && (0.0..60.0).contains(&sec)).then_some(m * 60.0 + sec)
}

/// Parse LRC (or plain text, which comes back unsynced).
pub fn parse(text: &str, source: &str) -> Lyrics {
    let mut offset = 0.0;
    let mut timed: Vec<Line> = Vec::new();
    let mut plain: Vec<Line> = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        while let Some(r) = rest.strip_prefix('[') {
            let Some(end) = r.find(']') else { break };
            let tag = &r[..end];
            if let Some(t) = stamp(tag) {
                times.push(t);
            } else if let Some(v) = tag.strip_prefix("offset:") {
                // Milliseconds; positive means the lyrics come sooner.
                offset = v.trim().parse::<f64>().unwrap_or(0.0) / 1000.0;
            } else if times.is_empty() && tag.contains(':') {
                // Metadata ([ar:…], [ti:…]): not a lyric line.
                rest = "";
                break;
            } else {
                break;
            }
            rest = r[end + 1..].trim_start();
        }
        if times.is_empty() {
            if !rest.is_empty() || !plain.is_empty() {
                plain.push(Line { t: None, text: rest.to_string(), words: Vec::new() });
            }
            continue;
        }
        let (text, words) = words_of(rest);
        for t in times {
            timed.push(Line { t: Some(t), text: text.clone(), words: words.clone() });
        }
    }
    while plain.last().is_some_and(|l| l.text.is_empty()) {
        plain.pop();
    }
    if timed.is_empty() {
        return Lyrics { synced: false, words: false, source: source.into(), lines: plain, instrumental: false };
    }
    timed.sort_by(|a, b| a.t.unwrap_or(0.0).total_cmp(&b.t.unwrap_or(0.0)));
    for l in &mut timed {
        l.t = l.t.map(|t| (t - offset).max(0.0));
        for w in &mut l.words {
            w.t = (w.t - offset).max(0.0);
        }
    }
    let words = timed.iter().any(|l| !l.words.is_empty());
    Lyrics { synced: true, words, source: source.into(), lines: timed, instrumental: false }
}

/// Enhanced LRC: `<00:12.30>Hello <00:12.80>world` — the plain line and
/// its word times.
fn words_of(s: &str) -> (String, Vec<Word>) {
    if !s.contains('<') {
        return (s.trim().to_string(), Vec::new());
    }
    let mut words = Vec::new();
    let mut text = String::new();
    let mut cur: Option<f64> = None;
    let mut buf = String::new();
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('<') {
            if let Some(end) = r.find('>') {
                if let Some(t) = stamp(&r[..end]) {
                    if let Some(c) = cur.take() {
                        if !buf.trim().is_empty() {
                            words.push(Word { t: c, text: buf.clone() });
                        }
                    }
                    text.push_str(&buf);
                    buf.clear();
                    cur = Some(t);
                    rest = &r[end + 1..];
                    continue;
                }
            }
        }
        let ch = rest.chars().next().unwrap();
        buf.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    if let Some(c) = cur {
        if !buf.trim().is_empty() {
            words.push(Word { t: c, text: buf.clone() });
        }
    }
    text.push_str(&buf);
    (text.split_whitespace().collect::<Vec<_>>().join(" "), words)
}

/// Write lyrics back out as LRC.
pub fn to_lrc(l: &Lyrics) -> String {
    let mut out = String::new();
    for line in &l.lines {
        match line.t {
            Some(t) => {
                let m = (t / 60.0).floor();
                out.push_str(&format!("[{:02}:{:05.2}]{}\n", m as u64, t - m * 60.0, line.text));
            }
            None => {
                out.push_str(&line.text);
                out.push('\n');
            }
        }
    }
    out
}

/// A `.lrc` beside the song file.
pub fn sidecar(track: &Track) -> Option<PathBuf> {
    let p = Path::new(&track.path);
    p.is_file().then(|| p.with_extension("lrc"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibHit {
    #[serde(default)]
    synced_lyrics: Option<String>,
    #[serde(default)]
    plain_lyrics: Option<String>,
    #[serde(default)]
    instrumental: bool,
}

/// Look a song up on LRCLIB. `Ok(None)` when it isn't there.
pub async fn lrclib(t: &Track) -> Result<Option<Lyrics>> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("Arcadia/", env!("CARGO_PKG_VERSION"), " (https://github.com/b2s-awesome/arcadia)"))
        .timeout(Duration::from_secs(15))
        .build()?;
    let mut q: Vec<(&str, String)> = vec![("track_name", t.title.clone()), ("artist_name", t.artist.clone())];
    if !t.album.is_empty() && t.album != "Unknown album" && t.album != "Singles" {
        q.push(("album_name", t.album.clone()));
    }
    if t.duration > 0.0 {
        q.push(("duration", format!("{}", t.duration.round() as i64)));
    }
    let r = client.get("https://lrclib.net/api/get").query(&q).send().await?;
    if r.status() == reqwest::StatusCode::NOT_FOUND {
        // Close enough wins: search without the album and duration.
        let r = client.get("https://lrclib.net/api/search").query(&[("track_name", t.title.as_str()), ("artist_name", t.artist.as_str())]).send().await?;
        if !r.status().is_success() {
            return Ok(None);
        }
        let hits: Vec<LrclibHit> = r.json().await?;
        let best = hits.into_iter().find(|h| h.synced_lyrics.is_some()).or(None);
        return Ok(best.and_then(from_hit));
    }
    let r = r.error_for_status()?;
    Ok(from_hit(r.json().await?))
}

fn from_hit(h: LrclibHit) -> Option<Lyrics> {
    if h.instrumental {
        return Some(Lyrics { synced: false, words: false, source: "lrclib".into(), lines: Vec::new(), instrumental: true });
    }
    let text = h.synced_lyrics.filter(|s| !s.trim().is_empty()).or(h.plain_lyrics.filter(|s| !s.trim().is_empty()))?;
    Some(parse(&text, "lrclib"))
}

impl Library {
    /// Lyrics already on this PC: a `.lrc` beside the file, the file's tags,
    /// or Arcadia's cache (`cache_dir`).
    pub fn local_lyrics(&self, id: i64, cache_dir: &Path) -> Option<Lyrics> {
        let t = self.track(id).ok().flatten()?;
        if let Some(p) = sidecar(&t) {
            if let Ok(s) = std::fs::read_to_string(&p) {
                return Some(parse(&s, "file"));
            }
        }
        if let Some(s) = self.lyrics_tag(id) {
            return Some(parse(&s, "tags"));
        }
        let cached = std::fs::read_to_string(cache_dir.join(format!("{id}.json"))).ok()?;
        serde_json::from_str(&cached).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lrc_with_offsets_repeats_and_words() {
        let l = parse(
            "[ar:Ivy Lantern]\n[offset:+500]\n[00:12.00][01:30.50]Every window\n[00:15.20]<00:15.20>You <00:15.60>said <00:16.10>the city\n[00:20.00]\n",
            "file",
        );
        assert!(l.synced && l.words);
        let ts: Vec<f64> = l.lines.iter().map(|x| x.t.unwrap()).collect();
        assert_eq!(ts, vec![11.5, 14.7, 19.5, 90.0]);
        assert_eq!(l.lines[1].text, "You said the city");
        assert_eq!(l.lines[1].words.len(), 3);
        assert_eq!(l.lines[1].words[2].text.trim(), "the city");
        assert!((l.lines[1].words[1].t - 15.1).abs() < 1e-9);
        assert_eq!(l.lines[2].text, "", "an empty timed line is a pause");
    }

    #[test]
    fn plain_text_is_unsynced() {
        let l = parse("One line\n\nAnother\n\n", "tags");
        assert!(!l.synced);
        assert_eq!(l.lines.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(), vec!["One line", "", "Another"]);
    }

    #[test]
    fn round_trips_through_lrc() {
        let l = parse("[00:01.00]a\n[01:02.25]b\n", "yours");
        let again = parse(&to_lrc(&l), "yours");
        assert_eq!(l, again);
    }
}
