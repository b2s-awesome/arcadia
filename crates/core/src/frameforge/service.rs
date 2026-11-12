//! The FrameForge facade: one object the desktop shell owns and calls.
//!
//! It holds the pieces that must be shared (GPU registry, telemetry sampler,
//! cached inventory, the running monitor) and keeps the expensive work
//! separate: a hardware *scan* happens once and on request, *telemetry* is a
//! cheap pull, *monitoring* only exists while switched on.

use super::config::Config;
use super::diagnostics::{self, Finding, FrameEvidence};
use super::gpu::{ApiStatus, GpuRegistry, Vendor};
use super::hardware::{self, Inventory};
use super::monitor::{Monitor, Snapshot, State};
use super::optimizer::ops::{SystemOps, WindowsOps};
use super::optimizer::{ActionView, ApplyRequest, Ctx, Engine, Session};
use super::presentmon::{self, Availability};
use super::processes::{GameIndex, ProcessInfo, ProcessLister, ProcessOps, SystemProcessOps};
use super::telemetry::{self, Sample, Sampler};
use crate::library::Game;
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Arc;

/// Telemetry samples kept for diagnostics when no game is being monitored.
const RECENT_SAMPLES: usize = 30;

/// Windows Settings pages FrameForge may open. Nothing else is accepted.
pub const SETTINGS_PAGES: &[&str] = &[
    "ms-settings:gaming-gamemode",
    "ms-settings:gaming-gamedvr",
    "ms-settings:gaming-gamebar",
    "ms-settings:display",
    "ms-settings:display-advanced",
    "ms-settings:display-advancedgraphics",
    "ms-settings:storagesense",
    "ms-settings:powersleep",
    "ms-settings:startupapps",
];

/// Result of one diagnostic run.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnosis {
    pub generated_at: u64,
    pub findings: Vec<Finding>,
    /// Plain-text report of the same findings.
    pub report: String,
    /// Frame measurements the findings rest on, when a game was monitored.
    pub frames: Option<FrameEvidence>,
    /// What data the run had, in plain words.
    pub basis: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VendorApi {
    pub vendor: Vendor,
    pub api: String,
    pub status: ApiStatus,
    /// Is hardware from this vendor present?
    pub hardware_present: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub inventory: Inventory,
    pub vendor_apis: Vec<VendorApi>,
    pub presentmon: Availability,
}

pub struct FrameForge {
    gpus: GpuRegistry,
    sampler: Arc<Sampler>,
    inventory: Mutex<Option<Inventory>>,
    config: Mutex<Config>,
    monitor: Mutex<Option<Monitor>>,
    lister: Mutex<Option<ProcessLister>>,
    games: Mutex<GameIndex>,
    recent: Mutex<VecDeque<Sample>>,
    engine: Engine,
    ops: Arc<dyn SystemOps>,
    procs: Arc<dyn ProcessOps>,
}

impl FrameForge {
    /// Detect real hardware. Cheap enough to run at start-up: adapters are
    /// listed once; nothing is polled until someone asks.
    pub fn new() -> Self {
        Self::with_registry(GpuRegistry::detect())
    }

    pub fn with_registry(gpus: GpuRegistry) -> Self {
        Self::with_parts(gpus, Arc::new(WindowsOps), Arc::new(SystemProcessOps))
    }

    /// Build with explicit system backends (tests use in-memory ones).
    pub fn with_parts(gpus: GpuRegistry, ops: Arc<dyn SystemOps>, procs: Arc<dyn ProcessOps>) -> Self {
        let _ = std::fs::create_dir_all(super::data_dir());
        Self {
            recent: Mutex::new(VecDeque::new()),
            engine: Engine::new(super::history_dir()),
            ops,
            procs,
            sampler: Arc::new(Sampler::new(gpus.clone())),
            gpus,
            inventory: Mutex::new(None),
            config: Mutex::new(Config::load()),
            monitor: Mutex::new(None),
            lister: Mutex::new(ProcessLister::new().ok()),
            games: Mutex::new(GameIndex::default()),
        }
    }

    pub fn gpus(&self) -> &GpuRegistry {
        &self.gpus
    }

    pub fn sampler(&self) -> Arc<Sampler> {
        self.sampler.clone()
    }

