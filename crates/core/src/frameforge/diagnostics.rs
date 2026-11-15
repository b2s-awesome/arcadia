//! Rule-based diagnostics: what is likely limiting performance, and why.
//!
//! The engine is deterministic. The same inputs always give the same
//! findings, in the same order, and every finding lists the measurements it
//! rests on. Rules that only have indirect evidence say so through
//! [`Confidence::Low`]; for example a busy processor on its own is never
//! reported as a CPU bottleneck.
//!
//! How the limiting component is decided (when PresentMon frame data exists):
//! * `GPU busy ÷ frame time ≥ 0.90`: the frame is spent waiting for the
//!   graphics card. GPU-bound.
//! * `GPU busy ÷ frame time ≤ 0.75` and `CPU busy ÷ frame time ≥ 0.75`: the
//!   game's own thread is the long pole. CPU-bound.
//! * Average FPS sitting on the display's refresh rate or a common cap with
//!   steady frame times: a frame limiter or V-Sync is in charge; neither.
//!
//! Thresholds are constants at the top of the file so they can be reviewed
//! in one place.

use super::gpu::{ApiStatus, Risk, Vendor};
use super::hardware::Inventory;
use super::optimizer::Restart;
use super::processes::{Category, ProcessInfo};
use super::stats::FrameStats;
use super::telemetry::Summary;
use serde::{Deserialize, Serialize};

pub const GPU_BOUND_SHARE: f64 = 0.90;
pub const CPU_BOUND_GPU_SHARE_MAX: f64 = 0.75;
pub const CPU_BOUND_CPU_SHARE: f64 = 0.75;
pub const VRAM_PRESSURE: f64 = 0.92;
pub const RAM_PRESSURE_PCT: f64 = 88.0;
pub const BACKGROUND_CPU_PCT: f64 = 15.0;
pub const BACKGROUND_MEM_SHARE: f64 = 0.25;
pub const CONSISTENCY_POOR_PCT: f64 = 60.0;
pub const DROPPED_PCT: f64 = 5.0;
pub const HOT_GPU_C: f64 = 87.0;
pub const CLOCK_SWING: f64 = 0.85;
/// Seconds of frame data needed before frame-based rules run.
pub const MIN_SECONDS: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Notice,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// Indirect evidence only.
    Low,
    Medium,
    /// Several independent measurements agree.
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: String,
    /// What was detected.
    pub title: String,
    pub severity: Severity,
    pub confidence: Confidence,
    /// Supporting measurements.
    pub evidence: Vec<String>,
    /// Why it may affect performance.
    pub why: String,
    pub recommendation: String,
    pub trade_off: String,
    pub risk: Risk,
    pub restart: Restart,
    pub reversible: bool,
    /// Optimiser actions that address it (may be empty: advice only).
    pub actions: Vec<String>,
    /// A Windows Settings page to open, when the fix lives there.
    pub settings_uri: Option<String>,
}

/// Frame measurements of the monitored game over the analysis window.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameEvidence {
    pub process: String,
    pub stats: FrameStats,
    pub cpu_busy_ms: Option<f64>,
    pub cpu_wait_ms: Option<f64>,
    pub gpu_busy_ms: Option<f64>,
    pub gpu_wait_ms: Option<f64>,
    pub dropped_pct: Option<f64>,
    pub generated_pct: Option<f64>,
}

pub struct Input<'a> {
    pub inv: &'a Inventory,
    /// Telemetry over the same window as `frames` (or recent samples when not monitoring).
    pub summary: Option<&'a Summary>,
    pub frames: Option<&'a FrameEvidence>,
    pub processes: &'a [ProcessInfo],
    pub vendor_apis: &'a [(Vendor, ApiStatus)],
    pub presentmon_ready: bool,
}

fn finding(id: &str, title: impl Into<String>, severity: Severity, confidence: Confidence) -> Finding {
    Finding {
        id: id.into(),
        title: title.into(),
        severity,
        confidence,
        evidence: Vec::new(),
        why: String::new(),
        recommendation: String::new(),
        trade_off: "None.".into(),
        risk: Risk::Low,
        restart: Restart::None,
        reversible: true,
        actions: Vec::new(),
        settings_uri: None,
    }
}

const COMMON_CAPS: &[f64] = &[30.0, 60.0, 72.0, 75.0, 90.0, 100.0, 120.0, 144.0, 165.0, 180.0, 240.0, 360.0];

/// The cap the average FPS is sitting on, if any (within 1.5%).
fn cap_hit(avg_fps: f64, refresh: Option<f64>) -> Option<f64> {
    COMMON_CAPS.iter().copied().chain(refresh).chain(refresh.map(|r| r - 3.0)).find(|c| *c > 0.0 && ((avg_fps - c) / c).abs() <= 0.015)
}

fn gb(bytes: u64) -> f64 {
    bytes as f64 / (1u64 << 30) as f64
}

