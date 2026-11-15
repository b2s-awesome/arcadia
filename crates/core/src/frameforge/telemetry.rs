//! Live system readings (CPU, memory, each GPU).
//!
//! Sampling is pull-based: nothing here runs on its own. The UI asks while a
//! Performance page is visible, and the monitor asks while it is capturing.
//! One sample costs a CPU/memory refresh plus one vendor call per GPU.

use super::gpu::{GpuDevice, GpuRegistry, GpuTelemetry};
use super::presentmon::PmTelemetry;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuSample {
    pub id: String,
    pub name: String,
    #[serde(flatten)]
    pub telemetry: GpuTelemetry,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    /// Unix milliseconds.
    pub at_ms: u64,
    pub cpu_pct: Option<f64>,
    /// Busiest logical core, which a multi-core average hides.
    pub cpu_max_core_pct: Option<f64>,
    pub cpu_frequency_mhz: Option<f64>,
    pub cpu_temperature_c: Option<f64>,
    pub cpu_temperature_note: Option<String>,
    pub ram_total_bytes: u64,
    pub ram_used_bytes: u64,
    pub ram_available_bytes: u64,
    pub ram_pct: Option<f64>,
    pub swap_used_bytes: u64,
    pub swap_total_bytes: u64,
    pub gpus: Vec<GpuSample>,
}

impl Sample {
    pub fn gpu(&self, id: &str) -> Option<&GpuSample> {
        self.gpus.iter().find(|g| g.id == id)
    }

    /// Fold PresentMon service telemetry into this sample. PresentMon values
    /// fill gaps only; a vendor reading already present is kept.
    pub fn merge_presentmon(&mut self, pm: &PmTelemetry, gpu_id: Option<&str>) {
        if self.cpu_temperature_c.is_none() {
            if let Some(t) = pm.cpu_temperature_c {
                self.cpu_temperature_c = Some(t);
                self.cpu_temperature_note = None;
            }
        }
        if self.cpu_frequency_mhz.is_none() {
            self.cpu_frequency_mhz = pm.cpu_frequency_mhz;
        }
        let target = match gpu_id {
            Some(id) => self.gpus.iter_mut().find(|g| g.id == id),
            None => self.gpus.first_mut(),
        };
        let Some(g) = target else { return };
        let t = &mut g.telemetry;
        let fill = |slot: &mut Option<f64>, value: Option<f64>, metric: &str, sources: &mut std::collections::BTreeMap<String, String>| {
            if slot.is_none() {
                if let Some(v) = value {
                    *slot = Some(v);
                    sources.insert(metric.to_string(), "PresentMon".to_string());
                }
            }
        };
        fill(&mut t.power_w, pm.gpu_power_w, "power", &mut t.sources);
        fill(&mut t.temperature_c, pm.gpu_temperature_c, "temperature", &mut t.sources);
        fill(&mut t.utilization_pct, pm.gpu_utilization_pct, "utilization", &mut t.sources);
        fill(&mut t.fan_rpm, pm.gpu_fan_rpm, "fanRpm", &mut t.sources);
        fill(&mut t.core_clock_mhz, pm.gpu_frequency_mhz, "coreClock", &mut t.sources);
        if t.vram_used_bytes.is_none() {
            if let Some(v) = pm.gpu_mem_used_bytes {
                t.vram_used_bytes = Some(v as u64);
                t.sources.insert("vramUsed".into(), "PresentMon".into());
            }
        }
        if t.vram_total_bytes.is_none() {
            t.vram_total_bytes = pm.gpu_mem_size_bytes.map(|v| v as u64);
        }
        t.prune();
    }
}

pub struct Sampler {
    sys: Mutex<Option<sysinfo::System>>,
    gpus: GpuRegistry,
    devices: Mutex<Vec<GpuDevice>>,
}

impl Sampler {
    pub fn new(gpus: GpuRegistry) -> Self {
        let devices = gpus.devices();
        Self { sys: Mutex::new(sysinfo::System::new().ok()), gpus, devices: Mutex::new(devices) }
    }

    /// Re-read the adapter list (after a hardware rescan).
    pub fn refresh_devices(&self) {
        *self.devices.lock() = self.gpus.devices();
    }

    pub fn devices(&self) -> Vec<GpuDevice> {
        self.devices.lock().clone()
    }

