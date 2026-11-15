//! Intel Graphics Control Library, loaded from the Intel graphics driver
//! (`ControlLib.dll`). Functions are resolved by name; one the installed
//! driver lacks is reported as unsupported instead of being called.
//!
//! IGCL telemetry reports *counters* (energy in joules, activity in
//! seconds). Power and utilisation are the change between two samples
//! divided by the time between them, so the first reading after start-up has
//! neither.

use super::igcl_types::*;
use crate::frameforge::gpu::{GpuControl, GpuDevice, GpuKind, GpuTelemetry};
use parking_lot::Mutex;
use std::collections::HashMap;

const CTL_OK: ctl_result_t = 0;

#[derive(Clone, Copy)]
struct Counters {
    time_s: f64,
    energy_j: Option<f64>,
    activity_s: Option<f64>,
}

pub(crate) struct Device {
    pub handle: usize,
    pub luid: Option<String>,
    pub pci_device_id: u32,
    pub name: String,
    pub integrated: bool,
    pub driver_version: u64,
}

pub struct Igcl {
    pub(crate) lib: IgclLib,
    api: usize,
    pub(crate) devices: Vec<Device>,
    version: Option<String>,
    last: Mutex<HashMap<usize, Counters>>,
}

// SAFETY: handles are opaque tokens owned by the driver library; IGCL calls
// are made one at a time behind the registry / adapter, and the counters map
// has its own lock.
unsafe impl Send for Igcl {}
unsafe impl Sync for Igcl {}

/// Read one telemetry item as f64, only when the driver marks it supported.
pub(crate) fn item(i: &ctl_oc_telemetry_item_t) -> Option<f64> {
    if !i.bSupported {
        return None;
    }
    // SAFETY: the union member read is the one named by `type_`.
    let v = unsafe {
        match i.type_ {
            CTL_DATA_TYPE_INT8 => i.value.data8 as f64,
            CTL_DATA_TYPE_UINT8 => i.value.datau8 as f64,
            CTL_DATA_TYPE_INT16 => i.value.data16 as f64,
            CTL_DATA_TYPE_UINT16 => i.value.datau16 as f64,
            CTL_DATA_TYPE_INT32 => i.value.data32 as f64,
            CTL_DATA_TYPE_UINT32 => i.value.datau32 as f64,
            CTL_DATA_TYPE_INT64 => i.value.data64 as f64,
            CTL_DATA_TYPE_UINT64 => i.value.datau64 as f64,
            CTL_DATA_TYPE_FLOAT => i.value.datafloat as f64,
            CTL_DATA_TYPE_DOUBLE => i.value.datadouble,
            _ => return None,
        }
    };
    v.is_finite().then_some(v)
}

impl Igcl {
    pub fn load() -> Result<Self, String> {
        // SAFETY: loading the driver's own DLL by bare name from the system search path.
        let lib = unsafe { IgclLib::new("ControlLib.dll") }
            .map_err(|_| "ControlLib.dll was not found. It ships with current Intel Arc and Iris Xe graphics drivers.".to_string())?;
        let init = lib.ctlInit.as_ref().map_err(|_| "ControlLib.dll has no ctlInit entry point".to_string())?;
        let mut api: ctl_api_handle_t = std::ptr::null_mut();
        let mut args = ctl_init_args_t {
            Size: std::mem::size_of::<ctl_init_args_t>() as u32,
            AppVersion: (CTL_IMPL_MAJOR_VERSION << 16) | (CTL_IMPL_MINOR_VERSION & 0xffff),
            // Level Zero is what supplies telemetry; fall back to plain init if it is missing.
            flags: 1,
            ..Default::default()
        };
        // SAFETY: sized, zero-initialised argument block and a valid out-pointer.
        let mut st = unsafe { init(&mut args, &mut api) };
        if st != CTL_OK {
            args.flags = 0;
            // SAFETY: as above.
            st = unsafe { init(&mut args, &mut api) };
        }
        if st != CTL_OK || api.is_null() {
            return Err(format!("IGCL could not start (ctlInit returned 0x{st:08X}). No supported Intel GPU or driver."));
        }
        let version = Some(format!("{}.{}", args.SupportedVersion >> 16, args.SupportedVersion & 0xffff));
        let mut me = Self { lib, api: api as usize, devices: Vec::new(), version, last: Mutex::new(HashMap::new()) };
        me.devices = me.enumerate();
        Ok(me)
    }

    pub fn version(&self) -> Option<String> {
        self.version.clone()
    }

    fn enumerate(&self) -> Vec<Device> {
        let (Ok(enumerate), Ok(props_fn)) = (&self.lib.ctlEnumerateDevices, &self.lib.ctlGetDeviceProperties) else { return Vec::new() };
        let api = self.api as ctl_api_handle_t;
        let mut count = 0u32;
        // SAFETY: count query with a null array, as documented.
        if unsafe { enumerate(api, &mut count, std::ptr::null_mut()) } != CTL_OK || count == 0 {
            return Vec::new();
        }
        let mut handles: Vec<ctl_device_adapter_handle_t> = vec![std::ptr::null_mut(); count as usize];
        // SAFETY: array of `count` handles.
        if unsafe { enumerate(api, &mut count, handles.as_mut_ptr()) } != CTL_OK {
            return Vec::new();
        }
        handles.truncate(count as usize);
        handles
            .into_iter()
            .filter_map(|h| {
                // LUID layout: { u32 LowPart; i32 HighPart }.
                let mut luid = [0u32; 2];
                let mut p = ctl_device_adapter_properties_t {
                    Size: std::mem::size_of::<ctl_device_adapter_properties_t>() as u32,
                    pDeviceID: luid.as_mut_ptr().cast(),
                    device_id_size: 8,
                    ..Default::default()
                };
                // SAFETY: sized struct; pDeviceID points at 8 writable bytes.
                if unsafe { props_fn(h, &mut p) } != CTL_OK || p.device_type != CTL_DEVICE_TYPE_GRAPHICS {
                    return None;
                }
                Some(Device {
                    handle: h as usize,
                    luid: (luid != [0, 0]).then(|| format!("{}:{}", luid[1] as i32, luid[0])),
                    pci_device_id: p.pci_device_id,
                    name: super::cchars_to_string(&p.name),
                    integrated: p.graphics_adapter_properties & 1 != 0,
                    driver_version: p.driver_version,
                })
            })
            .collect()
    }