pub fn analyze(input: &Input) -> Vec<Finding> {
    let mut out = Vec::new();
    let inv = input.inv;
    let sum = input.summary;
    let refresh = inv.displays.iter().find(|d| d.primary).or(inv.displays.first()).and_then(|d| d.refresh_hz);
    let frames = input.frames.filter(|f| f.stats.duration_ms >= MIN_SECONDS * 1000.0 && f.stats.avg_frame_time_ms.is_some());
    let mut gpu_bound = false;

    // ── What limits the frame rate ───────────────────────────────────────
    if let Some(f) = frames {
        let ft = f.stats.avg_frame_time_ms.unwrap_or(0.0);
        let fps = f.stats.avg_fps.unwrap_or(0.0);
        let gpu_share = f.gpu_busy_ms.filter(|_| ft > 0.0).map(|g| g / ft);
        let cpu_share = f.cpu_busy_ms.filter(|_| ft > 0.0).map(|c| c / ft);
        let steady = f.stats.consistency_pct.unwrap_or(0.0) >= 85.0;
        let capped = cap_hit(fps, refresh).filter(|_| steady && gpu_share.map(|g| g < GPU_BOUND_SHARE).unwrap_or(true));
        if let Some(cap) = capped {
            let mut x = finding("limit.frame_cap", format!("The frame rate is sitting on a limit of about {cap:.0} FPS"), Severity::Info, if gpu_share.is_some() { Confidence::Medium } else { Confidence::Low });
            x.evidence.push(format!("Average {fps:.1} FPS with {:.0}% frame-time consistency", f.stats.consistency_pct.unwrap_or(0.0)));
            if let Some(r) = refresh {
                x.evidence.push(format!("Display refresh rate {r:.0} Hz"));
            }
            if let Some(g) = gpu_share {
                x.evidence.push(format!("GPU busy for {:.0}% of each frame", g * 100.0));
            }
            x.why = "A frame limiter or V-Sync is holding the game at this rate, so neither the processor nor the graphics card is the limit right now.".into();
            x.recommendation = "No hardware optimisation is needed for more FPS. To go higher, raise or remove the in-game frame limit; to keep it, this is a healthy state.".into();
            out.push(x);
        } else if let Some(g) = gpu_share.filter(|g| *g >= GPU_BOUND_SHARE) {
            gpu_bound = true;
            let util = sum.and_then(|s| s.gpu_avg_pct);
            let mut x = finding("limit.gpu", "The game is limited by the graphics card (GPU-bound)", Severity::Info, if util.map(|u| u >= 90.0).unwrap_or(false) { Confidence::High } else { Confidence::Medium });
            x.evidence.push(format!("GPU busy for {:.0}% of each frame ({:.2} of {ft:.2} ms)", g * 100.0, f.gpu_busy_ms.unwrap_or(0.0)));
            if let Some(u) = util {
                x.evidence.push(format!("GPU utilisation averaged {u:.0}%"));
            }
            x.evidence.push(format!("Average {fps:.1} FPS"));
            x.why = "Each frame waits on the graphics card, so the frame rate follows how much work the card is given.".into();
            x.recommendation = "For more FPS, lower GPU-heavy settings in the game (resolution or render scale, ray tracing, shadows, volumetrics) or turn on an upscaler such as DLSS, FSR or XeSS. Processor tweaks and process priority will not help here.".into();
            x.trade_off = "Lower image quality.".into();
            x.restart = Restart::None;
            out.push(x);
        } else if let (Some(g), Some(c)) = (gpu_share.filter(|g| *g <= CPU_BOUND_GPU_SHARE_MAX), cpu_share.filter(|c| *c >= CPU_BOUND_CPU_SHARE)) {
            let core = sum.and_then(|s| s.cpu_max_core_avg_pct);
            let mut x = finding("limit.cpu", "The game is limited by the processor (CPU-bound)", Severity::Info, if core.map(|c| c >= 85.0).unwrap_or(false) { Confidence::High } else { Confidence::Medium });
            x.evidence.push(format!("CPU busy for {:.0}% of each frame ({:.2} of {ft:.2} ms)", c * 100.0, f.cpu_busy_ms.unwrap_or(0.0)));
            x.evidence.push(format!("GPU busy for only {:.0}% of each frame", g * 100.0));
            if let Some(w) = f.gpu_wait_ms {
                x.evidence.push(format!("The graphics card waits {w:.2} ms per frame for work"));
            }
            if let Some(core) = core {
                x.evidence.push(format!("Busiest processor core averaged {core:.0}%"));
            }
            x.why = "The graphics card finishes early and waits for the game's main thread, so the frame rate follows processor speed.".into();
            x.recommendation = "Lower CPU-heavy settings (view or draw distance, crowd and traffic density, physics, simulation quality) and close background apps that use the processor. Raising resolution or graphics quality will cost little FPS in this state.".into();
            x.trade_off = "Less detail in the distance or fewer simulated objects.".into();
            x.actions = vec!["process.priority".into()];
            out.push(x);
        }
    } else if let Some(s) = sum {
        // No frame data: utilisation can hint, never prove.
        if let Some(u) = s.gpu_avg_pct.filter(|u| *u >= 95.0) {
            gpu_bound = true;
            let mut x = finding("limit.gpu_hint", "The graphics card is running near full use", Severity::Info, Confidence::Low);
            x.evidence.push(format!("GPU utilisation averaged {u:.0}%"));
            x.why = "A fully used graphics card usually means the game is GPU-bound, but utilisation alone does not measure frames.".into();
            x.recommendation = "Start the Live Monitor on the game to confirm with frame data before changing settings.".into();
            out.push(x);
        } else if let (Some(core), Some(gpu)) = (s.cpu_max_core_avg_pct.filter(|c| *c >= 95.0), s.gpu_avg_pct.filter(|g| *g < 70.0)) {
            let mut x = finding("limit.cpu_hint", "One processor core is full while the graphics card has headroom", Severity::Info, Confidence::Low);
            x.evidence.push(format!("Busiest core averaged {core:.0}%"));
            x.evidence.push(format!("GPU utilisation averaged {gpu:.0}%"));
            x.why = "This pattern can mean a processor limit, but high processor use on its own does not prove one: a frame limiter, loading, or a background task looks the same.".into();
            x.recommendation = "Start the Live Monitor on the game. CPU busy and GPU wait times per frame will show whether the processor is the limit.".into();
            out.push(x);
        }
    }

    // ── Temperature and clocks ───────────────────────────────────────────
    if let Some(s) = sum {
        if s.thermal_limited_samples > 0 {
            let mut x = finding("thermal.throttle", "The graphics card is slowing itself down to control its temperature", Severity::Warning, Confidence::High);
            x.evidence.push(format!("The driver reported thermal limiting in {} of {} samples", s.thermal_limited_samples, s.samples));
            if let Some(t) = s.gpu_temp_max_c {
                x.evidence.push(format!("Peak GPU temperature {t:.0} °C"));
            }
            x.why = "While thermally limited, the card lowers its clock speed, which lowers FPS and can cause uneven frame times.".into();
            x.recommendation = "Improve cooling: clean dust from the card and case filters, check that all fans spin, and give the case fresh air. A frame limit also lowers heat.".into();
            x.trade_off = "None from cleaning; a frame limit trades peak FPS for lower temperature.".into();
            out.push(x);
        } else if let Some(t) = s.gpu_temp_max_c.filter(|t| *t >= HOT_GPU_C) {
            let mut x = finding("thermal.hot", "The graphics card is running hot", Severity::Notice, Confidence::Low);
            x.evidence.push(format!("Peak GPU temperature {t:.0} °C"));
            x.evidence.push("The driver did not report thermal limiting".into());
            x.why = "Limits differ by model, so this temperature may be within the card's normal range. Close to its limit a card starts lowering clocks.".into();
            x.recommendation = "Check the card's rated maximum temperature. If FPS falls during long sessions, improve case airflow.".into();
            out.push(x);
        }
        if s.power_limited_samples > 0 && s.samples > 0 && s.power_limited_samples * 4 >= s.samples {
            let mut x = finding("power.gpu_limited", "The driver reports the graphics card is being held back by its power limit", Severity::Notice, Confidence::Medium);
            x.evidence.push(format!("Power limiting reported in {} of {} samples", s.power_limited_samples, s.samples));
            x.why = "On laptops this usually means the charger is unplugged or underpowered, or a quiet/battery mode is active.".into();
            x.recommendation = "Plug in the original charger and select the vendor's performance mode. On desktops check the power cables to the card.".into();
            out.push(x);
        }
        if let (Some(lo), Some(hi), Some(util)) = (s.gpu_clock_min_mhz, s.gpu_clock_max_mhz, s.gpu_avg_pct) {
            if util >= 90.0 && hi > 0.0 && lo / hi < CLOCK_SWING && s.thermal_limited_samples == 0 {
                let mut x = finding("clock.unstable", "The graphics card's clock speed moved a lot while under load", Severity::Notice, Confidence::Low);
                x.evidence.push(format!("Core clock ranged from {lo:.0} to {hi:.0} MHz at {util:.0}% average utilisation"));
                x.why = "Large clock swings under steady load can come from power or temperature limits, or a power-saving mode, and can show up as uneven frame times.".into();
                x.recommendation = "Check the power plan and, on laptops, the vendor performance mode. If the driver's power mode is set to save power, set it to normal or prefer maximum performance for this game.".into();
                out.push(x);
            }
        }

        // ── Memory ───────────────────────────────────────────────────────
        if let (Some(used), Some(total)) = (s.vram_peak_bytes, s.vram_total_bytes.filter(|t| *t > 0)) {
            let share = used as f64 / total as f64;
            if share >= VRAM_PRESSURE {
                let mut x = finding("memory.vram", "Video memory is almost full", Severity::Warning, Confidence::Medium);
                x.evidence.push(format!("Peak video memory {:.1} of {:.1} GB ({:.0}%)", gb(used), gb(total), share * 100.0));
                x.why = "When video memory runs out, textures spill into system memory, which causes stutter and sudden FPS drops.".into();
                x.recommendation = "Lower texture quality one step first; it frees the most video memory for the least visible change. Then lower resolution or ray tracing. Close browsers and other apps that use the GPU.".into();
                x.trade_off = "Slightly softer textures.".into();
                x.restart = Restart::Game;
                out.push(x);
            }
        }
        let avail_low = sum.and_then(|s| s.ram_avg_pct).filter(|p| *p >= RAM_PRESSURE_PCT);
        if let Some(p) = avail_low {
            let mut x = finding("memory.ram", "System memory is nearly full", Severity::Warning, Confidence::Medium);
            x.evidence.push(format!("Memory use averaged {p:.0}% of {:.0} GB", gb(inv.memory.total_bytes)));
            x.why = "With little free memory Windows moves data to the page file on disk, which causes hitching when the game needs it back.".into();
            x.recommendation = "Close memory-heavy background apps before playing (see the process list). If this happens often with this game, more memory is the lasting fix.".into();
            x.actions = vec!["process.close".into()];
            out.push(x);
        }
    }
    if inv.memory.modules.len() == 1 && inv.memory.total_bytes > 0 {
        let mut x = finding("memory.single_module", "Only one memory module is installed", Severity::Info, Confidence::Low);
        x.evidence.push(format!("One module of {:.0} GB reported", gb(inv.memory.modules[0].capacity_bytes)));
        x.why = "A single module usually runs in single-channel mode, which lowers memory bandwidth and can cost FPS in CPU-bound games, most of all with integrated graphics.".into();
        x.recommendation = "If the board has a free slot, a second matching module enables dual-channel mode.".into();
        x.trade_off = "Hardware cost.".into();
        x.reversible = false;
        out.push(x);
    }

    // ── Background load ──────────────────────────────────────────────────
    let background: Vec<&ProcessInfo> = input.processes.iter().filter(|p| !matches!(p.category, Category::Game | Category::Protected)).collect();
    let bg_cpu: f64 = background.iter().map(|p| p.cpu_pct).sum();
    if bg_cpu >= BACKGROUND_CPU_PCT {
        let mut top: Vec<&&ProcessInfo> = background.iter().filter(|p| p.cpu_pct >= 1.0).collect();
        top.sort_by(|a, b| b.cpu_pct.partial_cmp(&a.cpu_pct).unwrap_or(std::cmp::Ordering::Equal));
        let mut x = finding("background.cpu", "Background apps are using a noticeable share of the processor", Severity::Notice, Confidence::Medium);
        x.evidence.push(format!("Apps other than the game use {bg_cpu:.0}% of the processor in total"));
        x.evidence.extend(top.iter().take(3).map(|p| format!("{}: {:.0}%", p.name, p.cpu_pct)));
        x.why = "Processor time taken by other apps is not available to the game. It matters most when the game is CPU-bound.".into();
        x.recommendation = if gpu_bound { "The game is GPU-bound, so this is unlikely to change FPS much; closing these apps can still smooth frame times.".into() } else { "Close the apps you do not need while playing.".into() };
        x.trade_off = "Those apps stop until you reopen them.".into();
        x.reversible = false;
        x.actions = vec!["process.close".into()];
        out.push(x);
    }
    let bg_mem: u64 = background.iter().map(|p| p.memory_bytes).sum();
    if inv.memory.total_bytes > 0 && bg_mem as f64 / inv.memory.total_bytes as f64 >= BACKGROUND_MEM_SHARE {
        let mut top = background.clone();
        top.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));
        let browsers: u64 = background.iter().filter(|p| p.category == Category::Browser).map(|p| p.memory_bytes).sum();
        let mut x = finding("background.memory", "Background apps hold a large share of memory", Severity::Notice, Confidence::Medium);
        x.evidence.push(format!("Apps other than the game hold {:.1} of {:.0} GB", gb(bg_mem), gb(inv.memory.total_bytes)));
        if browsers >= (1u64 << 30) {
            x.evidence.push(format!("Browsers: {:.1} GB", gb(browsers)));
        }
        x.evidence.extend(top.iter().take(3).map(|p| format!("{}: {:.1} GB", p.name, gb(p.memory_bytes))));
        x.why = "Less free memory leaves less room for the game and the file cache, and raises the chance of paging.".into();
        x.recommendation = "Review background browser windows and other large apps before playing.".into();
        x.trade_off = "Those apps stop until you reopen them.".into();
        x.reversible = false;
        x.actions = vec!["process.close".into()];
        out.push(x);
    }
    let overlays: Vec<&str> = input.processes.iter().filter(|p| p.category == Category::Overlay).map(|p| p.name.as_str()).collect();
    if !overlays.is_empty() {
        let mut names: Vec<&str> = overlays.clone();
        names.sort_unstable();
        names.dedup();
        let mut x = finding("background.overlays", "Overlay or recording software is running", Severity::Info, Confidence::Low);
        x.evidence.push(format!("Running: {}", names.join(", ")));
        x.why = "Overlays and recorders hook into the game's frames. Most cost little, but several at once, or one that is recording, can add frame-time spikes.".into();
        x.recommendation = "Keep only the ones you use. Compare a benchmark with and without them to see their real cost on this PC.".into();
        x.actions = vec!["process.close".into()];
        x.reversible = false;
        out.push(x);
    }

    // ── Frame pacing ─────────────────────────────────────────────────────
    if let Some(f) = frames {
        if let Some(d) = f.dropped_pct.filter(|d| *d >= DROPPED_PCT) {
            let mut x = finding("pacing.dropped", "Many frames never reach the screen", Severity::Notice, Confidence::Medium);
            x.evidence.push(format!("{d:.1}% of presented frames were not displayed"));
            if let Some(r) = refresh {
                x.evidence.push(format!("Display refresh rate {r:.0} Hz, game at {:.0} FPS", f.stats.avg_fps.unwrap_or(0.0)));
            }
            x.why = "The game renders more frames than the display shows. The extra frames cost power and heat without being seen.".into();
            x.recommendation = "Limit the frame rate a few FPS below the refresh rate, or turn on variable refresh (G-SYNC / FreeSync) if the display supports it.".into();
            x.trade_off = "A limit slightly below refresh can add a little input delay compared with uncapped tearing.".into();
            out.push(x);
        }
        let stutter_share = match (f.stats.stutter_frames, f.stats.frames) {
            (Some(s), n) if n > 0 => s as f64 / n as f64 * 100.0,
            _ => 0.0,
        };
        if f.stats.consistency_pct.map(|c| c < CONSISTENCY_POOR_PCT).unwrap_or(false) || stutter_share >= 1.0 {
            let mut x = finding("pacing.uneven", "Frame times are uneven", Severity::Notice, Confidence::Medium);
            if let (Some(p50), Some(p99)) = (f.stats.p50_ms, f.stats.p99_ms) {
                x.evidence.push(format!("Typical frame {p50:.1} ms, slowest 1% at {p99:.1} ms or more"));
            }
            if let (Some(avg), Some(low)) = (f.stats.avg_fps, f.stats.low_1pct_fps) {
                x.evidence.push(format!("Average {avg:.0} FPS, 1% low {low:.0} FPS"));
            }
            if stutter_share > 0.0 {
                x.evidence.push(format!("{stutter_share:.1}% of frames took more than twice the typical time"));
            }
            x.why = "Uneven frame times are felt as stutter even when the average FPS is high.".into();
            x.recommendation = "Common causes, in order to check: video memory or system memory running out (see above), shader compilation during the first minutes of play, background apps, and an uncapped frame rate. A frame limit a little below the average often evens out pacing.".into();
            x.trade_off = "A frame limit lowers peak FPS.".into();
            out.push(x);
        }
    }

    // ── Windows and display configuration ────────────────────────────────
    if let Some(plan) = inv.active_power_plan() {
        let saver = plan.guid.eq_ignore_ascii_case(super::optimizer::actions::PLAN_POWER_SAVER) || plan.name.to_lowercase().contains("power saver");
        if saver {
            let mut x = finding("power.plan", "Windows is on the Power saver plan", Severity::Warning, Confidence::High);
            x.evidence.push(format!("Active power plan: {}", plan.name));
            x.why = "Power saver caps processor speed, which lowers FPS in most games.".into();
            x.recommendation = "Switch to Balanced or High performance while playing.".into();
            x.trade_off = "More power use.".into();
            x.actions = vec!["power.plan.high_performance".into(), "power.plan.gaming".into()];
            out.push(x);
        }
    }
    if inv.system.gaming.on_battery == Some(true) {
        let mut x = finding("power.battery", "The PC is running on battery", Severity::Notice, Confidence::High);
        x.evidence.push("Windows reports battery power".into());
        x.why = "Laptops limit processor and graphics power on battery, often by half or more.".into();
        x.recommendation = "Plug in the charger before playing.".into();
        out.push(x);
    }
    if inv.system.gaming.game_mode == Some(false) {
        let mut x = finding("windows.game_mode", "Windows Game Mode is off", Severity::Notice, Confidence::High);
        x.evidence.push("Game Mode setting: Off".into());
        x.why = "Game Mode keeps Windows Update from installing drivers or prompting to restart during play and favours the game for processor time.".into();
        x.recommendation = "Turn Game Mode on.".into();
        x.restart = Restart::Game;
        x.actions = vec!["windows.game_mode".into()];
        x.settings_uri = Some("ms-settings:gaming-gamemode".into());
        out.push(x);
    }
    if inv.system.gaming.background_recording == Some(true) {
        let mut x = finding("windows.background_recording", "Background recording is on", Severity::Notice, Confidence::High);
        x.evidence.push("\"Record what happened\" setting: On".into());
        x.why = "Windows keeps encoding video of the game the whole time so it can save the last moments, which uses the GPU's encoder and some memory.".into();
        x.recommendation = "Turn it off unless you use the \"record that\" shortcut.".into();
        x.trade_off = "The \"record the last 30 seconds\" shortcut stops working.".into();
        x.restart = Restart::Game;
        x.actions = vec!["windows.background_recording_off".into()];
        x.settings_uri = Some("ms-settings:gaming-gamedvr".into());
        out.push(x);
    }
    for d in &inv.displays {
        let Some(hz) = d.refresh_hz else { continue };
        let best = d.available_refresh_hz.iter().copied().max().unwrap_or(0) as f64;
        if best >= hz * 1.2 {
            let mut x = finding("display.refresh", format!("{} is running below its highest refresh rate", if d.name.is_empty() { &d.device } else { &d.name }), Severity::Warning, Confidence::High);
            x.evidence.push(format!("Current {hz:.0} Hz; the display offers up to {best:.0} Hz at this resolution"));
            x.why = "The display shows fewer frames per second than it can, so motion looks less smooth and frames above the current rate are wasted.".into();
            x.recommendation = format!("Set the refresh rate to {best:.0} Hz in Windows display settings.");
            x.settings_uri = Some("ms-settings:display-advanced".into());
            out.push(x);
        }
    }
    let low_drives: Vec<_> = inv.drives.iter().filter(|d| !d.removable && d.total_bytes > 0 && (d.free_bytes as f64 / d.total_bytes as f64) < 0.10).collect();
    for d in low_drives {
        let mut x = finding("storage.low", format!("Drive {} is almost full", d.mount), Severity::Notice, Confidence::Medium);
        x.evidence.push(format!("{:.0} GB free of {:.0} GB", gb(d.free_bytes), gb(d.total_bytes)));
        x.why = "Games patch and build shader caches on their drive, and Windows needs room for the page file. A nearly full SSD also writes more slowly.".into();
        x.recommendation = "Free up space on this drive.".into();
        x.settings_uri = Some("ms-settings:storagesense".into());
        x.reversible = false;
        out.push(x);
    }

    // ── Measurement and driver interfaces ────────────────────────────────
    for (vendor, status) in input.vendor_apis {
        if let ApiStatus::Unavailable { api, reason } = status {
            if inv.gpus.iter().any(|g| g.vendor == *vendor) {
                let mut x = finding("driver.api", format!("{} driver interface ({api}) is unavailable", vendor.label()), Severity::Notice, Confidence::High);
                x.evidence.push(reason.clone());
                x.why = "Without it Arcadia cannot read this card's temperature, clocks or driver controls, so some checks above could not run.".into();
                x.recommendation = format!("Install or update the {} graphics driver.", vendor.label());
                x.restart = Restart::Windows;
                out.push(x);
            }
        }
    }
    if !input.presentmon_ready {
        let mut x = finding("measure.presentmon", "Frame measurement is not set up", Severity::Notice, Confidence::High);
        x.evidence.push("PresentMon was not found".into());
        x.why = "Without frame data Arcadia cannot tell what limits a game, measure 1% lows, or benchmark a change.".into();
        x.recommendation = "Set up PresentMon from the Live Monitor page.".into();
        out.push(x);
    }

    // Most important first; ties keep rule order, so output is stable.
    out.sort_by(|a, b| b.severity.cmp(&a.severity));
    out
}

