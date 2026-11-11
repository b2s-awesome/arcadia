//! PresentMon Service through `PresentMonAPI2.dll`.
//!
//! Follows Intel's guidance for clients (README-Service.md):
//! * the DLL path is read from the registry value the installer writes
//!   (`HKLM\SOFTWARE\INTEL\PresentMon\Service`, `sharedMiddlewarePath`) and
//!   the installed copy is loaded, so service and middleware always match;
//! * the API major version must equal the one these bindings were generated
//!   from;
//! * before any query, the introspection tree is read and only metrics the
//!   service reports as available are requested.

use super::{Frame, FrameSource, PmTelemetry, SourceKind};
use crate::frameforge::ffi::presentmon_types::*;
use crate::registry::{read_string, Hive};
use std::collections::HashMap;

const REG_PATH: &str = r"SOFTWARE\INTEL\PresentMon\Service";
const REG_VALUE: &str = "sharedMiddlewarePath";
const OK: PM_STATUS = PM_STATUS_SUCCESS;
/// Frames fetched per `pmConsumeFrames` call.
const BLOBS_PER_POLL: u32 = 512;
/// Averaging window for service telemetry, milliseconds.
const TELEMETRY_WINDOW_MS: f64 = 1000.0;

fn status_text(st: PM_STATUS) -> String {
    match st {
        PM_STATUS_SERVICE_ERROR | PM_STATUS_PIPE_ERROR => "The PresentMon Service is installed but not running. Start \"PresentMon Service\" in Windows Services, or reinstall PresentMon.".into(),
        PM_STATUS_INVALID_PID => "That process is no longer running".into(),
        PM_STATUS_ALREADY_TRACKING_PROCESS => "PresentMon is already tracking that process for this session".into(),
        PM_STATUS_MIDDLEWARE_SERVICE_MISMATCH => "PresentMonAPI2.dll and the PresentMon Service are different versions. Reinstall PresentMon.".into(),
        other => format!("PresentMon returned status {other}"),
    }
}

fn middleware_path() -> Result<String, String> {
    let p = read_string(Hive::LocalMachine, REG_PATH, REG_VALUE)
        .filter(|p| !p.trim().is_empty())
        .ok_or_else(|| "The PresentMon Service is not installed (version 2.3.1 or newer is needed).".to_string())?;
    let path = std::path::Path::new(&p);
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    if !path.is_absolute() || !name.eq_ignore_ascii_case("PresentMonAPI2.dll") || !path.is_file() {
        return Err("The PresentMon install looks damaged (its API library is missing). Reinstall PresentMon.".into());
    }
    Ok(p)
}

fn load() -> Result<(PmLib, PM_VERSION), String> {
    let path = middleware_path()?;
    // SAFETY: loading the DLL the PresentMon installer registered, by absolute path.
    let lib = unsafe { PmLib::new(&path) }.map_err(|e| format!("PresentMonAPI2.dll could not be loaded: {e}"))?;
    let get_version = lib.pmGetApiVersion.as_ref().map_err(|_| "This PresentMon is too old (no API version call). Install 2.3.1 or newer.".to_string())?;
    let mut v = PM_VERSION::default();
    // SAFETY: valid out-pointer.
    if unsafe { get_version(&mut v) } != OK {
        return Err("PresentMon did not report its API version".into());
    }
    if u32::from(v.major) != PM_API_VERSION_MAJOR {
        return Err(format!("PresentMon API {}.{} is not compatible with this Arcadia (built for API {PM_API_VERSION_MAJOR}.x). Update {}.", v.major, v.minor, if u32::from(v.major) < PM_API_VERSION_MAJOR { "PresentMon" } else { "Arcadia" }));
    }
    Ok((lib, v))
}

