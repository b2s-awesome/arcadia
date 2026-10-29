//! The music library: folders scanned into a small SQLite database, with
//! tags, cover art thumbnails and an accent colour per cover.

use anyhow::{Context, Result};
use lofty::picture::PictureType;
use lofty::prelude::*;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Files the engine can play.
pub const EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "wave", "ogg", "oga", "m4a", "mp4", "m4b", "aac"];
const COVER_NAMES: &[&str] = &["cover", "folder", "front", "album", "albumart", "albumartsmall"];
const ART_SIZE: u32 = 512;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album_artist: String,
    pub album: String,
    pub genre: String,
    pub year: Option<u32>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub duration: f64,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    pub channels: Option<u8>,
    pub bitrate: Option<u32>,
    pub format: String,
    /// Cover thumbnail on disk.
    pub art: Option<String>,
    /// An accent colour picked from the cover, `#rrggbb`.
    pub art_color: Option<String>,
    pub added_at: i64,
    pub play_count: i64,
    pub last_played: Option<i64>,
    pub favourite: bool,
    /// "local" (from a library folder), "stream" (online, cached) or
    /// "download" (fetched and kept).
    pub origin: String,
    /// Where an online song came from.
    pub source_url: Option<String>,
}

/// What's known about an online song before its file exists.
#[derive(Debug, Clone, Default)]
pub struct RemoteMeta {
    pub url: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub count: i64,
    pub duration: f64,
    pub updated: i64,
    /// Up to four covers for the playlist tile.
    pub arts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub total: usize,
    /// Folders that weren't there (an unplugged drive): their tracks are kept.
    pub missing_folders: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub done: usize,
    pub total: usize,
    pub current: String,
}