/// A plain-text report of the analysis.
pub fn report(input: &Input, findings: &[Finding]) -> String {
    let mut s = String::from("SYSTEM ANALYSIS\n\n");
    let inv = input.inv;
    s.push_str(&format!("System: {}", if inv.cpu.model.is_empty() { "unknown processor" } else { &inv.cpu.model }));
    if let Some(g) = inv.primary_gpu() {
        s.push_str(&format!(" · {}", g.name));
    }
    s.push_str(&format!(" · {:.0} GB memory\n", gb(inv.memory.total_bytes)));
    match input.frames {
        Some(f) if f.stats.avg_fps.is_some() => s.push_str(&format!("Measured: {} for {:.0} s, {} frames\n", f.process, f.stats.duration_ms / 1000.0, f.stats.frames)),
        _ => s.push_str("Measured: no frame data (monitor a game for frame-based findings)\n"),
    }
    s.push_str("\nDetected:\n");
    if findings.is_empty() {
        s.push_str("- Nothing that needs attention with the data available.\n");
    }
    for f in findings {
        s.push_str(&format!("- {} [{} confidence]\n", f.title, match f.confidence { Confidence::High => "high", Confidence::Medium => "medium", Confidence::Low => "low" }));
        for e in &f.evidence {
            s.push_str(&format!("    {e}\n"));
        }
    }
    s.push_str("\nRecommendations:\n");
    let mut any = false;
    for f in findings.iter().filter(|f| !f.recommendation.is_empty()) {
        any = true;
        s.push_str(&format!("- {}", f.recommendation));
        if f.trade_off != "None." {
            s.push_str(&format!(" Trade-off: {}", f.trade_off));
        }
        s.push('\n');
    }
    if !any {
        s.push_str("- No change recommended.\n");
    }
    s.push_str("\nEvery item above is based on a measurement or a configuration check listed with it. No FPS gain is promised; use the benchmark to measure a change.\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameforge::gpu::{GpuDevice, GpuKind};
    use crate::frameforge::hardware::{DisplayInfo, MemoryModule, PowerPlan};
    use crate::frameforge::stats;

    fn inv() -> Inventory {
        let mut i = Inventory::default();
        i.memory.total_bytes = 32 << 30;
        i.memory.modules = vec![MemoryModule::default(), MemoryModule::default()];
        i.gpus = vec![GpuDevice { vendor: Vendor::Nvidia, kind: GpuKind::Discrete, name: "RTX".into(), ..Default::default() }];
        i.displays = vec![DisplayInfo { primary: true, refresh_hz: Some(144.0), available_refresh_hz: vec![60, 144], ..Default::default() }];
        i.system.power_plans = vec![PowerPlan { guid: "381b4222-f694-41f0-9685-ff5bb260df2e".into(), name: "Balanced".into(), active: true }];
        i.system.gaming.game_mode = Some(true);
        i
    }

    fn frames(ft: f64, n: usize, cpu: f64, gpu: f64) -> FrameEvidence {
        FrameEvidence { process: "game.exe".into(), stats: stats::compute(&vec![ft; n]), cpu_busy_ms: Some(cpu), cpu_wait_ms: Some(ft - cpu), gpu_busy_ms: Some(gpu), gpu_wait_ms: Some((ft - gpu).max(0.0)), dropped_pct: Some(0.0), generated_pct: Some(0.0) }
    }

    fn run(inv: &Inventory, sum: Option<&Summary>, fr: Option<&FrameEvidence>, procs: &[ProcessInfo]) -> Vec<Finding> {
        analyze(&Input { inv, summary: sum, frames: fr, processes: procs, vendor_apis: &[], presentmon_ready: true })
    }

    fn ids(f: &[Finding]) -> Vec<&str> {
        f.iter().map(|x| x.id.as_str()).collect()
    }

    fn proc(name: &str, cat: Category, cpu: f64, mem_gb: u64) -> ProcessInfo {
        ProcessInfo { pid: 1, name: name.into(), exe: None, cpu_pct: cpu, memory_bytes: mem_gb << 30, gpu_pct: None, owner: None, category: cat, game_id: None, game_name: None, relevance: String::new(), closable: true, protected_reason: None }
    }

    #[test]
    fn healthy_system_has_no_findings() {
        let f = run(&inv(), None, None, &[]);
        assert!(f.is_empty(), "{:?}", ids(&f));
        let i = inv();
        let text = report(&Input { inv: &i, summary: None, frames: None, processes: &[], vendor_apis: &[], presentmon_ready: true }, &f);
        assert!(text.contains("Nothing that needs attention") && text.contains("No change recommended"));
    }

    #[test]
    fn gpu_bound_needs_frame_data_for_confidence() {
        // 13 ms frames (77 FPS, not on a cap), GPU busy 12.5 ms.
        let fr = frames(13.0, 2000, 6.0, 12.5);
        let sum = Summary { samples: 20, gpu_avg_pct: Some(98.0), ..Default::default() };
        let f = run(&inv(), Some(&sum), Some(&fr), &[]);
        let x = f.iter().find(|x| x.id == "limit.gpu").unwrap();
        assert_eq!(x.confidence, Confidence::High);
        assert!(x.recommendation.contains("priority will not help"));
        assert!(x.evidence.iter().any(|e| e.contains("96%")));
        // Same utilisation without frames: only a low-confidence hint.
        let f = run(&inv(), Some(&sum), None, &[]);
        assert_eq!(f.iter().find(|x| x.id == "limit.gpu_hint").unwrap().confidence, Confidence::Low);
        assert!(!ids(&f).contains(&"limit.gpu"));
    }

    #[test]
    fn cpu_bound_uses_busy_and_wait_times() {
        let fr = frames(13.0, 2000, 12.0, 6.0);
        let sum = Summary { samples: 20, gpu_avg_pct: Some(55.0), cpu_max_core_avg_pct: Some(97.0), cpu_avg_pct: Some(40.0), ..Default::default() };
        let f = run(&inv(), Some(&sum), Some(&fr), &[]);
        let x = f.iter().find(|x| x.id == "limit.cpu").unwrap();
        assert_eq!(x.confidence, Confidence::High);
        assert!(x.evidence.iter().any(|e| e.contains("waits")));
    }

    #[test]
    fn high_cpu_alone_is_not_called_a_bottleneck() {
        let sum = Summary { samples: 20, cpu_avg_pct: Some(96.0), cpu_max_core_avg_pct: Some(99.0), gpu_avg_pct: Some(40.0), ..Default::default() };
        let f = run(&inv(), Some(&sum), None, &[]);
        assert!(!ids(&f).contains(&"limit.cpu"));
        let hint = f.iter().find(|x| x.id == "limit.cpu_hint").unwrap();
        assert_eq!(hint.confidence, Confidence::Low);
        assert!(hint.why.contains("does not prove"));
    }

    #[test]
    fn a_frame_cap_is_recognised_instead_of_a_bottleneck() {
        // 60 FPS steady with the GPU busy half the frame: V-Sync or a limiter.
        let fr = frames(1000.0 / 60.0, 2000, 5.0, 8.0);
        let f = run(&inv(), None, Some(&fr), &[]);
        assert!(ids(&f).contains(&"limit.frame_cap"));
        assert!(!ids(&f).contains(&"limit.cpu") && !ids(&f).contains(&"limit.gpu"));
    }

    #[test]
    fn too_little_frame_data_runs_no_frame_rules() {
        let fr = frames(13.0, 100, 6.0, 12.5); // 1.3 s
        assert!(run(&inv(), None, Some(&fr), &[]).is_empty());
    }

    #[test]
    fn thermal_vram_and_ram_pressure() {
        let sum = Summary {
            samples: 40, thermal_limited_samples: 12, gpu_temp_max_c: Some(91.0), vram_peak_bytes: Some(11_800 << 20), vram_total_bytes: Some(12 << 30), ram_avg_pct: Some(93.0), ..Default::default()
        };
        let f = run(&inv(), Some(&sum), None, &[]);
        let i = ids(&f);
        assert!(i.contains(&"thermal.throttle") && i.contains(&"memory.vram") && i.contains(&"memory.ram"));
        assert!(!i.contains(&"thermal.hot"), "throttling supersedes the temperature note");
        assert_eq!(f[0].severity, Severity::Warning);
        // Hot without a throttle flag is only a low-confidence note.
        let warm = Summary { samples: 10, gpu_temp_max_c: Some(88.0), ..Default::default() };
        let f = run(&inv(), Some(&warm), None, &[]);
        assert_eq!(f.iter().find(|x| x.id == "thermal.hot").unwrap().confidence, Confidence::Low);
    }

    #[test]
    fn background_load_lists_the_heaviest_apps() {
        let procs = [proc("chrome.exe", Category::Browser, 12.0, 6), proc("obs64.exe", Category::Overlay, 7.0, 1), proc("game.exe", Category::Game, 60.0, 9), proc("MsMpEng.exe", Category::Protected, 30.0, 4), proc("slack.exe", Category::Chat, 0.5, 2)];
        let f = run(&inv(), None, None, &procs);
        let cpu = f.iter().find(|x| x.id == "background.cpu").unwrap();
        assert!(cpu.evidence[0].contains("20%"), "{:?}", cpu.evidence); // game and protected excluded
        assert!(cpu.evidence[1].starts_with("chrome.exe"));
        let mem = f.iter().find(|x| x.id == "background.memory").unwrap();
        assert!(mem.evidence.iter().any(|e| e.contains("Browsers: 6.0 GB")));
        assert!(ids(&f).contains(&"background.overlays"));
        assert!(!cpu.evidence.iter().any(|e| e.contains("MsMpEng")));
    }

    #[test]
    fn pacing_and_dropped_frames() {
        let mut v = vec![8.0; 1900];
        v.extend(vec![40.0; 100]);
        let fr = FrameEvidence { stats: stats::compute(&v), dropped_pct: Some(12.0), ..frames(8.0, 10, 4.0, 5.0) };
        let f = run(&inv(), None, Some(&fr), &[]);
        assert!(ids(&f).contains(&"pacing.uneven") && ids(&f).contains(&"pacing.dropped"));
    }

    #[test]
    fn configuration_checks() {
        let mut i = inv();
        i.system.power_plans[0] = PowerPlan { guid: "a1841308-3541-4fab-bc81-f71556f20b4a".into(), name: "Power saver".into(), active: true };
        i.system.gaming.game_mode = Some(false);
        i.system.gaming.background_recording = Some(true);
        i.system.gaming.on_battery = Some(true);
        i.displays[0].refresh_hz = Some(60.0);
        i.memory.modules.truncate(1);
        let apis = [(Vendor::Nvidia, ApiStatus::unavailable("NVAPI", "nvapi64.dll was not found")), (Vendor::Amd, ApiStatus::unavailable("ADLX", "no AMD driver"))];
        let f = analyze(&Input { inv: &i, summary: None, frames: None, processes: &[], vendor_apis: &apis, presentmon_ready: false });
        let got = ids(&f);
        for want in ["power.plan", "power.battery", "windows.game_mode", "windows.background_recording", "display.refresh", "memory.single_module", "driver.api", "measure.presentmon"] {
            assert!(got.contains(&want), "missing {want} in {got:?}");
        }
        // Only the vendor whose hardware is present is reported.
        assert_eq!(f.iter().filter(|x| x.id == "driver.api").count(), 1);
        let plan = f.iter().find(|x| x.id == "power.plan").unwrap();
        assert!(plan.actions.contains(&"power.plan.high_performance".to_string()));
        assert_eq!(f.iter().find(|x| x.id == "windows.game_mode").unwrap().settings_uri.as_deref(), Some("ms-settings:gaming-gamemode"));
        // Deterministic: the same input gives the same output.
        let again = analyze(&Input { inv: &i, summary: None, frames: None, processes: &[], vendor_apis: &apis, presentmon_ready: false });
        assert_eq!(ids(&again), got);
    }

    #[test]
    fn unstable_clock_needs_load_and_no_thermal_flag() {
        let s = Summary { samples: 30, gpu_avg_pct: Some(96.0), gpu_clock_min_mhz: Some(1500.0), gpu_clock_max_mhz: Some(2600.0), ..Default::default() };
        assert!(ids(&run(&inv(), Some(&s), None, &[])).contains(&"clock.unstable"));
        let idle = Summary { gpu_avg_pct: Some(20.0), ..s.clone() };
        assert!(!ids(&run(&inv(), Some(&idle), None, &[])).contains(&"clock.unstable"));
    }

    #[test]
    fn report_lists_evidence_and_makes_no_promises() {
        let fr = frames(13.0, 2000, 6.0, 12.5);
        let i = inv();
        let input = Input { inv: &i, summary: None, frames: Some(&fr), processes: &[], vendor_apis: &[], presentmon_ready: true };
        let f = analyze(&input);
        let text = report(&input, &f);
        assert!(text.starts_with("SYSTEM ANALYSIS"));
        assert!(text.contains("Detected:") && text.contains("Recommendations:"));
        assert!(text.contains("GPU busy for 96%"));
        assert!(text.contains("No FPS gain is promised"));
    }
}
