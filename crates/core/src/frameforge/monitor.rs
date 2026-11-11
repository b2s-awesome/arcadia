//! Live monitoring of one game process.
//!
//! A monitor owns a [`FrameSource`] and a background thread that drains it a
//! few times a second. The thread exists only while monitoring is on, sleeps
//! between polls, and keeps a bounded window of recent frames, so the cost
//! to the game is PresentMon's own (ETW tracing) plus a few wake-ups.
//!
//! Recording (for benchmarks) is a separate, explicit state: frames are only
//! accumulated beyond the rolling window while a recording is running, and a
//! recording stops by itself at [`MAX_RECORD_FRAMES`].

use super::presentmon::{Frame, FrameSource, SourceKind};
use super::stats::{self, FrameStats};
use super::telemetry::{Sample, Sampler};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Rolling window used for the live statistics, milliseconds of frame time.
pub const WINDOW_MS: f64 = 30_000.0;
/// Points sent to the UI for the frame-time graph.
pub const GRAPH_POINTS: usize = 240;
/// A recording stops by itself after this many frames (about 90 minutes at 240 FPS).
pub const MAX_RECORD_FRAMES: usize = 1_300_000;
/// Telemetry samples kept for the live charts.
pub const TELEMETRY_HISTORY: usize = 120;
const POLL_INTERVAL: Duration = Duration::from_millis(200);
const TELEMETRY_INTERVAL: Duration = Duration::from_millis(1000);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum State {
    #[default]
    Idle,
    Running,
    /// The game exited or the source ended; the last numbers are kept.
    Ended,
    Error,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub state: State,
    pub pid: Option<u32>,
    pub process: String,
    pub source: Option<SourceKind>,
    pub source_detail: String,
    pub error: Option<String>,
    pub elapsed_ms: f64,
    pub total_frames: u64,
    /// FPS over the last second of displayed frames. `None` until a second of data exists.
    pub displayed_fps_now: Option<f64>,
    /// FPS over the last second of presented frames (the game's own cadence).
    pub presented_fps_now: Option<f64>,
    /// Statistics of on-screen frame durations over the rolling window.
    pub displayed: FrameStats,
    /// Statistics of CPU frame times over the rolling window.
    pub presented: FrameStats,
    pub latest_frame_time_ms: Option<f64>,
    pub cpu_busy_ms: Option<f64>,
    pub cpu_wait_ms: Option<f64>,
    pub gpu_busy_ms: Option<f64>,
    pub gpu_wait_ms: Option<f64>,
    pub display_latency_ms: Option<f64>,
    /// Share of frames in the window that never reached the screen.
    pub dropped_pct: Option<f64>,
    /// Share of frames produced by frame generation.
    pub generated_pct: Option<f64>,
    /// Recent frame times for the graph, oldest first, down-sampled keeping spikes.
    pub frame_times_ms: Vec<f64>,
    pub telemetry: Option<Sample>,
    pub telemetry_history: Vec<Sample>,
    pub recording: bool,
    pub recorded_frames: usize,
    pub notes: Vec<String>,
}

/// Everything captured while recording, handed to the benchmark engine.
#[derive(Debug, Clone, Default)]
pub struct Recording {
    pub frames: Vec<Frame>,
    pub telemetry: Vec<Sample>,
    pub started_unix: u64,
    pub duration_ms: f64,
    /// True when the recording hit the frame cap and stopped early.
    pub truncated: bool,
}

struct Inner {
    state: State,
    pid: u32,
    process: String,
    source_kind: Option<SourceKind>,
    source_detail: String,
    error: Option<String>,
    started: Instant,
    total_frames: u64,
    window: VecDeque<Frame>,
    window_ms: f64,
    telemetry: VecDeque<Sample>,
    recording: Option<Recording>,
    record_started: Option<Instant>,
    gpu_id: Option<String>,
}

impl Inner {
    fn push_frames(&mut self, frames: Vec<Frame>) {
        self.total_frames += frames.len() as u64;
        if let Some(rec) = self.recording.as_mut() {
            let room = MAX_RECORD_FRAMES.saturating_sub(rec.frames.len());
            if room < frames.len() {
                rec.truncated = true;
            }
            rec.frames.extend(frames.iter().take(room).cloned());
        }
        for f in frames {
            self.window_ms += f.frame_time_ms;
            self.window.push_back(f);
        }
        while self.window_ms > WINDOW_MS && self.window.len() > 1 {
            if let Some(old) = self.window.pop_front() {
                self.window_ms -= old.frame_time_ms;
            }
        }
    }

