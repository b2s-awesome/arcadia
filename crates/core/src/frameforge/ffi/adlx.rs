//! AMD ADLX, loaded from the installed Radeon driver (`amdadlx64.dll`).
//!
//! Only compiled with the `adlx` cargo feature. The interface definitions
//! are generated at build time from an SDK checkout (see `build.rs`); no
//! AMD source is part of this repository.
//!
//! ADLX objects are C structures whose first field points to a table of
//! functions. Everything here goes through that table and nothing else:
//! * `IADLXSystem::GetGPUs`, `IADLXGPU` identification;
//! * `IADLXPerformanceMonitoringServices::GetSupportedGPUMetrics` and
//!   `GetCurrentGPUMetrics` for telemetry (each reading only when the driver
//!   marks it supported);
//! * `IADLX3DSettingsServices` for Anti-Lag, Chill, Boost, Image Sharpening,
//!   Enhanced Sync, Wait for Vertical Refresh, Frame Rate Target Control and
//!   Radeon Super Resolution, each behind its own `IsSupported`.
//!
//! Manual tuning (fan, power, clocks, voltage) is not used.
//!
//! This code has been type-checked against the SDK headers. It has not been
//! run on AMD hardware by the automated tests.

#[allow(non_camel_case_types, non_snake_case, non_upper_case_globals, dead_code, clippy::all)]
mod sys {
    include!(concat!(env!("OUT_DIR"), "/adlx_bindings.rs"));
}

use crate::frameforge::gpu::{ControlKind, ControlOption, GpuControl, GpuDevice, GpuKind, GpuTelemetry, Risk, Vendor};
use parking_lot::Mutex;
use std::ffi::{c_void, CStr};
use sys::*;

/// Call a method through an interface's function table. A slot the driver
/// left empty counts as "not supported".
macro_rules! call {
    ($obj:expr, $method:ident $(, $arg:expr)*) => {{
        let o = $obj;
        match (*(*o).pVtbl).$method {
            Some(f) => f(o $(, $arg)*),
            None => ADLX_NOT_SUPPORTED,
        }
    }};
}

/// Read a boolean out-parameter (`IsSupported`, `IsEnabled`).
macro_rules! flag {
    ($obj:expr, $method:ident) => {{
        let mut b: adlx_bool = 0;
        (call!($obj, $method, &mut b) == ADLX_OK).then_some(b != 0)
    }};
}

/// Ask a service for a per-GPU feature interface.
macro_rules! feature {
    ($svc:expr, $getter:ident, $ty:ty $(, $gpu:expr)?) => {{
        let mut p: *mut $ty = std::ptr::null_mut();
        let st = call!($svc, $getter $(, $gpu)?, &mut p);
        (st == ADLX_OK && !p.is_null()).then(|| Owned(p))
    }};
}

/// An interface pointer this code holds a reference to; released on drop.
struct Owned<T>(*mut T);

impl<T> Drop for Owned<T> {
    fn drop(&mut self) {
        // Every reference-counted ADLX interface starts its function table
        // with Acquire, Release (IADLXInterface).
        type Slot = Option<unsafe extern "C" fn(*mut c_void) -> std::ffi::c_long>;
        // SAFETY: `self.0` is a live interface pointer whose first field is
        // the table pointer; slot 1 is Release.
        unsafe {
            let table = *(self.0 as *const *const [Slot; 2]);
            if let Some(release) = (*table)[1] {
                release(self.0 as *mut c_void);
            }
        }
    }
}

struct Gpu {
    ptr: Owned<IADLXGPU>,
    device_id: u32,
    name: String,
    integrated: Option<bool>,
}

pub struct Adlx {
    _lib: libloading::Library,
    system: *mut IADLXSystem,
    terminate: Option<unsafe extern "C" fn() -> ADLX_RESULT>,
    version: Option<String>,
    gpus: Vec<Gpu>,
    perf: Option<Owned<IADLXPerformanceMonitoringServices>>,
    settings: Option<Owned<IADLX3DSettingsServices>>,
    /// ADLX calls are made one at a time.
    lock: Mutex<()>,
}

