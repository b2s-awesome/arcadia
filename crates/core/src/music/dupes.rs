//! Duplicate songs, each pair with a certainty from 0 to 100 %.
//!
//! How sure two songs are the same recording:
//!
//! ```text
//! certainty = 100 × (0.45·title + 0.25·artist + 0.20·length + 0.10·file name)
//!             × version penalty
//! ```
//!
//! * **title**: the better of edit-distance similarity (Levenshtein,
//!   normalised by the longer title) and word-set overlap (Jaccard), on
//!   titles cleaned of noise: "(Official Audio)", "[HQ]", "feat. …", a
//!   leading "Artist - ", punctuation and case.
//! * **artist**: word-set overlap of everyone credited (artist, album
//!   artist and anyone after "feat."). Unknown artists count as 0.5.
//! * **length**: 1 within a second, falling in a straight line to 0 at 8 s
//!   apart. Unknown lengths count as 0.5.
//! * **file name**: title similarity of the file names, which often keep
//!   the real title when the tags don't.
//! * **version penalty**: songs marked as different versions ("slowed",
//!   "sped up", "remix", "live", "acoustic", "instrumental", "nightcore",
//!   "reverb", "8D") aren't duplicates of each other, so a mismatch
//!   multiplies by 0.35.
//!
//! Two files of exactly the same size and length are byte-for-byte copies
//! in practice, so they score at least 99 %. Only pairs within 8 s of each
//! other that share a title word are compared, which keeps a 20 000-song
//! library to a fraction of a second.

use super::Track;
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};

const W_TITLE: f64 = 0.45;
const W_ARTIST: f64 = 0.25;
const W_LENGTH: f64 = 0.20;
const W_FILE: f64 = 0.10;
const VERSION_PENALTY: f64 = 0.35;
const MAX_GAP_SECS: f64 = 8.0;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pair {
    pub a: i64,
    pub b: i64,
    /// 0–100.
    pub certainty: f64,
    /// Each part's score, 0–1, for the "why" line.
    pub title: f64,
    pub artist: f64,
    pub length: f64,
    pub file: f64,
    pub same_file_size: bool,
    pub version_clash: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    /// Best copy first (see `quality`).
    pub ids: Vec<i64>,
    /// The highest certainty linking the group.
    pub certainty: f64,
    pub pairs: Vec<Pair>,
}

/// Words that mark a different version of a song, not a copy.
const VERSIONS: &[&str] = &[
    "slowed", "sped", "speed", "nightcore", "reverb", "remix", "live", "acoustic", "instrumental", "8d", "edit", "cover", "karaoke",
    "acapella", "a cappella", "bass boosted", "daycore", "mashup", "extended", "demo",
];

/// Bits of titles that say nothing about the song itself.
const NOISE: &[&str] = &[
    "official audio", "official music video", "official video", "official lyric video", "lyric video", "lyrics video", "official visualizer",
    "visualizer", "visualiser", "official", "audio", "lyrics", "lyric", "music video", "video", "hq", "hd", "4k", "explicit", "clean",
    "remastered", "remaster", "free download", "out now",
];

fn fold(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The version markers in a title or file name.
fn versions(s: &str) -> BTreeSet<&'static str> {
    let f = format!(" {} ", fold(s));
    VERSIONS
        .iter()
        .filter(|v| f.contains(&format!(" {v} ")) || (v.len() > 3 && f.contains(&format!(" {v}"))))
        .map(|v| match *v {
            "speed" => "sped",
            "a cappella" => "acapella",
            x => x,
        })
        .collect()
}