    pub fn config(&self) -> Config {
        self.config.lock().clone()
    }

    pub fn set_config(&self, config: Config) -> anyhow::Result<()> {
        if let Some(p) = config.presentmon_cli.as_deref() {
            presentmon::cli::validate_exe(std::path::Path::new(p))?;
        }
        config.save()?;
        // Re-read through the sanitiser so the in-memory copy matches the file.
        *self.config.lock() = Config::load();
        Ok(())
    }

    /// Cached hardware inventory; `rescan` forces a fresh scan.
    pub fn inventory(&self, rescan: bool) -> Inventory {
        let mut g = self.inventory.lock();
        if rescan || g.is_none() {
            if rescan {
                self.sampler.refresh_devices();
            }
            *g = Some(hardware::scan(&self.gpus));
        }
        g.clone().expect("inventory was just filled")
    }

    pub fn overview(&self, rescan: bool) -> Overview {
        let inventory = self.inventory(rescan);
        let vendor_apis = self
            .gpus
            .api_statuses()
            .into_iter()
            .map(|(vendor, status)| VendorApi { vendor, api: vendor.api_name().into(), status, hardware_present: inventory.gpus.iter().any(|g| g.vendor == vendor) })
            .collect();
        Overview { inventory, vendor_apis, presentmon: self.presentmon() }
    }

    pub fn presentmon(&self) -> Availability {
        presentmon::availability(self.config.lock().presentmon_cli.as_deref())
    }

