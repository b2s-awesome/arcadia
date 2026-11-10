//! Adapter list from DXGI plus vendor-neutral telemetry from PDH.

use super::pdh::GpuCounters;
use super::*;
use crate::frameforge::ffi::wide_to_string;
use parking_lot::Mutex;
use windows::core::Interface;
use windows::Win32::Graphics::Dxgi::*;

pub struct DxgiEnumerator {
    counters: Mutex<Option<GpuCounters>>,
}

impl DxgiEnumerator {
    pub fn new() -> Self {
        Self { counters: Mutex::new(GpuCounters::open()) }
    }

    /// Per-process GPU load (percent of the adapter's 3D engines), across all adapters.
    pub fn process_usage(&self) -> std::collections::HashMap<u32, f64> {
        let mut out = std::collections::HashMap::new();
        if let Some(c) = self.counters.lock().as_mut() {
            for usage in c.snapshot().usage.values() {
                for (pid, v) in &usage.by_pid {
                    let e = out.entry(*pid).or_insert(0.0f64);
                    *e = e.max(*v);
                }
            }
        }
        out
    }
}

impl Default for DxgiEnumerator {
    fn default() -> Self {
        Self::new()
    }
}

fn version_string(v: i64) -> String {
    let v = v as u64;
    format!("{}.{}.{}.{}", (v >> 48) & 0xffff, (v >> 32) & 0xffff, (v >> 16) & 0xffff, v & 0xffff)
}

impl BaseEnumerator for DxgiEnumerator {
    fn devices(&self) -> Vec<GpuDevice> {
        let mut out = Vec::new();
        // SAFETY: plain COM factory creation; failure returns an error.
        let Ok(factory) = (unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }) else { return out };
        let mut i = 0;
        // SAFETY: EnumAdapters1 returns DXGI_ERROR_NOT_FOUND past the last adapter.
        while let Ok(adapter) = unsafe { factory.EnumAdapters1(i) } {
            i += 1;
            // SAFETY: adapter is a live COM object.
            let Ok(desc) = (unsafe { adapter.GetDesc1() }) else { continue };
            let name = wide_to_string(&desc.Description);
            let software = desc.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0;
            let vendor = Vendor::from_pci(desc.VendorId);
            let luid = format!("{}:{}", desc.AdapterLuid.HighPart, desc.AdapterLuid.LowPart);
            // The same physical card can be listed twice (e.g. once per attached session); keep one.
            if out.iter().any(|d: &GpuDevice| d.luid.as_deref() == Some(luid.as_str())) {
                continue;
            }
            // SAFETY: documented way to read the user-mode driver version.
            let driver = unsafe { adapter.CheckInterfaceSupport(&IDXGIDevice::IID) }.ok().map(version_string);
            let mut outputs = Vec::new();
            let mut j = 0;
            // SAFETY: EnumOutputs fails past the last output.
            while let Ok(o) = unsafe { adapter.EnumOutputs(j) } {
                j += 1;
                // SAFETY: output is a live COM object.
                if let Ok(d) = unsafe { o.GetDesc() } {
                    outputs.push(wide_to_string(&d.DeviceName));
                }
            }
            let dedicated = desc.DedicatedVideoMemory as u64;
            out.push(GpuDevice {
                id: format!("{:04x}:{:04x}:{luid}", desc.VendorId, desc.DeviceId),
                vendor,
                kind: guess_kind(vendor, &name, dedicated, software),
                kind_source: "heuristic".into(),
                name,
                vendor_id: desc.VendorId,
                device_id: desc.DeviceId,
                subsystem_id: desc.SubSysId,
                revision: desc.Revision,
                luid: Some(luid),
                driver_version: driver,
                vram_bytes: (!software).then_some(dedicated),
                shared_bytes: Some(desc.SharedSystemMemory as u64),
                outputs,
                ..Default::default()
            });
        }
        out
    }

    fn telemetry(&self, device: &GpuDevice) -> GpuTelemetry {
        let mut t = GpuTelemetry::default();
        let Some(luid) = &device.luid else { return t };
        let mut guard = self.counters.lock();
        let Some(c) = guard.as_mut() else {
            t.unavailable.insert("utilization".into(), "Windows GPU performance counters are not available".into());
            return t;
        };
        let snap = c.snapshot();
        if let Some(u) = snap.usage.get(luid) {
            t.utilization_pct = Some(u.utilization_pct);
            t.set("utilization", "Windows counters");
        }
        if let Some(b) = snap.dedicated_bytes.get(luid) {
            t.vram_used_bytes = Some(*b);
            t.set("vramUsed", "Windows counters");
        }
        t
    }
}
