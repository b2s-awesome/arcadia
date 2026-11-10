//! NVIDIA adapter: telemetry and device facts through NVAPI.
//!
//! Public NVAPI does not expose board power draw, fan duty, or any
//! overclocking / fan / power-limit controls (those interfaces are under
//! NDA). FrameForge therefore reports them as unavailable here; power and
//! fan speed appear when the PresentMon service supplies them. NVIDIA driver
//! behaviour is changed through DRS profiles (see `frameforge::nvidia`).

use super::*;
use crate::frameforge::ffi::nvapi::{self as nv, NvApi};
use crate::frameforge::ffi::nvapi_types::*;
use crate::frameforge::ffi::cchars_to_string;
use std::sync::Arc;

struct PhysicalGpu {
    /// `NvPhysicalGpuHandle`, kept as an integer so the adapter is `Send`.
    handle: usize,
    device_id: u32,
    subsystem_id: u32,
    name: String,
}

pub struct NvidiaAdapter {
    api: Result<Arc<NvApi>, String>,
    gpus: Vec<PhysicalGpu>,
    driver: Option<(String, String)>,
}

impl NvidiaAdapter {
    pub fn load() -> Self {
        let api = nv::shared();
        let mut gpus = Vec::new();
        let mut driver = None;
        if let Ok(a) = &api {
            gpus = enumerate(a);
            driver = driver_version(a);
        }
        Self { api, gpus, driver }
    }

    /// The NVAPI handle for a DXGI device: same PCI ids, and the n-th such
    /// card maps to the n-th NVAPI GPU with those ids.
    fn handle_for(&self, device: &GpuDevice) -> Option<&PhysicalGpu> {
        let pci = (device.device_id << 16) | device.vendor_id;
        let same: Vec<&PhysicalGpu> = self.gpus.iter().filter(|g| g.device_id == pci).collect();
        match same.len() {
            0 => self.gpus.iter().find(|g| g.name == device.name),
            1 => same.into_iter().next(),
            _ => {
                let by_subsys: Vec<&PhysicalGpu> = same.iter().copied().filter(|g| g.subsystem_id == device.subsystem_id).collect();
                by_subsys.into_iter().next().or_else(|| same.into_iter().next())
            }
        }
    }
}

fn enumerate(api: &NvApi) -> Vec<PhysicalGpu> {
    let Some(enum_gpus) = api.EnumPhysicalGPUs else { return Vec::new() };
    let mut handles: [NvPhysicalGpuHandle; 64] = [std::ptr::null_mut(); 64];
    let mut count = 0u32;
    // SAFETY: array of NVAPI_MAX_PHYSICAL_GPUS (64) handles, as documented.
    if unsafe { enum_gpus(&mut handles, &mut count) } != nv::NVAPI_OK {
        return Vec::new();
    }
    handles[..(count as usize).min(64)]
        .iter()
        .map(|h| {
            let mut name: NvAPI_ShortString = [0; 64];
            if let Some(f) = api.GPU_GetFullName {
                // SAFETY: valid handle from EnumPhysicalGPUs and a 64-char buffer.
                unsafe { f(*h, &mut name) };
            }
            let (mut dev, mut sub, mut rev, mut ext) = (0u32, 0u32, 0u32, 0u32);
            if let Some(f) = api.GPU_GetPCIIdentifiers {
                // SAFETY: four valid out-pointers.
                unsafe { f(*h, &mut dev, &mut sub, &mut rev, &mut ext) };
            }
            PhysicalGpu { handle: *h as usize, device_id: dev, subsystem_id: sub, name: cchars_to_string(&name) }
        })
        .collect()
}

fn driver_version(api: &NvApi) -> Option<(String, String)> {
    let f = api.SYS_GetDriverAndBranchVersion?;
    let mut ver = 0u32;
    let mut branch: NvAPI_ShortString = [0; 64];
    // SAFETY: valid out-pointers of the documented types.
    (unsafe { f(&mut ver, &mut branch) } == nv::NVAPI_OK).then(|| (format!("{}.{:02}", ver / 100, ver % 100), cchars_to_string(&branch)))
}

impl VendorAdapter for NvidiaAdapter {
    fn vendor(&self) -> Vendor {
        Vendor::Nvidia
    }

    fn status(&self) -> ApiStatus {
        match &self.api {
            Ok(_) => ApiStatus::Available { api: "NVAPI".into(), version: self.driver.as_ref().map(|(v, _)| format!("driver {v}")) },
            Err(e) => ApiStatus::unavailable("NVAPI", e.clone()),
        }
    }