/// A title with the noise taken out, folded to plain lowercase words.
fn clean_title(title: &str, artist: &str) -> String {
    let mut t = title.to_lowercase();
    // "feat. x" / "ft. x" / "with x" in brackets or at the end.
    for open in ['(', '['] {
        let close = if open == '(' { ')' } else { ']' };
        while let Some(i) = t.find(open) {
            let Some(j) = t[i..].find(close).map(|j| i + j) else { break };
            let inner = fold(&t[i + 1..j]);
            let drop = inner.starts_with("feat") || inner.starts_with("ft ") || inner.starts_with("with ") || inner.starts_with("prod")
                || NOISE.iter().any(|n| inner == *n || inner.starts_with(&format!("{n} ")) || inner.ends_with(&format!(" {n}")));
            if drop {
                t.replace_range(i..=j, " ");
            } else {
                // Keep the words (a version marker, say), lose the brackets.
                t.replace_range(i..=i, " ");
                let j = t[i..].find(close).map(|k| i + k).unwrap_or(t.len() - 1);
                t.replace_range(j..=j, " ");
            }
        }
    }
    for marker in [" feat. ", " feat ", " ft. ", " ft "] {
        if let Some(i) = t.find(marker) {
            t.truncate(i);
        }
    }
    // "Artist - Title" uploads: drop the left side when it names the artist.
    if let Some(i) = t.find(" - ") {
        let left = fold(&t[..i]);
        let mine = fold(artist);
        let names: BTreeSet<&str> = words(&left);
        if !mine.is_empty() && (left.contains(&mine) || words(&mine).iter().any(|w| w.len() > 2 && names.contains(w))) {
            t = t[i + 3..].to_string();
        }
    }
    let mut f = fold(&t);
    for n in NOISE {
        f = format!(" {f} ").replace(&format!(" {n} "), " ").trim().to_string();
    }
    if f.is_empty() { fold(title) } else { f }
}

fn words(s: &str) -> BTreeSet<&str> {
    s.split_whitespace().collect()
}

fn jaccard(a: &BTreeSet<&str>, b: &BTreeSet<&str>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let inter = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    if union == 0.0 { 0.0 } else { inter / union }
}

/// Levenshtein distance over characters, in two rows.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + usize::from(ca != cb));
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// 0–1: how alike two cleaned strings are.
fn similar(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let longest = a.chars().count().max(b.chars().count());
    if longest == 0 {
        return 1.0;
    }
    let edit = 1.0 - levenshtein(a, b) as f64 / longest as f64;
    edit.max(jaccard(&words(a), &words(b)))
}

fn people(t: &Track) -> BTreeSet<String> {
    let mut all = format!("{} , {}", t.artist, t.album_artist).to_lowercase();
    for sep in [" feat. ", " feat ", " ft. ", " ft ", " x ", " & ", " and ", ";", "/", " vs "] {
        all = all.replace(sep, ",");
    }
    all.split(',')
        .map(fold)
        .filter(|p| !p.is_empty() && p != "unknown artist" && p != "various artists" && p != "unknown")
        .collect()
}

fn stem(path: &str) -> String {
    std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

struct Prepared<'a> {
    t: &'a Track,
    title: String,
    file: String,
    people: BTreeSet<String>,
    versions: BTreeSet<&'static str>,
}

fn prepare(t: &Track) -> Prepared<'_> {
    let file_raw = stem(&t.path);
    let mut v = versions(&t.title);
    v.extend(versions(&file_raw));
    Prepared {
        title: clean_title(&t.title, &t.artist),
        file: clean_title(&file_raw, &t.artist),
        people: people(t),
        versions: v,
        t,
    }
}

