//! Benchmark sessions: summarise a PresentMon recording, keep a history,
//! compare runs and export reports.
//!
//! A session stores measurements and the conditions they were taken under.
//! Comparing two sessions reports the differences and flags everything
//! that makes the comparison unreliable (different resolution, short runs,
//! hotter hardware, more background load). It never states that a change
//! *caused* a difference: that needs repeated runs under matching
//! conditions, which is what the flags and the repeat-run option are for.

use super::monitor::Recording;
use super::stats::{self, FrameStats};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const BENCH_SCHEMA: u32 = 1;
/// Shorter runs are kept but flagged: averages settle slowly and 0.1% lows need many frames.
pub const MIN_RELIABLE_SECONDS: f64 = 30.0;
/// Points kept for the frame-time chart of a stored session.
const CHART_POINTS: usize = 600;
/// Differences below this are within ordinary run-to-run variation.
pub const NOISE_PCT: f64 = 3.0;

/// The conditions a capture ran under, gathered by the service.
#[derive(Debug, Clone, Default)]
pub struct CaptureContext {
    pub label: String,
    pub game_id: Option<String>,
    pub game_name: String,
    pub exe: String,
    pub pid: u32,
    pub source: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub refresh_hz: Option<f64>,
    pub cpu: String,
    pub gpu: Option<String>,
    pub gpu_id: Option<String>,
    pub gpu_driver: Option<String>,
    pub os: String,
    pub active_optimizations: Vec<String>,
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchSession {
    pub schema: u32,
    pub id: String,
    pub label: String,
    pub game_id: Option<String>,
    pub game_name: String,
    pub exe: String,
    pub pid: u32,
    /// Unix seconds.
    pub started_at: u64,
    pub duration_ms: f64,
    /// PresentMon source the frames came from.
    pub source: String,
    /// Primary display mode during the capture. The game's own render
    /// resolution is not visible to PresentMon.
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub refresh_hz: Option<f64>,
    pub cpu: String,
    pub gpu: Option<String>,
    pub gpu_driver: Option<String>,
    pub os: String,
    /// Frames as shown on screen (falls back to presented when the source has no display timing).
    pub displayed: FrameStats,
    /// Frames as presented by the game.
    pub presented: FrameStats,
    pub cpu_busy_ms: Option<f64>,
    pub cpu_wait_ms: Option<f64>,
    pub gpu_busy_ms: Option<f64>,
    pub gpu_wait_ms: Option<f64>,
    pub display_latency_ms: Option<f64>,
    pub dropped_pct: Option<f64>,
    pub generated_pct: Option<f64>,
    pub cpu_util_pct: Option<f64>,
    pub cpu_temp_avg_c: Option<f64>,
    pub cpu_temp_max_c: Option<f64>,
    pub gpu_util_pct: Option<f64>,
    pub gpu_temp_avg_c: Option<f64>,
    pub gpu_temp_max_c: Option<f64>,
    pub gpu_power_avg_w: Option<f64>,
    pub vram_used_max_bytes: Option<u64>,
    pub ram_used_avg_bytes: Option<u64>,
    /// Optimiser changes in effect when the capture started.
    pub active_optimizations: Vec<String>,
    /// Game profile in effect, if any.
    pub profile: Option<String>,
    pub truncated: bool,
    /// Why some values are missing or less reliable.
    pub notes: Vec<String>,
    /// Presented frame times, downsampled for the chart.
    pub frame_times_ms: Vec<f64>,
}

impl BenchSession {
    /// The FPS figures to quote: displayed when measured, otherwise presented.
    pub fn headline(&self) -> &FrameStats {
        if self.displayed.frames > 0 {
            &self.displayed
        } else {
            &self.presented
        }
    }
    pub fn seconds(&self) -> f64 {
        self.duration_ms / 1000.0
    }
}

fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let (mut sum, mut n) = (0.0, 0usize);
    for v in values.filter(|v| v.is_finite()) {
        sum += v;
        n += 1;
    }
    (n > 0).then(|| sum / n as f64)
}

fn max(values: impl Iterator<Item = f64>) -> Option<f64> {
    values.filter(|v| v.is_finite()).fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))))
}

