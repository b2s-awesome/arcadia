//! Hardware inventory: one scan, reused by every page.
//!
//! [`scan`] is the slow, complete pass (run once at start-up and on request).
//! Live readings come from [`super::telemetry`], never from repeated scans.
//!
//! Every field that a machine might not expose is an `Option`; the UI shows
//! "unavailable" instead of a made-up value.

use serde::{Deserialize, Serialize};

use super::gpu::GpuDevice;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuInfo {
    pub manufacturer: String,
    pub model: String,
    pub architecture: String,
    pub physical_cores: Option<u32>,
    pub logical_cores: u32,
    /// Nominal frequency reported by the OS, MHz.
    pub base_frequency_mhz: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryModule {
    pub capacity_bytes: u64,
    /// "DDR4", "DDR5", … when SMBIOS reports a known type.
    pub kind: Option<String>,
    /// Configured speed in MHz (what the module actually runs at).
    pub speed_mhz: Option<u32>,
    pub manufacturer: Option<String>,
    pub part_number: Option<String>,
    pub slot: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub modules: Vec<MemoryModule>,
    /// Why `modules` is empty, when it is.
    pub modules_note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveInfo {
    pub mount: String,
    pub label: String,
    pub file_system: String,
    /// "SSD", "HDD" or `None` when Windows doesn't say.
    pub kind: Option<String>,
    /// "NVMe", "SATA", "USB", … when known.
    pub bus: Option<String>,
    pub model: Option<String>,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub removable: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    /// GDI device name, e.g. `\\.\DISPLAY1`.
    pub device: String,
    pub name: String,
    pub primary: bool,
    pub width: u32,
    pub height: u32,
    pub native_width: Option<u32>,
    pub native_height: Option<u32>,
    pub refresh_hz: Option<f64>,
    /// Refresh rates offered at the current resolution.
    pub available_refresh_hz: Vec<u32>,
    pub hdr_supported: Option<bool>,
    pub hdr_enabled: Option<bool>,
    /// Variable refresh rate (G-SYNC / FreeSync / Adaptive-Sync), when a
    /// vendor API can tell. `None` = not detectable on this setup.
    pub vrr_supported: Option<bool>,
    pub vrr_note: Option<String>,
    pub bits_per_color: Option<u32>,
    /// Name of the GPU driving this display.
    pub adapter: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerPlan {
    pub guid: String,
    pub name: String,
    pub active: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupApp {
    pub name: String,
    pub command: String,
    /// "HKCU" (this user; Arcadia can turn it off) or "HKLM" (all users; needs an administrator).
    pub scope: String,
    pub enabled: bool,
}

/// Gaming-related Windows configuration. `None` = could not be read.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamingConfig {
    pub game_mode: Option<bool>,
    /// Background recording ("Record what happened").
    pub background_recording: Option<bool>,
    /// Game Bar capture in general.
    pub capture_enabled: Option<bool>,
    /// Hardware-accelerated GPU scheduling.
    pub hags: Option<bool>,
    /// "Optimizations for windowed games".
    pub windowed_optimizations: Option<bool>,
    /// System-wide variable refresh rate switch.
    pub system_vrr: Option<bool>,
    /// Display stack supports tearing (needed for VRR in windowed modes).
    pub tearing_supported: Option<bool>,
    pub on_battery: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub os_name: String,
    pub os_version: String,
    pub os_build: Option<String>,
    pub host_name: Option<String>,
    pub is_windows: bool,
    pub is_elevated: bool,
    pub power_plans: Vec<PowerPlan>,
    pub startup_apps: Vec<StartupApp>,
    pub gaming: GamingConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    pub cpu: CpuInfo,
    pub gpus: Vec<GpuDevice>,
    pub memory: MemoryInfo,
    pub drives: Vec<DriveInfo>,
    pub displays: Vec<DisplayInfo>,
    pub system: SystemInfo,
    /// Non-fatal problems met while scanning.
    pub warnings: Vec<String>,
    /// Unix seconds.
    pub scanned_at: u64,
}

impl Inventory {
    /// The GPU games will normally use: the discrete one with the most VRAM,
    /// else the first non-software adapter.
    pub fn primary_gpu(&self) -> Option<&GpuDevice> {
        use super::gpu::GpuKind;
        self.gpus
            .iter()
            .filter(|g| g.kind == GpuKind::Discrete)
            .max_by_key(|g| g.vram_bytes.unwrap_or(0))
            .or_else(|| self.gpus.iter().find(|g| g.kind != GpuKind::Software))
    }

    pub fn active_power_plan(&self) -> Option<&PowerPlan> {
        self.system.power_plans.iter().find(|p| p.active)
    }

    /// Drive that holds `path`, by longest matching mount point.
    pub fn drive_for(&self, path: &str) -> Option<&DriveInfo> {
        let p = path.to_lowercase().replace('/', "\\");
        self.drives
            .iter()
            .filter(|d| !d.mount.is_empty() && p.starts_with(&d.mount.to_lowercase().replace('/', "\\")))
            .max_by_key(|d| d.mount.len())
    }
}

pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Full hardware scan. Never fails; anything that can't be read is left
/// empty and explained in `warnings`.
pub fn scan(gpus: &super::gpu::GpuRegistry) -> Inventory {
    let mut inv = Inventory { scanned_at: now_secs(), ..Default::default() };
    scan_portable(&mut inv);
    #[cfg(windows)]
    super::hardware_win::scan(&mut inv);
    #[cfg(not(windows))]
    inv.warnings.push("Displays, power plans and gaming settings are only read on Windows.".into());
    inv.gpus = gpus.devices();
    if inv.gpus.is_empty() {
        inv.warnings.push("No graphics adapter was found.".into());
    }
    let vrr = gpus.vrr();
    // Tie each display to the adapter that drives it, and add what the vendor says about VRR.
    for d in &mut inv.displays {
        if let Some(v) = vrr.get(&d.device) {
            d.vrr_supported = Some(*v);
            d.vrr_note = None;
        }
        if d.adapter.is_none() {
            d.adapter = inv.gpus.iter().find(|g| g.outputs.iter().any(|o| o.eq_ignore_ascii_case(&d.device))).map(|g| g.name.clone());
        }
    }
    inv
}

/// Re-read power plans, start-up apps and gaming settings (cheap; no WMI).
pub fn refresh_system(inv: &mut Inventory) {
    #[cfg(windows)]
    super::hardware_win::refresh_system(inv);
    #[cfg(not(windows))]
    let _ = inv;
}

/// CPU, memory totals and drives from `sysinfo` (works on every platform).
fn scan_portable(inv: &mut Inventory) {
    use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};
    let sys = System::new_with_specifics(
        RefreshKind::nothing().with_cpu(CpuRefreshKind::nothing().with_frequency()).with_memory(MemoryRefreshKind::nothing().with_ram()),
    );
    match sys {
        Ok(sys) => {
            let cpus = sys.cpus();
            if let Some(c) = cpus.first() {
                inv.cpu.model = c.brand().trim().to_string();
                inv.cpu.manufacturer = match c.vendor_id() {
                    "GenuineIntel" => "Intel".into(),
                    "AuthenticAMD" => "AMD".into(),
                    other => other.to_string(),
                };
                inv.cpu.base_frequency_mhz = (c.frequency() > 0).then(|| c.frequency());
            }
            inv.cpu.logical_cores = cpus.len() as u32;
            inv.cpu.physical_cores = System::physical_core_count().ok().map(|n| n as u32);
            inv.cpu.architecture = System::cpu_arch();
            inv.memory.total_bytes = sys.total_memory();
        }
        Err(e) => inv.warnings.push(format!("CPU and memory could not be read: {e}")),
    }
    inv.system.os_name = System::name().unwrap_or_else(|_| std::env::consts::OS.to_string());
    inv.system.os_version = System::os_version().unwrap_or_default();
    inv.system.os_build = System::kernel_version().ok();
    inv.system.host_name = System::host_name().ok();
    inv.system.is_windows = cfg!(windows);

    match Disks::new_with_refreshed_list() {
        Ok(disks) => {
            for d in disks.iter() {
                let kind = match d.kind() {
                    sysinfo::DiskKind::SSD => Some("SSD".to_string()),
                    sysinfo::DiskKind::HDD => Some("HDD".to_string()),
                    _ => None,
                };
                inv.drives.push(DriveInfo {
                    mount: d.mount_point().to_string_lossy().to_string(),
                    label: d.name().to_string_lossy().to_string(),
                    file_system: d.file_system().to_string_lossy().to_string(),
                    kind,
                    bus: None,
                    model: None,
                    total_bytes: d.total_space(),
                    free_bytes: d.available_space(),
                    removable: d.is_removable(),
                });
            }
        }
        Err(e) => inv.warnings.push(format!("Drives could not be listed: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameforge::gpu::{GpuDevice, GpuKind, GpuRegistry, Vendor};

    fn gpu(name: &str, kind: GpuKind, vram_gb: u64) -> GpuDevice {
        GpuDevice { name: name.into(), kind, vram_bytes: Some(vram_gb << 30), vendor: Vendor::Other, ..Default::default() }
    }

    #[test]
    fn portable_scan_reads_this_machine() {
        let inv = scan(&GpuRegistry::empty());
        assert!(inv.cpu.logical_cores >= 1);
        assert!(inv.memory.total_bytes > 0);
        assert!(inv.scanned_at > 0);
        // No adapters were supplied, and that is reported rather than hidden.
        assert!(inv.warnings.iter().any(|w| w.contains("No graphics adapter")));
    }

    #[test]
    fn primary_gpu_prefers_discrete_with_most_vram() {
        let inv = Inventory {
            gpus: vec![gpu("iGPU", GpuKind::Integrated, 0), gpu("Small", GpuKind::Discrete, 4), gpu("Big", GpuKind::Discrete, 12)],
            ..Default::default()
        };
        assert_eq!(inv.primary_gpu().unwrap().name, "Big");
        let only_igpu = Inventory { gpus: vec![gpu("Basic", GpuKind::Software, 0), gpu("iGPU", GpuKind::Integrated, 0)], ..Default::default() };
        assert_eq!(only_igpu.primary_gpu().unwrap().name, "iGPU");
        assert!(Inventory::default().primary_gpu().is_none());
    }

    #[test]
    fn drive_lookup_uses_longest_mount() {
        let inv = Inventory {
            drives: vec![
                DriveInfo { mount: "C:\\".into(), ..Default::default() },
                DriveInfo { mount: "D:\\".into(), kind: Some("SSD".into()), ..Default::default() },
            ],
            ..Default::default()
        };
        assert_eq!(inv.drive_for("d:\\SteamLibrary\\game").unwrap().kind.as_deref(), Some("SSD"));
        assert!(inv.drive_for("E:\\x").is_none());
    }
}