/// Score one pair. `same_size`: the two files are exactly the same size.
fn score(a: &Prepared, b: &Prepared, same_size: bool) -> Pair {
    let title = similar(&a.title, &b.title);
    let artist = if a.people.is_empty() || b.people.is_empty() {
        0.5
    } else {
        let pa: BTreeSet<&str> = a.people.iter().map(String::as_str).collect();
        let pb: BTreeSet<&str> = b.people.iter().map(String::as_str).collect();
        // Someone in common counts for a lot: features get credited unevenly.
        jaccard(&pa, &pb).max(if pa.intersection(&pb).next().is_some() { 0.85 } else { 0.0 })
    };
    let length = if a.t.duration <= 0.0 || b.t.duration <= 0.0 {
        0.5
    } else {
        let gap = (a.t.duration - b.t.duration).abs();
        (1.0 - (gap - 1.0).max(0.0) / (MAX_GAP_SECS - 1.0)).clamp(0.0, 1.0)
    };
    let file = similar(&a.file, &b.file);
    let version_clash = a.versions != b.versions;
    let same_file_size = same_size && (a.t.duration - b.t.duration).abs() < 0.05;
    let mut certainty = 100.0 * (W_TITLE * title + W_ARTIST * artist + W_LENGTH * length + W_FILE * file);
    if version_clash {
        certainty *= VERSION_PENALTY;
    }
    if same_file_size {
        certainty = certainty.max(99.0);
    }
    Pair { a: a.t.id, b: b.t.id, certainty: (certainty * 10.0).round() / 10.0, title, artist, length, file, same_file_size, version_clash }
}

/// Which copy to keep: lossless first, then the higher bitrate and sample
/// rate, then the one played most, then the oldest in the library.
fn quality(t: &Track) -> (u8, u32, u32, i64, i64) {
    let lossless = matches!(t.format.to_lowercase().as_str(), "flac" | "wav" | "alac" | "aiff" | "aif" | "ape" | "wv");
    (u8::from(lossless), t.bitrate.unwrap_or(0), t.sample_rate.unwrap_or(0), t.play_count, -t.added_at)
}

/// Find duplicates among `tracks`, keeping pairs at or above `min` %.
/// `size_of` gives a file's size in bytes (only asked for close pairs).
pub fn find(tracks: &[Track], min: f64, size_of: impl Fn(&Track) -> Option<u64>) -> Vec<Group> {
    let mut prep: Vec<Prepared> = tracks.iter().map(prepare).collect();
    // Sort by length so only neighbours within MAX_GAP_SECS are compared.
    prep.sort_by(|a, b| a.t.duration.total_cmp(&b.t.duration));
    let mut pairs = Vec::new();
    let mut sizes: HashMap<i64, Option<u64>> = HashMap::new();
    for i in 0..prep.len() {
        let wi: BTreeSet<String> = words(&prep[i].title).iter().chain(words(&prep[i].file).iter()).map(|w| w.to_string()).collect();
        for j in i + 1..prep.len() {
            let (a, b) = (&prep[i], &prep[j]);
            if a.t.duration > 0.0 && b.t.duration - a.t.duration > MAX_GAP_SECS {
                break;
            }
            // Cheap filter: at least one title or file-name word in common.
            if !words(&b.title).iter().chain(words(&b.file).iter()).any(|w| wi.contains(*w)) {
                continue;
            }
            let quick = score(a, b, false);
            // Same size and length? Only worth asking the disk for close pairs.
            let p = if quick.certainty >= min * 0.5 && (a.t.duration - b.t.duration).abs() < 0.05 {
                let sa = *sizes.entry(a.t.id).or_insert_with(|| size_of(a.t));
                let sb = *sizes.entry(b.t.id).or_insert_with(|| size_of(b.t));
                if matches!((sa, sb), (Some(x), Some(y)) if x == y && x > 0) { score(a, b, true) } else { quick }
            } else {
                quick
            };
            if p.certainty >= min {
                pairs.push(p);
            }
        }
    }
    group(tracks, pairs)
}

