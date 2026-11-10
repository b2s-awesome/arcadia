//! In-memory GPU backends for tests and for the browser-only UI preview.
//! Nothing here touches hardware.

use super::*;
use parking_lot::Mutex;

pub struct MockBase {
    pub devices: Vec<GpuDevice>,
    pub telemetry: GpuTelemetry,
}

impl MockBase {
    pub fn new(devices: Vec<GpuDevice>) -> Self {
        Self { devices, telemetry: GpuTelemetry::default() }
    }
}

impl BaseEnumerator for MockBase {
    fn devices(&self) -> Vec<GpuDevice> {
        self.devices.clone()
    }
    fn telemetry(&self, _: &GpuDevice) -> GpuTelemetry {
        self.telemetry.clone()
    }
}

pub struct MockVendor {
    pub vendor: Vendor,
    pub status: ApiStatus,
    pub telemetry: GpuTelemetry,
    pub controls: Vec<GpuControl>,
    /// Every control value that reached the "driver".
    pub applied: Mutex<Vec<(String, f64)>>,
    /// Make `set_control` fail, to test rollback paths.
    pub fail_sets: bool,
}

impl MockVendor {
    pub fn available(vendor: Vendor) -> Self {
        Self {
            vendor,
            status: ApiStatus::Available { api: vendor.api_name().into(), version: Some("mock".into()) },
            telemetry: GpuTelemetry::default(),
            controls: Vec::new(),
            applied: Mutex::new(Vec::new()),
            fail_sets: false,
        }
    }
    pub fn missing(vendor: Vendor, reason: &str) -> Self {
        Self { status: ApiStatus::unavailable(vendor.api_name(), reason), ..Self::available(vendor) }
    }
}

impl VendorAdapter for MockVendor {
    fn vendor(&self) -> Vendor {
        self.vendor
    }
    fn status(&self) -> ApiStatus {
        self.status.clone()
    }
    fn enrich(&self, device: &mut GpuDevice) {
        device.kind_source = "vendor API".into();
    }
    fn telemetry(&self, _: &GpuDevice) -> GpuTelemetry {
        self.telemetry.clone()
    }
    fn controls(&self, _: &GpuDevice) -> Vec<GpuControl> {
        let applied = self.applied.lock();
        self.controls
            .iter()
            .cloned()
            .map(|mut c| {
                if let Some((_, v)) = applied.iter().rev().find(|(id, _)| *id == c.id) {
                    c.value = Some(*v);
                }
                c
            })
            .collect()
    }
    fn set_control(&self, _: &GpuDevice, id: &str, value: f64) -> anyhow::Result<()> {
        if self.fail_sets {
            anyhow::bail!("mock driver rejected {id}");
        }
        self.applied.lock().push((id.to_string(), value));
        Ok(())
    }
}
