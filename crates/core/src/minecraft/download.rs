//! Verified, resumable-by-skip, parallel downloads.

use anyhow::{bail, Context, Result};
use futures_util::{stream, StreamExt};
use sha1::{Digest, Sha1};
use sha2::Sha512;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Job {
    pub url: String,
    pub dest: PathBuf,
    pub sha1: Option<String>,
    pub sha512: Option<String>,
    pub size: Option<u64>,
}

pub fn sha1_file(path: &Path) -> Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha1::new();
    std::io::copy(&mut f, &mut h)?;
    Ok(hex::encode(h.finalize()))
}

pub fn sha512_file(path: &Path) -> Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha512::new();
    std::io::copy(&mut f, &mut h)?;
    Ok(hex::encode(h.finalize()))
}

/// Does the file already exist with the expected contents?
pub fn is_valid(job: &Job) -> bool {
    let Ok(meta) = std::fs::metadata(&job.dest) else { return false };
    if let Some(size) = job.size {
        if size > 0 && meta.len() != size {
            return false;
        }
    }
    if let Some(want) = &job.sha1 {
        return sha1_file(&job.dest).map(|h| h.eq_ignore_ascii_case(want)).unwrap_or(false);
    }
    if let Some(want) = &job.sha512 {
        return sha512_file(&job.dest).map(|h| h.eq_ignore_ascii_case(want)).unwrap_or(false);
    }
    // No hash to check against: trust a non-empty existing file.
    meta.len() > 0
}

/// HTTPS, or a server on this machine (used by tests and local mirrors).
pub fn is_secure_url(u: &str) -> bool {
    u.starts_with("https://") || u.starts_with("http://127.0.0.1:") || u.starts_with("http://localhost:")
}

pub async fn fetch(client: &reqwest::Client, job: &Job) -> Result<()> {
    if is_valid(job) {
        return Ok(());
    }
    if !is_secure_url(&job.url) {
        bail!("refusing to download over an insecure connection: {}", job.url);
    }
    if let Some(p) = job.dest.parent() {
        std::fs::create_dir_all(p)?;
    }
    let tmp = job.dest.with_extension("part");
    let mut last = None;
    for attempt in 0..3 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(400 * attempt)).await;
        }
        let res: Result<()> = async {
            let r = client.get(&job.url).send().await?.error_for_status()?;
            let mut f = std::fs::File::create(&tmp)?;
            let mut s1 = Sha1::new();
            let mut s5 = Sha512::new();
            let mut body = r.bytes_stream();
            while let Some(chunk) = body.next().await {
                let chunk = chunk?;
                if job.sha1.is_some() {
                    s1.update(&chunk);
                }
                if job.sha512.is_some() {
                    s5.update(&chunk);
                }
                f.write_all(&chunk)?;
            }
            f.flush()?;
            drop(f);
            if let Some(want) = &job.sha1 {
                let got = hex::encode(s1.finalize());
                if !got.eq_ignore_ascii_case(want) {
                    bail!("checksum mismatch (sha1 {got}, expected {want})");
                }
            }
            if let Some(want) = &job.sha512 {
                let got = hex::encode(s5.finalize());
                if !got.eq_ignore_ascii_case(want) {
                    bail!("checksum mismatch (sha512)");
                }
            }
            Ok(())
        }
        .await;
        match res {
            Ok(()) => {
                std::fs::rename(&tmp, &job.dest)?;
                return Ok(());
            }
            Err(e) => last = Some(e),
        }
    }
    let _ = std::fs::remove_file(&tmp);
    Err(last.unwrap()).with_context(|| format!("downloading {}", job.url))
}

/// Download many files, 16 at a time. `on_done(done, total)` after each one.
pub async fn fetch_all(jobs: Vec<Job>, on_done: impl Fn(u64, u64) + Send + Sync) -> Result<()> {
    let total = jobs.len() as u64;
    if total == 0 {
        return Ok(());
    }
    let client = super::http();
    let done = Arc::new(AtomicU64::new(0));
    let mut errors = Vec::new();
    let mut s = stream::iter(jobs.into_iter().map(|j| {
        let c = client.clone();
        async move { fetch(&c, &j).await }
    }))
    .buffer_unordered(16);
    while let Some(r) = s.next().await {
        let n = done.fetch_add(1, Ordering::Relaxed) + 1;
        on_done(n, total);
        if let Err(e) = r {
            errors.push(format!("{e:#}"));
        }
    }
    if !errors.is_empty() {
        bail!("{} download(s) failed. First: {}", errors.len(), errors[0]);
    }
    Ok(())
}
