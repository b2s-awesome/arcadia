//! Decoding side of the engine: turns a current track (and the one queued
//! after it) into one continuous stream at the output's rate and channel
//! count. Tracks are joined sample-to-sample, so albums mixed without gaps
//! play without gaps.
//!
//! No audio device here, so this is what the tests exercise.

use crate::source::{remap, Source};
use anyhow::Result;
use rubato::{FftFixedIn, Resampler as _};
use std::path::{Path, PathBuf};

/// Input chunk size for the resampler, in frames.
const RS_CHUNK: usize = 1024;

/// Something that happened at a point in the produced stream.
#[derive(Debug, Clone, PartialEq)]
pub enum Mark {
    /// `token` starts at frame `at` of this chunk, `pos` seconds into it.
    Start { token: u64, at: usize, pos: f64, duration: Option<f64> },
    /// Nothing more to play after frame `at`.
    End { at: usize },
    /// `token` couldn't be played; it was skipped.
    Failed { token: u64, message: String },
}

struct Active {
    token: u64,
    src: Source,
    rs: Option<Rs>,
    /// Source frames read since the last load/seek (for resampler trimming).
    done: bool,
}

/// A rubato resampler with the bookkeeping that keeps tracks sample-exact:
/// the filter's start-up delay is trimmed, and the tail is flushed so the
/// output has exactly `in × ratio` frames.
struct Rs {
    r: FftFixedIn<f32>,
    ch: usize,
    ratio: f64,
    planar: Vec<Vec<f32>>,
    in_total: u64,
    out_total: u64,
    to_trim: usize,
}

impl Rs {
    fn new(from: u32, to: u32, ch: usize) -> Result<Rs> {
        let r = FftFixedIn::<f32>::new(from as usize, to as usize, RS_CHUNK, 2, ch)?;
        let to_trim = r.output_delay();
        Ok(Rs { r, ch, ratio: to as f64 / from as f64, planar: vec![Vec::new(); ch], in_total: 0, out_total: 0, to_trim })
    }

    fn push(&mut self, inter: &[f32], out: &mut Vec<f32>) -> Result<()> {
        for (i, s) in inter.iter().enumerate() {
            self.planar[i % self.ch].push(*s);
        }
        self.in_total += (inter.len() / self.ch) as u64;
        while self.planar[0].len() >= self.r.input_frames_next() {
            let n = self.r.input_frames_next();
            let chunk: Vec<&[f32]> = self.planar.iter().map(|c| &c[..n]).collect();
            let res = self.r.process(&chunk, None)?;
            for c in &mut self.planar {
                c.drain(..n);
            }
            self.emit(&res, out);
        }
        Ok(())
    }

    /// End of this track: flush everything still inside the filter.
    fn finish(&mut self, out: &mut Vec<f32>) -> Result<()> {
        let want = (self.in_total as f64 * self.ratio).round() as u64;
        let rest: Vec<&[f32]> = self.planar.iter().map(|c| c.as_slice()).collect();
        if !rest[0].is_empty() {
            let res = self.r.process_partial(Some(&rest), None)?;
            self.emit_limited(&res, out, want);
        }
        for c in &mut self.planar {
            c.clear();
        }
        // Feed silence until the delayed tail has come out.
        let mut guard = 0;
        while self.out_total < want && guard < 16 {
            let res = self.r.process_partial::<&[f32]>(None, None)?;
            self.emit_limited(&res, out, want);
            guard += 1;
        }
        Ok(())
    }

    fn emit(&mut self, res: &[Vec<f32>], out: &mut Vec<f32>) {
        self.emit_limited(res, out, u64::MAX)
    }

    fn emit_limited(&mut self, res: &[Vec<f32>], out: &mut Vec<f32>, limit: u64) {
        let frames = res[0].len();
        let mut start = 0;
        if self.to_trim > 0 {
            start = self.to_trim.min(frames);
            self.to_trim -= start;
        }
        for f in start..frames {
            if self.out_total >= limit {
                break;
            }
            for c in res.iter().take(self.ch) {
                out.push(c[f]);
            }
            self.out_total += 1;
        }
    }
}

pub struct Pipeline {
    pub out_rate: u32,
    pub out_ch: usize,
    cur: Option<Active>,
    next: Option<(u64, PathBuf)>,
    scratch: Vec<f32>,
}