    pub fn telemetry(&self) -> Sample {
        let s = self.sampler.sample();
        let mut r = self.recent.lock();
        r.push_back(s.clone());
        while r.len() > RECENT_SAMPLES {
            r.pop_front();
        }
        s
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Re-read the parts of the inventory that optimiser actions change.
    fn refresh_system(&self) {
        if let Some(inv) = self.inventory.lock().as_mut() {
            hardware::refresh_system(inv);
        }
    }

    /// Run `f` with everything an optimiser action may use.
    pub fn with_ctx<T>(&self, with_processes: bool, f: impl FnOnce(&Ctx) -> T) -> T {
        let inv = self.inventory(false);
        let processes = if with_processes { self.processes() } else { Vec::new() };
        f(&Ctx { ops: &*self.ops, procs: &*self.procs, gpus: &self.gpus, inv: &inv, processes: &processes })
    }

    /// Analyse the system. Uses the monitored game's frame data when there
    /// is some, otherwise configuration checks and recent hardware readings.
    pub fn diagnose(&self) -> Diagnosis {
        let inv = self.inventory(false);
        let processes = self.processes();
        let gpu_id = inv.primary_gpu().map(|g| g.id.clone());
        let monitored = self.with_monitor(|m| (m.snapshot(), m.telemetry_history()));
        let (frames, samples) = match monitored {
            Some((snap, history)) if snap.total_frames > 0 => (
                Some(FrameEvidence {
                    process: snap.process.clone(),
                    stats: snap.presented.clone(),
                    cpu_busy_ms: snap.cpu_busy_ms,
                    cpu_wait_ms: snap.cpu_wait_ms,
                    gpu_busy_ms: snap.gpu_busy_ms,
                    gpu_wait_ms: snap.gpu_wait_ms,
                    dropped_pct: snap.dropped_pct,
                    generated_pct: snap.generated_pct,
                }),
                history,
            ),
            _ => {
                self.telemetry();
                (None, self.recent.lock().iter().cloned().collect())
            }
        };
        let summary = (!samples.is_empty()).then(|| telemetry::summarize(&samples, gpu_id.as_deref()));
        let vendor_apis = self.gpus.api_statuses();
        let input = diagnostics::Input {
            inv: &inv,
            summary: summary.as_ref(),
            frames: frames.as_ref(),
            processes: &processes,
            vendor_apis: &vendor_apis,
            presentmon_ready: self.presentmon().will_use.is_some(),
        };
        let findings = diagnostics::analyze(&input);
        let report = diagnostics::report(&input, &findings);
        let basis = match &frames {
            Some(f) if f.stats.duration_ms >= diagnostics::MIN_SECONDS * 1000.0 => {
                format!("{:.0} seconds of frame data from {}, {} hardware readings and the current Windows configuration.", f.stats.duration_ms / 1000.0, f.process, samples.len())
            }
            Some(f) => format!(
                "Only {:.0} seconds of frame data from {} so far; frame-based checks need {:.0}. Configuration checks and {} hardware readings were used.",
                f.stats.duration_ms / 1000.0,
                f.process,
                diagnostics::MIN_SECONDS,
                samples.len()
            ),
            None => format!("No game is being monitored, so there is no frame data. Configuration checks and {} hardware readings were used.", samples.len()),
        };
        Diagnosis { generated_at: hardware::now_secs(), findings, report, frames, basis }
    }

    pub fn optimizer_list(&self) -> Vec<ActionView> {
        self.with_ctx(false, |ctx| self.engine.list(ctx))
    }

    /// Apply actions as one recorded session. `approved_level` is the level
    /// the user confirmed in the dialog; level 3 also needs the expert
    /// acknowledgement stored in the config.
    pub fn optimizer_apply(&self, requests: &[ApplyRequest], approved_level: u8, label: &str) -> anyhow::Result<Session> {
        let ceiling = if self.config.lock().expert_acknowledged { 3 } else { 2 };
        let level = approved_level.clamp(1, 3);
        if level > ceiling {
            anyhow::bail!("Expert actions are locked. Turn on Expert mode in the Optimizer first.");
        }
        let needs_processes = requests.iter().any(|r| r.id.starts_with("process."));
        if needs_processes {
            self.settle_exited_processes();
        }
        let session = self.with_ctx(needs_processes, |ctx| self.engine.apply(ctx, requests, level, label))?;
        self.refresh_system();
        Ok(session)
    }

    /// Close the record of temporary process changes whose process has
    /// exited, so a reused process id is never mistaken for the old one.
    pub fn settle_exited_processes(&self) {
        for (sid, c) in self.engine.active_changes() {
            let pid = c.params.get("pid").and_then(|p| p.as_u64()).and_then(|p| u32::try_from(p).ok());
            if let (true, Some(pid)) = (c.temporary, pid) {
                if !self.procs.is_running(pid) {
                    let _ = self.with_ctx(false, |ctx| self.engine.revert_change(ctx, &sid, c.id));
                }
            }
        }
    }

    pub fn optimizer_revert_change(&self, session_id: &str, change_id: u32) -> anyhow::Result<Session> {
        let r = self.with_ctx(false, |ctx| self.engine.revert_change(ctx, session_id, change_id));
        self.refresh_system();
        r
    }

    pub fn optimizer_revert_session(&self, session_id: &str) -> anyhow::Result<Session> {
        let r = self.with_ctx(false, |ctx| self.engine.revert_session(ctx, session_id));
        self.refresh_system();
        r
    }

    pub fn optimizer_history(&self) -> Vec<Session> {
        self.engine.history()
    }

    /// Open one of the allowed Windows Settings pages.
    pub fn open_settings_page(&self, uri: &str) -> anyhow::Result<()> {
        if !SETTINGS_PAGES.contains(&uri) {
            anyhow::bail!("Arcadia does not open that settings page");
        }
        #[cfg(windows)]
        {
            super::winsys::open_uri(uri)
        }
        #[cfg(not(windows))]
        {
            anyhow::bail!("Windows Settings only exists on Windows")
        }
    }

    /// Tell FrameForge which games the library knows, so processes can be matched to them.
    pub fn set_games(&self, games: &[Game]) {
        *self.games.lock() = GameIndex::new(games);
    }

    pub fn game_index(&self) -> GameIndex {
        self.games.lock().clone()
    }

    pub fn processes(&self) -> Vec<ProcessInfo> {
        let gpu = self.gpus.process_usage();
        let games = self.games.lock().clone();
        match self.lister.lock().as_mut() {
            Some(l) => l.list(&games, &gpu),
            None => Vec::new(),
        }
    }

    pub fn is_running(&self, pid: u32) -> bool {
        self.lister.lock().as_mut().map(|l| l.is_running(pid)).unwrap_or(false)
    }

    /// Start monitoring a process. Any earlier session is stopped first.
    pub fn monitor_start(&self, pid: u32, name: String) -> anyhow::Result<()> {
        if !self.is_running(pid) {
            anyhow::bail!("That process is no longer running");
        }
        let cli = self.config.lock().presentmon_cli.clone();
        let primary = self.inventory(false).primary_gpu().cloned();
        let source = presentmon::open(pid, cli.as_deref(), primary.as_ref().and_then(|g| g.luid.as_deref()))?;
        let monitor = Monitor::start(source, pid, name, Some(self.sampler.clone()), primary.map(|g| g.id));
        let mut slot = self.monitor.lock();
        if let Some(mut old) = slot.take() {
            old.stop();
        }
        *slot = Some(monitor);
        Ok(())
    }

    /// Start monitoring with a caller-supplied source (tests, CSV replay).
    pub fn monitor_start_with(&self, source: Box<dyn presentmon::FrameSource>, pid: u32, name: String) {
        let gpu = self.inventory(false).primary_gpu().map(|g| g.id.clone());
        let monitor = Monitor::start(source, pid, name, Some(self.sampler.clone()), gpu);
        let mut slot = self.monitor.lock();
        if let Some(mut old) = slot.take() {
            old.stop();
        }
        *slot = Some(monitor);
    }

    pub fn monitor_stop(&self) {
        if let Some(mut m) = self.monitor.lock().take() {
            m.stop();
        }
    }

    pub fn monitor_snapshot(&self) -> Snapshot {
        match self.monitor.lock().as_ref() {
            Some(m) => m.snapshot(),
            None => Snapshot { state: State::Idle, ..Default::default() },
        }
    }

    /// Run `f` with the active monitor, if there is one.
    pub fn with_monitor<T>(&self, f: impl FnOnce(&Monitor) -> T) -> Option<T> {
        self.monitor.lock().as_ref().map(f)
    }
}

impl Default for FrameForge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameforge::gpu::mock::{MockBase, MockVendor};
    use crate::frameforge::gpu::GpuDevice;
    use crate::frameforge::presentmon::{Frame, ReplaySource};

