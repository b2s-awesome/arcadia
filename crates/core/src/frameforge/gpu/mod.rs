//! Vendor-independent GPU layer.
//!
//! ```text
//!            GpuRegistry  (what the rest of FrameForge talks to)
//!           /     |      \
//!   base enumerator      VendorAdapter × N
//!   (DXGI + Windows      NVAPI (NVIDIA) · ADLX (AMD) · IGCL (Intel)
//!    perf counters)      each loaded at run time from the installed driver
//! ```
//!
//! * The **base enumerator** lists adapters and gives vendor-neutral
//!   telemetry (utilisation, VRAM use) that Windows itself measures.
//! * A **vendor adapter** adds what only the driver knows: temperature,
//!   clocks, fan speed, power, driver controls.
//! * A shared interface is not shared capability. Controls and presets stay
//!   tagged with their vendor and are never sent to another vendor's driver.
//!
//! Adding a vendor API means implementing [`VendorAdapter`] and registering
//! it in [`GpuRegistry::detect`]; nothing else in the engine changes.

pub mod amd;
pub mod counters;
#[cfg(windows)]
pub mod dxgi;
pub mod intel;
pub mod mock;
pub mod nvidia;
#[cfg(windows)]
pub mod pdh;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Vendor {
    Nvidia,
    Amd,
    Intel,
    #[default]
    Other,
}

impl Vendor {
    pub fn from_pci(vendor_id: u32) -> Self {
        match vendor_id {
            0x10DE => Vendor::Nvidia,
            0x1002 | 0x1022 => Vendor::Amd,
            0x8086 => Vendor::Intel,
            _ => Vendor::Other,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Vendor::Nvidia => "NVIDIA",
            Vendor::Amd => "AMD",
            Vendor::Intel => "Intel",
            Vendor::Other => "Other",
        }
    }
    /// The vendor's control API, as shown in the UI.
    pub fn api_name(self) -> &'static str {
        match self {
            Vendor::Nvidia => "NVAPI",
            Vendor::Amd => "ADLX",
            Vendor::Intel => "IGCL",
            Vendor::Other => "none",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GpuKind {
    Discrete,
    Integrated,
    /// Software renderer (e.g. Microsoft Basic Render Driver).
    Software,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuDevice {
    /// Stable within a session: `<vendor id>:<device id>:<luid>`.
    pub id: String,
    pub vendor: Vendor,
    pub name: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub subsystem_id: u32,
    pub revision: u32,
    /// Windows adapter LUID as `high:low`, when known.
    pub luid: Option<String>,
    pub kind: GpuKind,
    /// Where `kind` came from: "vendor API" or "heuristic".
    pub kind_source: String,
    pub driver_version: Option<String>,
    /// Vendor's marketing driver version (e.g. NVIDIA "566.14").
    pub driver_version_vendor: Option<String>,
    pub driver_branch: Option<String>,
    pub vram_bytes: Option<u64>,
    pub shared_bytes: Option<u64>,
    /// GDI names of the displays attached to this adapter.
    pub outputs: Vec<String>,
    /// Hardware / driver features the vendor API reported.
    pub features: Vec<String>,
    /// State of the vendor API for this device.
    pub api: ApiStatus,
}

/// Whether a vendor API can be used, and if not, why.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ApiStatus {
    Available {
        api: String,
        version: Option<String>,
    },
    Unavailable {
        api: String,
        reason: String,
    },
    /// No vendor API exists for this adapter (other vendors, software adapters).
    #[default]
    NotApplicable,
}

impl ApiStatus {
    pub fn is_available(&self) -> bool {
        matches!(self, ApiStatus::Available { .. })
    }
    pub fn unavailable(api: &str, reason: impl Into<String>) -> Self {
        ApiStatus::Unavailable { api: api.into(), reason: reason.into() }
    }
}

/// Live readings for one GPU. A missing value has an entry in `unavailable`
/// saying why; nothing is ever estimated.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuTelemetry {
    pub utilization_pct: Option<f64>,
    pub vram_used_bytes: Option<u64>,
    pub vram_total_bytes: Option<u64>,
    pub temperature_c: Option<f64>,
    pub hotspot_c: Option<f64>,
    pub power_w: Option<f64>,
    pub fan_rpm: Option<f64>,
    pub fan_pct: Option<f64>,
    pub core_clock_mhz: Option<f64>,
    pub memory_clock_mhz: Option<f64>,
    pub voltage_v: Option<f64>,
    /// Driver reports the GPU is being limited by temperature / power.
    pub thermal_limited: Option<bool>,
    pub power_limited: Option<bool>,
    /// metric → which API supplied it.
    pub sources: BTreeMap<String, String>,
    /// metric → why it is missing.
    pub unavailable: BTreeMap<String, String>,
}

pub const METRICS: &[&str] = &[
    "utilization", "vramUsed", "temperature", "power", "fanRpm", "fanPct", "coreClock", "memoryClock",
];

impl GpuTelemetry {
    /// Fill gaps in `self` from `other`; values already present win.
    pub fn merge_missing(&mut self, other: &GpuTelemetry) {
        macro_rules! take {
            ($($f:ident),*) => { $( if self.$f.is_none() { self.$f = other.$f; } )* };
        }
        take!(utilization_pct, vram_used_bytes, vram_total_bytes, temperature_c, hotspot_c, power_w, fan_rpm, fan_pct, core_clock_mhz, memory_clock_mhz, voltage_v, thermal_limited, power_limited);
        for (k, v) in &other.sources {
            self.sources.entry(k.clone()).or_insert_with(|| v.clone());
        }
        for (k, v) in &other.unavailable {
            self.unavailable.entry(k.clone()).or_insert_with(|| v.clone());
        }
        self.prune();
    }

