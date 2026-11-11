//! The FrameForge facade: one object the desktop shell owns and calls.
//!
//! It holds the pieces that must be shared (GPU registry, telemetry sampler,
//! cached inventory, the running monitor) and keeps the expensive work
//! separate: a hardware *scan* happens once and on request, *telemetry* is a
//! cheap pull, *monitoring* only exists while switched on.

use super::config::Config;
use super::gpu::{ApiStatus, GpuRegistry, Vendor};
use super::hardware::{self, Inventory};
use super::monitor::{Monitor, Snapshot, State};
use super::presentmon::{self, Availability};
use super::processes::{GameIndex, ProcessInfo, ProcessLister};
use super::telemetry::{Sample, Sampler};
use crate::library::Game;
use parking_lot::Mutex;
use serde::Serialize;
use std::sync::Arc;

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
}

impl FrameForge {
    /// Detect real hardware. Cheap enough to run at start-up: adapters are
    /// listed once; nothing is polled until someone asks.
    pub fn new() -> Self {
        Self::with_registry(GpuRegistry::detect())
    }

    pub fn with_registry(gpus: GpuRegistry) -> Self {
        let _ = std::fs::create_dir_all(super::data_dir());
        Self {
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
        self.sampler.sample()
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
