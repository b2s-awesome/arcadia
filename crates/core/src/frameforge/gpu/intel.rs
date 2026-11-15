//! Intel adapter (IGCL, `ControlLib.dll` from the Intel graphics driver).

use super::*;

pub struct IntelAdapter {
    #[cfg(windows)]
    inner: Result<crate::frameforge::ffi::igcl::Igcl, String>,
}

impl IntelAdapter {
    pub fn load() -> Self {
        Self {
            #[cfg(windows)]
            inner: crate::frameforge::ffi::igcl::Igcl::load(),
        }
    }
}

impl VendorAdapter for IntelAdapter {
    fn vendor(&self) -> Vendor {
        Vendor::Intel
    }

    #[cfg(windows)]
    fn status(&self) -> ApiStatus {
        match &self.inner {
            Ok(a) => ApiStatus::Available { api: "IGCL".into(), version: a.version() },
            Err(e) => ApiStatus::unavailable("IGCL", e.clone()),
        }
    }
    #[cfg(not(windows))]
    fn status(&self) -> ApiStatus {
        ApiStatus::unavailable("IGCL", "IGCL is only available on Windows")
    }

    #[cfg(windows)]
    fn enrich(&self, device: &mut GpuDevice) {
        if let Ok(a) = &self.inner {
            a.enrich(device);
        }
    }

    #[cfg(windows)]
    fn telemetry(&self, device: &GpuDevice) -> GpuTelemetry {
        match &self.inner {
            Ok(a) => a.telemetry(device),
            Err(_) => GpuTelemetry::default(),
        }
    }
    #[cfg(not(windows))]
    fn telemetry(&self, _device: &GpuDevice) -> GpuTelemetry {
        GpuTelemetry::default()
    }

    #[cfg(windows)]
    fn controls(&self, device: &GpuDevice) -> Vec<GpuControl> {
        self.inner.as_ref().map(|a| a.controls(device)).unwrap_or_default()
    }

    #[cfg(windows)]
    fn set_control(&self, device: &GpuDevice, id: &str, value: f64) -> anyhow::Result<()> {
        match &self.inner {
            Ok(a) => a.set_control(device, id, value),
            Err(e) => anyhow::bail!("IGCL is unavailable: {e}"),
        }
    }
}