/// Is the service usable? Returns the API version text.
pub fn probe() -> Result<String, String> {
    let (lib, v) = load()?;
    let open = lib.pmOpenSession.as_ref().map_err(|_| "PresentMonAPI2.dll has no pmOpenSession".to_string())?;
    let close = lib.pmCloseSession.as_ref().map_err(|_| "PresentMonAPI2.dll has no pmCloseSession".to_string())?;
    let mut session: PM_SESSION_HANDLE = std::ptr::null_mut();
    // SAFETY: valid out-pointer.
    let st = unsafe { open(&mut session) };
    if st != OK {
        return Err(status_text(st));
    }
    // SAFETY: session handle from pmOpenSession.
    unsafe { close(session) };
    Ok(format!("{}.{}.{}", v.major, v.minor, v.patch))
}

#[derive(Clone, Copy)]
struct MetricInfo {
    kind: PM_METRIC_TYPE,
    frame_type: PM_DATA_TYPE,
    polled_type: PM_DATA_TYPE,
}

/// What the service says it can measure.
#[derive(Default)]
struct Introspection {
    metrics: HashMap<PM_METRIC, MetricInfo>,
    /// (metric, device id) pairs marked available.
    available: std::collections::HashSet<(PM_METRIC, u32)>,
    /// Graphics adapters: (device id, luid as `high:low`, name).
    adapters: Vec<(u32, Option<String>, String)>,
    system_device: Option<u32>,
}

/// SAFETY: `arr` must be null or point to a live PM_INTROSPECTION_OBJARRAY whose elements are `*const T`.
unsafe fn objarray<'a, T>(arr: *const PM_INTROSPECTION_OBJARRAY) -> Vec<&'a T> {
    let mut out = Vec::new();
    if arr.is_null() {
        return out;
    }
    let a = unsafe { &*arr };
    if a.pData.is_null() {
        return out;
    }
    for i in 0..a.size.min(4096) {
        let p = unsafe { *a.pData.add(i) } as *const T;
        if !p.is_null() {
            out.push(unsafe { &*p });
        }
    }
    out
}

fn introspect(lib: &PmLib, session: PM_SESSION_HANDLE) -> Introspection {
    let mut out = Introspection::default();
    let (Ok(get), Ok(free)) = (&lib.pmGetIntrospectionRoot, &lib.pmFreeIntrospectionRoot) else { return out };
    let mut root: *const PM_INTROSPECTION_ROOT = std::ptr::null();
    // SAFETY: valid session and out-pointer.
    if unsafe { get(session, &mut root) } != OK || root.is_null() {
        return out;
    }
    // SAFETY: the tree stays alive until pmFreeIntrospectionRoot below; all
    // data is copied out before then.
    unsafe {
        let r = &*root;
        for d in objarray::<PM_INTROSPECTION_DEVICE>(r.pDevices) {
            match d.type_ {
                PM_DEVICE_TYPE_GRAPHICS_ADAPTER => {
                    let name = if d.pName.is_null() || (*d.pName).pData.is_null() { String::new() } else { std::ffi::CStr::from_ptr((*d.pName).pData).to_string_lossy().to_string() };
                    let luid = (!d.pLuid.is_null() && !(*d.pLuid).pData.is_null() && (*d.pLuid).size >= 8).then(|| {
                        let b = std::slice::from_raw_parts((*d.pLuid).pData, 8);
                        let low = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                        let high = i32::from_le_bytes([b[4], b[5], b[6], b[7]]);
                        format!("{high}:{low}")
                    });
                    out.adapters.push((d.id, luid, name));
                }
                PM_DEVICE_TYPE_SYSTEM => out.system_device = Some(d.id),
                _ => {}
            }
        }
        for m in objarray::<PM_INTROSPECTION_METRIC>(r.pMetrics) {
            let (frame_type, polled_type) = if m.pTypeInfo.is_null() { (PM_DATA_TYPE_VOID, PM_DATA_TYPE_VOID) } else { ((*m.pTypeInfo).frameType, (*m.pTypeInfo).polledType) };
            out.metrics.insert(m.id, MetricInfo { kind: m.type_, frame_type, polled_type });
            for dm in objarray::<PM_INTROSPECTION_DEVICE_METRIC_INFO>(m.pDeviceMetricInfo) {
                if dm.availability == PM_METRIC_AVAILABILITY_AVAILABLE {
                    out.available.insert((m.id, dm.deviceId));
                }
            }
        }
        free(root);
    }
    out
}

