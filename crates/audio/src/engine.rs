//! The engine: a decoder thread feeding the output thread, driven by
//! commands, reporting back through events.

use crate::output::{self, OutCmd, OutputInfo, Shared};
use crate::pipeline::{Mark, Pipeline};
use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How far ahead the decoder keeps the buffer filled.
const AHEAD_SECS: f64 = 0.6;
const CHUNK_FRAMES: usize = 4096;
const POSITION_EVERY: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// `token` is now audible, `pos` seconds in.
    Track { token: u64, pos: f64, duration: Option<f64> },
    Position { token: u64, pos: f64 },
    Paused(bool),
    /// Everything queued has finished playing.
    Ended,
    /// `token` couldn't be played and was skipped.
    Failed { token: u64, message: String },
    /// The output device opened (or reopened), or couldn't.
    Output(Result<OutputInfo, String>),
}

enum Cmd {
    Load { token: u64, path: PathBuf, start: f64, paused: bool },
    Next(Option<(u64, PathBuf)>),
    Seek(f64),
    Pause(bool),
    Stop,
    Output(OutputInfo),
    Loop(Option<(u64, f64, f64)>),
}

pub struct Engine {
    tx: Sender<Cmd>,
    out: Sender<OutCmd>,
    shared: Arc<Shared>,
    info: Arc<Mutex<Option<OutputInfo>>>,
    analyser: Mutex<Option<crate::spectrum::Analyser>>,
}

impl Engine {
    pub fn start(on_event: impl Fn(Event) + Send + Sync + 'static) -> Engine {
        Self::start_with(on_event, output::spawn)
    }

    /// An engine with a fake device that plays `speed` times faster than
    /// real time and makes no sound. For tests.
    #[doc(hidden)]
    pub fn start_headless(on_event: impl Fn(Event) + Send + Sync + 'static, rate: u32, speed: f64) -> Engine {
        Self::start_with(on_event, move |shared, notify| {
            let (tx, rx) = crossbeam_channel::unbounded::<OutCmd>();
            std::thread::spawn(move || {
                notify(Ok(OutputInfo { device: "Headless".into(), rate, channels: 2 }));
                let step = Duration::from_millis(5);
                let per = ((rate as f64 * step.as_secs_f64() * speed) as usize).max(1);
                loop {
                    match rx.recv_timeout(step) {
                        Ok(OutCmd::Shutdown) | Err(RecvTimeoutError::Disconnected) => return,
                        _ => {}
                    }
                    if shared.paused.load(Ordering::Relaxed) {
                        continue;
                    }
                    let mut r = shared.ring.lock();
                    let n = per.min(r.data.len() / 2);
                    r.data.drain(..n * 2);
                    r.played += n as u64;
                }
            });
            tx
        })
    }

    fn start_with(
        on_event: impl Fn(Event) + Send + Sync + 'static,
        spawn_out: impl FnOnce(Arc<Shared>, Box<dyn Fn(Result<OutputInfo, String>) + Send>) -> Sender<OutCmd>,
    ) -> Engine {
        let on_event = Arc::new(on_event);
        let shared = Shared::new();
        let (tx, rx) = crossbeam_channel::unbounded::<Cmd>();
        let info: Arc<Mutex<Option<OutputInfo>>> = Arc::new(Mutex::new(None));
        let tx2 = tx.clone();
        let ev = on_event.clone();
        let info2 = info.clone();
        let out = spawn_out(
            shared.clone(),
            Box::new(move |r| {
                if let Ok(i) = &r {
                    *info2.lock() = Some(i.clone());
                    let _ = tx2.send(Cmd::Output(i.clone()));
                }
                ev(Event::Output(r));
            }),
        );
        let sh = shared.clone();
        std::thread::Builder::new()
            .name("arcadia-audio-decode".into())
            .spawn(move || Decoder::new(sh, rx, Box::new(move |e| on_event(e))).run())
            .expect("spawn audio decoder thread");
        Engine { tx, out, shared, info, analyser: Mutex::new(None) }
    }