    fn enrich(&self, device: &mut GpuDevice) {
        let Ok(api) = &self.api else { return };
        if let Some((v, b)) = &self.driver {
            device.driver_version_vendor = Some(v.clone());
            device.driver_branch = Some(b.clone());
        }
        let Some(g) = self.handle_for(device) else { return };
        let h = g.handle as NvPhysicalGpuHandle;
        if let Some(f) = api.GPU_GetGPUType {
            let mut t = 0i32;
            // SAFETY: valid handle and out-pointer.
            if unsafe { f(h, &mut t) } == nv::NVAPI_OK {
                match t {
                    NV_SYSTEM_TYPE_IGPU => device.kind = GpuKind::Integrated,
                    NV_SYSTEM_TYPE_DGPU => device.kind = GpuKind::Discrete,
                    _ => {}
                }
                if t != NV_SYSTEM_TYPE_GPU_UNKNOWN {
                    device.kind_source = "vendor API".into();
                }
            }
        }
        if let Some(f) = api.GPU_GetSystemType {
            let mut t = 0i32;
            // SAFETY: valid handle and out-pointer.
            if unsafe { f(h, &mut t) } == nv::NVAPI_OK && t == NV_SYSTEM_TYPE_LAPTOP {
                device.features.push("Laptop GPU".into());
            }
        }
        if let Some(f) = api.GPU_GetVbiosVersionString {
            let mut s: NvAPI_ShortString = [0; 64];
            // SAFETY: valid handle and 64-char buffer.
            if unsafe { f(h, &mut s) } == nv::NVAPI_OK {
                device.features.push(format!("VBIOS {}", cchars_to_string(&s)));
            }
        }
        device.features.push("Driver profiles (DRS)".into());
    }

    fn telemetry(&self, device: &GpuDevice) -> GpuTelemetry {
        let mut t = GpuTelemetry::default();
        let Ok(api) = &self.api else { return t };
        let Some(g) = self.handle_for(device) else {
            t.note_missing("NVAPI did not list this adapter");
            return t;
        };
        let h = g.handle as NvPhysicalGpuHandle;
        let why = |st: nv::Status| -> String {
            match st {
                nv::NVAPI_NOT_SUPPORTED | nv::NVAPI_NO_IMPLEMENTATION => "Not supported by this GPU or driver (NVAPI)".into(),
                other => api.error_message(other),
            }
        };

        if let Some(f) = api.GPU_GetDynamicPstatesInfoEx {
            let mut info = NV_GPU_DYNAMIC_PSTATES_INFO_EX { version: nv::NV_GPU_DYNAMIC_PSTATES_INFO_EX_VER, ..Default::default() };
            // SAFETY: versioned struct of the documented size.
            let st = unsafe { f(h, &mut info) };
            let u = &info.utilization[nv::NVAPI_GPU_UTILIZATION_DOMAIN_GPU];
            if st == nv::NVAPI_OK && u.bIsPresent() != 0 {
                t.utilization_pct = Some(u.percentage as f64);
                t.set("utilization", "NVAPI");
            } else if st != nv::NVAPI_OK {
                t.unavailable.insert("utilization".into(), why(st));
            }
        }

        if let Some(f) = api.GPU_GetThermalSettings {
            let mut th = NV_GPU_THERMAL_SETTINGS_V2 { version: nv::NV_GPU_THERMAL_SETTINGS_VER, ..Default::default() };
            // SAFETY: versioned struct; 15 = NVAPI_THERMAL_TARGET_ALL.
            let st = unsafe { f(h, NVAPI_THERMAL_TARGET_ALL as u32, &mut th) };
            if st == nv::NVAPI_OK {
                let sensors = &th.sensor[..(th.count as usize).min(th.sensor.len())];
                if let Some(s) = sensors.iter().find(|s| s.target == NVAPI_THERMAL_TARGET_GPU).or(sensors.first()) {
                    t.temperature_c = Some(s.currentTemp as f64);
                    t.set("temperature", "NVAPI");
                }
            } else {
                t.unavailable.insert("temperature".into(), why(st));
            }
        }

        if let Some(f) = api.GPU_GetAllClockFrequencies {
            let mut c = NV_GPU_CLOCK_FREQUENCIES_V2 { version: nv::NV_GPU_CLOCK_FREQUENCIES_VER, ..Default::default() };
            c.set_ClockType(NV_GPU_CLOCK_FREQUENCIES_CURRENT_FREQ as u32);
            // SAFETY: versioned struct of the documented size.
            let st = unsafe { f(h, &mut c) };
            if st == nv::NVAPI_OK {
                let core = &c.domain[nv::NVAPI_GPU_PUBLIC_CLOCK_GRAPHICS];
                if core.bIsPresent() != 0 {
                    t.core_clock_mhz = Some(core.frequency as f64 / 1000.0);
                    t.set("coreClock", "NVAPI");
                }
                let mem = &c.domain[nv::NVAPI_GPU_PUBLIC_CLOCK_MEMORY];
                if mem.bIsPresent() != 0 {
                    t.memory_clock_mhz = Some(mem.frequency as f64 / 1000.0);
                    t.set("memoryClock", "NVAPI");
                }
            } else {
                t.unavailable.insert("coreClock".into(), why(st));
                t.unavailable.insert("memoryClock".into(), why(st));
            }
        }

        if let Some(f) = api.GPU_GetMemoryInfoEx {
            let mut m = NV_GPU_MEMORY_INFO_EX_V1 { version: nv::NV_GPU_MEMORY_INFO_EX_VER, ..Default::default() };
            // SAFETY: versioned struct of the documented size.
            if unsafe { f(h, &mut m) } == nv::NVAPI_OK && m.dedicatedVideoMemory > 0 {
                // NVAPI reports kilobytes.
                t.vram_total_bytes = Some(m.dedicatedVideoMemory * 1024);
                t.vram_used_bytes = Some(m.dedicatedVideoMemory.saturating_sub(m.curAvailableDedicatedVideoMemory) * 1024);
                t.set("vramUsed", "NVAPI");
            }
        }

        if let Some(f) = api.GPU_GetTachReading {
            let mut rpm = 0u32;
            // SAFETY: valid handle and out-pointer.
            let st = unsafe { f(h, &mut rpm) };
            if st == nv::NVAPI_OK {
                t.fan_rpm = Some(rpm as f64);
                t.set("fanRpm", "NVAPI");
            } else {
                t.unavailable.insert("fanRpm".into(), why(st));
            }
        }

        if let Some(f) = api.GPU_GetPerfDecreaseInfo {
            let mut bits = 0u32;
            // SAFETY: valid handle and out-pointer.
            if unsafe { f(h, &mut bits) } == nv::NVAPI_OK {
                t.thermal_limited = Some(bits & nv::NV_GPU_PERF_DECREASE_REASON_THERMAL_PROTECTION != 0);
                t.power_limited = Some(bits & nv::NV_GPU_PERF_DECREASE_REASON_POWER_CONTROL != 0);
            }
        }

        let nda = "Not part of public NVAPI; shown when the PresentMon service reports it";
        t.unavailable.entry("power".into()).or_insert_with(|| nda.into());
        t.unavailable.entry("fanPct".into()).or_insert_with(|| nda.into());
        t
    }

