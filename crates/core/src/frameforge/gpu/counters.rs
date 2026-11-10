//! Parsing and aggregation for Windows' "GPU Engine" and "GPU Adapter
//! Memory" performance counters. Pure code, shared by the PDH reader and its
//! tests.
//!
//! Instance names look like
//! `pid_1234_luid_0x00000000_0x0000D3E5_phys_0_eng_3_engtype_3D` and
//! `luid_0x00000000_0x0000D3E5_phys_0`.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct EngineInstance {
    pub pid: u32,
    /// `high:low`, the same form as [`super::GpuDevice::luid`].
    pub luid: String,
    pub engine: u32,
    pub engine_type: String,
}

fn field<'a>(parts: &[&'a str], key: &str) -> Option<&'a str> {
    parts.iter().position(|p| *p == key).and_then(|i| parts.get(i + 1)).copied()
}

fn hex(s: &str) -> Option<u32> {
    u32::from_str_radix(s.trim_start_matches("0x").trim_start_matches("0X"), 16).ok()
}

/// LUID from an instance name, as `high:low`.
pub fn parse_luid(instance: &str) -> Option<String> {
    let parts: Vec<&str> = instance.split('_').collect();
    let i = parts.iter().position(|p| *p == "luid")?;
    let high = hex(parts.get(i + 1)?)?;
    let low = hex(parts.get(i + 2)?)?;
    Some(format!("{}:{}", high as i32, low))
}

pub fn parse_engine(instance: &str) -> Option<EngineInstance> {
    let parts: Vec<&str> = instance.split('_').collect();
    Some(EngineInstance {
        pid: field(&parts, "pid")?.parse().ok()?,
        luid: parse_luid(instance)?,
        engine: field(&parts, "eng")?.parse().ok()?,
        // The type may itself contain underscores ("engtype_Video_Decode").
        engine_type: instance.split("engtype_").nth(1).unwrap_or("").to_string(),
    })
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AdapterUsage {
    /// Busiest engine on the adapter, 0–100.
    pub utilization_pct: f64,
    /// Per-process share of that adapter's 3D engines, 0–100.
    pub by_pid: HashMap<u32, f64>,
}

/// Turn raw `(instance, percent)` pairs into per-adapter utilisation.
///
/// Each engine's load is the sum over processes; the adapter's figure is its
/// busiest engine (how Task Manager's GPU column works). Summing every
/// engine would double-count and can exceed 100%.
pub fn aggregate_engines(samples: &[(String, f64)]) -> HashMap<String, AdapterUsage> {
    let mut per_engine: HashMap<(String, u32), f64> = HashMap::new();
    let mut out: HashMap<String, AdapterUsage> = HashMap::new();
    for (name, value) in samples {
        let Some(e) = parse_engine(name) else { continue };
        if !value.is_finite() || *value < 0.0 {
            continue;
        }
        *per_engine.entry((e.luid.clone(), e.engine)).or_default() += value;
        let usage = out.entry(e.luid).or_default();
        if e.engine_type == "3D" {
            *usage.by_pid.entry(e.pid).or_default() += value;
        }
    }
    for ((luid, _), total) in per_engine {
        let u = out.entry(luid).or_default();
        u.utilization_pct = u.utilization_pct.max(total.min(100.0));
    }
    for u in out.values_mut() {
        u.by_pid.values_mut().for_each(|v| *v = v.min(100.0));
    }
    out
}

/// Sum "Dedicated Usage" bytes per adapter.
pub fn aggregate_memory(samples: &[(String, f64)]) -> HashMap<String, u64> {
    let mut out: HashMap<String, u64> = HashMap::new();
    for (name, value) in samples {
        if let Some(luid) = parse_luid(name) {
            if value.is_finite() && *value >= 0.0 {
                *out.entry(luid).or_default() += *value as u64;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_engine_instances() {
        let e = parse_engine("pid_4321_luid_0x00000000_0x0000D3E5_phys_0_eng_3_engtype_3D").unwrap();
        assert_eq!(e, EngineInstance { pid: 4321, luid: "0:54245".into(), engine: 3, engine_type: "3D".into() });
        let v = parse_engine("pid_8_luid_0x00000001_0x00000010_phys_0_eng_7_engtype_Video_Decode").unwrap();
        assert_eq!((v.luid.as_str(), v.engine_type.as_str()), ("1:16", "Video_Decode"));
        assert!(parse_engine("garbage").is_none());
        assert_eq!(parse_luid("luid_0x00000000_0x0000D3E5_phys_0").as_deref(), Some("0:54245"));
    }

    #[test]
    fn utilisation_is_the_busiest_engine_not_the_sum() {
        let s = vec![
            ("pid_1_luid_0x00000000_0x0000000A_phys_0_eng_0_engtype_3D".to_string(), 60.0),
            ("pid_2_luid_0x00000000_0x0000000A_phys_0_eng_0_engtype_3D".to_string(), 25.0),
            ("pid_1_luid_0x00000000_0x0000000A_phys_0_eng_1_engtype_Copy".to_string(), 40.0),
            ("pid_9_luid_0x00000000_0x0000000B_phys_0_eng_0_engtype_3D".to_string(), 5.0),
            ("bad".to_string(), 99.0),
        ];
        let a = aggregate_engines(&s);
        assert!((a["0:10"].utilization_pct - 85.0).abs() < 1e-9);
        assert!((a["0:10"].by_pid[&1] - 60.0).abs() < 1e-9);
        assert!((a["0:11"].utilization_pct - 5.0).abs() < 1e-9);
    }

    #[test]
    fn utilisation_is_capped_and_memory_is_summed() {
        let s = vec![
            ("pid_1_luid_0x00000000_0x0000000A_phys_0_eng_0_engtype_3D".to_string(), 80.0),
            ("pid_2_luid_0x00000000_0x0000000A_phys_0_eng_0_engtype_3D".to_string(), 80.0),
        ];
        assert_eq!(aggregate_engines(&s)["0:10"].utilization_pct, 100.0);
        let m = vec![("luid_0x00000000_0x0000000A_phys_0".to_string(), 1024.0), ("luid_0x00000000_0x0000000A_phys_1".to_string(), 1024.0)];
        assert_eq!(aggregate_memory(&m)["0:10"], 2048);
    }
}