    /// Play `path` from `start` seconds. `token` identifies it in events.
    pub fn load(&self, token: u64, path: PathBuf, start: f64, paused: bool) {
        let _ = self.tx.send(Cmd::Load { token, path, start, paused });
    }
    /// Queue what plays straight after the current track, without a gap.
    pub fn set_next(&self, next: Option<(u64, PathBuf)>) {
        let _ = self.tx.send(Cmd::Next(next));
    }
    pub fn seek(&self, secs: f64) {
        let _ = self.tx.send(Cmd::Seek(secs));
    }
    pub fn pause(&self, paused: bool) {
        let _ = self.tx.send(Cmd::Pause(paused));
    }
    pub fn is_paused(&self) -> bool {
        self.shared.paused.load(Ordering::Relaxed)
    }
    pub fn stop(&self) {
        let _ = self.tx.send(Cmd::Stop);
    }
    /// Linear gain, 0–1.
    pub fn set_volume(&self, v: f32) {
        self.shared.volume.store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
    /// Use this output device, or follow the system default with `None`.
    pub fn set_device(&self, name: Option<String>) {
        let _ = self.out.send(OutCmd::Device(name));
    }
    pub fn output(&self) -> Option<OutputInfo> {
        self.info.lock().clone()
    }

    /// Loop `token` between `a` and `b` seconds, sample-accurately, or stop
    /// looping with `None`.
    pub fn set_loop(&self, ab: Option<(u64, f64, f64)>) {
        let _ = self.tx.send(Cmd::Loop(ab));
    }

    /// Levels for the visualiser (see [`crate::spectrum`]). The first call
    /// starts collecting samples; [`Engine::stop_spectrum`] stops.
    pub fn spectrum(&self, bands: usize) -> Vec<f32> {
        self.shared.tap.store(true, Ordering::Relaxed);
        let Some(rate) = self.output().map(|o| o.rate) else { return vec![0.0; bands] };
        let samples = if self.is_paused() { Vec::new() } else { self.shared.ring.lock().tapped() };
        let mut a = self.analyser.lock();
        a.get_or_insert_with(|| crate::spectrum::Analyser::new(crate::output::TAP_LEN)).bands(&samples, rate, bands)
    }

    pub fn stop_spectrum(&self) {
        self.shared.tap.store(false, Ordering::Relaxed);
        let mut r = self.shared.ring.lock();
        r.tap.clear();
        r.tap_at = 0;
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.out.send(OutCmd::Shutdown);
    }
}

struct Seg {
    seq: u64,
    token: u64,
    /// Frame (in the shared stream) where it starts.
    start: u64,
    pos: f64,
    duration: Option<f64>,
    /// Starts because of a seek, not a new track.
    seek: bool,
}

struct Decoder {
    shared: Arc<Shared>,
    rx: Receiver<Cmd>,
    emit: Box<dyn Fn(Event) + Send>,
    pipe: Option<Pipeline>,
    out: Option<OutputInfo>,
    /// What's loaded, to reload after the device changes.
    cur: Option<(u64, PathBuf)>,
    next: Option<(u64, PathBuf)>,
    written: u64,
    segs: Vec<Seg>,
    seq: u64,
    end_at: Option<u64>,
    ended_sent: bool,
    shown: Option<u64>,
    last_pos: Instant,
    buf: Vec<f32>,
    marks: Vec<Mark>,
    /// A–B loop on a track: token, A and B in seconds.
    ab: Option<(u64, f64, f64)>,
}

impl Decoder {
    fn new(shared: Arc<Shared>, rx: Receiver<Cmd>, emit: Box<dyn Fn(Event) + Send>) -> Decoder {
        Decoder {
            shared,
            rx,
            emit,
            pipe: None,
            out: None,
            cur: None,
            next: None,
            written: 0,
            segs: Vec::new(),
            seq: 0,
            end_at: None,
            ended_sent: true,
            shown: None,
            last_pos: Instant::now(),
            buf: Vec::new(),
            marks: Vec::new(),
            ab: None,
        }
    }

