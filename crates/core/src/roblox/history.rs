//! Games you've played on Roblox: every visit (which server, which account,
//! how long) and a per-game total, from the client log's join and leave lines.

use anyhow::Result;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const MAX_VISITS: usize = 400;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Visit {
    pub place_id: u64,
    pub job_id: String,
    /// The Roblox client this happened in (one per launch).
    pub session: u64,
    #[serde(default)]
    pub account: Option<u64>,
    pub started: u64,
    #[serde(default)]
    pub ended: Option<u64>,
    /// "City, Country" of the server, once known.
    #[serde(default)]
    pub region: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Played {
    pub place_id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub creator: String,
    #[serde(default)]
    pub icon: Option<String>,
    /// Seconds played in total.
    pub seconds: u64,
    pub sessions: u32,
    pub last: u64,
    #[serde(default)]
    pub favourite: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct History {
    pub visits: Vec<Visit>,
    pub games: Vec<Played>,
}

fn file() -> PathBuf {
    crate::paths::roblox_dir().join("history.json")
}

static LOCK: Mutex<()> = Mutex::new(());

pub fn load() -> History {
    std::fs::read(file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn edit<T>(f: impl FnOnce(&mut History) -> T) -> Result<T> {
    let _g = LOCK.lock();
    let mut h = load();
    let out = f(&mut h);
    let p = file();
    std::fs::create_dir_all(p.parent().unwrap())?;
    crate::paths::write_atomic(&p, &serde_json::to_vec(&h)?)?;
    Ok(out)
}

impl History {
    fn game(&mut self, place: u64) -> &mut Played {
        if let Some(i) = self.games.iter().position(|g| g.place_id == place) {
            return &mut self.games[i];
        }
        self.games.push(Played { place_id: place, ..Default::default() });
        self.games.last_mut().unwrap()
    }

    /// Close the open visit of `session`, adding its time to the game.
    fn close(&mut self, session: u64, at: u64) {
        let open: Vec<usize> = self.visits.iter().enumerate().filter(|(_, v)| v.session == session && v.ended.is_none()).map(|(i, _)| i).collect();
        for i in open {
            let v = &mut self.visits[i];
            v.ended = Some(at.max(v.started));
            let (place, secs) = (v.place_id, at.saturating_sub(v.started));
            let g = self.game(place);
            g.seconds += secs;
            g.last = g.last.max(at);
        }
    }

    pub fn joined(&mut self, session: u64, place: u64, job: &str, account: Option<u64>, at: u64) {
        // A teleport is a leave and a join in the same client.
        self.close(session, at);
        self.visits.push(Visit { place_id: place, job_id: job.into(), session, account, started: at, ended: None, region: None });
        if self.visits.len() > MAX_VISITS {
            let extra = self.visits.len() - MAX_VISITS;
            self.visits.drain(..extra);
        }
        let g = self.game(place);
        g.sessions += 1;
        g.last = at;
    }

    pub fn left(&mut self, session: u64, at: u64) {
        self.close(session, at);
    }
}

pub fn joined(session: u64, place: u64, job: &str, account: Option<u64>, at: u64) -> Result<()> {
    edit(|h| h.joined(session, place, job, account, at))
}

pub fn left(session: u64, at: u64) -> Result<()> {
    edit(|h| h.left(session, at))
}

pub fn set_region(job: &str, region: &str) -> Result<()> {
    edit(|h| {
        for v in h.visits.iter_mut().filter(|v| v.job_id == job) {
            v.region = Some(region.to_string());
        }
    })
}

/// Names and icons, fetched after a join.
pub fn set_info(place: u64, name: &str, creator: &str, icon: Option<&str>) -> Result<()> {
    edit(|h| {
        let g = h.game(place);
        g.name = name.into();
        g.creator = creator.into();
        if icon.is_some() {
            g.icon = icon.map(str::to_string);
        }
    })
}

pub fn set_favourite(place: u64, on: bool) -> Result<()> {
    edit(|h| h.game(place).favourite = on)
}

/// Remember a game you haven't played yet (added from a link).
pub fn add(place: u64) -> Result<()> {
    edit(|h| {
        h.game(place);
    })
}

pub fn forget(place: u64) -> Result<()> {
    edit(|h| {
        h.games.retain(|g| g.place_id != place);
        h.visits.retain(|v| v.place_id != place);
    })
}

/// Clear the visit list (totals and favourites stay).
pub fn clear_visits() -> Result<()> {
    edit(|h| h.visits.retain(|v| v.ended.is_none()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_time_across_teleports_and_clients() {
        let mut h = History::default();
        h.joined(1, 100, "a", None, 1_000);
        h.joined(2, 200, "b", Some(7), 1_010);
        // Teleport inside client 1.
        h.joined(1, 101, "c", None, 1_300);
        h.left(1, 1_400);
        h.left(2, 1_610);
        let secs = |h: &History, p| h.games.iter().find(|g| g.place_id == p).unwrap().seconds;
        assert_eq!(secs(&h, 100), 300);
        assert_eq!(secs(&h, 101), 100);
        assert_eq!(secs(&h, 200), 600);
        assert!(h.visits.iter().all(|v| v.ended.is_some()));
        assert_eq!(h.visits[1].account, Some(7));
        // Leaving twice doesn't count twice.
        h.left(2, 9_999);
        assert_eq!(secs(&h, 200), 600);
    }
}