    pub fn registry(&self) -> &GpuRegistry {
        &self.gpus
    }

    pub fn sample(&self) -> Sample {
        let mut s = Sample {
            at_ms: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0),
            cpu_temperature_note: Some("Windows has no general CPU temperature interface; shown when the PresentMon service reports it.".into()),
            ..Default::default()
        };
        if let Some(sys) = self.sys.lock().as_mut() {
            sys.refresh_cpu_usage();
            sys.refresh_memory();
            let cpus = sys.cpus();
            if !cpus.is_empty() {
                // The first refresh has no interval to measure, so usage reads 0 for every core.
                let any = cpus.iter().any(|c| c.usage() > 0.0);
                if any {
                    s.cpu_pct = Some(sys.global_cpu_usage() as f64);
                    s.cpu_max_core_pct = cpus.iter().map(|c| c.usage() as f64).fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))));
                }
                let f = cpus.iter().map(|c| c.frequency()).max().unwrap_or(0);
                s.cpu_frequency_mhz = (f > 0).then_some(f as f64);
            }
            s.ram_total_bytes = sys.total_memory();
            s.ram_used_bytes = sys.used_memory();
            s.ram_available_bytes = sys.available_memory();
            s.swap_total_bytes = sys.total_swap();
            s.swap_used_bytes = sys.used_swap();
            if s.ram_total_bytes > 0 {
                s.ram_pct = Some(s.ram_used_bytes as f64 / s.ram_total_bytes as f64 * 100.0);
            }
        }
        for d in self.devices.lock().iter() {
            s.gpus.push(GpuSample { id: d.id.clone(), name: d.name.clone(), telemetry: self.gpus.telemetry(d) });
        }
        s
    }
}

/// Averages over a run of samples, for benchmark records and diagnostics.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub samples: usize,
    pub cpu_avg_pct: Option<f64>,
    pub cpu_max_core_avg_pct: Option<f64>,
    pub ram_avg_pct: Option<f64>,
    pub ram_peak_bytes: Option<u64>,
    pub gpu_avg_pct: Option<f64>,
    pub gpu_temp_avg_c: Option<f64>,
    pub gpu_temp_max_c: Option<f64>,
    pub gpu_power_avg_w: Option<f64>,
    pub gpu_clock_avg_mhz: Option<f64>,
    pub gpu_clock_min_mhz: Option<f64>,
    pub gpu_clock_max_mhz: Option<f64>,
    pub vram_peak_bytes: Option<u64>,
    pub vram_total_bytes: Option<u64>,
    pub cpu_temp_max_c: Option<f64>,
    pub thermal_limited_samples: usize,
    pub power_limited_samples: usize,
}

