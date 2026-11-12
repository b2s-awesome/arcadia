//! Windows half of the hardware scan: displays, memory modules, disk types,
//! power plans, startup programs and gaming-related settings.
//!
//! Sources, all documented Windows interfaces:
//! * displays       — `QueryDisplayConfig` / `DisplayConfigGetDeviceInfo`, `EnumDisplaySettingsW`
//! * memory modules — WMI `Win32_PhysicalMemory`
//! * disk types     — WMI `MSFT_PhysicalDisk` / `MSFT_Partition` (storage namespace)
//! * power plans    — `PowerEnumerate`, `PowerGetActiveScheme`
//! * startup items  — the `Run` registry keys
//! * tearing        — `IDXGIFactory5::CheckFeatureSupport`

use super::hardware::*;
use super::{ffi::wide_to_string, winsys};
use crate::registry::{read_string, read_u32, string_values, Hive};
use serde::Deserialize;
use windows::Win32::Devices::Display::*;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW, ENUM_DISPLAY_SETTINGS_MODE};

pub(crate) fn scan(inv: &mut Inventory) {
    windows_version(inv);
    inv.system.is_elevated = winsys::is_elevated();
    inv.system.power_plans = winsys::power_schemes().into_iter().map(|(guid, name, active)| PowerPlan { guid, name, active }).collect();
    if inv.system.power_plans.is_empty() {
        inv.warnings.push("Power plans could not be read.".into());
    }
    inv.system.startup_apps = startup_apps();
    inv.system.gaming = gaming_config();
    match displays() {
        Ok(d) => inv.displays = d,
        Err(e) => inv.warnings.push(format!("Displays could not be read: {e}")),
    }
    if let Err(e) = memory_modules(&mut inv.memory) {
        inv.memory.modules_note = Some(format!("Memory module details are unavailable: {e}"));
    }
    if let Err(e) = disk_details(&mut inv.drives) {
        inv.warnings.push(format!("Drive models and types are unavailable: {e}"));
    }
}

pub(crate) fn refresh_system(inv: &mut Inventory) {
    let plans: Vec<PowerPlan> = winsys::power_schemes().into_iter().map(|(guid, name, active)| PowerPlan { guid, name, active }).collect();
    if !plans.is_empty() {
        inv.system.power_plans = plans;
    }
    inv.system.startup_apps = startup_apps();
    inv.system.gaming = gaming_config();
}

fn windows_version(inv: &mut Inventory) {
    const KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let build = read_string(Hive::LocalMachine, KEY, "CurrentBuildNumber").unwrap_or_default();
    let ubr = read_u32(Hive::LocalMachine, KEY, "UBR");
    let display = read_string(Hive::LocalMachine, KEY, "DisplayVersion").unwrap_or_default();
    let mut product = read_string(Hive::LocalMachine, KEY, "ProductName").unwrap_or_else(|| "Windows".into());
    // The registry still says "Windows 10" on Windows 11; the build number is the documented test.
    if build.parse::<u32>().unwrap_or(0) >= 22000 {
        product = product.replace("Windows 10", "Windows 11");
    }
    inv.system.os_name = product;
    inv.system.os_version = display;
    inv.system.os_build = Some(match ubr {
        Some(u) => format!("{build}.{u}"),
        None => build,
    });
}

fn startup_apps() -> Vec<StartupApp> {
    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const RUN32: &str = r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run";
    let mut out = Vec::new();
    for (hive, scope, path) in [(Hive::CurrentUser, "HKCU", RUN), (Hive::LocalMachine, "HKLM", RUN), (Hive::LocalMachine, "HKLM", RUN32)] {
        for (name, command) in string_values(hive, path) {
            if name.is_empty() || command.trim().is_empty() {
                continue;
            }
            out.push(StartupApp { name, command, scope: scope.into(), enabled: true });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub(crate) fn gaming_config() -> GamingConfig {
    let user = |path: &str, name: &str| read_u32(Hive::CurrentUser, path, name);
    let dx_settings = read_string(Hive::CurrentUser, r"Software\Microsoft\DirectX\UserGpuPreferences", "DirectXUserGlobalSettings").unwrap_or_default();
    let dx_flag = |key: &str| -> Option<bool> {
        dx_settings.split(';').find_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            (k.trim() == key).then(|| v.trim() == "1")
        })
    };
    GamingConfig {
        // Absent means the Windows default, which is on.
        game_mode: Some(user(r"Software\Microsoft\GameBar", "AutoGameModeEnabled").map(|v| v != 0).unwrap_or(true)),
        background_recording: user(r"Software\Microsoft\Windows\CurrentVersion\GameDVR", "HistoricalCaptureEnabled").map(|v| v != 0),
        capture_enabled: user(r"Software\Microsoft\Windows\CurrentVersion\GameDVR", "AppCaptureEnabled").map(|v| v != 0),
        hags: read_u32(Hive::LocalMachine, r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers", "HwSchMode").map(|v| v == 2),
        windowed_optimizations: dx_flag("SwapEffectUpgradeEnable"),
        system_vrr: dx_flag("VRROptimizeEnable"),
        tearing_supported: tearing_supported(),
        on_battery: winsys::on_battery(),
    }
}

fn tearing_supported() -> Option<bool> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory5, DXGI_FEATURE_PRESENT_ALLOW_TEARING};
    // SAFETY: plain COM factory creation.
    let factory = unsafe { CreateDXGIFactory1::<IDXGIFactory5>() }.ok()?;
    let mut allow = windows::core::BOOL(0);
    // SAFETY: the feature data for PRESENT_ALLOW_TEARING is one BOOL.
    unsafe { factory.CheckFeatureSupport(DXGI_FEATURE_PRESENT_ALLOW_TEARING, (&mut allow as *mut windows::core::BOOL).cast(), std::mem::size_of::<windows::core::BOOL>() as u32) }.ok()?;
    Some(allow.as_bool())
}