    fn run(mut self) {
        loop {
            let wait = if self.wants_fill() { Duration::ZERO } else { Duration::from_millis(20) };
            match self.rx.recv_timeout(wait) {
                Ok(cmd) => self.handle(cmd),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            while let Ok(cmd) = self.rx.try_recv() {
                self.handle(cmd);
            }
            if self.wants_fill() {
                self.fill();
            }
            self.monitor();
        }
    }

    fn flush(&mut self) {
        let mut r = self.shared.ring.lock();
        r.data.clear();
        r.played = 0;
        drop(r);
        self.written = 0;
        self.segs.clear();
        self.end_at = None;
        self.ended_sent = false;
        self.shown = None;
    }

    fn handle(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Output(info) => {
                let changed = self.out.as_ref().map(|o| (o.rate, o.channels)) != Some((info.rate, info.channels));
                self.out = Some(info.clone());
                if changed {
                    // New rate or layout: rebuild and pick up where we were.
                    let pos = self.position().map(|(_, p)| p).unwrap_or(0.0);
                    self.pipe = Some(Pipeline::new(info.rate, info.channels));
                    if let Some((t, p)) = self.cur.clone() {
                        let paused = self.shared.paused.load(Ordering::Relaxed);
                        self.handle(Cmd::Load { token: t, path: p, start: pos, paused });
                        let n = self.next.clone();
                        self.handle(Cmd::Next(n));
                    }
                }
            }
            Cmd::Load { token, path, start, paused } => {
                self.flush();
                if self.ab.is_some_and(|(t, ..)| t != token) {
                    self.ab = None;
                }
                self.cur = Some((token, path.clone()));
                self.next = None;
                self.shared.paused.store(paused, Ordering::Relaxed);
                (self.emit)(Event::Paused(paused));
                // No device yet: it loads as soon as one opens.
                let Some(pipe) = self.pipe.as_mut() else { return };
                match pipe.load(token, &path, start) {
                    Ok(m) => self.place(m, false),
                    Err(e) => {
                        self.end_at = Some(0);
                        (self.emit)(Event::Failed { token, message: e.to_string() });
                    }
                }
            }
            Cmd::Next(n) => {
                self.next = n.clone();
                let Some(p) = self.pipe.as_mut() else { return };
                p.set_next(n);
                // The current track already finished decoding with nothing
                // queued, but is still playing out: carry straight on.
                if !p.is_active() && self.end_at.is_some() && !self.ended_sent && self.next.is_some() {
                    self.end_at = None;
                    self.resume_after_end();
                }
            }
            Cmd::Seek(secs) => {
                let Some(token) = self.pipe.as_ref().map(|p| p.current_token()) else { return };
                self.flush();
                let r = self.pipe.as_mut().map(|p| p.seek(secs)).unwrap_or(Ok(None));
                match r {
                    Ok(Some(m)) => self.place(m, true),
                    Ok(None) => {}
                    Err(e) => (self.emit)(Event::Failed { token: token.unwrap_or(0), message: e.to_string() }),
                }
            }
            Cmd::Pause(p) => {
                self.shared.paused.store(p, Ordering::Relaxed);
                (self.emit)(Event::Paused(p));
            }
            Cmd::Loop(ab) => {
                self.ab = ab.filter(|&(_, a, b)| b > a + 0.05);
                // Already past B (or the buffer is): start the loop now.
                if let (Some((t, a, b)), Some((now_t, pos))) = (self.ab, self.position()) {
                    if t == now_t && pos > b {
                        self.handle(Cmd::Seek(a));
                    }
                }
            }
            Cmd::Stop => {
                self.ab = None;
                if let Some(p) = self.pipe.as_mut() {
                    p.stop();
                }
                self.flush();
                self.ended_sent = true;
                self.cur = None;
                self.next = None;
                self.shared.paused.store(true, Ordering::Relaxed);
                (self.emit)(Event::Paused(true));
            }
        }
    }

    /// The queue ran dry and a next track arrived late: load it straight
    /// after what's still in the buffer.
    fn resume_after_end(&mut self) {
        let Some((t, p)) = self.next.take() else { return };
        let Some(pipe) = self.pipe.as_mut() else { return };
        match pipe.load(t, &p, 0.0) {
            Ok(m) => {
                self.cur = Some((t, p));
                self.place(m, false);
            }
            Err(e) => (self.emit)(Event::Failed { token: t, message: e.to_string() }),
        }
    }

    fn place(&mut self, m: Mark, seek: bool) {
        match m {
            Mark::Start { token, at, pos, duration } => {
                self.seq += 1;
                self.segs.push(Seg { seq: self.seq, token, start: self.written + at as u64, pos, duration, seek });
                self.end_at = None;
                self.ended_sent = false;
            }
            Mark::End { at } => self.end_at = Some(self.written + at as u64),
            Mark::Failed { token, message } => (self.emit)(Event::Failed { token, message }),
        }
    }

