//! Live request log for the proxy's Inspector tab.

use parking_lot::Mutex;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::broadcast;

const KEEP: usize = 2000;
/// Bodies larger than this are truncated in the inspector (traffic itself is untouched).
pub const BODY_PREVIEW: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: u64,
    pub time_ms: u64,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub req_bytes: usize,
    pub resp_bytes: usize,
    pub duration_ms: u64,
    /// "tunnel", "passthrough", "modified", "served-file", "blocked", "error"
    pub kind: String,
    /// Human summary, e.g. "replaced 2 · removed 1".
    pub note: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resp_body: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub requests: u64,
    pub replaced: u64,
    pub removed: u64,
    pub served_files: u64,
    pub fflag_injections: u64,
}

pub struct Inspector {
    next: AtomicU64,
    log: Mutex<VecDeque<Entry>>,
    stats: Mutex<Stats>,
    pub tx: broadcast::Sender<Entry>,
    pub capture_bodies: std::sync::atomic::AtomicBool,
}

impl Default for Inspector {
    fn default() -> Self {
        Self {
            next: AtomicU64::new(1),
            log: Mutex::new(VecDeque::with_capacity(KEEP)),
            stats: Mutex::new(Stats::default()),
            tx: broadcast::channel(512).0,
            capture_bodies: std::sync::atomic::AtomicBool::new(true),
        }
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn preview(b: &[u8]) -> Option<String> {
    if b.is_empty() {
        return None;
    }
    let cut = &b[..b.len().min(BODY_PREVIEW)];
    // Only text-ish bodies are worth showing.
    match std::str::from_utf8(cut) {
        Ok(s) => Some(s.to_string()),
        Err(e) if e.valid_up_to() > cut.len().saturating_sub(4) => Some(String::from_utf8_lossy(cut).into()),
        Err(_) => Some(format!("<{} bytes binary>", b.len())),
    }
}

impl Inspector {
    pub fn record(&self, mut e: Entry) {
        e.id = self.next.fetch_add(1, Ordering::Relaxed);
        if !self.capture_bodies.load(Ordering::Relaxed) {
            e.req_body = None;
            e.resp_body = None;
        }
        self.stats.lock().requests += 1;
        {
            let mut l = self.log.lock();
            if l.len() >= KEEP {
                l.pop_front();
            }
            l.push_back(e.clone());
        }
        let _ = self.tx.send(e);
    }

    pub fn bump(&self, f: impl FnOnce(&mut Stats)) {
        f(&mut self.stats.lock());
    }

    pub fn snapshot(&self) -> Vec<Entry> {
        self.log.lock().iter().cloned().collect()
    }

    pub fn stats(&self) -> Stats {
        self.stats.lock().clone()
    }

    pub fn clear(&self) {
        self.log.lock().clear();
    }
}