impl Pipeline {
    pub fn new(out_rate: u32, out_ch: usize) -> Pipeline {
        Pipeline { out_rate, out_ch: out_ch.max(1), cur: None, next: None, scratch: Vec::new() }
    }

    pub fn is_active(&self) -> bool {
        self.cur.as_ref().is_some_and(|c| !c.done)
    }

    pub fn current_token(&self) -> Option<u64> {
        self.cur.as_ref().map(|c| c.token)
    }

    fn activate(&self, token: u64, path: &Path) -> Result<Active> {
        let src = Source::open(path)?;
        let rs = if src.rate != self.out_rate { Some(Rs::new(src.rate, self.out_rate, self.out_ch)?) } else { None };
        Ok(Active { token, src, rs, done: false })
    }

    /// Replace whatever is playing. Returns the opening mark.
    pub fn load(&mut self, token: u64, path: &Path, start: f64) -> Result<Mark> {
        self.next = None;
        self.cur = None;
        let mut a = self.activate(token, path)?;
        let pos = if start > 0.0 { a.src.seek(start)? } else { 0.0 };
        let duration = a.src.duration;
        self.cur = Some(a);
        Ok(Mark::Start { token, at: 0, pos, duration })
    }

    /// What plays straight after the current track, with no gap.
    pub fn set_next(&mut self, next: Option<(u64, PathBuf)>) {
        self.next = next;
    }

    pub fn seek(&mut self, secs: f64) -> Result<Option<Mark>> {
        let Some(a) = self.cur.as_mut() else { return Ok(None) };
        let pos = a.src.seek(secs)?;
        if let Some(rs) = &a.rs {
            // Fresh filter state, so nothing from before the seek leaks out.
            a.rs = Some(Rs::new(a.src.rate, self.out_rate, rs.ch)?);
        }
        a.done = false;
        Ok(Some(Mark::Start { token: a.token, at: 0, pos, duration: a.src.duration }))
    }

    pub fn stop(&mut self) {
        self.cur = None;
        self.next = None;
    }

    /// Produce at least `want` frames (unless the queue runs out) into `out`,
    /// interleaved at the output layout, noting track changes in `marks`.
    pub fn fill(&mut self, want: usize, out: &mut Vec<f32>, marks: &mut Vec<Mark>) {
        let ch = self.out_ch;
        let start_len = out.len();
        while (out.len() - start_len) / ch < want {
            let Some(a) = self.cur.as_mut() else { break };
            if a.done {
                break;
            }
            match a.src.read() {
                Ok(Some(block)) => {
                    self.scratch.clear();
                    remap(&block, a.src.channels, ch, &mut self.scratch);
                    match &mut a.rs {
                        None => out.extend_from_slice(&self.scratch),
                        Some(rs) => {
                            if let Err(e) = rs.push(&self.scratch, out) {
                                marks.push(Mark::Failed { token: a.token, message: e.to_string() });
                                a.done = true;
                            }
                        }
                    }
                }
                Ok(None) => {
                    if let Some(rs) = &mut a.rs {
                        let _ = rs.finish(out);
                    }
                    self.advance(out, start_len, marks);
                }
                Err(e) => {
                    marks.push(Mark::Failed { token: a.token, message: e.to_string() });
                    self.advance(out, start_len, marks);
                }
            }
        }
    }

    /// The current track ended: switch to the queued one, gaplessly.
    fn advance(&mut self, out: &[f32], start_len: usize, marks: &mut Vec<Mark>) {
        // Relative to the start of this fill.
        let at = (out.len() - start_len) / self.out_ch;
        // A queued track that won't open is skipped, and the queue is asked
        // again (by the caller, after the Failed mark).
        if let Some((token, path)) = self.next.take() {
            match self.activate(token, &path) {
                Ok(a) => {
                    marks.push(Mark::Start { token, at, pos: 0.0, duration: a.src.duration });
                    self.cur = Some(a);
                    return;
                }
                Err(e) => marks.push(Mark::Failed { token, message: e.to_string() }),
            }
        }
        if let Some(a) = self.cur.as_mut() {
            a.done = true;
        }
        marks.push(Mark::End { at });
    }
}