    fn controls(&self, _device: &GpuDevice) -> Vec<GpuControl> {
        // Honest placeholders: these exist on the card but the public API has no setter.
        let locked = |id: &str, name: &str, description: &str| GpuControl {
            id: id.into(),
            vendor: Vendor::Nvidia,
            name: name.into(),
            description: description.into(),
            category: "Tuning".into(),
            kind: ControlKind::ReadOnly,
            value: None,
            default: None,
            supported: false,
            unsupported_reason: Some("NVIDIA's public NVAPI has no interface for this. Arcadia does not use undocumented calls.".into()),
            risk: Risk::High,
            level: 3,
            requires_restart: false,
        };
        vec![
            locked("nvapi.power_limit", "Power limit", "Board power target."),
            locked("nvapi.fan", "Fan control", "Manual fan speed."),
            locked("nvapi.clock_offset", "Clock offsets", "Core and memory clock offsets."),
        ]
    }

    fn vrr(&self, device: &GpuDevice) -> BTreeMap<String, bool> {
        let mut out = BTreeMap::new();
        let Ok(api) = &self.api else { return out };
        let (Some(by_name), Some(vrr)) = (api.DISP_GetDisplayIdByDisplayName, api.Disp_GetVRRInfo) else { return out };
        for output in &device.outputs {
            let Ok(c) = std::ffi::CString::new(output.as_str()) else { continue };
            let mut id = 0u32;
            // SAFETY: NUL-terminated display name and valid out-pointer.
            if unsafe { by_name(c.as_ptr(), &mut id) } != nv::NVAPI_OK {
                continue;
            }
            let mut info = NV_GET_VRR_INFO_V1 { version: nv::NV_GET_VRR_INFO_VER, ..Default::default() };
            // SAFETY: versioned struct of the documented size.
            if unsafe { vrr(id, &mut info) } == nv::NVAPI_OK {
                out.insert(output.clone(), info.bIsVRRPossible() != 0 || info.bIsVRREnabled() != 0);
            }
        }
        out
    }
}
