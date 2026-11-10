//! AMD adapter (ADLX). Filled in by `frameforge::ffi::adlx` when the crate is
//! built with the `adlx` feature; see `docs/frameforge/vendor-apis.md` for
//! why AMD support is a build-time option.

use super::*;

pub struct AmdAdapter {
    #[cfg(all(windows, feature = "adlx"))]
    inner: Result<crate::frameforge::ffi::adlx::Adlx, String>,
}

impl AmdAdapter {
    pub fn load() -> Self {
        Self {
            #[cfg(all(windows, feature = "adlx"))]
            inner: crate::frameforge::ffi::adlx::Adlx::load(),
        }
    }
}

impl VendorAdapter for AmdAdapter {
    fn vendor(&self) -> Vendor {
        Vendor::Amd
    }

    #[cfg(all(windows, feature = "adlx"))]
    fn status(&self) -> ApiStatus {
        match &self.inner {
            Ok(a) => ApiStatus::Available { api: "ADLX".into(), version: a.version() },
            Err(e) => ApiStatus::unavailable("ADLX", e.clone()),
        }
    }
    #[cfg(not(all(windows, feature = "adlx")))]
    fn status(&self) -> ApiStatus {
        ApiStatus::unavailable(
            "ADLX",
            if cfg!(windows) {
                "This build of Arcadia was compiled without AMD ADLX support (the `adlx` build feature). Utilisation and VRAM still come from Windows; temperature, power and fan speed come from PresentMon when it is installed."
            } else {
                "ADLX is only available on Windows"
            },
        )
    }

    #[cfg(all(windows, feature = "adlx"))]
    fn enrich(&self, device: &mut GpuDevice) {
        if let Ok(a) = &self.inner {
            a.enrich(device);
        }
    }

    #[cfg(all(windows, feature = "adlx"))]
    fn telemetry(&self, device: &GpuDevice) -> GpuTelemetry {
        match &self.inner {
            Ok(a) => a.telemetry(device),
            Err(_) => GpuTelemetry::default(),
        }
    }
    #[cfg(not(all(windows, feature = "adlx")))]
    fn telemetry(&self, _device: &GpuDevice) -> GpuTelemetry {
        GpuTelemetry::default()
    }

    #[cfg(all(windows, feature = "adlx"))]
    fn controls(&self, device: &GpuDevice) -> Vec<GpuControl> {
        self.inner.as_ref().map(|a| a.controls(device)).unwrap_or_default()
    }

    #[cfg(all(windows, feature = "adlx"))]
    fn set_control(&self, device: &GpuDevice, id: &str, value: f64) -> anyhow::Result<()> {
        match &self.inner {
            Ok(a) => a.set_control(device, id, value),
            Err(e) => anyhow::bail!("ADLX is unavailable: {e}"),
        }
    }
}