// SAFETY: the pointers are owned by this struct, ADLX is initialised once for
// the process, and every call is serialised by `lock`.
unsafe impl Send for Adlx {}
unsafe impl Sync for Adlx {}

fn cstr(p: *const std::ffi::c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: ADLX returns NUL-terminated strings owned by the interface.
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

const FULL_VERSION: u64 = ((ADLX_VER_MAJOR as u64) << 48) | ((ADLX_VER_MINOR as u64) << 32) | ((ADLX_VER_RELEASE as u64) << 16) | ADLX_VER_BUILD_NUM as u64;

impl Adlx {
    pub fn load() -> Result<Self, String> {
        // SAFETY: loading the driver's own DLL by bare name from the system search path.
        let lib = unsafe { libloading::Library::new("amdadlx64.dll") }.map_err(|_| "amdadlx64.dll was not found. Install AMD Software: Adrenalin Edition.".to_string())?;
        // SAFETY: the symbol names and signatures are the documented ADLX entry points.
        let (init, terminate, query) = unsafe {
            let init = *lib.get::<unsafe extern "C" fn(u64, *mut *mut IADLXSystem) -> ADLX_RESULT>(b"ADLXInitialize\0").map_err(|_| "amdadlx64.dll has no ADLXInitialize entry point".to_string())?;
            let terminate = lib.get::<unsafe extern "C" fn() -> ADLX_RESULT>(b"ADLXTerminate\0").ok().map(|s| *s);
            let query = lib.get::<unsafe extern "C" fn(*mut u64) -> ADLX_RESULT>(b"ADLXQueryFullVersion\0").ok().map(|s| *s);
            (init, terminate, query)
        };
        let mut system: *mut IADLXSystem = std::ptr::null_mut();
        // SAFETY: valid out-pointer; the version is the one the bindings were generated from.
        let st = unsafe { init(FULL_VERSION, &mut system) };
        if st != ADLX_OK || system.is_null() {
            return Err(format!("ADLX could not start (ADLXInitialize returned {st}). No supported AMD GPU, or the Radeon driver is too old."));
        }
        let version = query.and_then(|q| {
            let mut v = 0u64;
            // SAFETY: valid out-pointer.
            (unsafe { q(&mut v) } == ADLX_OK).then(|| format!("{}.{}.{}.{}", v >> 48, (v >> 32) & 0xffff, (v >> 16) & 0xffff, v & 0xffff))
        });
        let mut me = Self { _lib: lib, system, terminate, version, gpus: Vec::new(), perf: None, settings: None, lock: Mutex::new(()) };
        // SAFETY: `system` is the live ADLX system interface.
        unsafe {
            me.gpus = me.enumerate();
            me.perf = feature!(me.system, GetPerformanceMonitoringServices, IADLXPerformanceMonitoringServices);
            me.settings = feature!(me.system, Get3DSettingsServices, IADLX3DSettingsServices);
        }
        Ok(me)
    }

    pub fn version(&self) -> Option<String> {
        self.version.clone()
    }

    unsafe fn enumerate(&self) -> Vec<Gpu> {
        let Some(list) = feature!(self.system, GetGPUs, IADLXGPUList) else { return Vec::new() };
        let (begin, end) = match ((*(*list.0).pVtbl).Begin, (*(*list.0).pVtbl).End) {
            (Some(b), Some(e)) => (b(list.0), e(list.0)),
            _ => return Vec::new(),
        };
        let mut out = Vec::new();
        for i in begin..end.min(begin + 16) {
            let mut p: *mut IADLXGPU = std::ptr::null_mut();
            if call!(list.0, At_GPUList, i, &mut p) != ADLX_OK || p.is_null() {
                continue;
            }
            let ptr = Owned(p);
            let mut id: *const std::ffi::c_char = std::ptr::null();
            let mut name: *const std::ffi::c_char = std::ptr::null();
            let _ = call!(p, DeviceId, &mut id);
            let _ = call!(p, Name, &mut name);
            let mut kind: ADLX_GPU_TYPE = GPUTYPE_UNDEFINED;
            let integrated = (call!(p, Type, &mut kind) == ADLX_OK && kind != GPUTYPE_UNDEFINED).then_some(kind == GPUTYPE_INTEGRATED);
            // ADLX reports the PCI device id as hex text.
            let device_id = u32::from_str_radix(cstr(id).trim().trim_start_matches("0x"), 16).unwrap_or(0);
            out.push(Gpu { ptr, device_id, name: cstr(name), integrated });
        }
        out
    }

    /// The ADLX GPU for a device from the common list. With several identical
    /// cards the first match is used.
    fn gpu_for(&self, device: &GpuDevice) -> Option<&Gpu> {
        self.gpus.iter().find(|g| g.device_id != 0 && g.device_id == device.device_id).or_else(|| self.gpus.iter().find(|g| !g.name.is_empty() && g.name.eq_ignore_ascii_case(&device.name)))
    }

    pub fn enrich(&self, device: &mut GpuDevice) {
        let _guard = self.lock.lock();
        if let Some(g) = self.gpu_for(device) {
            if let Some(integrated) = g.integrated {
                device.kind = if integrated { GpuKind::Integrated } else { GpuKind::Discrete };
                device.kind_source = "vendor API".into();
            }
            if self.settings.is_some() && !device.features.iter().any(|f| f == "Radeon 3D settings (ADLX)") {
                device.features.push("Radeon 3D settings (ADLX)".into());
            }
        }
    }

    pub fn telemetry(&self, device: &GpuDevice) -> GpuTelemetry {
        let mut t = GpuTelemetry::default();
        let _guard = self.lock.lock();
        let (Some(g), Some(perf)) = (self.gpu_for(device), &self.perf) else { return t };
        // SAFETY: live interface pointers owned by `self`; out-pointers are valid for each call.
        unsafe {
            let Some(support) = feature!(perf.0, GetSupportedGPUMetrics, IADLXGPUMetricsSupport, g.ptr.0) else { return t };
            let Some(m) = feature!(perf.0, GetCurrentGPUMetrics, IADLXGPUMetrics, g.ptr.0) else { return t };
            macro_rules! read {
                ($sup:ident, $get:ident, $ty:ty) => {{
                    if flag!(support.0, $sup) == Some(true) {
                        let mut v: $ty = Default::default();
                        (call!(m.0, $get, &mut v) == ADLX_OK).then_some(v as f64).filter(|v| v.is_finite())
                    } else {
                        None
                    }
                }};
            }
            let mut put = |slot: &mut Option<f64>, v: Option<f64>, metric: &str| {
                if v.is_some() {
                    *slot = v;
                    t.sources.insert(metric.to_string(), "ADLX".to_string());
                }
            };
            let mut s = GpuTelemetry::default();
            put(&mut s.utilization_pct, read!(IsSupportedGPUUsage, GPUUsage, adlx_double), "utilization");
            put(&mut s.temperature_c, read!(IsSupportedGPUTemperature, GPUTemperature, adlx_double), "temperature");
            put(&mut s.hotspot_c, read!(IsSupportedGPUHotspotTemperature, GPUHotspotTemperature, adlx_double), "hotspot");
            // Board power when the card reports it, otherwise the GPU chip's own power.
            let power = read!(IsSupportedGPUTotalBoardPower, GPUTotalBoardPower, adlx_double).or(read!(IsSupportedGPUPower, GPUPower, adlx_double));
            put(&mut s.power_w, power, "power");
            put(&mut s.fan_rpm, read!(IsSupportedGPUFanSpeed, GPUFanSpeed, adlx_int), "fanRpm");
            put(&mut s.core_clock_mhz, read!(IsSupportedGPUClockSpeed, GPUClockSpeed, adlx_int), "coreClock");
            put(&mut s.memory_clock_mhz, read!(IsSupportedGPUVRAMClockSpeed, GPUVRAMClockSpeed, adlx_int), "memoryClock");
            // ADLX reports used VRAM in megabytes and voltage in millivolts.
            let vram = read!(IsSupportedGPUVRAM, GPUVRAM, adlx_int);
            let volts = read!(IsSupportedGPUVoltage, GPUVoltage, adlx_int).map(|mv| mv / 1000.0);
            put(&mut s.voltage_v, volts, "voltage");
            drop(put);
            s.vram_used_bytes = vram.map(|mb| (mb * 1024.0 * 1024.0) as u64);
            if s.vram_used_bytes.is_some() {
                t.sources.insert("vramUsed".into(), "ADLX".into());
            }
            s.sources = std::mem::take(&mut t.sources);
            t = s;
        }
        t
    }

    fn control(id: &str, name: &str, description: &str, kind: ControlKind, supported: Option<bool>, value: Option<f64>, level: u8) -> GpuControl {
        let ok = supported == Some(true);
        GpuControl {
            id: id.into(),
            vendor: Vendor::Amd,
            name: name.into(),
            description: description.into(),
            category: "Radeon 3D settings".into(),
            kind: if ok { kind } else { ControlKind::ReadOnly },
            value: if ok { value } else { None },
            default: None,
            supported: ok,
            unsupported_reason: (!ok).then(|| match supported {
                Some(false) => "The Radeon driver reports this feature as not supported on this adapter.".to_string(),
                _ => "The Radeon driver does not offer this feature through ADLX on this adapter.".to_string(),
            }),
            risk: Risk::Low,
            level,
            requires_restart: false,
        }
    }

    pub fn controls(&self, device: &GpuDevice) -> Vec<GpuControl> {
        let _guard = self.lock.lock();
        let (Some(g), Some(svc)) = (self.gpu_for(device), &self.settings) else { return Vec::new() };
        let gpu = g.ptr.0;
        let on = |b: Option<bool>| b.map(|x| x as u8 as f64);
        let mut out = Vec::new();
        // SAFETY: live interface pointers owned by `self` or by the `Owned`
        // values below; out-pointers are valid for each call.
        unsafe {
            macro_rules! toggle {
                ($getter:ident, $ty:ty, $id:literal, $name:literal, $desc:literal) => {{
                    let f = feature!(svc.0, $getter, $ty, gpu);
                    let sup = f.as_ref().and_then(|f| flag!(f.0, IsSupported));
                    let en = f.as_ref().and_then(|f| flag!(f.0, IsEnabled));
                    out.push(Self::control($id, $name, $desc, ControlKind::Toggle, sup, on(en), 2));
                    (f, sup)
                }};
            }
            toggle!(GetAntiLag, IADLX3DAntiLag, "adlx.antilag", "Radeon Anti-Lag", "Shortens the render queue to reduce input delay when the graphics card is the limit.");
            toggle!(GetChill, IADLX3DChill, "adlx.chill", "Radeon Chill", "Lowers the frame rate when little is moving, to save power and heat. The driver turns Anti-Lag and Boost off while Chill is on.");
            toggle!(GetBoost, IADLX3DBoost, "adlx.boost", "Radeon Boost", "Lowers render resolution during fast motion to raise the frame rate. The driver turns Chill off while Boost is on.");
            toggle!(GetEnhancedSync, IADLX3DEnhancedSync, "adlx.enhanced_sync", "Radeon Enhanced Sync", "Reduces tearing without the full delay of vertical sync.");

            let (sharp, sup) = toggle!(GetImageSharpening, IADLX3DImageSharpening, "adlx.sharpening", "Radeon Image Sharpening", "Sharpens the image after rendering.");
            if let (Some(f), Some(true)) = (&sharp, sup) {
                let mut r = ADLX_IntRange::default();
                let mut v: adlx_int = 0;
                if call!(f.0, GetSharpnessRange, &mut r) == ADLX_OK && r.maxValue > r.minValue {
                    let cur = (call!(f.0, GetSharpness, &mut v) == ADLX_OK).then_some(v as f64);
                    out.push(Self::control("adlx.sharpness", "Image Sharpening strength", "How strong the sharpening is. Only used while Image Sharpening is on.", ControlKind::Range { min: r.minValue as f64, max: r.maxValue as f64, step: r.step.max(1) as f64, unit: "%".into() }, Some(true), cur, 2));
                }
            }

            let (frtc, sup) = toggle!(GetFrameRateTargetControl, IADLX3DFrameRateTargetControl, "adlx.frtc", "Frame Rate Target Control", "Caps the frame rate in the driver.");
            if let (Some(f), Some(true)) = (&frtc, sup) {
                let mut r = ADLX_IntRange::default();
                let mut v: adlx_int = 0;
                if call!(f.0, GetFPSRange, &mut r) == ADLX_OK && r.maxValue > r.minValue {
                    let cur = (call!(f.0, GetFPS, &mut v) == ADLX_OK).then_some(v as f64);
                    out.push(Self::control("adlx.frtc_fps", "Frame rate target", "The cap used while Frame Rate Target Control is on.", ControlKind::Range { min: r.minValue as f64, max: r.maxValue as f64, step: r.step.max(1) as f64, unit: "FPS".into() }, Some(true), cur, 2));
                }
            }

            let wfvr = feature!(svc.0, GetWaitForVerticalRefresh, IADLX3DWaitForVerticalRefresh, gpu);
            let sup = wfvr.as_ref().and_then(|f| flag!(f.0, IsSupported));
            let mode = wfvr.as_ref().and_then(|f| {
                let mut m: ADLX_WAIT_FOR_VERTICAL_REFRESH_MODE = 0;
                (call!(f.0, GetMode, &mut m) == ADLX_OK).then_some(m as f64)
            });
            let options = [(WFVR_ALWAYS_OFF, "Always off"), (WFVR_OFF_UNLESS_APP_SPECIFIES, "Off, unless the application specifies"), (WFVR_ON_UNLESS_APP_SPECIFIES, "On, unless the application specifies"), (WFVR_ALWAYS_ON, "Always on")];
            out.push(Self::control(
                "adlx.vsync",
                "Wait for Vertical Refresh",
                "Vertical sync behaviour for games on this adapter.",
                ControlKind::Choice { options: options.iter().map(|(v, l)| ControlOption { value: *v as f64, label: (*l).to_string() }).collect() },
                sup,
                mode,
                2,
            ));

            // Radeon Super Resolution is one switch for the whole system, not per adapter.
            let rsr = feature!(svc.0, GetRadeonSuperResolution, IADLX3DRadeonSuperResolution);
            let sup = rsr.as_ref().and_then(|f| flag!(f.0, IsSupported));
            let en = rsr.as_ref().and_then(|f| flag!(f.0, IsEnabled));
            out.push(Self::control("adlx.rsr", "Radeon Super Resolution", "Driver upscaling for games running below the display's resolution in exclusive full screen. Applies to the whole system.", ControlKind::Toggle, sup, on(en), 2));
        }
        out
    }

    pub fn set_control(&self, device: &GpuDevice, id: &str, value: f64) -> anyhow::Result<()> {
        let _guard = self.lock.lock();
        let g = self.gpu_for(device).ok_or_else(|| anyhow::anyhow!("ADLX does not list this adapter"))?;
        let svc = self.settings.as_ref().ok_or_else(|| anyhow::anyhow!("The Radeon driver does not offer 3D settings through ADLX"))?;
        let gpu = g.ptr.0;
        let enable: adlx_bool = (value != 0.0) as adlx_bool;
        let missing = || anyhow::anyhow!("The Radeon driver does not offer this feature on this adapter");
        let done = |st: ADLX_RESULT| match st {
            ADLX_OK | ADLX_ALREADY_ENABLED => Ok(()),
            ADLX_NOT_SUPPORTED => Err(anyhow::anyhow!("The Radeon driver does not support this change on this adapter")),
            other => Err(anyhow::anyhow!("The Radeon driver refused the change (ADLX result {other})")),
        };
        // SAFETY: live interface pointers; values are range-checked against
        // the driver's own limits before they are passed.
        unsafe {
            macro_rules! set_enabled {
                ($getter:ident, $ty:ty) => {{
                    let f = feature!(svc.0, $getter, $ty, gpu).ok_or_else(missing)?;
                    if flag!(f.0, IsSupported) != Some(true) {
                        return Err(missing());
                    }
                    done(call!(f.0, SetEnabled, enable))
                }};
            }
            match id {
                "adlx.antilag" => set_enabled!(GetAntiLag, IADLX3DAntiLag),
                "adlx.chill" => set_enabled!(GetChill, IADLX3DChill),
                "adlx.boost" => set_enabled!(GetBoost, IADLX3DBoost),
                "adlx.enhanced_sync" => set_enabled!(GetEnhancedSync, IADLX3DEnhancedSync),
                "adlx.sharpening" => set_enabled!(GetImageSharpening, IADLX3DImageSharpening),
                "adlx.frtc" => set_enabled!(GetFrameRateTargetControl, IADLX3DFrameRateTargetControl),
                "adlx.sharpness" => {
                    let f = feature!(svc.0, GetImageSharpening, IADLX3DImageSharpening, gpu).ok_or_else(missing)?;
                    let mut r = ADLX_IntRange::default();
                    if call!(f.0, GetSharpnessRange, &mut r) != ADLX_OK || !value.is_finite() || value < r.minValue as f64 || value > r.maxValue as f64 {
                        anyhow::bail!("Sharpening strength must be between {} and {}", r.minValue, r.maxValue);
                    }
                    done(call!(f.0, SetSharpness, value.round() as adlx_int))
                }
                "adlx.frtc_fps" => {
                    let f = feature!(svc.0, GetFrameRateTargetControl, IADLX3DFrameRateTargetControl, gpu).ok_or_else(missing)?;
                    let mut r = ADLX_IntRange::default();
                    if call!(f.0, GetFPSRange, &mut r) != ADLX_OK || !value.is_finite() || value < r.minValue as f64 || value > r.maxValue as f64 {
                        anyhow::bail!("The frame rate target must be between {} and {}", r.minValue, r.maxValue);
                    }
                    done(call!(f.0, SetFPS, value.round() as adlx_int))
                }
                "adlx.vsync" => {
                    let f = feature!(svc.0, GetWaitForVerticalRefresh, IADLX3DWaitForVerticalRefresh, gpu).ok_or_else(missing)?;
                    let mode = value as ADLX_WAIT_FOR_VERTICAL_REFRESH_MODE;
                    if ![WFVR_ALWAYS_OFF, WFVR_OFF_UNLESS_APP_SPECIFIES, WFVR_ON_UNLESS_APP_SPECIFIES, WFVR_ALWAYS_ON].contains(&mode) || value.fract() != 0.0 {
                        anyhow::bail!("That is not a vertical refresh mode");
                    }
                    if flag!(f.0, IsSupported) != Some(true) {
                        return Err(missing());
                    }
                    done(call!(f.0, SetMode, mode))
                }
                "adlx.rsr" => {
                    let f = feature!(svc.0, GetRadeonSuperResolution, IADLX3DRadeonSuperResolution).ok_or_else(missing)?;
                    if flag!(f.0, IsSupported) != Some(true) {
                        return Err(missing());
                    }
                    done(call!(f.0, SetEnabled, enable))
                }
                other => anyhow::bail!("ADLX does not expose the control '{other}'"),
            }
        }
    }
}

impl Drop for Adlx {
    fn drop(&mut self) {
        // Release every interface before ADLX itself is shut down.
        self.perf = None;
        self.settings = None;
        self.gpus.clear();
        if let Some(terminate) = self.terminate {
            // SAFETY: balanced with the successful ADLXInitialize in `load`.
            unsafe { terminate() };
        }
    }
}