    pub(crate) fn device_for(&self, device: &GpuDevice) -> Option<&Device> {
        self.devices
            .iter()
            .find(|d| d.luid.is_some() && d.luid == device.luid)
            .or_else(|| self.devices.iter().find(|d| d.pci_device_id == device.device_id))
    }

    pub fn enrich(&self, device: &mut GpuDevice) {
        let Some(d) = self.device_for(device) else { return };
        device.kind = if d.integrated { GpuKind::Integrated } else { GpuKind::Discrete };
        device.kind_source = "vendor API".into();
        if d.driver_version != 0 {
            let v = d.driver_version;
            device.driver_version_vendor = Some(format!("{}.{}.{}.{}", (v >> 48) & 0xffff, (v >> 32) & 0xffff, (v >> 16) & 0xffff, v & 0xffff));
        }
        if !d.name.is_empty() && device.name.is_empty() {
            device.name = d.name.clone();
        }
    }

    pub fn telemetry(&self, device: &GpuDevice) -> GpuTelemetry {
        let mut t = GpuTelemetry::default();
        let Some(d) = self.device_for(device) else {
            t.note_missing("IGCL did not list this adapter");
            return t;
        };
        let Ok(get) = &self.lib.ctlPowerTelemetryGet else {
            t.note_missing("This Intel driver has no IGCL power telemetry");
            return t;
        };
        let mut p = ctl_power_telemetry_t { Size: std::mem::size_of::<ctl_power_telemetry_t>() as u32, Version: 1, ..Default::default() };
        // SAFETY: sized, versioned struct.
        let st = unsafe { get(d.handle as ctl_device_adapter_handle_t, &mut p) };
        if st != CTL_OK {
            t.note_missing(&format!("IGCL telemetry is not available on this GPU (0x{st:08X})"));
            return t;
        }
        if let Some(v) = item(&p.gpuCurrentTemperature) {
            t.temperature_c = Some(v);
            t.set("temperature", "IGCL");
        }
        if let Some(v) = item(&p.gpuCurrentClockFrequency) {
            t.core_clock_mhz = Some(v);
            t.set("coreClock", "IGCL");
        }
        if let Some(v) = item(&p.vramCurrentEffectiveFrequency).or_else(|| item(&p.vramCurrentClockFrequency)) {
            t.memory_clock_mhz = Some(v);
            t.set("memoryClock", "IGCL");
        }
        if let Some(v) = item(&p.gpuVoltage) {
            t.voltage_v = Some(v);
        }
        if let Some(v) = p.fanSpeed.iter().find_map(item) {
            t.fan_rpm = Some(v);
            t.set("fanRpm", "IGCL");
        }
        t.thermal_limited = Some(p.gpuTemperatureLimited);
        t.power_limited = Some(p.gpuPowerLimited);

        // Counter-based metrics need the previous sample.
        let now = Counters { time_s: item(&p.timeStamp).unwrap_or(0.0), energy_j: item(&p.totalCardEnergyCounter).or_else(|| item(&p.gpuEnergyCounter)), activity_s: item(&p.globalActivityCounter) };
        let prev = self.last.lock().insert(d.handle, now);
        match prev {
            Some(prev) if now.time_s > prev.time_s => {
                let dt = now.time_s - prev.time_s;
                if let (Some(a), Some(b)) = (prev.energy_j, now.energy_j) {
                    if b >= a {
                        t.power_w = Some((b - a) / dt);
                        t.set("power", "IGCL");
                    }
                }
                if let (Some(a), Some(b)) = (prev.activity_s, now.activity_s) {
                    if b >= a {
                        t.utilization_pct = Some(((b - a) / dt * 100.0).clamp(0.0, 100.0));
                        t.set("utilization", "IGCL");
                    }
                }
            }
            _ => {
                for m in ["power", "utilization"] {
                    t.unavailable.insert(m.into(), "Needs a second sample (IGCL reports counters)".into());
                }
            }
        }
        t
    }

    pub fn controls(&self, device: &GpuDevice) -> Vec<GpuControl> {
        super::igcl_controls::list(self, device)
    }

    pub fn set_control(&self, device: &GpuDevice, id: &str, value: f64) -> anyhow::Result<()> {
        super::igcl_controls::set(self, device, id, value)
    }
}

impl Drop for Igcl {
    fn drop(&mut self) {
        if let Ok(close) = &self.lib.ctlClose {
            // SAFETY: handle from the successful ctlInit in `load`.
            unsafe { close(self.api as ctl_api_handle_t) };
        }
    }
}