/// Ask `DisplayConfigGetDeviceInfo` for one packet type.
macro_rules! device_info {
    ($ty:ty, $kind:expr, $adapter:expr, $id:expr) => {{
        let mut packet: $ty = unsafe { std::mem::zeroed() };
        packet.header.r#type = $kind;
        packet.header.size = std::mem::size_of::<$ty>() as u32;
        packet.header.adapterId = $adapter;
        packet.header.id = $id;
        // SAFETY: the packet starts with a correctly sized DISPLAYCONFIG_DEVICE_INFO_HEADER.
        (unsafe { DisplayConfigGetDeviceInfo(&mut packet.header) } == 0).then_some(packet)
    }};
}

fn displays() -> anyhow::Result<Vec<DisplayInfo>> {
    let (mut n_paths, mut n_modes) = (0u32, 0u32);
    // SAFETY: valid out-pointers.
    if unsafe { GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut n_paths, &mut n_modes) } != ERROR_SUCCESS {
        anyhow::bail!("GetDisplayConfigBufferSizes failed");
    }
    let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); n_paths as usize];
    let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); n_modes as usize];
    // SAFETY: buffers sized by the call above.
    if unsafe { QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut n_paths, paths.as_mut_ptr(), &mut n_modes, modes.as_mut_ptr(), None) } != ERROR_SUCCESS {
        anyhow::bail!("QueryDisplayConfig failed");
    }
    paths.truncate(n_paths as usize);
    modes.truncate(n_modes as usize);

    let mut out = Vec::new();
    for p in &paths {
        let mut d = DisplayInfo::default();
        if let Some(src) = device_info!(DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, p.sourceInfo.adapterId, p.sourceInfo.id) {
            d.device = wide_to_string(&src.viewGdiDeviceName);
        }
        if let Some(t) = device_info!(DISPLAYCONFIG_TARGET_DEVICE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME, p.targetInfo.adapterId, p.targetInfo.id) {
            d.name = wide_to_string(&t.monitorFriendlyDeviceName);
        }
        if d.name.is_empty() {
            d.name = if d.device.is_empty() { "Display".into() } else { d.device.trim_start_matches(r"\\.\").to_string() };
        }
        if let Some(pref) = device_info!(DISPLAYCONFIG_TARGET_PREFERRED_MODE, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_PREFERRED_MODE, p.targetInfo.adapterId, p.targetInfo.id) {
            d.native_width = Some(pref.width);
            d.native_height = Some(pref.height);
        }
        if let Some(c) = device_info!(DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO, DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO, p.targetInfo.adapterId, p.targetInfo.id) {
            // SAFETY: `value` is the documented raw view of the flag bit-field.
            let bits = unsafe { c.Anonymous.value };
            d.hdr_supported = Some(bits & 0x1 != 0);
            d.hdr_enabled = Some(bits & 0x2 != 0);
            d.bits_per_color = Some(c.bitsPerColorChannel);
        }
        let r = p.targetInfo.refreshRate;
        if r.Denominator != 0 {
            d.refresh_hz = Some(r.Numerator as f64 / r.Denominator as f64);
        }
        // SAFETY: for an active path the union holds a mode index.
        let idx = unsafe { p.sourceInfo.Anonymous.modeInfoIdx } as usize;
        if let Some(m) = modes.get(idx).filter(|m| m.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE) {
            // SAFETY: infoType says the union holds a source mode.
            let s = unsafe { m.Anonymous.sourceMode };
            d.width = s.width;
            d.height = s.height;
            d.primary = s.position.x == 0 && s.position.y == 0;
        }
        if !d.device.is_empty() {
            d.available_refresh_hz = refresh_rates(&d.device, d.width, d.height);
        }
        d.vrr_note = Some("Shown when the GPU vendor's API reports it".into());
        out.push(d);
    }
    Ok(out)
}

/// Refresh rates Windows offers for this display at the given resolution.
fn refresh_rates(device: &str, width: u32, height: u32) -> Vec<u32> {
    let name = windows::core::HSTRING::from(device);
    let mut rates = Vec::new();
    for i in 0..4096u32 {
        let mut dm = DEVMODEW { dmSize: std::mem::size_of::<DEVMODEW>() as u16, ..Default::default() };
        // SAFETY: dmSize is set; the name is NUL-terminated.
        if !unsafe { EnumDisplaySettingsW(&name, ENUM_DISPLAY_SETTINGS_MODE(i), &mut dm) }.as_bool() {
            break;
        }
        if dm.dmPelsWidth == width && dm.dmPelsHeight == height && dm.dmDisplayFrequency > 1 && !rates.contains(&dm.dmDisplayFrequency) {
            rates.push(dm.dmDisplayFrequency);
        }
    }
    rates.sort_unstable();
    rates
}

#[derive(Deserialize)]
#[serde(rename = "Win32_PhysicalMemory")]
#[serde(rename_all = "PascalCase")]
struct PhysicalMemory {
    capacity: Option<u64>,
    speed: Option<u32>,
    configured_clock_speed: Option<u32>,
    #[serde(rename = "SMBIOSMemoryType")]
    smbios_memory_type: Option<u32>,
    manufacturer: Option<String>,
    part_number: Option<String>,
    device_locator: Option<String>,
}

/// SMBIOS memory type codes (DMTF SMBIOS specification, "Memory Device — Type").
pub(crate) fn smbios_memory_type(code: u32) -> Option<&'static str> {
    Some(match code {
        20 => "DDR",
        21 => "DDR2",
        24 => "DDR3",
        26 => "DDR4",
        27 => "LPDDR",
        28 => "LPDDR2",
        29 => "LPDDR3",
        30 => "LPDDR4",
        34 => "DDR5",
        35 => "LPDDR5",
        _ => return None,
    })
}