    pub(crate) fn test_registry() -> GpuRegistry {
        let dev = GpuDevice { id: "gpu0".into(), vendor: Vendor::Nvidia, name: "Test GPU".into(), vram_bytes: Some(8 << 30), ..Default::default() };
        GpuRegistry::new(Arc::new(MockBase::new(vec![dev])), vec![Arc::new(MockVendor::available(Vendor::Nvidia)), Arc::new(MockVendor::missing(Vendor::Amd, "not installed"))])
    }

    #[test]
    fn overview_lists_each_vendor_api_independently() {
        let (_g, _home) = crate::minecraft::test_home();
        let ff = FrameForge::with_registry(test_registry());
        let o = ff.overview(false);
        assert_eq!(o.inventory.gpus.len(), 1);
        let nv = o.vendor_apis.iter().find(|v| v.vendor == Vendor::Nvidia).unwrap();
        let amd = o.vendor_apis.iter().find(|v| v.vendor == Vendor::Amd).unwrap();
        assert!(nv.status.is_available() && nv.hardware_present);
        assert!(!amd.status.is_available() && !amd.hardware_present);
        // The scan is cached: a second call returns the same timestamp.
        assert_eq!(ff.inventory(false).scanned_at, o.inventory.scanned_at);
    }

    #[test]
    fn monitor_lifecycle_through_the_facade() {
        let (_g, _home) = crate::minecraft::test_home();
        let ff = FrameForge::with_registry(test_registry());
        assert_eq!(ff.monitor_snapshot().state, State::Idle);
        assert!(ff.monitor_start(u32::MAX - 3, "ghost.exe".into()).unwrap_err().to_string().contains("no longer running"));
        let frames: Vec<Frame> = (0..400).map(|_| Frame { frame_time_ms: 8.0, displayed_time_ms: Some(8.0), ..Default::default() }).collect();
        ff.monitor_start_with(Box::new(ReplaySource::new(vec![frames])), 5, "game.exe".into());
        let mut s = ff.monitor_snapshot();
        for _ in 0..300 {
            if s.state == State::Ended {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            s = ff.monitor_snapshot();
        }
        assert_eq!(s.total_frames, 400);
        assert!((s.presented.avg_fps.unwrap() - 125.0).abs() < 1e-6);
        ff.monitor_stop();
        assert_eq!(ff.monitor_snapshot().state, State::Idle);
    }

    #[test]
    fn diagnose_without_a_game_says_what_it_used() {
        let (_g, _home) = crate::minecraft::test_home();
        let ff = FrameForge::with_registry(test_registry());
        let d = ff.diagnose();
        assert!(d.frames.is_none());
        assert!(d.basis.contains("No game is being monitored"));
        assert!(d.report.starts_with("SYSTEM ANALYSIS"));
        // Deterministic: the same state gives the same finding ids.
        let again: Vec<String> = ff.diagnose().findings.into_iter().map(|f| f.id).collect();
        assert_eq!(d.findings.iter().map(|f| f.id.clone()).collect::<Vec<_>>(), again);
    }

    #[test]
    fn diagnose_uses_the_monitored_game() {
        let (_g, _home) = crate::minecraft::test_home();
        let ff = FrameForge::with_registry(test_registry());
        // 20 s at 10 ms per frame with the GPU busy 9.6 ms of each: GPU-bound.
        let frames: Vec<Frame> = (0..2000).map(|_| Frame { frame_time_ms: 10.0, displayed_time_ms: Some(10.0), gpu_busy_ms: Some(9.6), cpu_busy_ms: Some(3.0), ..Default::default() }).collect();
        ff.monitor_start_with(Box::new(ReplaySource::new(vec![frames])), 5, "game.exe".into());
        for _ in 0..300 {
            if ff.monitor_snapshot().state == State::Ended {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let d = ff.diagnose();
        assert_eq!(d.frames.as_ref().unwrap().process, "game.exe");
        assert!(d.findings.iter().any(|f| f.id == "limit.gpu"), "{:?}", d.findings.iter().map(|f| &f.id).collect::<Vec<_>>());
        assert!(d.basis.contains("game.exe"));
    }

    #[test]
    fn optimizer_through_the_facade_applies_reverts_and_gates_expert() {
        use crate::frameforge::optimizer::ops::{MockOps, MockProcs};
        use crate::frameforge::optimizer::{ChangeStatus, SessionStatus};
        let (_g, _home) = crate::minecraft::test_home();
        let ops = Arc::new(MockOps::typical());
        let ff = FrameForge::with_parts(test_registry(), ops.clone(), Arc::new(MockProcs::with(&[10])));
        // The portable scan marks non-Windows hosts; power actions only need plans from `ops`.
        let req = |id: &str| ApplyRequest { id: id.into(), params: serde_json::Value::Null };
        assert!(ff.optimizer_apply(&[req("power.plan.high_performance")], 3, "x").unwrap_err().to_string().contains("Expert"));
        let s = ff.optimizer_apply(&[req("power.plan.high_performance")], 1, "Safe pass").unwrap();
        assert_eq!(s.status, SessionStatus::Applied);
        assert_eq!(ff.optimizer_history().len(), 1);
        assert!(ff.optimizer_list().iter().any(|v| v.meta.id == "power.plan.high_performance" && v.state == "applied"));
        let s = ff.optimizer_revert_session(&s.id).unwrap();
        assert_eq!(s.changes[0].status, ChangeStatus::Reverted);
        assert!(ops.power_plans().iter().any(|p| p.active && p.name == "Balanced"));
        assert!(ff.open_settings_page("ms-settings:../cmd").is_err());
    }

    #[test]
    fn config_round_trip_and_cli_validation() {
        let (_g, _home) = crate::minecraft::test_home();
        let ff = FrameForge::with_registry(test_registry());
        let mut c = ff.config();
        c.auto_profiles = true;
        c.monitor_tiles = vec!["fps".into(), "nonsense".into()];
        ff.set_config(c).unwrap();
        assert!(ff.config().auto_profiles);
        assert_eq!(ff.config().monitor_tiles, vec!["fps"]);
        let mut bad = ff.config();
        bad.presentmon_cli = Some("relative\\evil.exe".into());
        assert!(ff.set_config(bad).is_err());
        assert!(ff.config().presentmon_cli.is_none());
    }
}