/// Turn a recording into a session. Pure: no clock, no hardware.
pub fn summarise(rec: &Recording, ctx: &CaptureContext) -> anyhow::Result<BenchSession> {
    if rec.frames.is_empty() {
        anyhow::bail!("No frames were captured, so there is nothing to save. Check that the game was running and visible.");
    }
    let presented_times: Vec<f64> = rec.frames.iter().map(|f| f.frame_time_ms).collect();
    let displayed_times: Vec<f64> = rec.frames.iter().filter_map(|f| f.displayed_time_ms).collect();
    let n = rec.frames.len() as f64;
    let presented = stats::compute(&presented_times);
    let displayed = stats::compute(&displayed_times);
    // The frames themselves say how long the capture covered; the wall clock
    // includes time before the first frame arrived.
    let duration_ms = if presented.duration_ms > 0.0 { presented.duration_ms } else { rec.duration_ms };
    let mut notes = Vec::new();
    if displayed_times.is_empty() {
        notes.push("This source reported no on-screen time per frame: the figures are for presented frames.".to_string());
    }
    if duration_ms / 1000.0 < MIN_RELIABLE_SECONDS {
        notes.push(format!("Short capture ({:.0} s). Use at least {MIN_RELIABLE_SECONDS:.0} s for figures you want to compare.", duration_ms / 1000.0));
    }
    if rec.frames.len() < stats::frames_needed(1) {
        notes.push(format!("Too few frames for a 0.1% low ({} captured, {} needed).", rec.frames.len(), stats::frames_needed(1)));
    }
    if rec.truncated {
        notes.push("The recording reached its size limit and stopped adding frames before the capture ended.".to_string());
    }
    let avg = |f: fn(&super::presentmon::Frame) -> Option<f64>| stats::mean_opt(rec.frames.iter().map(f));
    let gpu = |s: &super::telemetry::Sample| match &ctx.gpu_id {
        Some(id) => s.gpus.iter().find(|g| &g.id == id).cloned(),
        None => s.gpus.first().cloned(),
    };
    let gpu_samples: Vec<_> = rec.telemetry.iter().filter_map(gpu).collect();
    if rec.telemetry.is_empty() {
        notes.push("No hardware readings were collected during this capture.".to_string());
    }
    Ok(BenchSession {
        schema: BENCH_SCHEMA,
        id: String::new(),
        label: ctx.label.trim().to_string(),
        game_id: ctx.game_id.clone(),
        game_name: ctx.game_name.clone(),
        exe: ctx.exe.clone(),
        pid: ctx.pid,
        started_at: rec.started_unix,
        duration_ms,
        source: ctx.source.clone(),
        width: ctx.width,
        height: ctx.height,
        refresh_hz: ctx.refresh_hz,
        cpu: ctx.cpu.clone(),
        gpu: ctx.gpu.clone(),
        gpu_driver: ctx.gpu_driver.clone(),
        os: ctx.os.clone(),
        cpu_busy_ms: avg(|f| f.cpu_busy_ms),
        cpu_wait_ms: avg(|f| f.cpu_wait_ms),
        gpu_busy_ms: avg(|f| f.gpu_busy_ms),
        gpu_wait_ms: avg(|f| f.gpu_wait_ms),
        display_latency_ms: avg(|f| f.display_latency_ms),
        dropped_pct: (!displayed_times.is_empty()).then(|| rec.frames.iter().filter(|f| f.dropped).count() as f64 / n * 100.0),
        generated_pct: Some(rec.frames.iter().filter(|f| f.generated).count() as f64 / n * 100.0),
        cpu_util_pct: mean(rec.telemetry.iter().filter_map(|s| s.cpu_pct)),
        cpu_temp_avg_c: mean(rec.telemetry.iter().filter_map(|s| s.cpu_temperature_c)),
        cpu_temp_max_c: max(rec.telemetry.iter().filter_map(|s| s.cpu_temperature_c)),
        gpu_util_pct: mean(gpu_samples.iter().filter_map(|g| g.telemetry.utilization_pct)),
        gpu_temp_avg_c: mean(gpu_samples.iter().filter_map(|g| g.telemetry.temperature_c)),
        gpu_temp_max_c: max(gpu_samples.iter().filter_map(|g| g.telemetry.temperature_c)),
        gpu_power_avg_w: mean(gpu_samples.iter().filter_map(|g| g.telemetry.power_w)),
        vram_used_max_bytes: gpu_samples.iter().filter_map(|g| g.telemetry.vram_used_bytes).max(),
        ram_used_avg_bytes: mean(rec.telemetry.iter().map(|s| s.ram_used_bytes as f64)).map(|v| v as u64),
        active_optimizations: ctx.active_optimizations.clone(),
        profile: ctx.profile.clone(),
        truncated: rec.truncated,
        notes,
        frame_times_ms: stats::downsample_max(&presented_times, CHART_POINTS),
        displayed,
        presented,
    })
}

/// Sessions on disk: `FrameForge\benchmarks\<id>.json`.
pub struct BenchStore {
    dir: PathBuf,
}

impl BenchStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn path(&self, id: &str) -> Option<PathBuf> {
        (!id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')).then(|| self.dir.join(format!("{id}.json")))
    }

    pub fn save(&self, mut s: BenchSession) -> anyhow::Result<BenchSession> {
        if s.id.is_empty() {
            s.id = format!("{}-{}", s.started_at, &uuid::Uuid::new_v4().simple().to_string()[..8]);
        }
        let path = self.path(&s.id).ok_or_else(|| anyhow::anyhow!("Invalid benchmark id"))?;
        super::write_atomic(&path, &serde_json::to_vec_pretty(&s)?)?;
        Ok(s)
    }