    fn push_telemetry(&mut self, s: Sample) {
        if let Some(rec) = self.recording.as_mut() {
            rec.telemetry.push(s.clone());
        }
        self.telemetry.push_back(s);
        while self.telemetry.len() > TELEMETRY_HISTORY {
            self.telemetry.pop_front();
        }
    }
}

/// FPS over the most recent `span_ms` of the given durations (newest last).
fn recent_fps(durations: impl DoubleEndedIterator<Item = f64>, span_ms: f64) -> Option<f64> {
    let (mut sum, mut n) = (0.0, 0usize);
    for d in durations.rev() {
        if sum >= span_ms {
            break;
        }
        sum += d;
        n += 1;
    }
    // Not enough history to cover the span yet: report nothing instead of a guess.
    (sum >= span_ms * 0.9 && n >= 2).then(|| 1000.0 * n as f64 / sum)
}

pub struct Monitor {
    inner: Arc<Mutex<Inner>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Monitor {
    /// Start monitoring. `sampler` is optional so tests can run without hardware.
    pub fn start(mut source: Box<dyn FrameSource>, pid: u32, process: String, sampler: Option<Arc<Sampler>>, gpu_id: Option<String>) -> Self {
        let inner = Arc::new(Mutex::new(Inner {
            state: State::Running,
            pid,
            process,
            source_kind: Some(source.kind()),
            source_detail: source.detail(),
            error: None,
            started: Instant::now(),
            total_frames: 0,
            window: VecDeque::new(),
            window_ms: 0.0,
            telemetry: VecDeque::new(),
            recording: None,
            record_started: None,
            gpu_id,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let (i, s) = (inner.clone(), stop.clone());
        let thread = std::thread::Builder::new()
            .name("frameforge-monitor".into())
            .spawn(move || {
                let mut last_telemetry = Instant::now() - TELEMETRY_INTERVAL;
                while !s.load(Ordering::Relaxed) {
                    match source.poll() {
                        Ok(frames) => {
                            if !frames.is_empty() {
                                i.lock().push_frames(frames);
                            }
                        }
                        Err(e) => {
                            let mut g = i.lock();
                            g.state = State::Error;
                            g.error = Some(e.to_string());
                            break;
                        }
                    }
                    if last_telemetry.elapsed() >= TELEMETRY_INTERVAL {
                        last_telemetry = Instant::now();
                        if let Some(sampler) = &sampler {
                            let mut sample = sampler.sample();
                            if let Some(pm) = source.telemetry() {
                                let gpu = i.lock().gpu_id.clone();
                                sample.merge_presentmon(&pm, gpu.as_deref());
                            }
                            i.lock().push_telemetry(sample);
                        }
                    }
                    if !source.alive() {
                        // One last drain, then stop: the game has exited.
                        if let Ok(frames) = source.poll() {
                            if !frames.is_empty() {
                                i.lock().push_frames(frames);
                            }
                        }
                        let mut g = i.lock();
                        if g.state == State::Running {
                            g.state = State::Ended;
                        }
                        break;
                    }
                    std::thread::sleep(POLL_INTERVAL);
                }
                // `source` drops here, which stops tracking / ends the capture tool.
            })
            .ok();
        Self { inner, stop, thread }
    }

    pub fn pid(&self) -> u32 {
        self.inner.lock().pid
    }

    pub fn state(&self) -> State {
        self.inner.lock().state
    }

    pub fn snapshot(&self) -> Snapshot {
        let g = self.inner.lock();
        let presented_times: Vec<f64> = g.window.iter().map(|f| f.frame_time_ms).collect();
        let displayed_times: Vec<f64> = g.window.iter().filter_map(|f| f.displayed_time_ms).collect();
        let n = g.window.len();
        let mut notes = Vec::new();
        let has_display_data = !displayed_times.is_empty();
        if n > 0 && !has_display_data {
            notes.push("This source reports no on-screen time per frame, so displayed FPS is unavailable; presented FPS is shown.".into());
        }
        let avg = |f: fn(&Frame) -> Option<f64>| stats::mean_opt(g.window.iter().map(f));
        let (elapsed, recording, recorded) = match (&g.recording, g.record_started) {
            (Some(r), _) => (g.started.elapsed().as_secs_f64() * 1000.0, true, r.frames.len()),
            _ => (g.started.elapsed().as_secs_f64() * 1000.0, false, 0),
        };
        if let Some(r) = &g.recording {
            if r.truncated {
                notes.push("The recording reached its size limit and stopped adding frames.".into());
            }
        }
        Snapshot {
            state: g.state,
            pid: Some(g.pid),
            process: g.process.clone(),
            source: g.source_kind,
            source_detail: g.source_detail.clone(),
            error: g.error.clone(),
            elapsed_ms: elapsed,
            total_frames: g.total_frames,
            displayed_fps_now: recent_fps(g.window.iter().filter_map(|f| f.displayed_time_ms), 1000.0),
            presented_fps_now: recent_fps(g.window.iter().map(|f| f.frame_time_ms), 1000.0),
            displayed: stats::compute(&displayed_times),
            presented: stats::compute(&presented_times),
            latest_frame_time_ms: g.window.back().map(|f| f.frame_time_ms),
            cpu_busy_ms: avg(|f| f.cpu_busy_ms),
            cpu_wait_ms: avg(|f| f.cpu_wait_ms),
            gpu_busy_ms: avg(|f| f.gpu_busy_ms),
            gpu_wait_ms: avg(|f| f.gpu_wait_ms),
            display_latency_ms: avg(|f| f.display_latency_ms),
            dropped_pct: (n > 0 && has_display_data).then(|| g.window.iter().filter(|f| f.dropped).count() as f64 / n as f64 * 100.0),
            generated_pct: (n > 0).then(|| g.window.iter().filter(|f| f.generated).count() as f64 / n as f64 * 100.0),
            frame_times_ms: {
                let tail = presented_times.len().saturating_sub(GRAPH_POINTS * 8);
                stats::downsample_max(&presented_times[tail..], GRAPH_POINTS)
            },
            telemetry: g.telemetry.back().cloned(),
            telemetry_history: g.telemetry.iter().cloned().collect(),
            recording,
            recorded_frames: recorded,
            notes,
        }
    }

    /// Frames currently in the rolling window (for diagnostics).
    pub fn window_frames(&self) -> Vec<Frame> {
        self.inner.lock().window.iter().cloned().collect()
    }

    pub fn telemetry_history(&self) -> Vec<Sample> {
        self.inner.lock().telemetry.iter().cloned().collect()
    }

    /// Begin accumulating frames for a benchmark. No-op if already recording.
    pub fn start_recording(&self) {
        let mut g = self.inner.lock();
        if g.recording.is_none() {
            g.recording = Some(Recording { started_unix: super::hardware::now_secs(), ..Default::default() });
            g.record_started = Some(Instant::now());
        }
    }

    pub fn is_recording(&self) -> bool {
        self.inner.lock().recording.is_some()
    }

    /// Stop recording and take what was captured.
    pub fn stop_recording(&self) -> Option<Recording> {
        let mut g = self.inner.lock();
        let started = g.record_started.take();
        let mut rec = g.recording.take()?;
        rec.duration_ms = started.map(|s| s.elapsed().as_secs_f64() * 1000.0).unwrap_or(0.0);
        Some(rec)
    }

    /// Stop the thread and release the source. Idempotent.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        let mut g = self.inner.lock();
        if g.state == State::Running {
            g.state = State::Ended;
        }
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameforge::presentmon::ReplaySource;

    fn frames(n: usize, ft: f64) -> Vec<Frame> {
        (0..n).map(|i| Frame { time_ms: i as f64 * ft, frame_time_ms: ft, displayed_time_ms: Some(ft), cpu_busy_ms: Some(ft * 0.6), gpu_busy_ms: Some(ft * 0.9), ..Default::default() }).collect()
    }

    fn wait_until(m: &Monitor, f: impl Fn(&Snapshot) -> bool) -> Snapshot {
        for _ in 0..400 {
            let s = m.snapshot();
            if f(&s) {
                return s;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        m.snapshot()
    }

    #[test]
    fn capture_start_and_stop_with_process_exit() {
        // 600 frames at 10 ms = 6 s of game time, delivered in two batches; then the source ends.
        let src = ReplaySource::new(vec![frames(300, 10.0), frames(300, 10.0)]);
        let mut m = Monitor::start(Box::new(src), 77, "game.exe".into(), None, None);
        let s = wait_until(&m, |s| s.state == State::Ended);
        assert_eq!(s.state, State::Ended);
        assert_eq!(s.total_frames, 600);
        assert_eq!(s.pid, Some(77));
        assert!((s.displayed_fps_now.unwrap() - 100.0).abs() < 1e-6);
        assert!((s.presented.avg_fps.unwrap() - 100.0).abs() < 1e-6);
        assert!((s.displayed.low_1pct_fps.unwrap() - 100.0).abs() < 1e-6);
        assert!(s.displayed.low_01pct_fps.is_none()); // only 600 frames
        assert_eq!(s.cpu_busy_ms, Some(6.0));
        assert_eq!(s.dropped_pct, Some(0.0));
        assert!(s.frame_times_ms.len() <= GRAPH_POINTS);
        m.stop();
        m.stop(); // idempotent
    }

    #[test]
    fn window_is_bounded() {
        // 10 000 frames at 10 ms = 100 s; only about 30 s may stay in the window.
        let src = ReplaySource::new(vec![frames(10_000, 10.0)]);
        let m = Monitor::start(Box::new(src), 1, "g".into(), None, None);
        let s = wait_until(&m, |s| s.state == State::Ended);
        assert_eq!(s.total_frames, 10_000);
        assert!(s.presented.frames <= 3001, "window held {} frames", s.presented.frames);
    }

    #[test]
    fn insufficient_data_reports_nothing() {
        let src = ReplaySource::new(vec![frames(5, 10.0)]);
        let m = Monitor::start(Box::new(src), 1, "g".into(), None, None);
        let s = wait_until(&m, |s| s.state == State::Ended);
        assert!(s.displayed_fps_now.is_none());
        assert!(s.displayed.avg_fps.is_none());
        assert!(!s.displayed.notes.is_empty());
    }

    #[test]
    fn source_without_display_times_says_so() {
        let mut f = frames(200, 10.0);
        f.iter_mut().for_each(|x| x.displayed_time_ms = None);
        let m = Monitor::start(Box::new(ReplaySource::new(vec![f])), 1, "g".into(), None, None);
        let s = wait_until(&m, |s| s.state == State::Ended);
        assert!(s.displayed_fps_now.is_none() && s.dropped_pct.is_none());
        assert!(s.presented_fps_now.is_some());
        assert!(s.notes.iter().any(|n| n.contains("displayed FPS is unavailable")));
    }

    struct Failing;
    impl FrameSource for Failing {
        fn kind(&self) -> SourceKind {
            SourceKind::Service
        }
        fn poll(&mut self) -> anyhow::Result<Vec<Frame>> {
            anyhow::bail!("The PresentMon Service is installed but not running")
        }
    }

    #[test]
    fn service_failure_surfaces_as_error_state() {
        let m = Monitor::start(Box::new(Failing), 1, "g".into(), None, None);
        let s = wait_until(&m, |s| s.state == State::Error);
        assert_eq!(s.state, State::Error);
        assert!(s.error.unwrap().contains("not running"));
        assert!(s.displayed_fps_now.is_none());
    }

    #[test]
    fn recording_collects_only_while_on() {
        struct Slow(usize);
        impl FrameSource for Slow {
            fn kind(&self) -> SourceKind {
                SourceKind::Replay
            }
            fn poll(&mut self) -> anyhow::Result<Vec<Frame>> {
                self.0 += 1;
                Ok(frames(50, 10.0))
            }
        }
        let m = Monitor::start(Box::new(Slow(0)), 1, "g".into(), None, None);
        wait_until(&m, |s| s.total_frames >= 50);
        assert!(m.stop_recording().is_none());
        m.start_recording();
        wait_until(&m, |s| s.recorded_frames >= 100);
        let rec = m.stop_recording().unwrap();
        assert!(rec.frames.len() >= 100 && !rec.truncated);
        assert!(rec.frames.len() as u64 <= m.snapshot().total_frames);
        assert!(!m.is_recording());
    }
}