/// Read one value out of a query blob as f64, by its PresentMon data type.
fn read_value(blob: &[u8], el: &PM_QUERY_ELEMENT, ty: PM_DATA_TYPE) -> Option<f64> {
    let (off, size) = (el.dataOffset as usize, el.dataSize as usize);
    let bytes = blob.get(off..off.checked_add(size)?)?;
    let v = match ty {
        PM_DATA_TYPE_DOUBLE if size >= 8 => f64::from_le_bytes(bytes[..8].try_into().ok()?),
        PM_DATA_TYPE_INT32 | PM_DATA_TYPE_ENUM if size >= 4 => i32::from_le_bytes(bytes[..4].try_into().ok()?) as f64,
        PM_DATA_TYPE_UINT32 if size >= 4 => u32::from_le_bytes(bytes[..4].try_into().ok()?) as f64,
        PM_DATA_TYPE_UINT64 if size >= 8 => u64::from_le_bytes(bytes[..8].try_into().ok()?) as f64,
        PM_DATA_TYPE_BOOL if size >= 1 => (bytes[0] != 0) as u8 as f64,
        _ => return None,
    };
    v.is_finite().then_some(v)
}

struct Element {
    metric: PM_METRIC,
    ty: PM_DATA_TYPE,
}

pub struct ServiceSource {
    lib: PmLib,
    session: PM_SESSION_HANDLE,
    pid: u32,
    version: String,
    frame_query: PM_FRAME_QUERY_HANDLE,
    frame_elements: Vec<PM_QUERY_ELEMENT>,
    frame_meta: Vec<Element>,
    blob_size: usize,
    buffer: Vec<u8>,
    dyn_query: PM_DYNAMIC_QUERY_HANDLE,
    dyn_elements: Vec<PM_QUERY_ELEMENT>,
    dyn_meta: Vec<Element>,
    dyn_buffer: Vec<u8>,
    first_time_ms: Option<f64>,
    alive: bool,
}

// SAFETY: the handles are owned by this struct and used from one thread at a
// time (the monitor thread owns the source).
unsafe impl Send for ServiceSource {}