    /// Every readable session, newest first. Unreadable files are skipped.
    pub fn list(&self) -> Vec<BenchSession> {
        let mut out: Vec<BenchSession> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
            .filter_map(|e| std::fs::read(e.path()).ok())
            .filter_map(|b| serde_json::from_slice::<BenchSession>(&b).ok())
            .filter(|s| s.schema >= 1 && s.schema <= BENCH_SCHEMA && !s.id.is_empty())
            .collect();
        out.sort_by(|a, b| b.started_at.cmp(&a.started_at).then_with(|| b.id.cmp(&a.id)));
        out
    }

    pub fn get(&self, id: &str) -> anyhow::Result<BenchSession> {
        let path = self.path(id).ok_or_else(|| anyhow::anyhow!("That benchmark does not exist"))?;
        let bytes = std::fs::read(path).map_err(|_| anyhow::anyhow!("That benchmark does not exist"))?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn rename(&self, id: &str, label: &str) -> anyhow::Result<BenchSession> {
        let label = label.trim();
        if label.chars().count() > 120 || label.chars().any(|c| c.is_control()) {
            anyhow::bail!("The name can be at most 120 characters");
        }
        let mut s = self.get(id)?;
        s.label = label.to_string();
        self.save(s)
    }

    pub fn delete(&self, id: &str) -> anyhow::Result<()> {
        let path = self.path(id).ok_or_else(|| anyhow::anyhow!("That benchmark does not exist"))?;
        std::fs::remove_file(path).map_err(|_| anyhow::anyhow!("That benchmark does not exist"))
    }
}

/// One measured quantity across the compared sessions.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricRow {
    pub key: String,
    pub name: String,
    pub unit: String,
    /// `true`: a larger number is better (FPS). `false`: smaller is better
    /// (frame time). `None`: neither (temperature, utilisation).
    pub higher_is_better: Option<bool>,
    /// One value per session, in the order given.
    pub values: Vec<Option<f64>>,
    /// Difference from the first session, absolute. `None` for the first.
    pub delta: Vec<Option<f64>>,
    /// Difference from the first session, in percent of the first.
    pub delta_pct: Vec<Option<f64>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Flag {
    /// "warning": the comparison may be misleading. "info": worth knowing.
    pub severity: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    /// Ids in the order compared; the first is the baseline.
    pub ids: Vec<String>,
    pub labels: Vec<String>,
    pub metrics: Vec<MetricRow>,
    pub flags: Vec<Flag>,
    /// Plain-words summary of the measured differences. No causes are claimed.
    pub summary: Vec<String>,
    /// No warning flags: conditions matched as far as Arcadia can tell.
    pub comparable: bool,
}

type Getter = fn(&BenchSession) -> Option<f64>;

const METRICS: &[(&str, &str, &str, Option<bool>, Getter)] = &[
    ("avgFps", "Average FPS", "FPS", Some(true), |s| s.headline().avg_fps),
    ("low1", "1% low FPS", "FPS", Some(true), |s| s.headline().low_1pct_fps),
    ("low01", "0.1% low FPS", "FPS", Some(true), |s| s.headline().low_01pct_fps),
    ("avgFrameTime", "Average frame time", "ms", Some(false), |s| s.headline().avg_frame_time_ms),
    ("p50", "Frame time, median", "ms", Some(false), |s| s.headline().p50_ms),
    ("p95", "Frame time, 95th percentile", "ms", Some(false), |s| s.headline().p95_ms),
    ("p99", "Frame time, 99th percentile", "ms", Some(false), |s| s.headline().p99_ms),
    ("p999", "Frame time, 99.9th percentile", "ms", Some(false), |s| s.headline().p999_ms),
    ("consistency", "Frame-time consistency", "%", Some(true), |s| s.headline().consistency_pct),
    ("stddev", "Frame-time deviation", "ms", Some(false), |s| s.headline().stddev_ms),
    ("cpuBusy", "CPU busy per frame", "ms", Some(false), |s| s.cpu_busy_ms),
    ("gpuBusy", "GPU busy per frame", "ms", Some(false), |s| s.gpu_busy_ms),
    ("latency", "Display latency", "ms", Some(false), |s| s.display_latency_ms),
    ("dropped", "Dropped frames", "%", Some(false), |s| s.dropped_pct),
    ("gpuUtil", "GPU utilisation", "%", None, |s| s.gpu_util_pct),
    ("cpuUtil", "CPU utilisation", "%", None, |s| s.cpu_util_pct),
    ("gpuTemp", "GPU temperature, average", "°C", None, |s| s.gpu_temp_avg_c),
    ("gpuTempMax", "GPU temperature, highest", "°C", None, |s| s.gpu_temp_max_c),
    ("cpuTemp", "CPU temperature, average", "°C", None, |s| s.cpu_temp_avg_c),
    ("gpuPower", "GPU power, average", "W", None, |s| s.gpu_power_avg_w),
];

fn warn(flags: &mut Vec<Flag>, text: String) {
    let f = Flag { severity: "warning".into(), text };
    if !flags.contains(&f) {
        flags.push(f);
    }
}

fn name_of(s: &BenchSession) -> String {
    if s.label.is_empty() { format!("{} ({})", s.game_name, s.id) } else { s.label.clone() }
}

/// Compare two or more sessions against the first one.
pub fn compare(sessions: &[BenchSession]) -> anyhow::Result<Comparison> {
    if sessions.len() < 2 {
        anyhow::bail!("Choose at least two benchmarks to compare");
    }
    let base = &sessions[0];
    let metrics: Vec<MetricRow> = METRICS
        .iter()
        .map(|(key, name, unit, hib, get)| {
            let values: Vec<Option<f64>> = sessions.iter().map(get).collect();
            let delta: Vec<Option<f64>> = values.iter().enumerate().map(|(i, v)| if i == 0 { None } else { Some(v.as_ref()? - values[0]?) }).collect();
            let delta_pct = delta.iter().map(|d| values[0].filter(|b| b.abs() > f64::EPSILON).and_then(|b| d.map(|d| d / b * 100.0))).collect();
            MetricRow { key: key.to_string(), name: name.to_string(), unit: unit.to_string(), higher_is_better: *hib, values, delta, delta_pct }
        })
        .filter(|m| m.values.iter().any(|v| v.is_some()))
        .collect();

    let mut flags = Vec::new();
    for s in sessions {
        let n = name_of(s);
        if s.seconds() < MIN_RELIABLE_SECONDS {
            warn(&mut flags, format!("\"{n}\" is a short capture ({:.0} s). Its figures, the lows above all, are not reliable enough to compare.", s.seconds()));
        }
        if s.truncated {
            warn(&mut flags, format!("\"{n}\" hit the recording size limit, so it does not cover its whole run."));
        }
        if s.headline().low_01pct_fps.is_none() {
            flags.push(Flag { severity: "info".into(), text: format!("\"{n}\" has too few frames for a 0.1% low.") });
        }
    }
    for s in &sessions[1..] {
        let (a, b) = (name_of(base), name_of(s));
        if !s.exe.eq_ignore_ascii_case(&base.exe) {
            warn(&mut flags, format!("Different games: \"{a}\" measured {} and \"{b}\" measured {}.", base.exe, s.exe));
        }
        if (s.width, s.height) != (base.width, base.height) {
            let res = |x: &BenchSession| match (x.width, x.height) {
                (Some(w), Some(h)) => format!("{w}x{h}"),
                _ => "unknown".to_string(),
            };
            warn(&mut flags, format!("Different display resolution: {} for \"{a}\", {} for \"{b}\".", res(base), res(s)));
        }
        if let (Some(x), Some(y)) = (base.refresh_hz, s.refresh_hz) {
            if (x - y).abs() > 1.0 {
                warn(&mut flags, format!("Different refresh rate: {x:.0} Hz for \"{a}\", {y:.0} Hz for \"{b}\". A frame cap or V-Sync would change the result."));
            }
        }
        if s.gpu != base.gpu || s.cpu != base.cpu {
            warn(&mut flags, format!("Different hardware: \"{a}\" ran on {} / {}, \"{b}\" on {} / {}.", base.cpu, base.gpu.clone().unwrap_or_else(|| "unknown GPU".into()), s.cpu, s.gpu.clone().unwrap_or_else(|| "unknown GPU".into())));
        } else if s.gpu_driver != base.gpu_driver {
            flags.push(Flag { severity: "info".into(), text: format!("Different graphics driver: {} for \"{a}\", {} for \"{b}\".", base.gpu_driver.clone().unwrap_or_else(|| "unknown".into()), s.gpu_driver.clone().unwrap_or_else(|| "unknown".into())) });
        }
        if s.source != base.source {
            flags.push(Flag { severity: "info".into(), text: format!("Different measurement source: {} for \"{a}\", {} for \"{b}\".", base.source, s.source) });
        }
        let ratio = if base.duration_ms > 0.0 { s.duration_ms / base.duration_ms } else { 1.0 };
        if !(0.67..=1.5).contains(&ratio) {
            warn(&mut flags, format!("Different capture length: {:.0} s for \"{a}\", {:.0} s for \"{b}\". They may not cover the same part of the game.", base.seconds(), s.seconds()));
        }
        if let (Some(x), Some(y)) = (base.gpu_temp_avg_c, s.gpu_temp_avg_c) {
            if (x - y).abs() >= 8.0 {
                warn(&mut flags, format!("GPU temperature differed by {:.0} °C ({x:.0} °C for \"{a}\", {y:.0} °C for \"{b}\"). A hotter card may have been slowed by its own limits.", (x - y).abs()));
            }
        }
        if let (Some(x), Some(y)) = (base.cpu_temp_avg_c, s.cpu_temp_avg_c) {
            if (x - y).abs() >= 10.0 {
                warn(&mut flags, format!("CPU temperature differed by {:.0} °C between \"{a}\" and \"{b}\".", (x - y).abs()));
            }
        }
        // Whole-system CPU load far apart while the game's own per-frame CPU work is
        // similar points at other programs being busy during one of the runs.
        if let (Some(x), Some(y)) = (base.cpu_util_pct, s.cpu_util_pct) {
            let game_work_similar = match (base.cpu_busy_ms, s.cpu_busy_ms) {
                (Some(p), Some(q)) if p > 0.0 => ((q - p) / p).abs() < 0.15,
                _ => true,
            };
            if (x - y).abs() >= 15.0 && game_work_similar {
                warn(&mut flags, format!("Overall CPU load differed by {:.0} points ({x:.0}% for \"{a}\", {y:.0}% for \"{b}\"). Other programs may have been busy during one run.", (x - y).abs()));
            }
        }
        if let (Some(x), Some(y)) = (base.generated_pct, s.generated_pct) {
            if (x - y).abs() >= 5.0 {
                warn(&mut flags, format!("Frame generation differed ({x:.0}% generated frames for \"{a}\", {y:.0}% for \"{b}\"). Generated frames raise FPS without the game rendering more."));
            }
        }
        if s.active_optimizations != base.active_optimizations {
            flags.push(Flag { severity: "info".into(), text: format!("Different optimisations were active for \"{a}\" and \"{b}\".") });
        }
    }
    flags.push(Flag { severity: "info".into(), text: "Arcadia cannot tell whether the runs covered the same scene. Repeat the same route or built-in benchmark for each run.".into() });

    let mut summary = Vec::new();
    for (i, s) in sessions.iter().enumerate().skip(1) {
        for key in ["avgFps", "low1", "low01"] {
            let Some(m) = metrics.iter().find(|m| m.key == key) else { continue };
            let (Some(d), Some(p), Some(v0), Some(v)) = (m.delta[i], m.delta_pct[i], m.values[0], m.values[i]) else { continue };
            let direction = if p.abs() < NOISE_PCT {
                "about the same as".to_string()
            } else {
                format!("{} than", if d > 0.0 { "higher" } else { "lower" })
            };
            summary.push(format!("{}: \"{}\" measured {v:.1} FPS, {direction} \"{}\" at {v0:.1} FPS ({d:+.1} FPS, {p:+.1}%).", m.name, name_of(s), name_of(base)));
        }
        if let Some(p) = metrics.iter().find(|m| m.key == "avgFps").and_then(|m| m.delta_pct[i]) {
            if p.abs() < NOISE_PCT {
                summary.push(format!("A {:.1}% difference in average FPS is within ordinary run-to-run variation. Repeat both runs before reading anything into it.", p.abs()));
            }
        }
    }
    let comparable = !flags.iter().any(|f| f.severity == "warning");
    summary.push(if comparable {
        "These are measurements of two or more runs. One run each does not prove what caused a difference; repeat the runs to confirm it.".to_string()
    } else {
        "The runs were not taken under matching conditions (see the warnings), so the differences cannot be attributed to any one change.".to_string()
    });
    Ok(Comparison { ids: sessions.iter().map(|s| s.id.clone()).collect(), labels: sessions.iter().map(name_of).collect(), metrics, flags, summary, comparable })
}

fn csv_cell(v: &str) -> String {
    // Quote when needed, and stop spreadsheet apps treating a cell as a formula.
    let v = if v.starts_with(['=', '+', '-', '@']) && v.parse::<f64>().is_err() { format!("'{v}") } else { v.to_string() };
    if v.contains([',', '"', '\n', '\r']) { format!("\"{}\"", v.replace('"', "\"\"")) } else { v }
}

/// One row per session, every stored measurement.
pub fn to_csv(sessions: &[BenchSession]) -> String {
    let num = |v: Option<f64>| v.map(|v| format!("{v:.3}")).unwrap_or_default();
    let int = |v: Option<u64>| v.map(|v| v.to_string()).unwrap_or_default();
    let header = [
        "id", "label", "game", "executable", "pid", "started_unix", "duration_s", "source", "width", "height", "refresh_hz", "cpu", "gpu", "gpu_driver", "os",
        "frames", "avg_fps", "low_1pct_fps", "low_01pct_fps", "avg_frame_time_ms", "p50_ms", "p90_ms", "p95_ms", "p99_ms", "p999_ms", "max_frame_time_ms", "stddev_ms", "consistency_pct",
        "presented_avg_fps", "cpu_busy_ms", "cpu_wait_ms", "gpu_busy_ms", "gpu_wait_ms", "display_latency_ms", "dropped_pct", "generated_pct",
        "cpu_util_pct", "cpu_temp_avg_c", "cpu_temp_max_c", "gpu_util_pct", "gpu_temp_avg_c", "gpu_temp_max_c", "gpu_power_avg_w", "vram_used_max_bytes", "ram_used_avg_bytes",
        "profile", "active_optimizations", "truncated", "notes",
    ];
    let mut out = header.join(",");
    out.push_str("\r\n");
    for s in sessions {
        let h = s.headline();
        let row = [
            s.id.clone(), s.label.clone(), s.game_name.clone(), s.exe.clone(), s.pid.to_string(), s.started_at.to_string(), format!("{:.1}", s.seconds()), s.source.clone(),
            int(s.width.map(u64::from)), int(s.height.map(u64::from)), num(s.refresh_hz), s.cpu.clone(), s.gpu.clone().unwrap_or_default(), s.gpu_driver.clone().unwrap_or_default(), s.os.clone(),
            h.frames.to_string(), num(h.avg_fps), num(h.low_1pct_fps), num(h.low_01pct_fps), num(h.avg_frame_time_ms), num(h.p50_ms), num(h.p90_ms), num(h.p95_ms), num(h.p99_ms), num(h.p999_ms),
            num(h.max_frame_time_ms), num(h.stddev_ms), num(h.consistency_pct), num(s.presented.avg_fps), num(s.cpu_busy_ms), num(s.cpu_wait_ms), num(s.gpu_busy_ms), num(s.gpu_wait_ms),
            num(s.display_latency_ms), num(s.dropped_pct), num(s.generated_pct), num(s.cpu_util_pct), num(s.cpu_temp_avg_c), num(s.cpu_temp_max_c), num(s.gpu_util_pct), num(s.gpu_temp_avg_c),
            num(s.gpu_temp_max_c), num(s.gpu_power_avg_w), int(s.vram_used_max_bytes), int(s.ram_used_avg_bytes), s.profile.clone().unwrap_or_default(), s.active_optimizations.join("; "),
            s.truncated.to_string(), s.notes.join(" "),
        ];
        out.push_str(&row.iter().map(|c| csv_cell(c)).collect::<Vec<_>>().join(","));
        out.push_str("\r\n");
    }
    out
}

/// The sessions, plus their comparison when there are two or more.
pub fn to_json(sessions: &[BenchSession]) -> anyhow::Result<Vec<u8>> {
    let comparison = if sessions.len() >= 2 { Some(compare(sessions)?) } else { None };
    Ok(serde_json::to_vec_pretty(&serde_json::json!({
        "kind": "arcadia-benchmark-report",
        "schema": BENCH_SCHEMA,
        "generatedWith": format!("Arcadia {}", env!("CARGO_PKG_VERSION")),
        "sessions": sessions,
        "comparison": comparison,
    }))?)
}

/// Write a report. The format follows the extension: `.csv` or `.json`.
pub fn export(sessions: &[BenchSession], path: &Path) -> anyhow::Result<()> {
    if sessions.is_empty() {
        anyhow::bail!("Choose at least one benchmark to export");
    }
    if !path.is_absolute() {
        anyhow::bail!("Choose a full path for the report");
    }
    let bytes = match path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("csv") => to_csv(sessions).into_bytes(),
        Some("json") => to_json(sessions)?,
        _ => anyhow::bail!("Reports are saved as .csv or .json"),
    };
    super::write_atomic(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameforge::gpu::GpuTelemetry;
    use crate::frameforge::presentmon::Frame;
    use crate::frameforge::telemetry::{GpuSample, Sample};

    /// `seconds` of frames at a steady `fps`, with one slow frame every `hitch_every`.
    fn recording(fps: f64, seconds: f64, hitch_every: usize, gpu_temp: f64, cpu_pct: f64) -> Recording {
        let ft = 1000.0 / fps;
        let n = (seconds * fps) as usize;
        let frames = (0..n)
            .map(|i| {
                let t = if hitch_every > 0 && i % hitch_every == hitch_every - 1 { ft * 3.0 } else { ft };
                Frame { time_ms: i as f64 * ft, frame_time_ms: t, displayed_time_ms: Some(t), cpu_busy_ms: Some(ft * 0.4), gpu_busy_ms: Some(ft * 0.9), ..Default::default() }
            })
            .collect();
        let telemetry = (0..(seconds as usize / 2).max(1))
            .map(|_| Sample { cpu_pct: Some(cpu_pct), ram_used_bytes: 8 << 30, gpus: vec![GpuSample { id: "g".into(), name: "GPU".into(), telemetry: GpuTelemetry { utilization_pct: Some(97.0), temperature_c: Some(gpu_temp), power_w: Some(200.0), vram_used_bytes: Some(6 << 30), ..Default::default() } }], ..Default::default() })
            .collect();
        Recording { frames, telemetry, started_unix: 1_760_000_000, duration_ms: seconds * 1000.0 + 500.0, truncated: false }
    }

    fn ctx(label: &str) -> CaptureContext {
        CaptureContext { label: label.into(), game_name: "Game".into(), exe: "game.exe".into(), pid: 10, source: "PresentMon service".into(), width: Some(2560), height: Some(1440), refresh_hz: Some(165.0), cpu: "CPU".into(), gpu: Some("GPU".into()), gpu_id: Some("g".into()), gpu_driver: Some("566.14".into()), os: "Windows 11".into(), ..Default::default() }
    }

    fn session(label: &str, fps: f64, seconds: f64) -> BenchSession {
        let mut s = summarise(&recording(fps, seconds, 0, 70.0, 30.0), &ctx(label)).unwrap();
        s.id = label.to_lowercase().replace(' ', "-");
        s
    }

    #[test]
    fn summary_holds_the_measured_values() {
        let s = summarise(&recording(100.0, 60.0, 200, 71.0, 35.0), &ctx("  Run 1 ")).unwrap();
        assert_eq!(s.label, "Run 1");
        assert_eq!(s.presented.frames, 6000);
        // 30 slow frames of 30 ms among 5970 of 10 ms.
        let avg = s.displayed.avg_fps.unwrap();
        assert!((avg - 6000.0 / (5970.0 * 0.010 + 30.0 * 0.030)).abs() < 0.01, "{avg}");
        assert!((s.displayed.low_1pct_fps.unwrap() - 1000.0 / 20.0).abs() < 0.01, "{:?}", s.displayed.low_1pct_fps);
        assert!((s.displayed.low_01pct_fps.unwrap() - 1000.0 / 30.0).abs() < 0.01);
        assert_eq!(s.displayed.p50_ms, Some(10.0));
        assert!((s.cpu_busy_ms.unwrap() - 4.0).abs() < 1e-9 && (s.gpu_busy_ms.unwrap() - 9.0).abs() < 1e-9);
        assert_eq!((s.gpu_temp_avg_c, s.gpu_temp_max_c, s.gpu_util_pct, s.cpu_util_pct), (Some(71.0), Some(71.0), Some(97.0), Some(35.0)));
        assert_eq!(s.vram_used_max_bytes, Some(6 << 30));
        // Duration comes from the frames, not from the wall clock.
        assert!((s.duration_ms - (5970.0 * 10.0 + 30.0 * 30.0)).abs() < 1e-6);
        assert_eq!((s.width, s.height, s.refresh_hz), (Some(2560), Some(1440), Some(165.0)));
        assert!(s.notes.is_empty(), "{:?}", s.notes);
        assert!(s.frame_times_ms.len() <= 600 && s.frame_times_ms.iter().any(|t| *t > 29.0), "the chart keeps the spikes");
    }

    #[test]
    fn short_or_empty_captures_are_flagged_not_dressed_up() {
        assert!(summarise(&Recording::default(), &ctx("x")).unwrap_err().to_string().contains("No frames"));
        let s = summarise(&recording(60.0, 5.0, 0, 60.0, 20.0), &ctx("x")).unwrap();
        assert_eq!(s.displayed.low_01pct_fps, None, "300 frames cannot give a 0.1% low");
        assert!(s.notes.iter().any(|n| n.contains("Short capture")) && s.notes.iter().any(|n| n.contains("0.1% low")));
        // No display timing: presented figures are used and the session says so.
        let mut rec = recording(60.0, 40.0, 0, 60.0, 20.0);
        rec.frames.iter_mut().for_each(|f| f.displayed_time_ms = None);
        rec.telemetry.clear();
        let s = summarise(&rec, &ctx("x")).unwrap();
        assert_eq!(s.displayed.frames, 0);
        assert!(s.headline().avg_fps.is_some() && s.dropped_pct.is_none() && s.gpu_temp_avg_c.is_none());
        assert!(s.notes.iter().any(|n| n.contains("presented frames")) && s.notes.iter().any(|n| n.contains("No hardware readings")));
    }

    #[test]
    fn store_round_trip_rename_delete() {
        let dir = tempfile::tempdir().unwrap();
        let store = BenchStore::new(dir.path().join("b"));
        let a = store.save(BenchSession { id: String::new(), ..session("A", 60.0, 40.0) }).unwrap();
        let mut later = session("B", 90.0, 40.0);
        later.id = String::new();
        later.started_at += 100;
        let b = store.save(later).unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(store.list().iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), vec!["B", "A"], "newest first");
        assert_eq!(store.get(&a.id).unwrap(), a);
        assert_eq!(store.rename(&a.id, " Baseline ").unwrap().label, "Baseline");
        assert!(store.rename(&a.id, &"x".repeat(121)).is_err());
        std::fs::write(dir.path().join("b").join("junk.json"), b"{not json").unwrap();
        assert_eq!(store.list().len(), 2, "unreadable files are skipped");
        store.delete(&a.id).unwrap();
        assert!(store.get(&a.id).is_err() && store.delete(&a.id).is_err());
        assert!(store.get("..\\..\\x").is_err());
    }

