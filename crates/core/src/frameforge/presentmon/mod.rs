//! Frame data from Intel PresentMon.
//!
//! Two ways in, tried in this order:
//!
//! 1. **PresentMon Service** (2.3.1 or newer). The installer registers
//!    `PresentMonAPI2.dll`; Arcadia loads that copy at run time (never its
//!    own), checks the API version, and asks the service which metrics the
//!    machine supports before querying them.
//! 2. **PresentMon console application**, if the user points Arcadia at the
//!    executable. Arcadia starts it with fixed, documented arguments and
//!    parses the CSV it writes to stdout.
//!
//! When neither is present, monitoring is reported as unavailable with
//! set-up steps. FPS is never derived from GPU utilisation or anything else.

#[cfg(windows)]
pub mod api;
pub mod cli;
pub mod csv;

use serde::{Deserialize, Serialize};

pub const RELEASES_URL: &str = "https://github.com/GameTechDev/PresentMon/releases";

/// One presented frame. Times are milliseconds.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    /// Start of the frame relative to the start of the capture.
    pub time_ms: f64,
    /// CPU frame time (start of this frame to start of the next): the app's own cadence.
    pub frame_time_ms: f64,
    /// How long the frame stayed on screen; `None` if it was never displayed.
    pub displayed_time_ms: Option<f64>,
    pub cpu_busy_ms: Option<f64>,
    pub cpu_wait_ms: Option<f64>,
    pub gpu_busy_ms: Option<f64>,
    pub gpu_wait_ms: Option<f64>,
    pub gpu_time_ms: Option<f64>,
    pub display_latency_ms: Option<f64>,
    /// Presented but never shown.
    pub dropped: bool,
    /// Produced by frame generation (driver or SDK), not rendered by the game.
    pub generated: bool,
    pub allows_tearing: Option<bool>,
}

/// Hardware readings the PresentMon service measures alongside frames.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PmTelemetry {
    pub gpu_power_w: Option<f64>,
    pub gpu_temperature_c: Option<f64>,
    pub gpu_utilization_pct: Option<f64>,
    pub gpu_fan_rpm: Option<f64>,
    pub gpu_frequency_mhz: Option<f64>,
    pub gpu_mem_used_bytes: Option<f64>,
    pub gpu_mem_size_bytes: Option<f64>,
    pub cpu_utilization_pct: Option<f64>,
    pub cpu_temperature_c: Option<f64>,
    pub cpu_power_w: Option<f64>,
    pub cpu_frequency_mhz: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    /// PresentMon Service through PresentMonAPI2.
    Service,
    /// PresentMon console application, CSV on stdout.
    Cli,
    /// Recorded frames (tests, CSV import).
    Replay,
}

/// Something that yields frames for one process.
pub trait FrameSource: Send {
    fn kind(&self) -> SourceKind;
    /// Frames that arrived since the last call. An error ends the session.
    fn poll(&mut self) -> anyhow::Result<Vec<Frame>>;
    /// Service telemetry, when the source has any.
    fn telemetry(&mut self) -> Option<PmTelemetry> {
        None
    }
    /// False once the target process or the capture tool has gone away.
    fn alive(&self) -> bool {
        true
    }
    /// Human-readable detail, e.g. the service version.
    fn detail(&self) -> String {
        String::new()
    }
}

/// Frames handed over up front; used by tests and CSV replays.
pub struct ReplaySource {
    batches: std::collections::VecDeque<Vec<Frame>>,
}

impl ReplaySource {
    pub fn new(batches: Vec<Vec<Frame>>) -> Self {
        Self { batches: batches.into() }
    }
}