impl ServiceSource {
    pub fn open(pid: u32, preferred_gpu_luid: Option<&str>) -> Result<Self, String> {
        let (lib, v) = load()?;
        let missing = |name: &str| format!("PresentMonAPI2.dll is missing {name}. Update PresentMon.");
        let open = *lib.pmOpenSession.as_ref().map_err(|_| missing("pmOpenSession"))?;
        let close = *lib.pmCloseSession.as_ref().map_err(|_| missing("pmCloseSession"))?;
        let track = *lib.pmStartTrackingProcess.as_ref().map_err(|_| missing("pmStartTrackingProcess"))?;
        let reg_frames = *lib.pmRegisterFrameQuery.as_ref().map_err(|_| missing("pmRegisterFrameQuery"))?;
        if lib.pmConsumeFrames.is_err() {
            return Err(missing("pmConsumeFrames"));
        }

        let mut session: PM_SESSION_HANDLE = std::ptr::null_mut();
        // SAFETY: valid out-pointer.
        let st = unsafe { open(&mut session) };
        if st != OK {
            return Err(status_text(st));
        }
        let intro = introspect(&lib, session);

        // Frame-event metrics, in the order they are decoded in `poll`.
        let wanted = [
            PM_METRIC_CPU_START_TIME,
            PM_METRIC_CPU_FRAME_TIME,
            PM_METRIC_CPU_BUSY,
            PM_METRIC_CPU_WAIT,
            PM_METRIC_GPU_TIME,
            PM_METRIC_GPU_BUSY,
            PM_METRIC_GPU_WAIT,
            PM_METRIC_DISPLAYED_TIME,
            PM_METRIC_DISPLAY_LATENCY,
            PM_METRIC_ALLOWS_TEARING,
            PM_METRIC_FRAME_TYPE,
        ];
        let mut frame_elements = Vec::new();
        let mut frame_meta = Vec::new();
        for m in wanted {
            // A metric usable in a frame query is one the service lists with a frame data type.
            let Some(info) = intro.metrics.get(&m) else { continue };
            if info.kind == PM_METRIC_TYPE_STATIC || info.kind == PM_METRIC_TYPE_DYNAMIC || info.frame_type == PM_DATA_TYPE_VOID {
                continue;
            }
            frame_elements.push(PM_QUERY_ELEMENT { metric: m, stat: PM_STAT_NONE, deviceId: 0, arrayIndex: 0, dataOffset: 0, dataSize: 0 });
            frame_meta.push(Element { metric: m, ty: info.frame_type });
        }
        if !frame_meta.iter().any(|e| e.metric == PM_METRIC_CPU_FRAME_TIME) {
            // SAFETY: session from pmOpenSession.
            unsafe { close(session) };
            return Err("This PresentMon Service does not offer per-frame CPU frame time, which Arcadia needs. Update PresentMon.".into());
        }
        let mut frame_query: PM_FRAME_QUERY_HANDLE = std::ptr::null_mut();
        let mut blob_size = 0u32;
        // SAFETY: element array of the stated length; out-pointers valid.
        let st = unsafe { reg_frames(session, &mut frame_query, frame_elements.as_mut_ptr(), frame_elements.len() as u64, &mut blob_size) };
        if st != OK || blob_size == 0 {
            // SAFETY: session from pmOpenSession.
            unsafe { close(session) };
            return Err(format!("PresentMon rejected the frame query: {}", status_text(st)));
        }

        // SAFETY: valid session.
        let st = unsafe { track(session, pid) };
        if st != OK && st != PM_STATUS_ALREADY_TRACKING_PROCESS {
            if let Ok(free) = &lib.pmFreeFrameQuery {
                // SAFETY: query registered above.
                unsafe { free(frame_query) };
            }
            // SAFETY: session from pmOpenSession.
            unsafe { close(session) };
            return Err(status_text(st));
        }

        // Hardware telemetry: only what the service marks available for the chosen adapter.
        let adapter = intro
            .adapters
            .iter()
            .find(|(_, luid, _)| luid.is_some() && luid.as_deref() == preferred_gpu_luid)
            .or_else(|| intro.adapters.first())
            .map(|(id, _, _)| *id);
        let mut dyn_elements = Vec::new();
        let mut dyn_meta = Vec::new();
        let mut want = |metric: PM_METRIC, device: Option<u32>| {
            let (Some(device), Some(info)) = (device, intro.metrics.get(&metric)) else { return };
            if intro.available.contains(&(metric, device)) && info.kind != PM_METRIC_TYPE_STATIC {
                dyn_elements.push(PM_QUERY_ELEMENT { metric, stat: PM_STAT_AVG, deviceId: device, arrayIndex: 0, dataOffset: 0, dataSize: 0 });
                // Averages are reported as doubles; fall back to the declared polled type.
                dyn_meta.push(Element { metric, ty: if info.polled_type == PM_DATA_TYPE_VOID { PM_DATA_TYPE_DOUBLE } else { info.polled_type } });
            }
        };
        for m in [PM_METRIC_GPU_POWER, PM_METRIC_GPU_TEMPERATURE, PM_METRIC_GPU_UTILIZATION, PM_METRIC_GPU_FAN_SPEED, PM_METRIC_GPU_FREQUENCY, PM_METRIC_GPU_MEM_USED, PM_METRIC_GPU_MEM_SIZE] {
            want(m, adapter);
        }
        for m in [PM_METRIC_CPU_UTILIZATION, PM_METRIC_CPU_TEMPERATURE, PM_METRIC_CPU_POWER, PM_METRIC_CPU_FREQUENCY] {
            want(m, intro.system_device);
        }
        let mut dyn_query: PM_DYNAMIC_QUERY_HANDLE = std::ptr::null_mut();
        let mut dyn_buffer = Vec::new();
        if !dyn_elements.is_empty() {
            if let Ok(reg) = &lib.pmRegisterDynamicQuery {
                // SAFETY: element array of the stated length; out-pointer valid.
                let st = unsafe { reg(session, &mut dyn_query, dyn_elements.as_mut_ptr(), dyn_elements.len() as u64, TELEMETRY_WINDOW_MS, 0.0) };
                if st == OK && !dyn_query.is_null() {
                    let per_chain = dyn_elements.iter().map(|e| (e.dataOffset + e.dataSize) as usize).max().unwrap_or(0);
                    // Room for several swap chains; only the first is read.
                    dyn_buffer = vec![0u8; per_chain * 8];
                } else {
                    dyn_query = std::ptr::null_mut();
                }
            }
        }

        Ok(Self {
            lib,
            session,
            pid,
            version: format!("{}.{}.{}", v.major, v.minor, v.patch),
            frame_query,
            frame_elements,
            frame_meta,
            blob_size: blob_size as usize,
            buffer: vec![0u8; blob_size as usize * BLOBS_PER_POLL as usize],
            dyn_query,
            dyn_elements,
            dyn_meta,
            dyn_buffer,
            first_time_ms: None,
            alive: true,
        })
    }
}