    #[test]
    fn comparison_reports_absolute_and_percentage_changes() {
        let (a, b, c) = (session("Before", 80.0, 60.0), session("After", 100.0, 60.0), session("Third", 60.0, 60.0));
        let cmp = compare(&[a, b, c]).unwrap();
        let avg = cmp.metrics.iter().find(|m| m.key == "avgFps").unwrap();
        assert_eq!(avg.delta[0], None);
        assert!((avg.delta[1].unwrap() - 20.0).abs() < 1e-6 && (avg.delta_pct[1].unwrap() - 25.0).abs() < 1e-6);
        assert!((avg.delta[2].unwrap() + 20.0).abs() < 1e-6 && (avg.delta_pct[2].unwrap() + 25.0).abs() < 1e-6);
        let ft = cmp.metrics.iter().find(|m| m.key == "avgFrameTime").unwrap();
        assert_eq!(ft.higher_is_better, Some(false));
        assert!((ft.delta[1].unwrap() + 2.5).abs() < 1e-6 && (ft.delta_pct[1].unwrap() + 20.0).abs() < 1e-6);
        assert!(cmp.metrics.iter().any(|m| m.key == "gpuTemp") && cmp.metrics.iter().any(|m| m.key == "low01"));
        assert!(cmp.comparable, "{:?}", cmp.flags);
        assert!(cmp.summary.iter().any(|s| s.contains("+20.0 FPS") && s.contains("+25.0%")), "{:?}", cmp.summary);
        // Never a causal claim, and the scene caveat is always present.
        assert!(cmp.summary.last().unwrap().contains("does not prove"));
        assert!(cmp.flags.iter().any(|f| f.text.contains("same scene")));
        assert!(compare(&[session("x", 60.0, 60.0)]).is_err());
    }