pub fn summarize(samples: &[Sample], gpu_id: Option<&str>) -> Summary {
    use super::stats::mean_opt;
    let gpu = |s: &Sample| -> Option<GpuTelemetry> {
        match gpu_id {
            Some(id) => s.gpu(id).map(|g| g.telemetry.clone()),
            None => s.gpus.first().map(|g| g.telemetry.clone()),
        }
    };
    let g: Vec<GpuTelemetry> = samples.iter().filter_map(gpu).collect();
    let max = |it: &mut dyn Iterator<Item = Option<f64>>| it.flatten().fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))));
    let min = |it: &mut dyn Iterator<Item = Option<f64>>| it.flatten().fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.min(v))));
    Summary {
        samples: samples.len(),
        cpu_avg_pct: mean_opt(samples.iter().map(|s| s.cpu_pct)),
        cpu_max_core_avg_pct: mean_opt(samples.iter().map(|s| s.cpu_max_core_pct)),
        ram_avg_pct: mean_opt(samples.iter().map(|s| s.ram_pct)),
        ram_peak_bytes: samples.iter().map(|s| s.ram_used_bytes).max().filter(|v| *v > 0),
        gpu_avg_pct: mean_opt(g.iter().map(|t| t.utilization_pct)),
        gpu_temp_avg_c: mean_opt(g.iter().map(|t| t.temperature_c)),
        gpu_temp_max_c: max(&mut g.iter().map(|t| t.temperature_c)),
        gpu_power_avg_w: mean_opt(g.iter().map(|t| t.power_w)),
        gpu_clock_avg_mhz: mean_opt(g.iter().map(|t| t.core_clock_mhz)),
        gpu_clock_min_mhz: min(&mut g.iter().map(|t| t.core_clock_mhz)),
        gpu_clock_max_mhz: max(&mut g.iter().map(|t| t.core_clock_mhz)),
        vram_peak_bytes: g.iter().filter_map(|t| t.vram_used_bytes).max(),
        vram_total_bytes: g.iter().filter_map(|t| t.vram_total_bytes).max(),
        cpu_temp_max_c: max(&mut samples.iter().map(|s| s.cpu_temperature_c)),
        thermal_limited_samples: g.iter().filter(|t| t.thermal_limited == Some(true)).count(),
        power_limited_samples: g.iter().filter(|t| t.power_limited == Some(true)).count(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameforge::gpu::mock::{MockBase, MockVendor};
    use crate::frameforge::gpu::Vendor;
    use std::sync::Arc;

    fn registry() -> GpuRegistry {
        let dev = GpuDevice { id: "gpu0".into(), vendor: Vendor::Nvidia, name: "Test GPU".into(), vram_bytes: Some(8 << 30), ..Default::default() };
        let mut nv = MockVendor::available(Vendor::Nvidia);
        nv.telemetry.utilization_pct = Some(90.0);
        nv.telemetry.temperature_c = Some(70.0);
        GpuRegistry::new(Arc::new(MockBase::new(vec![dev])), vec![Arc::new(nv)])
    }

    #[test]
    fn sample_reads_memory_and_gpus() {
        let s = Sampler::new(registry()).sample();
        assert!(s.ram_total_bytes > 0);
        assert_eq!(s.gpus.len(), 1);
        assert_eq!(s.gpus[0].telemetry.temperature_c, Some(70.0));
        // Power is not available from the mock and says so.
        assert!(s.gpus[0].telemetry.power_w.is_none());
        assert!(s.gpus[0].telemetry.unavailable.contains_key("power"));
    }

    #[test]
    fn presentmon_fills_gaps_but_never_overrides_vendor_values() {
        let mut s = Sampler::new(registry()).sample();
        let pm = PmTelemetry { gpu_power_w: Some(215.0), gpu_temperature_c: Some(99.0), cpu_temperature_c: Some(64.0), ..Default::default() };
        s.merge_presentmon(&pm, Some("gpu0"));
        let t = &s.gpus[0].telemetry;
        assert_eq!(t.power_w, Some(215.0));
        assert_eq!(t.sources["power"], "PresentMon");
        assert!(!t.unavailable.contains_key("power"));
        assert_eq!(t.temperature_c, Some(70.0)); // vendor reading kept
        assert_eq!(s.cpu_temperature_c, Some(64.0));
        assert!(s.cpu_temperature_note.is_none());
    }

    #[test]
    fn device_metric_unavailable_stays_unavailable() {
        let s = Sample { gpus: vec![GpuSample { id: "g".into(), name: "g".into(), telemetry: GpuTelemetry::default() }], ..Default::default() };
        let sum = summarize(&[s.clone(), s], Some("g"));
        assert_eq!(sum.samples, 2);
        assert!(sum.gpu_avg_pct.is_none() && sum.gpu_temp_max_c.is_none() && sum.vram_peak_bytes.is_none());
    }

    #[test]
    fn summary_averages_and_peaks() {
        let mk = |util: f64, temp: f64, clock: f64, limited: bool| Sample {
            cpu_pct: Some(40.0),
            ram_used_bytes: 8 << 30,
            gpus: vec![GpuSample {
                id: "g".into(),
                name: "g".into(),
                telemetry: GpuTelemetry { utilization_pct: Some(util), temperature_c: Some(temp), core_clock_mhz: Some(clock), thermal_limited: Some(limited), ..Default::default() },
            }],
            ..Default::default()
        };
        let sum = summarize(&[mk(90.0, 70.0, 2000.0, false), mk(100.0, 84.0, 1500.0, true)], None);
        assert_eq!(sum.gpu_avg_pct, Some(95.0));
        assert_eq!(sum.gpu_temp_max_c, Some(84.0));
        assert_eq!((sum.gpu_clock_min_mhz, sum.gpu_clock_max_mhz), (Some(1500.0), Some(2000.0)));
        assert_eq!(sum.thermal_limited_samples, 1);
    }
}