impl FrameSource for ServiceSource {
    fn kind(&self) -> SourceKind {
        SourceKind::Service
    }

    fn poll(&mut self) -> anyhow::Result<Vec<Frame>> {
        let consume = self.lib.pmConsumeFrames.as_ref().map_err(|_| anyhow::anyhow!("pmConsumeFrames is missing"))?;
        let mut out = Vec::new();
        // Drain in batches; stop when a batch comes back short.
        for _ in 0..16 {
            let mut n = BLOBS_PER_POLL;
            // SAFETY: buffer holds BLOBS_PER_POLL blobs of the size the service gave at registration.
            let st = unsafe { consume(self.frame_query, self.pid, self.buffer.as_mut_ptr(), &mut n) };
            if st != OK {
                self.alive = false;
                if st == PM_STATUS_INVALID_PID {
                    return Ok(out);
                }
                anyhow::bail!(status_text(st));
            }
            for i in 0..(n as usize).min(BLOBS_PER_POLL as usize) {
                let blob = &self.buffer[i * self.blob_size..(i + 1) * self.blob_size];
                let mut f = Frame::default();
                let mut start = None;
                for (el, meta) in self.frame_elements.iter().zip(&self.frame_meta) {
                    let v = read_value(blob, el, meta.ty);
                    match meta.metric {
                        PM_METRIC_CPU_START_TIME => start = v,
                        PM_METRIC_CPU_FRAME_TIME => f.frame_time_ms = v.unwrap_or(0.0),
                        PM_METRIC_CPU_BUSY => f.cpu_busy_ms = v,
                        PM_METRIC_CPU_WAIT => f.cpu_wait_ms = v,
                        PM_METRIC_GPU_TIME => f.gpu_time_ms = v,
                        PM_METRIC_GPU_BUSY => f.gpu_busy_ms = v,
                        PM_METRIC_GPU_WAIT => f.gpu_wait_ms = v,
                        PM_METRIC_DISPLAYED_TIME => f.displayed_time_ms = v.filter(|d| *d > 0.0),
                        PM_METRIC_DISPLAY_LATENCY => f.display_latency_ms = v.filter(|d| *d > 0.0),
                        PM_METRIC_ALLOWS_TEARING => f.allows_tearing = v.map(|b| b != 0.0),
                        PM_METRIC_FRAME_TYPE => {
                            f.generated = v.map(|t| t as i32 != PM_FRAME_TYPE_APPLICATION && t as i32 != PM_FRAME_TYPE_NOT_SET && t as i32 != PM_FRAME_TYPE_UNSPECIFIED).unwrap_or(false)
                        }
                        _ => {}
                    }
                }
                if f.frame_time_ms <= 0.0 {
                    continue;
                }
                let has_displayed = self.frame_meta.iter().any(|e| e.metric == PM_METRIC_DISPLAYED_TIME);
                f.dropped = has_displayed && f.displayed_time_ms.is_none();
                // CPU start time is relative to the service session; rebase to this capture.
                if let Some(s) = start {
                    let base = *self.first_time_ms.get_or_insert(s);
                    f.time_ms = s - base;
                }
                out.push(f);
            }
            if n < BLOBS_PER_POLL {
                break;
            }
        }
        Ok(out)
    }