    #[test]
    fn unreliable_comparisons_are_flagged() {
        let base = session("Base", 80.0, 60.0);
        let warnings = |other: BenchSession| compare(&[base.clone(), other]).unwrap().flags.into_iter().filter(|f| f.severity == "warning").map(|f| f.text).collect::<Vec<_>>();
        assert!(warnings(session("Same", 81.0, 60.0)).is_empty());
        assert!(warnings(session("Short", 80.0, 10.0)).iter().any(|w| w.contains("short capture")));
        assert!(warnings(session("Long", 80.0, 200.0)).iter().any(|w| w.contains("capture length")));
        let mut res = session("1080p", 120.0, 60.0);
        (res.width, res.height) = (Some(1920), Some(1080));
        assert!(warnings(res).iter().any(|w| w.contains("2560x1440") && w.contains("1920x1080")));
        let mut hot = summarise(&recording(70.0, 60.0, 0, 86.0, 30.0), &ctx("Hot")).unwrap();
        hot.id = "hot".into();
        assert!(warnings(hot).iter().any(|w| w.contains("GPU temperature differed by 16")));
        let mut busy = summarise(&recording(80.0, 60.0, 0, 70.0, 75.0), &ctx("Busy")).unwrap();
        busy.id = "busy".into();
        assert!(warnings(busy).iter().any(|w| w.contains("Other programs")));
        let mut other_game = session("Other", 80.0, 60.0);
        other_game.exe = "other.exe".into();
        assert!(warnings(other_game).iter().any(|w| w.contains("Different games")));
        let mut hw = session("New GPU", 140.0, 60.0);
        hw.gpu = Some("Other GPU".into());
        assert!(warnings(hw).iter().any(|w| w.contains("Different hardware")));
        let mut fg = session("FG", 150.0, 60.0);
        fg.generated_pct = Some(50.0);
        let cmp = compare(&[base.clone(), fg]).unwrap();
        assert!(!cmp.comparable && cmp.summary.last().unwrap().contains("cannot be attributed"));
        // A difference inside the noise band is called that.
        let cmp = compare(&[base.clone(), session("Same", 81.0, 60.0)]).unwrap();
        assert!(cmp.summary.iter().any(|s| s.contains("about the same")) && cmp.summary.iter().any(|s| s.contains("run-to-run variation")));
    }