/// Join pairs that share a song into groups (union–find).
fn group(tracks: &[Track], pairs: Vec<Pair>) -> Vec<Group> {
    let mut parent: HashMap<i64, i64> = HashMap::new();
    fn root(p: &mut HashMap<i64, i64>, x: i64) -> i64 {
        let up = *p.entry(x).or_insert(x);
        if up == x {
            return x;
        }
        let r = root(p, up);
        p.insert(x, r);
        r
    }
    for p in &pairs {
        let (ra, rb) = (root(&mut parent, p.a), root(&mut parent, p.b));
        if ra != rb {
            parent.insert(ra, rb);
        }
    }
    let by_id: HashMap<i64, &Track> = tracks.iter().map(|t| (t.id, t)).collect();
    let mut groups: HashMap<i64, Group> = HashMap::new();
    for p in pairs {
        let r = root(&mut parent, p.a);
        let g = groups.entry(r).or_insert_with(|| Group { ids: vec![], certainty: 0.0, pairs: vec![] });
        for id in [p.a, p.b] {
            if !g.ids.contains(&id) {
                g.ids.push(id);
            }
        }
        g.certainty = g.certainty.max(p.certainty);
        g.pairs.push(p);
    }
    let mut out: Vec<Group> = groups.into_values().collect();
    for g in &mut out {
        g.ids.sort_by(|a, b| by_id.get(b).map(|t| quality(t)).cmp(&by_id.get(a).map(|t| quality(t))));
        g.pairs.sort_by(|a, b| b.certainty.total_cmp(&a.certainty));
    }
    out.sort_by(|a, b| b.certainty.total_cmp(&a.certainty));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(id: i64, title: &str, artist: &str, dur: f64, path: &str) -> Track {
        Track { id, title: title.into(), artist: artist.into(), album_artist: artist.into(), duration: dur, path: path.into(), format: "mp3".into(), ..Default::default() }
    }

    #[test]
    fn cleans_titles() {
        assert_eq!(clean_title("2hollis - crush (official audio)", "2hollis"), "crush");
        assert_eq!(clean_title("koke (feat. vnmpire)", "22juggn"), "koke");
        assert_eq!(clean_title("BUCKSHOT & FAKEMINK - FEVER [HQ]", "BUCKSHOT"), "fever");
        assert_eq!(clean_title("Melancholie (slowed)", "x"), "melancholie slowed");
    }

    #[test]
    fn spots_a_youtube_copy() {
        let a = t(1, "crush", "2hollis", 146.0, "C:/Music/2hollis/crush.flac");
        let b = t(2, "2hollis - crush (official audio)", "2hollis", 146.4, "C:/Music/Downloads/2hollis - crush (official audio).m4a");
        let g = find(&[a, b], 85.0, |_| None);
        assert_eq!(g.len(), 1);
        assert!(g[0].certainty >= 90.0, "{}", g[0].certainty);
        assert_eq!(g[0].ids[0], 1, "FLAC is kept first");
    }

    #[test]
    fn versions_are_not_duplicates() {
        let a = t(1, "Melancholie", "Opfedolch", 147.0, "a.mp3");
        let b = t(2, "Melancholie (slowed)", "Opfedolch", 150.0, "b.mp3");
        assert!(find(&[a, b], 50.0, |_| None).is_empty());
    }

    #[test]
    fn different_songs_by_one_artist_are_apart() {
        let a = t(1, "emobitch", "22juggn", 65.0, "a.mp3");
        let b = t(2, "fried", "22juggn", 43.0, "b.mp3");
        let c = t(3, "koke", "22juggn", 129.0, "c.mp3");
        assert!(find(&[a, b, c], 60.0, |_| None).is_empty());
    }

    #[test]
    fn identical_files_are_certain() {
        let a = t(1, "Track 01", "", 200.0, "x/Track 01.mp3");
        let b = t(2, "Track 01", "", 200.0, "y/Track 01.mp3");
        let g = find(&[a, b], 85.0, |_| Some(4_812_331));
        assert!(g[0].certainty >= 99.0);
        assert!(g[0].pairs[0].same_file_size);
    }

    #[test]
    fn groups_join_chains() {
        let a = t(1, "FEVER", "BUCKSHOT, FAKEMINK", 144.0, "a.flac");
        let b = t(2, "BUCKSHOT & FAKEMINK - FEVER", "BUCKSHOT", 144.5, "b.m4a");
        let c = t(3, "Fever", "Buckshot", 145.0, "c.mp3");
        let g = find(&[a, b, c], 80.0, |_| None);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].ids.len(), 3);
    }

    #[test]
    fn levenshtein_works() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("", "abc"), 3);
    }
}