impl FrameSource for ReplaySource {
    fn kind(&self) -> SourceKind {
        SourceKind::Replay
    }
    fn poll(&mut self) -> anyhow::Result<Vec<Frame>> {
        Ok(self.batches.pop_front().unwrap_or_default())
    }
    fn alive(&self) -> bool {
        !self.batches.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Availability {
    /// The service API can be used right now.
    pub service_ready: bool,
    /// Why not, or the version when it is.
    pub service_detail: String,
    /// A console executable is configured and passes validation.
    pub cli_ready: bool,
    pub cli_detail: String,
    /// Which source monitoring will use, if any.
    pub will_use: Option<SourceKind>,
    /// Steps to show when nothing is available.
    pub setup_steps: Vec<String>,
    pub download_url: String,
}

/// What can be used on this machine. Cheap; safe to call from the UI.
pub fn availability(cli_path: Option<&str>) -> Availability {
    let mut a = Availability { download_url: RELEASES_URL.into(), ..Default::default() };

    #[cfg(windows)]
    match api::probe() {
        Ok(version) => {
            a.service_ready = true;
            a.service_detail = format!("PresentMon Service, API {version}");
        }
        Err(e) => a.service_detail = e,
    }
    #[cfg(not(windows))]
    {
        a.service_detail = "PresentMon only runs on Windows".into();
    }

    match cli_path.filter(|p| !p.trim().is_empty()) {
        Some(p) => match cli::validate_exe(std::path::Path::new(p)) {
            Ok(_) => {
                a.cli_ready = cfg!(windows);
                a.cli_detail = if cfg!(windows) { format!("Console application at {p}") } else { "PresentMon only runs on Windows".into() };
            }
            Err(e) => a.cli_detail = e.to_string(),
        },
        None => a.cli_detail = "No PresentMon console application has been chosen".into(),
    }

    a.will_use = if a.service_ready {
        Some(SourceKind::Service)
    } else if a.cli_ready {
        Some(SourceKind::Cli)
    } else {
        None
    };
    if a.will_use.is_none() {
        a.setup_steps = vec![
            "Download the PresentMon installer (PresentMon 2.3.1 or newer) from Intel's GitHub releases page.".into(),
            "Run the installer. It adds the PresentMon Service, which measures frames for any game.".into(),
            "Come back here and press Check again. Arcadia finds the service by itself.".into(),
            "Alternative: download the single-file console application from the same page and choose it below. It needs administrator rights, or your account in the Windows \"Performance Log Users\" group.".into(),
        ];
    }
    a
}

/// Open the best available source for a process.
pub fn open(pid: u32, cli_path: Option<&str>, preferred_gpu_luid: Option<&str>) -> anyhow::Result<Box<dyn FrameSource>> {
    let _ = preferred_gpu_luid;
    #[cfg(windows)]
    {
        let service_error = match api::ServiceSource::open(pid, preferred_gpu_luid) {
            Ok(s) => return Ok(Box::new(s)),
            Err(e) => e,
        };
        if let Some(p) = cli_path.filter(|p| !p.trim().is_empty()) {
            return Ok(Box::new(cli::CliSource::spawn(std::path::Path::new(p), pid)?));
        }
        anyhow::bail!("PresentMon is not available: {service_error}");
    }
    #[cfg(not(windows))]
    {
        let _ = (pid, cli_path);
        anyhow::bail!("PresentMon only runs on Windows")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_everywhere_gives_setup_steps() {
        let a = availability(None);
        if !a.service_ready {
            assert!(a.will_use.is_none());
            assert!(a.setup_steps.len() >= 3);
            assert!(!a.service_detail.is_empty() && !a.cli_detail.is_empty());
            assert_eq!(a.download_url, RELEASES_URL);
        }
    }

    #[test]
    fn a_bad_cli_path_is_explained() {
        // An absolute path on whichever system runs the test ("/x" has no drive on Windows).
        let missing = std::env::temp_dir().join("arcadia-definitely-not-here").join("PresentMon.exe");
        let a = availability(Some(missing.to_str().unwrap()));
        assert!(!a.cli_ready);
        assert!(a.cli_detail.contains("not found"), "{}", a.cli_detail);
        // A relative path is refused with its own explanation.
        let a = availability(Some("PresentMon.exe"));
        assert!(!a.cli_ready && a.cli_detail.contains("full path"), "{}", a.cli_detail);
    }

    #[test]
    fn replay_source_drains() {
        let mut s = ReplaySource::new(vec![vec![Frame::default()], vec![Frame::default(), Frame::default()]]);
        assert!(s.alive());
        assert_eq!(s.poll().unwrap().len(), 1);
        assert_eq!(s.poll().unwrap().len(), 2);
        assert!(!s.alive());
        assert!(s.poll().unwrap().is_empty());
    }
}