    #[test]
    fn reports_export_as_csv_and_json() {
        let mut a = session("Before, \"stock\"", 80.0, 60.0);
        a.active_optimizations = vec!["=cmd()".into()];
        let b = session("After", 100.0, 60.0);
        let csv = to_csv(&[a.clone(), b.clone()]);
        let lines: Vec<&str> = csv.split("\r\n").collect();
        assert_eq!(lines.len(), 4, "header, two rows, trailing newline");
        assert!(lines[0].starts_with("id,label,game,executable"));
        assert!(lines[1].contains("\"Before, \"\"stock\"\"\"") && lines[1].contains("'=cmd()") && lines[1].contains(",80.000,"));
        assert_eq!(lines[0].split(',').count(), 49);
        let json: serde_json::Value = serde_json::from_slice(&to_json(&[a.clone(), b.clone()]).unwrap()).unwrap();
        assert_eq!(json["kind"], "arcadia-benchmark-report");
        assert_eq!(json["sessions"].as_array().unwrap().len(), 2);
        assert!(json["comparison"]["metrics"].as_array().unwrap().len() > 5);
        assert!(serde_json::from_slice::<serde_json::Value>(&to_json(std::slice::from_ref(&a)).unwrap()).unwrap()["comparison"].is_null());
        let dir = tempfile::tempdir().unwrap();
        export(&[a.clone(), b.clone()], &dir.path().join("r.csv")).unwrap();
        export(&[a.clone()], &dir.path().join("r.JSON")).unwrap();
        assert!(std::fs::read_to_string(dir.path().join("r.csv")).unwrap().starts_with("id,label"));
        assert!(export(&[a.clone()], &dir.path().join("r.exe")).is_err());
        assert!(export(&[a], Path::new("r.csv")).is_err());
        assert!(export(&[], &dir.path().join("r.csv")).is_err());
    }
}