    fn wants_fill(&self) -> bool {
        let (Some(pipe), Some(out)) = (self.pipe.as_ref(), self.out.as_ref()) else { return false };
        if !pipe.is_active() {
            return false;
        }
        let buffered = self.shared.ring.lock().data.len() / out.channels;
        (buffered as f64) < out.rate as f64 * AHEAD_SECS
    }

    fn fill(&mut self) {
        let Some(pipe) = self.pipe.as_mut() else { return };
        let ch = pipe.out_ch;
        self.buf.clear();
        self.marks.clear();
        pipe.fill(CHUNK_FRAMES, &mut self.buf, &mut self.marks);
        let mut frames = (self.buf.len() / ch) as u64;
        // Marks refer to frames in this chunk.
        let mut marks = std::mem::take(&mut self.marks);
        let jump = self.loop_cut(&mut frames, &mut marks, ch);
        for m in marks {
            // A gapless switch also means the queued track is now current.
            if let Mark::Start { token, .. } = &m {
                if let Some((t, p)) = self.next.take() {
                    if t == *token {
                        self.cur = Some((t, p));
                    }
                }
            }
            self.place(m, false);
        }
        self.shared.ring.lock().data.extend(self.buf.iter());
        self.written += frames;
        // Back to A, straight after B: the next chunk carries on from there.
        if let Some(a) = jump {
            let r = self.pipe.as_mut().map(|p| p.seek(a)).unwrap_or(Ok(None));
            match r {
                Ok(Some(m)) => self.place(m, true),
                Ok(None) => {}
                Err(e) => {
                    self.ab = None;
                    log::warn!("loop: {e}");
                }
            }
        }
    }

    /// With an A–B loop on the track being decoded, cut this chunk at B and
    /// say where to go back to.
    fn loop_cut(&mut self, frames: &mut u64, marks: &mut Vec<Mark>, ch: usize) -> Option<f64> {
        let (tok, a, b) = self.ab?;
        let rate = self.out.as_ref()?.rate as f64;
        let seg = self.segs.last()?;
        // A new track starts in this chunk: the loop's track is done.
        if seg.token != tok || marks.iter().any(|m| matches!(m, Mark::Start { .. })) {
            return None;
        }
        let head = seg.pos + self.written.saturating_sub(seg.start) as f64 / rate;
        if head + *frames as f64 / rate < b {
            return None;
        }
        let cut = (((b - head) * rate).max(0.0) as u64).min(*frames);
        // The track ends before B: let it.
        if marks.iter().any(|m| matches!(m, Mark::End { at } if (*at as u64) < cut)) {
            return None;
        }
        marks.retain(|m| !matches!(m, Mark::End { .. }));
        self.buf.truncate(cut as usize * ch);
        *frames = cut;
        Some(a)
    }

    fn position(&self) -> Option<(u64, f64)> {
        let out = self.out.as_ref()?;
        let played = self.shared.ring.lock().played;
        let seg = self.segs.iter().rev().find(|s| s.start <= played)?;
        Some((seg.token, seg.pos + (played - seg.start) as f64 / out.rate as f64))
    }

    fn monitor(&mut self) {
        let Some(out) = self.out.as_ref() else { return };
        let rate = out.rate as f64;
        let played = self.shared.ring.lock().played;
        if let Some(i) = self.segs.iter().rposition(|s| s.start <= played) {
            let seg = &self.segs[i];
            let pos = seg.pos + (played - seg.start) as f64 / rate;
            if self.shown != Some(seg.seq) {
                self.shown = Some(seg.seq);
                let ev = if seg.seek { Event::Position { token: seg.token, pos } } else { Event::Track { token: seg.token, pos, duration: seg.duration } };
                (self.emit)(ev);
                self.last_pos = Instant::now();
            } else if self.last_pos.elapsed() >= POSITION_EVERY && !self.shared.paused.load(Ordering::Relaxed) {
                self.last_pos = Instant::now();
                (self.emit)(Event::Position { token: seg.token, pos });
            }
            // Old segments are done with.
            if i > 0 {
                self.segs.drain(..i);
            }
        }
        if let Some(e) = self.end_at {
            if played >= e && !self.ended_sent {
                self.ended_sent = true;
                (self.emit)(Event::Ended);
            }
        }
    }
}
