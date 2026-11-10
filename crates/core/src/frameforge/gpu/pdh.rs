//! Vendor-neutral GPU readings from Windows performance counters (PDH):
//! per-adapter utilisation, dedicated memory in use, and per-process GPU
//! load. These are the counters Task Manager shows, so they exist for every
//! WDDM 2.x adapter regardless of vendor.

use super::counters::{aggregate_engines, aggregate_memory, AdapterUsage};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::System::Performance::*;

const PDH_MORE_DATA: u32 = 0x800007D2;
const ERROR_SUCCESS: u32 = 0;

#[derive(Default, Clone)]
pub struct Snapshot {
    pub usage: HashMap<String, AdapterUsage>,
    pub dedicated_bytes: HashMap<String, u64>,
}

pub struct GpuCounters {
    query: PDH_HQUERY,
    engine: Option<PDH_HCOUNTER>,
    memory: Option<PDH_HCOUNTER>,
    collected_once: bool,
    cache: Option<(Instant, Snapshot)>,
}

// SAFETY: PDH handles may be used from any thread; access is serialised by
// the Mutex that owns this struct.
unsafe impl Send for GpuCounters {}

impl GpuCounters {
    pub fn open() -> Option<Self> {
        let mut query = PDH_HQUERY::default();
        // SAFETY: valid out-pointer; real-time data source (null).
        if unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) } != ERROR_SUCCESS {
            return None;
        }
        let add = |path: &str| -> Option<PDH_HCOUNTER> {
            let mut c = PDH_HCOUNTER::default();
            // SAFETY: NUL-terminated wide string that outlives the call.
            (unsafe { PdhAddEnglishCounterW(query, &HSTRING::from(path), 0, &mut c) } == ERROR_SUCCESS).then_some(c)
        };
        let engine = add(r"\GPU Engine(*)\Utilization Percentage");
        let memory = add(r"\GPU Adapter Memory(*)\Dedicated Usage");
        if engine.is_none() && memory.is_none() {
            // SAFETY: query handle from PdhOpenQueryW.
            unsafe { PdhCloseQuery(query) };
            return None;
        }
        Some(Self { query, engine, memory, collected_once: false, cache: None })
    }

    fn read_array(counter: PDH_HCOUNTER) -> Vec<(String, f64)> {
        let (mut size, mut count) = (0u32, 0u32);
        // SAFETY: size query with no buffer.
        let st = unsafe { PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, None) };
        if st != PDH_MORE_DATA || size == 0 {
            return Vec::new();
        }
        // The buffer holds the item array followed by the name strings; align it for the items.
        let mut buf = vec![0u64; (size as usize).div_ceil(8)];
        let items = buf.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
        // SAFETY: buffer is at least `size` bytes and suitably aligned.
        if unsafe { PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, Some(items)) } != ERROR_SUCCESS {
            return Vec::new();
        }
        (0..count as usize)
            .filter_map(|i| {
                // SAFETY: PDH wrote `count` items; names point into `buf`.
                let it = unsafe { &*items.add(i) };
                if it.FmtValue.CStatus != 0 || it.szName.is_null() {
                    return None;
                }
                let name = unsafe { it.szName.to_string() }.ok()?;
                Some((name, unsafe { it.FmtValue.Anonymous.doubleValue }))
            })
            .collect()
    }

    /// Latest readings, at most one collection per 500 ms.
    pub fn snapshot(&mut self) -> Snapshot {
        if let Some((at, s)) = &self.cache {
            if at.elapsed() < Duration::from_millis(500) {
                return s.clone();
            }
        }
        // SAFETY: query handle owned by self.
        let ok = unsafe { PdhCollectQueryData(self.query) } == ERROR_SUCCESS;
        let mut snap = Snapshot::default();
        if ok {
            // Rate counters need two collections before they have a value.
            if self.collected_once {
                if let Some(c) = self.engine {
                    snap.usage = aggregate_engines(&Self::read_array(c));
                }
            }
            if let Some(c) = self.memory {
                snap.dedicated_bytes = aggregate_memory(&Self::read_array(c));
            }
            self.collected_once = true;
        }
        self.cache = Some((Instant::now(), snap.clone()));
        snap
    }
}

impl Drop for GpuCounters {
    fn drop(&mut self) {
        // SAFETY: query handle owned by self.
        unsafe { PdhCloseQuery(self.query) };
    }
}