fn memory_modules(mem: &mut MemoryInfo) -> anyhow::Result<()> {
    let con = wmi::WMIConnection::new()?;
    let rows: Vec<PhysicalMemory> = con.query()?;
    mem.modules = rows
        .into_iter()
        .map(|r| MemoryModule {
            capacity_bytes: r.capacity.unwrap_or(0),
            kind: r.smbios_memory_type.and_then(smbios_memory_type).map(str::to_string),
            speed_mhz: r.configured_clock_speed.or(r.speed).filter(|s| *s > 0),
            manufacturer: r.manufacturer.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
            part_number: r.part_number.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
            slot: r.device_locator,
        })
        .collect();
    if mem.modules.is_empty() {
        mem.modules_note = Some("Windows reported no memory modules (common in virtual machines).".into());
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename = "MSFT_PhysicalDisk")]
#[serde(rename_all = "PascalCase")]
struct PhysicalDisk {
    device_id: Option<String>,
    friendly_name: Option<String>,
    media_type: Option<u16>,
    bus_type: Option<u16>,
}

#[derive(Deserialize)]
#[serde(rename = "MSFT_Partition")]
#[serde(rename_all = "PascalCase")]
struct Partition {
    disk_number: Option<u32>,
    drive_letter: Option<u16>,
}

/// `MSFT_PhysicalDisk.BusType` values from the Windows Storage Management API.
pub(crate) fn bus_name(code: u16) -> Option<&'static str> {
    Some(match code {
        3 => "ATA",
        7 => "USB",
        8 => "RAID",
        10 => "SAS",
        11 => "SATA",
        12 => "SD",
        17 => "NVMe",
        _ => return None,
    })
}

fn disk_details(drives: &mut [DriveInfo]) -> anyhow::Result<()> {
    let con = wmi::WMIConnection::with_namespace_path(r"ROOT\Microsoft\Windows\Storage")?;
    let disks: Vec<PhysicalDisk> = con.query()?;
    let parts: Vec<Partition> = con.query()?;
    for d in drives.iter_mut() {
        let Some(letter) = d.mount.chars().next().filter(|c| c.is_ascii_alphabetic()) else { continue };
        let Some(part) = parts.iter().find(|p| p.drive_letter.map(|l| (l as u8 as char).eq_ignore_ascii_case(&letter)).unwrap_or(false)) else { continue };
        let Some(number) = part.disk_number else { continue };
        let Some(disk) = disks.iter().find(|k| k.device_id.as_deref() == Some(number.to_string().as_str())) else { continue };
        d.model = disk.friendly_name.clone();
        d.bus = disk.bus_type.and_then(bus_name).map(str::to_string);
        // MediaType: 3 = HDD, 4 = SSD, 5 = storage-class memory. 0 = unspecified, left as-is.
        match disk.media_type {
            Some(3) => d.kind = Some("HDD".into()),
            Some(4) | Some(5) => d.kind = Some("SSD".into()),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_tables() {
        assert_eq!(smbios_memory_type(34), Some("DDR5"));
        assert_eq!(smbios_memory_type(0), None);
        assert_eq!(bus_name(17), Some("NVMe"));
        assert_eq!(bus_name(999), None);
    }
}