    fn has(&self, metric: &str) -> bool {
        match metric {
            "utilization" => self.utilization_pct.is_some(),
            "vramUsed" => self.vram_used_bytes.is_some(),
            "temperature" => self.temperature_c.is_some(),
            "power" => self.power_w.is_some(),
            "fanRpm" => self.fan_rpm.is_some(),
            "fanPct" => self.fan_pct.is_some(),
            "coreClock" => self.core_clock_mhz.is_some(),
            "memoryClock" => self.memory_clock_mhz.is_some(),
            _ => false,
        }
    }

    /// Drop "unavailable" notes for metrics that did arrive, and add a
    /// generic note for any still missing without one.
    pub fn prune(&mut self) {
        for m in METRICS {
            if self.has(m) {
                self.unavailable.remove(*m);
            }
        }
    }

    pub fn note_missing(&mut self, reason: &str) {
        for m in METRICS {
            if !self.has(m) {
                self.unavailable.entry((*m).to_string()).or_insert_with(|| reason.to_string());
            }
        }
    }

    pub(crate) fn set(&mut self, metric: &str, source: &str) {
        self.sources.insert(metric.to_string(), source.to_string());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    /// Documented, reversible, no stability impact.
    Low,
    /// Reversible, but changes how the system or driver behaves.
    Medium,
    /// Can make the system unstable; needs explicit confirmation.
    High,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ControlKind {
    Toggle,
    Range { min: f64, max: f64, step: f64, unit: String },
    Choice { options: Vec<ControlOption> },
    /// Shown, never editable.
    ReadOnly,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlOption {
    pub value: f64,
    pub label: String,
}

/// One driver control a vendor API exposes for a device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuControl {
    /// Vendor-scoped id, e.g. `adlx.antilag` or `igcl.power_limit`.
    pub id: String,
    pub vendor: Vendor,
    pub name: String,
    pub description: String,
    pub category: String,
    pub kind: ControlKind,
    /// Current value (toggles: 0 / 1). `None` when it can't be read.
    pub value: Option<f64>,
    pub default: Option<f64>,
    pub supported: bool,
    /// Why it can't be used on this device, when `supported` is false.
    pub unsupported_reason: Option<String>,
    pub risk: Risk,
    /// 1 = safe, 2 = advanced, 3 = expert.
    pub level: u8,
    pub requires_restart: bool,
}

/// What a vendor driver contributes for the devices it owns.
pub trait VendorAdapter: Send + Sync {
    fn vendor(&self) -> Vendor;
    /// Can the API be used right now?
    fn status(&self) -> ApiStatus;
    /// Add vendor-only facts (driver version, type, features) to a device.
    fn enrich(&self, _device: &mut GpuDevice) {}
    /// Vendor telemetry for one device. Must leave unknown fields `None`.
    fn telemetry(&self, device: &GpuDevice) -> GpuTelemetry;
    /// Controls the API exposes for this device (supported or not, with reasons).
    fn controls(&self, _device: &GpuDevice) -> Vec<GpuControl> {
        Vec::new()
    }
    /// Apply one control. Implementations validate range and support first.
    fn set_control(&self, _device: &GpuDevice, id: &str, _value: f64) -> anyhow::Result<()> {
        anyhow::bail!("{} does not expose the control '{id}'", self.vendor().api_name())
    }
    /// Per-display variable-refresh information: GDI device name → supported.
    fn vrr(&self, _device: &GpuDevice) -> BTreeMap<String, bool> {
        BTreeMap::new()
    }
}

/// Lists adapters and supplies the vendor-neutral readings.
pub trait BaseEnumerator: Send + Sync {
    fn devices(&self) -> Vec<GpuDevice>;
    fn telemetry(&self, device: &GpuDevice) -> GpuTelemetry;
}

/// Every GPU in the machine plus the vendor adapters that can talk to them.
#[derive(Clone)]
pub struct GpuRegistry {
    base: Option<Arc<dyn BaseEnumerator>>,
    adapters: Vec<Arc<dyn VendorAdapter>>,
}

impl GpuRegistry {
    pub fn empty() -> Self {
        Self { base: None, adapters: Vec::new() }
    }

    pub fn new(base: Arc<dyn BaseEnumerator>, adapters: Vec<Arc<dyn VendorAdapter>>) -> Self {
        Self { base: Some(base), adapters }
    }

    /// Real hardware: DXGI for the adapter list, then each vendor API that
    /// loads. A vendor API that fails to load stays registered so the UI can
    /// say why it is unavailable.
    pub fn detect() -> Self {
        #[cfg(windows)]
        {
            let base: Arc<dyn BaseEnumerator> = Arc::new(dxgi::DxgiEnumerator::new());
            let adapters: Vec<Arc<dyn VendorAdapter>> = vec![
                Arc::new(nvidia::NvidiaAdapter::load()),
                Arc::new(amd::AmdAdapter::load()),
                Arc::new(intel::IntelAdapter::load()),
            ];
            Self { base: Some(base), adapters }
        }
        #[cfg(not(windows))]
        {
            Self::empty()
        }
    }

    pub fn adapter_for(&self, vendor: Vendor) -> Option<&Arc<dyn VendorAdapter>> {
        self.adapters.iter().find(|a| a.vendor() == vendor)
    }

    /// Status of each vendor API, whether or not that vendor's hardware is present.
    pub fn api_statuses(&self) -> Vec<(Vendor, ApiStatus)> {
        self.adapters.iter().map(|a| (a.vendor(), a.status())).collect()
    }

    pub fn devices(&self) -> Vec<GpuDevice> {
        let Some(base) = &self.base else { return Vec::new() };
        let mut devices = base.devices();
        for d in &mut devices {
            match self.adapter_for(d.vendor) {
                Some(a) => {
                    d.api = a.status();
                    if d.api.is_available() {
                        a.enrich(d);
                    }
                }
                None => d.api = ApiStatus::NotApplicable,
            }
            if d.kind_source.is_empty() {
                d.kind_source = "heuristic".into();
            }
        }
        devices
    }

    /// Vendor readings first, vendor-neutral readings fill the gaps.
    pub fn telemetry(&self, device: &GpuDevice) -> GpuTelemetry {
        let mut t = GpuTelemetry::default();
        if let Some(a) = self.adapter_for(device.vendor) {
            match a.status() {
                ApiStatus::Available { .. } => t = a.telemetry(device),
                ApiStatus::Unavailable { api, reason } => t.note_missing(&format!("{api} unavailable: {reason}")),
                ApiStatus::NotApplicable => {}
            }
        }
        if let Some(base) = &self.base {
            t.merge_missing(&base.telemetry(device));
        }
        if t.vram_total_bytes.is_none() {
            t.vram_total_bytes = device.vram_bytes;
        }
        t.note_missing("Not exposed by the driver or Windows for this adapter");
        t.prune();
        t
    }

    pub fn controls(&self, device: &GpuDevice) -> Vec<GpuControl> {
        match self.adapter_for(device.vendor) {
            Some(a) if a.status().is_available() => a.controls(device),
            _ => Vec::new(),
        }
    }

    /// Apply a control after checking it belongs to this device's vendor,
    /// is supported, and the value is inside the driver-reported range.
    pub fn set_control(&self, device: &GpuDevice, id: &str, value: f64) -> anyhow::Result<()> {
        let adapter = self
            .adapter_for(device.vendor)
            .filter(|a| a.status().is_available())
            .ok_or_else(|| anyhow::anyhow!("{} is not available for {}", device.vendor.api_name(), device.name))?;
        let control = adapter
            .controls(device)
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| anyhow::anyhow!("'{id}' is not a control of {}", device.name))?;
        validate_control(&control, device.vendor, value)?;
        adapter.set_control(device, id, value)
    }
}

/// Reject a value before it reaches a driver.
pub fn validate_control(control: &GpuControl, vendor: Vendor, value: f64) -> anyhow::Result<()> {
    if control.vendor != vendor {
        anyhow::bail!("'{}' is a {} control and cannot be applied to a {} GPU", control.name, control.vendor.label(), vendor.label());
    }
    if !control.supported {
        anyhow::bail!("'{}' is not supported here: {}", control.name, control.unsupported_reason.clone().unwrap_or_else(|| "no reason given by the driver".into()));
    }
    if !value.is_finite() {
        anyhow::bail!("'{}' needs a finite number", control.name);
    }
    match &control.kind {
        ControlKind::ReadOnly => anyhow::bail!("'{}' is read-only", control.name),
        ControlKind::Toggle => {
            if value != 0.0 && value != 1.0 {
                anyhow::bail!("'{}' is an on/off control", control.name);
            }
        }
        ControlKind::Range { min, max, .. } => {
            if value < *min || value > *max {
                anyhow::bail!("'{}' must be between {min} and {max} (driver-reported limits)", control.name);
            }
        }
        ControlKind::Choice { options } => {
            if !options.iter().any(|o| o.value == value) {
                anyhow::bail!("'{}' has no option with value {value}", control.name);
            }
        }
    }
    Ok(())
}

/// Guess discrete vs integrated when no vendor API says. Marked "heuristic".
pub fn guess_kind(vendor: Vendor, name: &str, dedicated_vram: u64, software: bool) -> GpuKind {
    if software {
        return GpuKind::Software;
    }
    let n = name.to_lowercase();
    match vendor {
        Vendor::Nvidia => GpuKind::Discrete,
        Vendor::Intel => {
            if n.contains("arc") && !n.contains("arc(tm) graphics") && dedicated_vram >= (1 << 30) {
                GpuKind::Discrete
            } else if dedicated_vram >= (2u64 << 30) {
                GpuKind::Discrete
            } else {
                GpuKind::Integrated
            }
        }
        Vendor::Amd => {
            // APUs report little dedicated memory and usually say "Graphics" without an RX/Pro model.
            if dedicated_vram < (1 << 30) || (n.contains("radeon(tm) graphics") || n.contains("vega")) && !n.contains(" rx ") {
                GpuKind::Integrated
            } else {
                GpuKind::Discrete
            }
        }
        Vendor::Other => GpuKind::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::mock::{MockBase, MockVendor};
    use super::*;

    fn dev(vendor: Vendor, name: &str) -> GpuDevice {
        GpuDevice { id: name.into(), vendor, name: name.into(), vram_bytes: Some(8 << 30), ..Default::default() }
    }

    #[test]
    fn enumerates_multiple_gpus_and_tags_api_state() {
        let base = MockBase::new(vec![dev(Vendor::Nvidia, "RTX"), dev(Vendor::Intel, "UHD"), dev(Vendor::Other, "Basic")]);
        let nv = MockVendor::available(Vendor::Nvidia);
        let intel = MockVendor::missing(Vendor::Intel, "ControlLib.dll was not found");
        let reg = GpuRegistry::new(Arc::new(base), vec![Arc::new(nv), Arc::new(intel)]);
        let d = reg.devices();
        assert_eq!(d.len(), 3);
        assert!(d[0].api.is_available());
        assert_eq!(d[0].kind_source, "vendor API"); // set by the mock's enrich
        assert!(matches!(&d[1].api, ApiStatus::Unavailable { reason, .. } if reason.contains("ControlLib")));
        assert_eq!(d[2].api, ApiStatus::NotApplicable);
        assert_eq!(d[2].kind_source, "heuristic");
    }

    #[test]
    fn missing_vendor_api_falls_back_to_base_readings_with_reasons() {
        let mut base = MockBase::new(vec![dev(Vendor::Amd, "RX")]);
        base.telemetry.utilization_pct = Some(55.0);
        base.telemetry.vram_used_bytes = Some(3 << 30);
        let amd = MockVendor::missing(Vendor::Amd, "driver not installed");
        let reg = GpuRegistry::new(Arc::new(base), vec![Arc::new(amd)]);
        let d = &reg.devices()[0];
        let t = reg.telemetry(d);
        assert_eq!(t.utilization_pct, Some(55.0));
        assert_eq!(t.vram_total_bytes, Some(8 << 30));
        assert!(t.temperature_c.is_none());
        assert!(t.unavailable["temperature"].contains("driver not installed"));
        assert!(!t.unavailable.contains_key("utilization"));
    }

    #[test]
    fn vendor_values_win_over_base_values() {
        let mut base = MockBase::new(vec![dev(Vendor::Nvidia, "RTX")]);
        base.telemetry.utilization_pct = Some(10.0);
        let mut nv = MockVendor::available(Vendor::Nvidia);
        nv.telemetry.utilization_pct = Some(97.0);
        nv.telemetry.temperature_c = Some(71.0);
        let reg = GpuRegistry::new(Arc::new(base), vec![Arc::new(nv)]);
        let t = reg.telemetry(&reg.devices()[0]);
        assert_eq!(t.utilization_pct, Some(97.0));
        assert_eq!(t.temperature_c, Some(71.0));
        assert!(t.power_w.is_none() && t.unavailable.contains_key("power"));
    }

    #[test]
    fn unsupported_hardware_has_no_controls_and_rejects_sets() {
        let base = MockBase::new(vec![dev(Vendor::Other, "Matrox")]);
        let reg = GpuRegistry::new(Arc::new(base), vec![]);
        let d = &reg.devices()[0];
        assert!(reg.controls(d).is_empty());
        assert!(reg.set_control(d, "x", 1.0).is_err());
    }

    #[test]
    fn controls_are_validated_before_reaching_the_driver() {
        let base = MockBase::new(vec![dev(Vendor::Amd, "RX")]);
        let mut amd = MockVendor::available(Vendor::Amd);
        amd.controls = vec![
            GpuControl {
                id: "adlx.power".into(), vendor: Vendor::Amd, name: "Power limit".into(), description: String::new(), category: "Tuning".into(),
                kind: ControlKind::Range { min: -10.0, max: 15.0, step: 1.0, unit: "%".into() }, value: Some(0.0), default: Some(0.0),
                supported: true, unsupported_reason: None, risk: Risk::High, level: 3, requires_restart: false,
            },
            GpuControl {
                id: "adlx.chill".into(), vendor: Vendor::Amd, name: "Chill".into(), description: String::new(), category: "3D".into(),
                kind: ControlKind::Toggle, value: None, default: None, supported: false, unsupported_reason: Some("not on this GPU".into()),
                risk: Risk::Low, level: 2, requires_restart: false,
            },
        ];
        let amd = Arc::new(amd);
        let reg = GpuRegistry::new(Arc::new(base), vec![amd.clone()]);
        let d = &reg.devices()[0];
        assert!(reg.set_control(d, "adlx.power", 16.0).unwrap_err().to_string().contains("between"));
        assert!(reg.set_control(d, "adlx.chill", 1.0).unwrap_err().to_string().contains("not on this GPU"));
        assert!(reg.set_control(d, "nope", 1.0).is_err());
        assert!(amd.applied.lock().is_empty());
        reg.set_control(d, "adlx.power", 10.0).unwrap();
        assert_eq!(amd.applied.lock().as_slice(), &[("adlx.power".to_string(), 10.0)]);
    }

    #[test]
    fn a_control_never_crosses_vendors() {
        let c = GpuControl {
            id: "adlx.antilag".into(), vendor: Vendor::Amd, name: "Anti-Lag".into(), description: String::new(), category: String::new(),
            kind: ControlKind::Toggle, value: None, default: None, supported: true, unsupported_reason: None, risk: Risk::Low, level: 2, requires_restart: false,
        };
        let e = validate_control(&c, Vendor::Nvidia, 1.0).unwrap_err().to_string();
        assert!(e.contains("AMD control") && e.contains("NVIDIA GPU"));
    }

    #[test]
    fn kind_heuristics() {
        assert_eq!(guess_kind(Vendor::Intel, "Intel(R) UHD Graphics 770", 128 << 20, false), GpuKind::Integrated);
        assert_eq!(guess_kind(Vendor::Intel, "Intel(R) Arc(TM) A770 Graphics", 16u64 << 30, false), GpuKind::Discrete);
        assert_eq!(guess_kind(Vendor::Amd, "AMD Radeon(TM) Graphics", 512 << 20, false), GpuKind::Integrated);
        assert_eq!(guess_kind(Vendor::Amd, "AMD Radeon RX 7800 XT", 16u64 << 30, false), GpuKind::Discrete);
        assert_eq!(guess_kind(Vendor::Other, "Microsoft Basic Render Driver", 0, true), GpuKind::Software);
    }
}