    fn telemetry(&mut self) -> Option<PmTelemetry> {
        if self.dyn_query.is_null() || self.dyn_buffer.is_empty() {
            return None;
        }
        let poll = self.lib.pmPollDynamicQuery.as_ref().ok()?;
        let mut chains = 8u32;
        // SAFETY: buffer sized for 8 swap chains of this query's element layout.
        if unsafe { poll(self.dyn_query, self.pid, self.dyn_buffer.as_mut_ptr(), &mut chains) } != OK {
            return None;
        }
        let mut t = PmTelemetry::default();
        for (el, meta) in self.dyn_elements.iter().zip(&self.dyn_meta) {
            let v = read_value(&self.dyn_buffer, el, meta.ty).or_else(|| read_value(&self.dyn_buffer, el, PM_DATA_TYPE_DOUBLE));
            match meta.metric {
                PM_METRIC_GPU_POWER => t.gpu_power_w = v.filter(|x| *x > 0.0),
                PM_METRIC_GPU_TEMPERATURE => t.gpu_temperature_c = v.filter(|x| *x > 0.0),
                PM_METRIC_GPU_UTILIZATION => t.gpu_utilization_pct = v,
                PM_METRIC_GPU_FAN_SPEED => t.gpu_fan_rpm = v,
                PM_METRIC_GPU_FREQUENCY => t.gpu_frequency_mhz = v.filter(|x| *x > 0.0),
                PM_METRIC_GPU_MEM_USED => t.gpu_mem_used_bytes = v,
                PM_METRIC_GPU_MEM_SIZE => t.gpu_mem_size_bytes = v.filter(|x| *x > 0.0),
                PM_METRIC_CPU_UTILIZATION => t.cpu_utilization_pct = v,
                PM_METRIC_CPU_TEMPERATURE => t.cpu_temperature_c = v.filter(|x| *x > 0.0),
                PM_METRIC_CPU_POWER => t.cpu_power_w = v.filter(|x| *x > 0.0),
                PM_METRIC_CPU_FREQUENCY => t.cpu_frequency_mhz = v.filter(|x| *x > 0.0),
                _ => {}
            }
        }
        Some(t)
    }

    fn alive(&self) -> bool {
        self.alive
    }

    fn detail(&self) -> String {
        format!("PresentMon Service, API {}", self.version)
    }
}

impl Drop for ServiceSource {
    fn drop(&mut self) {
        // SAFETY: every handle below was created in `open` and is released exactly once.
        unsafe {
            if !self.dyn_query.is_null() {
                if let Ok(f) = &self.lib.pmFreeDynamicQuery {
                    f(self.dyn_query);
                }
            }
            if let Ok(f) = &self.lib.pmFreeFrameQuery {
                f(self.frame_query);
            }
            if let Ok(f) = &self.lib.pmStopTrackingProcess {
                f(self.session, self.pid);
            }
            if let Ok(f) = &self.lib.pmCloseSession {
                f(self.session);
            }
        }
    }
}