pub struct Library {
    db: Mutex<Connection>,
    art_dir: PathBuf,
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

const SCHEMA: &str = "
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS folders (path TEXT PRIMARY KEY);
CREATE TABLE IF NOT EXISTS tracks (
  id INTEGER PRIMARY KEY,
  path TEXT NOT NULL UNIQUE,
  mtime INTEGER NOT NULL,
  size INTEGER NOT NULL,
  title TEXT NOT NULL,
  artist TEXT NOT NULL,
  album_artist TEXT NOT NULL,
  album TEXT NOT NULL,
  genre TEXT NOT NULL DEFAULT '',
  year INTEGER, track_no INTEGER, disc_no INTEGER,
  duration REAL NOT NULL DEFAULT 0,
  sample_rate INTEGER, bit_depth INTEGER, channels INTEGER, bitrate INTEGER,
  format TEXT NOT NULL DEFAULT '',
  art TEXT, art_color TEXT,
  added_at INTEGER NOT NULL,
  play_count INTEGER NOT NULL DEFAULT 0,
  last_played INTEGER,
  favourite INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS playlists (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  created INTEGER NOT NULL,
  updated INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS playlist_items (
  playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
  pos INTEGER NOT NULL,
  track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS playlist_items_by_list ON playlist_items(playlist_id, pos);
";

const TRACK_COLS: &str = "id, path, title, artist, album_artist, album, genre, year, track_no, disc_no, duration, sample_rate, bit_depth, channels, bitrate, format, art, art_color, added_at, play_count, last_played, favourite, origin, source_url";

/// Schema changes after the first release, applied in order.
const MIGRATIONS: &[&str] = &["ALTER TABLE tracks ADD COLUMN origin TEXT NOT NULL DEFAULT 'local'; ALTER TABLE tracks ADD COLUMN source_url TEXT; CREATE INDEX IF NOT EXISTS tracks_by_source ON tracks(source_url);"];

impl Library {
    pub fn open(dir: &Path) -> Result<Library> {
        std::fs::create_dir_all(dir.join("art"))?;
        let conn = Connection::open(dir.join("library.db")).context("Couldn't open the music library")?;
        conn.execute_batch(SCHEMA)?;
        let version: usize = conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))? as usize;
        for (i, m) in MIGRATIONS.iter().enumerate().skip(version) {
            conn.execute_batch(m)?;
            conn.execute_batch(&format!("PRAGMA user_version = {}", i + 1))?;
        }
        Ok(Library { db: Mutex::new(conn), art_dir: dir.join("art") })
    }

    fn row(&self, r: &rusqlite::Row) -> rusqlite::Result<Track> {
        let art: Option<String> = r.get(16)?;
        Ok(Track {
            id: r.get(0)?,
            path: r.get(1)?,
            title: r.get(2)?,
            artist: r.get(3)?,
            album_artist: r.get(4)?,
            album: r.get(5)?,
            genre: r.get(6)?,
            year: r.get(7)?,
            track_no: r.get(8)?,
            disc_no: r.get(9)?,
            duration: r.get(10)?,
            sample_rate: r.get(11)?,
            bit_depth: r.get(12)?,
            channels: r.get(13)?,
            bitrate: r.get(14)?,
            format: r.get(15)?,
            art: art.map(|a| self.art_dir.join(format!("{a}.jpg")).display().to_string()),
            art_color: r.get(17)?,
            added_at: r.get(18)?,
            play_count: r.get(19)?,
            last_played: r.get(20)?,
            favourite: r.get::<_, i64>(21)? != 0,
            origin: r.get(22)?,
            source_url: r.get(23)?,
        })
    }

    // ── folders ────────────────────────────────────────────────

    pub fn folders(&self) -> Result<Vec<String>> {
        let db = self.db.lock();
        let mut st = db.prepare("SELECT path FROM folders ORDER BY path")?;
        let v = st.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(v)
    }

    pub fn set_folders(&self, folders: &[String]) -> Result<()> {
        let mut db = self.db.lock();
        let tx = db.transaction()?;
        tx.execute("DELETE FROM folders", [])?;
        for f in folders {
            let f = f.trim();
            if !f.is_empty() {
                tx.execute("INSERT OR IGNORE INTO folders(path) VALUES (?1)", [f])?;
            }
        }
        // Tracks outside every folder go; their playlists entries with them.
        let keep: Vec<String> = folders.iter().map(|f| f.trim().to_string()).filter(|f| !f.is_empty()).collect();
        let paths: Vec<(i64, String)> = {
            let mut st = tx.prepare("SELECT id, path FROM tracks WHERE origin = 'local'")?;
            let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
            v
        };
        for (id, p) in paths {
            if !keep.iter().any(|k| Path::new(&p).starts_with(k)) {
                tx.execute("DELETE FROM tracks WHERE id = ?1", [id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    // ── reading ────────────────────────────────────────────────

    pub fn tracks(&self) -> Result<Vec<Track>> {
        let db = self.db.lock();
        let mut st = db.prepare(&format!("SELECT {TRACK_COLS} FROM tracks"))?;
        let v = st.query_map([], |r| self.row(r))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    pub fn track(&self, id: i64) -> Result<Option<Track>> {
        let db = self.db.lock();
        Ok(db.query_row(&format!("SELECT {TRACK_COLS} FROM tracks WHERE id = ?1"), [id], |r| self.row(r)).optional()?)
    }

    pub fn tracks_by_id(&self, ids: &[i64]) -> Result<Vec<Track>> {
        let all: HashMap<i64, Track> = {
            let db = self.db.lock();
            let mut st = db.prepare(&format!("SELECT {TRACK_COLS} FROM tracks WHERE id = ?1"))?;
            let mut m = HashMap::new();
            for id in ids.iter().collect::<HashSet<_>>() {
                if let Some(t) = st.query_row([id], |r| self.row(r)).optional()? {
                    m.insert(*id, t);
                }
            }
            m
        };
        Ok(ids.iter().filter_map(|i| all.get(i).cloned()).collect())
    }

    pub fn path_of(&self, id: i64) -> Option<PathBuf> {
        let db = self.db.lock();
        db.query_row("SELECT path FROM tracks WHERE id = ?1", [id], |r| r.get::<_, String>(0)).ok().map(PathBuf::from)
    }

    // ── online songs ───────────────────────────────────────────

    /// The library entry for an online song, made if it's new. It has no
    /// file until it's fetched (`path` is empty).
    pub fn remote(&self, m: &RemoteMeta) -> Result<i64> {
        let db = self.db.lock();
        if let Some(id) = db.query_row("SELECT id FROM tracks WHERE source_url = ?1", [&m.url], |r| r.get::<_, i64>(0)).optional()? {
            return Ok(id);
        }
        // `path` is unique: stand-in until the file exists.
        let placeholder = format!("remote:{}", m.url);
        db.execute(
            "INSERT INTO tracks(path, mtime, size, title, artist, album_artist, album, duration, format, added_at, origin, source_url)
             VALUES (?1, 0, 0, ?2, ?3, ?3, ?4, ?5, '', ?6, 'stream', ?7)",
            params![placeholder, m.title, m.artist, m.album.clone().unwrap_or_else(|| "Singles".into()), m.duration.unwrap_or(0.0), now(), m.url],
        )?;
        Ok(db.last_insert_rowid())
    }

    /// A playable file on disk for this track, if there is one.
    pub fn file_of(&self, id: i64) -> Option<PathBuf> {
        self.path_of(id).filter(|p| p.is_file())
    }

    pub fn source_of(&self, id: i64) -> Option<String> {
        self.db.lock().query_row("SELECT source_url FROM tracks WHERE id = ?1", [id], |r| r.get::<_, Option<String>>(0)).ok().flatten()
    }

    /// Point an online song at its file: a cached stream or a download.
    /// Tags and covers in the file win; `fallback` and `cover` fill gaps.
    pub fn attach_file(&self, id: i64, path: &Path, origin: &str, fallback: Option<&RemoteMeta>, cover: Option<&[u8]>) -> Result<()> {
        let meta = std::fs::metadata(path)?;
        let mtime = meta.modified().ok().and_then(|m| m.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0);
        let mut t = read_tags(path).unwrap_or_default();
        if let Some(f) = fallback {
            let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            if t.title.is_empty() || t.title == stem {
                t.title = f.title.clone();
            }
            if t.artist.is_empty() || t.artist == "Unknown artist" {
                t.artist = f.artist.clone();
                t.album_artist = f.artist.clone();
            }
            if t.album.is_empty() || t.album == "Unknown album" {
                t.album = f.album.clone().unwrap_or_else(|| "Singles".into());
            }
            if t.duration <= 0.0 {
                t.duration = f.duration.unwrap_or(0.0);
            }
        }
        if t.title.is_empty() {
            t.title = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        }
        if t.format.is_empty() {
            t.format = path.extension().map(|e| e.to_string_lossy().to_ascii_uppercase()).unwrap_or_default();
        }
        if let Some(bytes) = t.picture.take().or_else(|| cover.map(<[u8]>::to_vec)) {
            let hash = hex::encode(Sha1::digest(&bytes));
            if let Ok(c) = self.save_art(&hash, &bytes) {
                t.art = Some(hash);
                t.art_color = Some(c);
            }
        }
        let db = self.db.lock();
        // A file already in the library (e.g. a download inside a scanned
        // folder) merges into this entry instead of duplicating it.
        let p = path.display().to_string();
        if let Some(other) = db.query_row("SELECT id FROM tracks WHERE path = ?1 AND id != ?2", params![p, id], |r| r.get::<_, i64>(0)).optional()? {
            db.execute("UPDATE playlist_items SET track_id = ?1 WHERE track_id = ?2", params![id, other])?;
            db.execute("DELETE FROM tracks WHERE id = ?1", [other])?;
        }
        db.execute(
            "UPDATE tracks SET path=?2, mtime=?3, size=?4, title=?5, artist=?6, album_artist=?7, album=?8, genre=?9, year=?10, track_no=?11, disc_no=?12,
             duration=?13, sample_rate=?14, bit_depth=?15, channels=?16, bitrate=?17, format=?18, art=COALESCE(?19, art), art_color=COALESCE(?20, art_color), origin=?21 WHERE id=?1",
            params![id, p, mtime, meta.len() as i64, t.title, t.artist, if t.album_artist.is_empty() { t.artist.clone() } else { t.album_artist.clone() }, t.album, t.genre, t.year, t.track_no, t.disc_no,
                t.duration, t.sample_rate, t.bit_depth, t.channels, t.bitrate, t.format, t.art, t.art_color, origin],
        )?;
        Ok(())
    }

    /// Give a track a cover from image bytes (e.g. an online thumbnail).
    pub fn set_art(&self, id: i64, bytes: &[u8]) -> Result<()> {
        let hash = hex::encode(Sha1::digest(bytes));
        let color = self.save_art(&hash, bytes)?;
        self.db.lock().execute("UPDATE tracks SET art = ?2, art_color = ?3 WHERE id = ?1", params![id, hash, color])?;
        Ok(())
    }

    pub fn has_art(&self, id: i64) -> bool {
        self.db.lock().query_row("SELECT art IS NOT NULL FROM tracks WHERE id = ?1", [id], |r| r.get::<_, bool>(0)).unwrap_or(false)
    }

    /// Cached stream files, least recently played first (for trimming the cache).
    pub fn cached_streams(&self) -> Result<Vec<(i64, PathBuf)>> {
        let db = self.db.lock();
        let mut st = db.prepare("SELECT id, path FROM tracks WHERE origin = 'stream' AND path NOT LIKE 'remote:%' ORDER BY COALESCE(last_played, added_at)")?;
        let v = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, PathBuf::from(r.get::<_, String>(1)?))))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// Forget a cached file (it's fetched again next time it plays).
    pub fn uncache(&self, id: i64) -> Result<()> {
        self.db.lock().execute("UPDATE tracks SET path = 'remote:' || source_url, size = 0 WHERE id = ?1 AND origin = 'stream' AND source_url IS NOT NULL", [id])?;
        Ok(())
    }

    // ── play history and favourites ────────────────────────────

    pub fn mark_played(&self, id: i64) -> Result<()> {
        self.db.lock().execute("UPDATE tracks SET play_count = play_count + 1, last_played = ?2 WHERE id = ?1", params![id, now()])?;
        Ok(())
    }

    pub fn set_favourite(&self, id: i64, on: bool) -> Result<()> {
        self.db.lock().execute("UPDATE tracks SET favourite = ?2 WHERE id = ?1", params![id, on as i64])?;
        Ok(())
    }

    // ── playlists ──────────────────────────────────────────────

    pub fn playlists(&self) -> Result<Vec<Playlist>> {
        let db = self.db.lock();
        let mut st = db.prepare(
            "SELECT p.id, p.name, p.updated, COUNT(t.id), COALESCE(SUM(t.duration), 0)
             FROM playlists p LEFT JOIN playlist_items i ON i.playlist_id = p.id LEFT JOIN tracks t ON t.id = i.track_id
             GROUP BY p.id ORDER BY p.updated DESC",
        )?;
        let mut v = st
            .query_map([], |r| Ok(Playlist { id: r.get(0)?, name: r.get(1)?, updated: r.get(2)?, count: r.get(3)?, duration: r.get(4)?, arts: Vec::new() }))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut arts = db.prepare(
            "SELECT DISTINCT t.art FROM playlist_items i JOIN tracks t ON t.id = i.track_id
             WHERE i.playlist_id = ?1 AND t.art IS NOT NULL ORDER BY i.pos LIMIT 4",
        )?;
        for p in &mut v {
            p.arts = arts
                .query_map([p.id], |r| r.get::<_, String>(0))?
                .filter_map(|a| a.ok())
                .map(|a| self.art_dir.join(format!("{a}.jpg")).display().to_string())
                .collect();
        }
        Ok(v)
    }

    pub fn playlist_items(&self, id: i64) -> Result<Vec<i64>> {
        let db = self.db.lock();
        let mut st = db.prepare("SELECT track_id FROM playlist_items WHERE playlist_id = ?1 ORDER BY pos")?;
        let v = st.query_map([id], |r| r.get(0))?.collect::<rusqlite::Result<Vec<i64>>>()?;
        Ok(v)
    }

    pub fn playlist_create(&self, name: &str, ids: &[i64]) -> Result<i64> {
        let name = if name.trim().is_empty() { "New playlist" } else { name.trim() };
        let id = {
            let db = self.db.lock();
            db.execute("INSERT INTO playlists(name, created, updated) VALUES (?1, ?2, ?2)", params![name, now()])?;
            db.last_insert_rowid()
        };
        self.playlist_set(id, ids)?;
        Ok(id)
    }

    pub fn playlist_rename(&self, id: i64, name: &str) -> Result<()> {
        self.db.lock().execute("UPDATE playlists SET name = ?2, updated = ?3 WHERE id = ?1", params![id, name.trim(), now()])?;
        Ok(())
    }

    pub fn playlist_delete(&self, id: i64) -> Result<()> {
        self.db.lock().execute("DELETE FROM playlists WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Replace a playlist's tracks (used for add, remove and reorder).
    pub fn playlist_set(&self, id: i64, ids: &[i64]) -> Result<()> {
        let mut db = self.db.lock();
        let tx = db.transaction()?;
        tx.execute("DELETE FROM playlist_items WHERE playlist_id = ?1", [id])?;
        {
            let mut st = tx.prepare("INSERT INTO playlist_items(playlist_id, pos, track_id) SELECT ?1, ?2, id FROM tracks WHERE id = ?3")?;
            for (pos, t) in ids.iter().enumerate() {
                st.execute(params![id, pos as i64, t])?;
            }
        }
        tx.execute("UPDATE playlists SET updated = ?2 WHERE id = ?1", params![id, now()])?;
        tx.commit()?;
        Ok(())
    }

    pub fn playlist_add(&self, id: i64, ids: &[i64]) -> Result<()> {
        let mut cur = self.playlist_items(id)?;
        cur.extend_from_slice(ids);
        self.playlist_set(id, &cur)
    }

    // ── scanning ───────────────────────────────────────────────

    /// Scan every folder: new and changed files are read, missing ones
    /// removed (unless their whole folder is missing, e.g. an unplugged drive).
    pub fn scan(&self, progress: impl Fn(ScanProgress)) -> Result<ScanReport> {
        let folders = self.folders()?;
        let mut report = ScanReport::default();
        let mut files: Vec<PathBuf> = Vec::new();
        let mut present_roots = Vec::new();
        for f in &folders {
            let root = Path::new(f);
            if root.is_dir() {
                present_roots.push(root.to_path_buf());
                walk(root, &mut files);
            } else {
                report.missing_folders.push(f.clone());
            }
        }
        let known: HashMap<String, (i64, i64, i64)> = {
            let db = self.db.lock();
            let mut st = db.prepare("SELECT path, mtime, size, id FROM tracks")?;
            let v = st
                .query_map([], |r| Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?, r.get(3)?))))?
                .collect::<rusqlite::Result<HashMap<_, _>>>()?;
            v
        };
        let total = files.len();
        let mut seen: HashSet<String> = HashSet::new();
        let mut folder_art: HashMap<PathBuf, Option<Vec<u8>>> = HashMap::new();
        let mut art_done: HashMap<String, Option<String>> = HashMap::new();
        let mut pending: Vec<(Option<i64>, Tags, String, i64, i64)> = Vec::new();

        for (i, path) in files.iter().enumerate() {
            let key = path.display().to_string();
            seen.insert(key.clone());
            if i % 25 == 0 || i + 1 == total {
                progress(ScanProgress { done: i, total, current: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default() });
            }
            let Ok(meta) = std::fs::metadata(path) else { continue };
            let mtime = meta.modified().ok().and_then(|m| m.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0);
            let size = meta.len() as i64;
            let existing = known.get(&key);
            if let Some((m, s, _)) = existing {
                if *m == mtime && *s == size {
                    continue;
                }
            }
            let mut tags = match read_tags(path) {
                Ok(t) => t,
                Err(e) => {
                    report.errors.push(format!("{}: {e}", path.display()));
                    continue;
                }
            };
            // Cover: embedded, else an image beside the file.
            let image = tags.picture.take().or_else(|| {
                let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
                folder_art.entry(dir.clone()).or_insert_with(|| folder_cover(&dir)).clone()
            });
            if let Some(bytes) = image {
                let hash = hex::encode(Sha1::digest(&bytes));
                let color = art_done.entry(hash.clone()).or_insert_with(|| self.save_art(&hash, &bytes).ok()).clone();
                if let Some(c) = color {
                    tags.art = Some(hash);
                    tags.art_color = Some(c);
                }
            }
            pending.push((existing.map(|e| e.2), tags, key, mtime, size));
            if pending.len() >= 200 {
                self.write(&mut pending, &mut report)?;
            }
        }
        self.write(&mut pending, &mut report)?;

        // Remove what's gone, but only under folders that are actually there.
        let gone: Vec<i64> = known
            .iter()
            .filter(|(p, _)| !seen.contains(*p) && present_roots.iter().any(|r| Path::new(p).starts_with(r)))
            .map(|(_, v)| v.2)
            .collect();
        {
            let mut db = self.db.lock();
            let tx = db.transaction()?;
            for id in &gone {
                tx.execute("DELETE FROM tracks WHERE id = ?1", [id])?;
            }
            tx.commit()?;
        }
        report.removed = gone.len();
        report.total = self.db.lock().query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get::<_, i64>(0))? as usize;
        progress(ScanProgress { done: total, total, current: String::new() });
        Ok(report)
    }

    fn write(&self, pending: &mut Vec<(Option<i64>, Tags, String, i64, i64)>, report: &mut ScanReport) -> Result<()> {
        let mut db = self.db.lock();
        let tx = db.transaction()?;
        for (id, t, path, mtime, size) in pending.drain(..) {
            match id {
                Some(id) => {
                    tx.execute(
                        "UPDATE tracks SET mtime=?2, size=?3, title=?4, artist=?5, album_artist=?6, album=?7, genre=?8, year=?9, track_no=?10, disc_no=?11,
                         duration=?12, sample_rate=?13, bit_depth=?14, channels=?15, bitrate=?16, format=?17, art=?18, art_color=?19 WHERE id=?1",
                        params![id, mtime, size, t.title, t.artist, t.album_artist, t.album, t.genre, t.year, t.track_no, t.disc_no, t.duration, t.sample_rate, t.bit_depth, t.channels, t.bitrate, t.format, t.art, t.art_color],
                    )?;
                    report.updated += 1;
                }
                None => {
                    tx.execute(
                        "INSERT INTO tracks(path, mtime, size, title, artist, album_artist, album, genre, year, track_no, disc_no, duration, sample_rate, bit_depth, channels, bitrate, format, art, art_color, added_at)
                         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)",
                        params![path, mtime, size, t.title, t.artist, t.album_artist, t.album, t.genre, t.year, t.track_no, t.disc_no, t.duration, t.sample_rate, t.bit_depth, t.channels, t.bitrate, t.format, t.art, t.art_color, now()],
                    )?;
                    report.added += 1;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Save a cover thumbnail and return its accent colour.
    fn save_art(&self, hash: &str, bytes: &[u8]) -> Result<String> {
        let img = image::load_from_memory(bytes)?;
        let thumb = img.thumbnail(ART_SIZE, ART_SIZE).to_rgb8();
        let out = self.art_dir.join(format!("{hash}.jpg"));
        if !out.exists() {
            let mut f = std::io::BufWriter::new(std::fs::File::create(&out)?);
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut f, 86).encode_image(&thumb)?;
        }
        Ok(accent(&thumb))
    }
}

#[derive(Debug, Default)]
struct Tags {
    title: String,
    artist: String,
    album_artist: String,
    album: String,
    genre: String,
    year: Option<u32>,
    track_no: Option<u32>,
    disc_no: Option<u32>,
    duration: f64,
    sample_rate: Option<u32>,
    bit_depth: Option<u8>,
    channels: Option<u8>,
    bitrate: Option<u32>,
    format: String,
    picture: Option<Vec<u8>>,
    art: Option<String>,
    art_color: Option<String>,
}

fn read_tags(path: &Path) -> Result<Tags> {
    let tagged = lofty::read_from_path(path)?;
    let props = tagged.properties();
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut t = Tags {
        title: stem,
        artist: "Unknown artist".into(),
        album: "Unknown album".into(),
        duration: props.duration().as_secs_f64(),
        sample_rate: props.sample_rate(),
        bit_depth: props.bit_depth(),
        channels: props.channels(),
        bitrate: props.audio_bitrate(),
        format: path.extension().map(|e| e.to_string_lossy().to_ascii_uppercase()).unwrap_or_default(),
        ..Default::default()
    };
    if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
        let clean = |s: Option<std::borrow::Cow<'_, str>>| s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        if let Some(v) = clean(tag.title()) {
            t.title = v;
        }
        if let Some(v) = clean(tag.artist()) {
            t.artist = v;
        }
        if let Some(v) = clean(tag.album()) {
            t.album = v;
        }
        t.genre = clean(tag.genre()).unwrap_or_default();
        t.year = tag.year();
        t.track_no = tag.track();
        t.disc_no = tag.disk();
        t.album_artist = tag.get_string(&ItemKey::AlbumArtist).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_default();
        let pics = tag.pictures();
        t.picture = pics
            .iter()
            .find(|p| p.pic_type() == PictureType::CoverFront)
            .or_else(|| pics.first())
            .map(|p| p.data().to_vec());
    }
    if t.album_artist.is_empty() {
        t.album_artist = t.artist.clone();
    }
    Ok(t)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let name = e.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let Ok(ft) = e.file_type() else { continue };
        let p = e.path();
        if ft.is_dir() {
            walk(&p, out);
        } else if ft.is_file() {
            let ext = p.extension().and_then(|x| x.to_str()).map(|x| x.to_ascii_lowercase()).unwrap_or_default();
            if EXTENSIONS.contains(&ext.as_str()) {
                out.push(p);
            }
        }
    }
}

fn folder_cover(dir: &Path) -> Option<Vec<u8>> {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut best: Option<(usize, PathBuf)> = None;
    for e in rd.flatten() {
        let p = e.path();
        let ext = p.extension().and_then(|x| x.to_str()).map(|x| x.to_ascii_lowercase()).unwrap_or_default();
        if !matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp" | "bmp") {
            continue;
        }
        let stem = p.file_stem().map(|s| s.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if let Some(rank) = COVER_NAMES.iter().position(|n| stem == *n) {
            if best.as_ref().is_none_or(|(r, _)| rank < *r) {
                best = Some((rank, p));
            }
        }
    }
    std::fs::read(best?.1).ok()
}

/// A lively, readable accent colour from a cover: the most common
/// colourful hue, nudged to a mid lightness so it works on dark and light.
pub fn accent(img: &image::RgbImage) -> String {
    let small = image::imageops::thumbnail(img, 32, 32);
    let mut buckets = [(0f64, 0f64, 0f64, 0f64); 12];
    let mut grey = (0f64, 0f64, 0f64, 0f64);
    for p in small.pixels() {
        let (h, s, l) = hsl(p[0], p[1], p[2]);
        if !(0.08..=0.92).contains(&l) {
            continue;
        }
        let w = s * s * (1.0 - (l - 0.5).abs() * 1.4).max(0.05);
        if s < 0.18 {
            grey.0 += 1.0;
            grey.1 += p[0] as f64;
            grey.2 += p[1] as f64;
            grey.3 += p[2] as f64;
            continue;
        }
        let b = &mut buckets[((h * 12.0) as usize).min(11)];
        b.0 += w;
        b.1 += p[0] as f64 * w;
        b.2 += p[1] as f64 * w;
        b.3 += p[2] as f64 * w;
    }
    let best = buckets.iter().copied().fold((0.0, 0.0, 0.0, 0.0), |a, b| if b.0 > a.0 { b } else { a });
    let (r, g, b) = if best.0 > 0.6 {
        (best.1 / best.0, best.2 / best.0, best.3 / best.0)
    } else if grey.0 > 0.0 {
        (grey.1 / grey.0, grey.2 / grey.0, grey.3 / grey.0)
    } else {
        (136.0, 136.0, 160.0)
    };
    let (h, s, l) = hsl(r as u8, g as u8, b as u8);
    let (r, g, b) = from_hsl(h, s.clamp(0.0, 0.75), l.clamp(0.52, 0.68));
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn hsl(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let (r, g, b) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if max == min {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    (h, s, l)
}

fn from_hsl(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    if s == 0.0 {
        let v = (l * 255.0).round() as u8;
        return (v, v, v);
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |mut t: f64| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round().clamp(0.0, 255.0) as u8
    };
    (f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn wav(p: &Path, secs: u32) {
        let rate = 8000u32;
        let frames = rate * secs;
        let mut f = std::fs::File::create(p).unwrap();
        let data = frames * 2;
        f.write_all(b"RIFF").unwrap();
        f.write_all(&(36 + data).to_le_bytes()).unwrap();
        f.write_all(b"WAVEfmt ").unwrap();
        f.write_all(&16u32.to_le_bytes()).unwrap();
        f.write_all(&1u16.to_le_bytes()).unwrap();
        f.write_all(&1u16.to_le_bytes()).unwrap();
        f.write_all(&rate.to_le_bytes()).unwrap();
        f.write_all(&(rate * 2).to_le_bytes()).unwrap();
        f.write_all(&2u16.to_le_bytes()).unwrap();
        f.write_all(&16u16.to_le_bytes()).unwrap();
        f.write_all(b"data").unwrap();
        f.write_all(&data.to_le_bytes()).unwrap();
        f.write_all(&vec![0u8; data as usize]).unwrap();
    }

    #[test]
    fn scans_updates_and_removes() {
        let t = tempfile::tempdir().unwrap();
        let music = t.path().join("Music");
        let album = music.join("Artist").join("Album");
        std::fs::create_dir_all(&album).unwrap();
        wav(&album.join("01 First.wav"), 2);
        wav(&album.join("02 Second.wav"), 1);
        std::fs::write(album.join("notes.txt"), "not music").unwrap();
        // A red folder cover.
        let mut img = image::RgbImage::new(8, 8);
        for p in img.pixels_mut() {
            *p = image::Rgb([220, 30, 40]);
        }
        img.save(album.join("cover.png")).unwrap();

        let lib = Library::open(&t.path().join("db")).unwrap();
        lib.set_folders(&[music.display().to_string()]).unwrap();
        let r = lib.scan(|_| {}).unwrap();
        assert_eq!((r.added, r.total), (2, 2));
        let tracks = lib.tracks().unwrap();
        let first = tracks.iter().find(|t| t.title == "01 First").unwrap();
        assert!((first.duration - 2.0).abs() < 0.05);
        assert!(first.art.as_deref().is_some_and(|a| Path::new(a).exists()));
        let c = first.art_color.clone().unwrap();
        assert!(c.starts_with("#") && u8::from_str_radix(&c[1..3], 16).unwrap() > 150, "red cover gives a red accent: {c}");

        // Nothing changed: nothing re-read.
        let r = lib.scan(|_| {}).unwrap();
        assert_eq!((r.added, r.updated, r.removed), (0, 0, 0));

        // Playlists follow tracks.
        let ids: Vec<i64> = tracks.iter().map(|t| t.id).collect();
        let pl = lib.playlist_create("Mix", &ids).unwrap();
        assert_eq!(lib.playlist_items(pl).unwrap().len(), 2);
        std::fs::remove_file(album.join("02 Second.wav")).unwrap();
        let r = lib.scan(|_| {}).unwrap();
        assert_eq!((r.removed, r.total), (1, 1));
        assert_eq!(lib.playlist_items(pl).unwrap().len(), 1);
        assert_eq!(lib.playlists().unwrap()[0].count, 1);

        // An unplugged drive doesn't wipe the library.
        std::fs::rename(&music, t.path().join("away")).unwrap();
        let r = lib.scan(|_| {}).unwrap();
        assert_eq!((r.removed, r.total, r.missing_folders.len()), (0, 1, 1));

        lib.mark_played(first.id).unwrap();
        lib.set_favourite(first.id, true).unwrap();
        let again = lib.track(first.id).unwrap().unwrap();
        assert_eq!((again.play_count, again.favourite), (1, true));
    }

    #[test]
    fn accents_are_readable() {
        let mut dark_blue = image::RgbImage::new(4, 4);
        for p in dark_blue.pixels_mut() {
            *p = image::Rgb([10, 20, 90]);
        }
        let c = accent(&dark_blue);
        let (_, _, l) = hsl(u8::from_str_radix(&c[1..3], 16).unwrap(), u8::from_str_radix(&c[3..5], 16).unwrap(), u8::from_str_radix(&c[5..7], 16).unwrap());
        assert!(l >= 0.5, "{c} is too dark to use as an accent");
    }
}

#[cfg(test)]
mod remote_tests {
    use super::*;

    #[test]
    fn online_songs_get_files_later_and_survive_folder_changes() {
        let t = tempfile::tempdir().unwrap();
        let lib = Library::open(&t.path().join("db")).unwrap();
        let m = RemoteMeta { url: "https://soundcloud.com/a/b".into(), title: "B".into(), artist: "A".into(), album: None, duration: Some(1.0) };
        let id = lib.remote(&m).unwrap();
        assert_eq!(lib.remote(&m).unwrap(), id, "same link, same entry");
        assert!(lib.file_of(id).is_none());
        let tr = lib.track(id).unwrap().unwrap();
        assert_eq!((tr.origin.as_str(), tr.album.as_str(), tr.source_url.as_deref()), ("stream", "Singles", Some("https://soundcloud.com/a/b")));

        // Folder changes don't touch online songs.
        lib.set_folders(&[t.path().join("nowhere").display().to_string()]).unwrap();
        assert!(lib.track(id).unwrap().is_some());

        // The file arrives (no tags in it): the known details are kept.
        let f = t.path().join("b.wav");
        std::fs::write(&f, minimal_wav()).unwrap();
        lib.attach_file(id, &f, "stream", Some(&m), None).unwrap();
        let tr = lib.track(id).unwrap().unwrap();
        assert_eq!((tr.title.as_str(), tr.artist.as_str()), ("B", "A"));
        assert_eq!(lib.file_of(id).as_deref(), Some(f.as_path()));
        assert_eq!(lib.cached_streams().unwrap().len(), 1);
        lib.uncache(id).unwrap();
        assert!(lib.file_of(id).is_none());
        assert!(lib.cached_streams().unwrap().is_empty());
    }

    fn minimal_wav() -> Vec<u8> {
        let mut v = Vec::new();
        let data = 1600u32;
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36 + data).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&8000u32.to_le_bytes());
        v.extend_from_slice(&16000u32.to_le_bytes());
        v.extend_from_slice(&2u16.to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data.to_le_bytes());
        v.extend(std::iter::repeat_n(0u8, data as usize));
        v
    }
}
