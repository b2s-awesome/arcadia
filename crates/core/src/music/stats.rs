//! Listening stats, worked out from the play log (`plays` table). Nothing
//! leaves the PC.

use super::Library;
use anyhow::Result;
use rusqlite::params;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    /// Seconds listened in the range.
    pub listened: f64,
    /// Seconds listened in the same length of time just before, when the
    /// range has a start.
    pub previous: Option<f64>,
    /// Songs heard properly (past 30 seconds or halfway).
    pub plays: i64,
    /// Songs skipped before that.
    pub skips: i64,
    /// Different songs heard.
    pub songs: i64,
    /// Songs heard for the first time ever in the range.
    pub new_songs: i64,
    pub top_artists: Vec<TopArtist>,
    pub top_songs: Vec<TopSong>,
    pub top_albums: Vec<TopAlbum>,
    /// Seconds by local hour of day, 0–23.
    pub hours: Vec<f64>,
    /// Seconds per local day for the heatmap, oldest first, ending today.
    pub days: Vec<f64>,
    pub sources: Vec<Share>,
    pub formats: Vec<Share>,
    /// Most days in a row with some listening, within the range.
    pub streak: i64,
    pub busiest_day: Option<Busiest>,
    /// Seconds listened today.
    pub today: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopArtist {
    pub name: String,
    pub plays: i64,
    pub listened: f64,
    pub art: Option<String>,
    pub art_color: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopSong {
    /// Still in the library?
    pub id: Option<i64>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub plays: i64,
    pub skips: i64,
    /// Average seconds listened per play.
    pub avg: f64,
    pub art: Option<String>,
    pub art_color: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopAlbum {
    pub album: String,
    pub artist: String,
    pub plays: i64,
    pub art: Option<String>,
    pub art_color: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Share {
    pub key: String,
    pub listened: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Busiest {
    /// Unix time of that local midnight.
    pub at: i64,
    pub listened: f64,
}

struct Play {
    id: Option<i64>,
    at: i64,
    listened: f64,
    skipped: bool,
    title: String,
    artist: String,
    album: String,
    origin: String,
    format: String,
}

const TOP: usize = 10;

impl Library {
    /// Stats for plays from `since` (unix seconds; `None` for all time) up
    /// to `now`. `tz` is the local offset from UTC in seconds, for hours and
    /// days; `heat_days` is how many days the heatmap covers.
    pub fn stats(&self, since: Option<i64>, now: i64, tz: i64, heat_days: usize) -> Result<Stats> {
        let day = |at: i64| (at + tz).div_euclid(86_400);
        let today = day(now);
        let heat_from = (today - heat_days as i64 + 1) * 86_400 - tz;
        let span = since.map(|s| now - s);
        let from = [since.map(|s| s - span.unwrap_or(0)), Some(heat_from)].into_iter().flatten().min().unwrap_or(0);
        let (rows, firsts) = self.with_db(|db| -> Result<_> {
            let mut st = db.prepare("SELECT track_id, at, listened, skipped, title, artist, album, origin, format FROM plays WHERE at >= ?1 AND at <= ?2 ORDER BY at")?;
            let rows = st
                .query_map(params![if since.is_none() { 0 } else { from }, now], |r| {
                    Ok(Play {
                        id: r.get(0)?,
                        at: r.get(1)?,
                        listened: r.get(2)?,
                        skipped: r.get::<_, i64>(3)? != 0,
                        title: r.get(4)?,
                        artist: r.get(5)?,
                        album: r.get(6)?,
                        origin: r.get(7)?,
                        format: r.get(8)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            // When each song was first ever heard.
            let mut st = db.prepare("SELECT title, artist, MIN(at) FROM plays GROUP BY lower(title), lower(artist)")?;
            let firsts = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok((rows, firsts))
        })?;

        let in_range = |p: &Play| since.is_none_or(|s| p.at >= s);
        let mut s = Stats { hours: vec![0.0; 24], days: vec![0.0; heat_days], ..Default::default() };
        if let Some(sp) = span {
            let start = since.unwrap_or(0) - sp;
            s.previous = Some(rows.iter().filter(|p| p.at >= start && p.at < since.unwrap_or(0)).map(|p| p.listened).sum());
        }

        let key = |t: &str, a: &str| (t.to_lowercase(), a.to_lowercase());
        let mut songs: HashMap<(String, String), TopSong> = HashMap::new();
        let mut artists: HashMap<String, TopArtist> = HashMap::new();
        let mut albums: HashMap<(String, String), TopAlbum> = HashMap::new();
        let mut sources: BTreeMap<String, f64> = BTreeMap::new();
        let mut formats: BTreeMap<String, f64> = BTreeMap::new();
        let mut by_day: BTreeMap<i64, f64> = BTreeMap::new();
        for p in &rows {
            let d = day(p.at);
            if d > today - heat_days as i64 && d <= today {
                s.days[(d - (today - heat_days as i64 + 1)) as usize] += p.listened;
            }
            if d == today {
                s.today += p.listened;
            }
            if !in_range(p) {
                continue;
            }
            s.listened += p.listened;
            *by_day.entry(d).or_default() += p.listened;
            s.hours[((p.at + tz).rem_euclid(86_400) / 3600) as usize] += p.listened;
            *sources.entry(p.origin.clone()).or_default() += p.listened;
            let f = match p.format.to_ascii_uppercase().as_str() {
                "" => "Other".to_string(),
                "M4A" | "MP4" | "AAC" | "M4B" => "M4A / AAC".to_string(),
                "OGG" | "OGA" => "Ogg".to_string(),
                "WAV" | "WAVE" => "WAV".to_string(),
                x => x.to_string(),
            };
            *formats.entry(f).or_default() += p.listened;
            if p.skipped {
                s.skips += 1;
            } else {
                s.plays += 1;
            }
            let counted = !p.skipped as i64;
            let song = songs.entry(key(&p.title, &p.artist)).or_insert_with(|| TopSong {
                id: None,
                title: p.title.clone(),
                artist: p.artist.clone(),
                album: p.album.clone(),
                plays: 0,
                skips: 0,
                avg: 0.0,
                art: None,
                art_color: None,
            });
            song.id = p.id.or(song.id);
            song.plays += counted;
            song.skips += p.skipped as i64;
            song.avg += p.listened;
            let a = artists.entry(p.artist.to_lowercase()).or_insert_with(|| TopArtist { name: p.artist.clone(), plays: 0, listened: 0.0, art: None, art_color: None });
            a.plays += counted;
            a.listened += p.listened;
            let al = albums.entry(key(&p.album, &p.artist)).or_insert_with(|| TopAlbum { album: p.album.clone(), artist: p.artist.clone(), plays: 0, art: None, art_color: None });
            al.plays += counted;
        }
        s.songs = songs.len() as i64;
        let firsts: HashSet<(String, String)> = firsts.into_iter().filter(|(_, _, at)| since.is_none_or(|s| *at >= s)).map(|(t, a, _)| key(&t, &a)).collect();
        s.new_songs = songs.keys().filter(|k| firsts.contains(*k)).count() as i64;

        // Covers come from the library, where the song still is.
        let mut top_songs: Vec<TopSong> = songs.into_values().collect();
        for t in &mut top_songs {
            let n = (t.plays + t.skips).max(1) as f64;
            t.avg /= n;
        }
        top_songs.sort_by(|a, b| b.plays.cmp(&a.plays).then(b.avg.total_cmp(&a.avg)).then(a.title.cmp(&b.title)));
        let art_of = |id: Option<i64>| id.and_then(|i| self.track(i).ok().flatten()).map(|t| (t.art, t.art_color)).unwrap_or((None, None));
        let mut top_artists: Vec<TopArtist> = artists.into_values().collect();
        top_artists.sort_by(|a, b| b.plays.cmp(&a.plays).then(b.listened.total_cmp(&a.listened)).then(a.name.cmp(&b.name)));
        top_artists.truncate(TOP);
        for a in &mut top_artists {
            // The cover of their most-played song.
            let best = top_songs.iter().find(|t| t.artist.eq_ignore_ascii_case(&a.name) && t.id.is_some());
            (a.art, a.art_color) = art_of(best.and_then(|t| t.id));
        }
        let mut top_albums: Vec<TopAlbum> = albums.into_values().collect();
        top_albums.sort_by(|a, b| b.plays.cmp(&a.plays).then(a.album.cmp(&b.album)));
        top_albums.truncate(TOP);
        for al in &mut top_albums {
            let best = top_songs.iter().find(|t| t.album.eq_ignore_ascii_case(&al.album) && t.artist.eq_ignore_ascii_case(&al.artist) && t.id.is_some());
            (al.art, al.art_color) = art_of(best.and_then(|t| t.id));
        }
        top_songs.truncate(TOP);
        for t in &mut top_songs {
            (t.art, t.art_color) = art_of(t.id);
        }
        s.top_songs = top_songs;
        s.top_artists = top_artists;
        s.top_albums = top_albums;

        let share = |m: BTreeMap<String, f64>| {
            let mut v: Vec<Share> = m.into_iter().map(|(key, listened)| Share { key, listened }).collect();
            v.sort_by(|a, b| b.listened.total_cmp(&a.listened));
            v
        };
        s.sources = share(sources);
        s.formats = share(formats);

        let mut run = 0;
        let mut last: Option<i64> = None;
        for (&d, &secs) in &by_day {
            if secs <= 0.0 {
                continue;
            }
            run = if last == Some(d - 1) { run + 1 } else { 1 };
            s.streak = s.streak.max(run);
            last = Some(d);
        }
        s.busiest_day = by_day.iter().max_by(|a, b| a.1.total_cmp(b.1)).map(|(&d, &l)| Busiest { at: d * 86_400 - tz, listened: l });
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use crate::music::{library::RemoteMeta, Library};

    #[test]
    fn counts_plays_skips_streaks_and_hours() {
        let t = tempfile::tempdir().unwrap();
        let lib = Library::open(t.path()).unwrap();
        let a = lib.remote(&RemoteMeta { url: "https://example.com/a".into(), title: "Alpha".into(), artist: "Ivy".into(), ..Default::default() }).unwrap();
        let b = lib.remote(&RemoteMeta { url: "https://example.com/b".into(), title: "Beta".into(), artist: "Moth".into(), ..Default::default() }).unwrap();
        lib.record_play(a, 200.0, false).unwrap();
        lib.record_play(a, 180.0, false).unwrap();
        lib.record_play(b, 8.0, true).unwrap();
        // Two days ago, twelve hours round the clock from now (UTC).
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        let hour = (now.rem_euclid(86_400) / 3600 + 12) % 24;
        let two_days = (now.div_euclid(86_400) - 2) * 86_400 + hour * 3600;
        lib.with_db(|db| db.execute("UPDATE plays SET at = ?1 WHERE track_id = ?2", rusqlite::params![two_days, b])).unwrap();

        let s = lib.stats(None, now, 0, 7).unwrap();
        assert_eq!((s.plays, s.skips, s.songs, s.new_songs), (2, 1, 2, 2));
        assert_eq!(s.listened, 388.0);
        assert_eq!(s.top_artists[0].name, "Ivy");
        assert_eq!(s.top_songs[0].title, "Alpha");
        assert_eq!(s.top_songs[0].avg, 190.0);
        assert_eq!(s.hours[hour as usize], 8.0);
        assert_eq!(s.days.len(), 7);
        assert_eq!(s.days[6], 380.0);
        assert_eq!(s.days[4], 8.0);
        assert_eq!(s.streak, 1, "a gap breaks the streak");
        assert_eq!(s.today, 380.0);
        assert_eq!(s.sources[0].key, "stream");

        // Just today: the older skip drops out, and the day before counts as the previous period.
        let s = lib.stats(Some(now - 86_400), now, 0, 7).unwrap();
        assert_eq!((s.plays, s.skips), (2, 0));
        // The older skip lands in the previous day only for part of the day.
        let in_previous = two_days >= now - 2 * 86_400;
        assert_eq!(s.previous, Some(if in_previous { 8.0 } else { 0.0 }));
    }
}
