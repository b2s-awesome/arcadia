//! Follows the Roblox client log to know which game you're in and which
//! server you're on (like Bloxstrap/Froststrap's activity tracker).

use regex::Regex;
use serde::Serialize;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Activity {
    Joining { place_id: u64, job_id: String },
    ServerIp { ip: String },
    /// The server dropped us: kicked for being idle, a lost connection or a
    /// shutdown. `reason` is Roblox's disconnect code when the log has one.
    Disconnected { reason: Option<u32> },
    Left,
}

fn re_join() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"! Joining game '([0-9a-fA-F\-]{36})' place ([0-9]+) at").unwrap())
}
fn re_server() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"RCC Server Address = ([0-9\.]+), Port").unwrap())
}
fn re_udmux() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"Connection accepted from ([0-9\.]+)\|").unwrap())
}

fn re_dropped() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"Disconnection Notification\. Reason: ([0-9]+)|(?i)(lost connection to the game server|kicked (?:due to|for) being idle)").unwrap())
}

pub fn parse_line(line: &str) -> Option<Activity> {
    if let Some(c) = re_join().captures(line) {
        return Some(Activity::Joining { place_id: c[2].parse().ok()?, job_id: c[1].to_string() });
    }
    if let Some(c) = re_server().captures(line).or_else(|| re_udmux().captures(line)) {
        return Some(Activity::ServerIp { ip: c[1].to_string() });
    }
    if let Some(c) = re_dropped().captures(line) {
        return Some(Activity::Disconnected { reason: c.get(1).and_then(|m| m.as_str().parse().ok()) });
    }
    if line.contains("[FLog::Network] Time to disconnect replication data") {
        return Some(Activity::Left);
    }
    None
}

fn newest_log_since(since: SystemTime) -> Option<PathBuf> {
    let dir = crate::paths::roblox_local_dir().join("logs");
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().contains("Player"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .filter(|(t, _)| *t >= since)
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

/// Watch the log of the client started at `since` until `stop` is set.
pub fn watch(since: SystemTime, stop: Arc<AtomicBool>, on: impl Fn(Activity) + Send + 'static) {
    std::thread::spawn(move || {
        let since = since - Duration::from_secs(2);
        let mut path = None;
        for _ in 0..60 {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            path = newest_log_since(since);
            if path.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        let Some(path) = path else { return };
        let Ok(f) = std::fs::File::open(&path) else { return };
        let mut r = BufReader::new(f);
        let mut pos = 0u64;
        let mut line = String::new();
        while !stop.load(Ordering::Relaxed) {
            line.clear();
            match r.read_line(&mut line) {
                Ok(0) => {
                    std::thread::sleep(Duration::from_millis(500));
                    let _ = r.seek(SeekFrom::Start(pos));
                }
                Ok(n) => {
                    // Partial line at EOF: wait for the rest.
                    if !line.ends_with('\n') {
                        let _ = r.seek(SeekFrom::Start(pos));
                        std::thread::sleep(Duration::from_millis(200));
                        continue;
                    }
                    pos += n as u64;
                    if let Some(a) = parse_line(&line) {
                        on(a);
                    }
                }
                Err(_) => break,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_join_lines() {
        let l = "2026-10-28T18:00:00.000Z,1.0,abcd,6 [FLog::Output] ! Joining game '4f1c9a3e-2b7d-4e8f-9a10-1234567890ab' place 920587237 at 10.0.0.1";
        assert_eq!(
            parse_line(l),
            Some(Activity::Joining { place_id: 920587237, job_id: "4f1c9a3e-2b7d-4e8f-9a10-1234567890ab".into() })
        );
        let s = "[FLog::Network] UDMUX Address = 1.2.3.4, Port = 5 | RCC Server Address = 128.116.1.2, Port = 6";
        assert_eq!(parse_line(s), Some(Activity::ServerIp { ip: "128.116.1.2".into() }));
        let d = "2026-10-28T18:20:00.000Z,1.0,abcd,6 [FLog::Network] Disconnection Notification. Reason: 277";
        assert_eq!(parse_line(d), Some(Activity::Disconnected { reason: Some(277) }));
        let k = "[FLog::Output] You were kicked for being idle more than 20 minutes";
        assert_eq!(parse_line(k), Some(Activity::Disconnected { reason: None }));
    }
}
