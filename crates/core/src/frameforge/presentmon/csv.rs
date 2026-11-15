//! Parser for the CSV the PresentMon console application writes.
//!
//! Columns are found by header name, so all three header generations work:
//! 1.x (`msBetweenPresents`, `Dropped`), 2.0–2.2 (`FrameTime`, `CPUBusy`,
//! `DisplayedTime`) and current (`MsBetweenAppStart`, `MsCPUBusy`,
//! `MsBetweenDisplayChange`). Unknown columns are ignored; a file with no
//! usable frame-time column is rejected.

use super::Frame;

#[derive(Debug, Clone, Default)]
struct Columns {
    time: Option<(usize, f64)>, // column, multiplier to ms
    frame_time: Option<usize>,
    between_presents: Option<usize>,
    displayed_time: Option<usize>,
    cpu_busy: Option<usize>,
    cpu_wait: Option<usize>,
    gpu_busy: Option<usize>,
    gpu_wait: Option<usize>,
    gpu_time: Option<usize>,
    display_latency: Option<usize>,
    dropped: Option<usize>,
    frame_type: Option<usize>,
    allows_tearing: Option<usize>,
    process_id: Option<usize>,
    application: Option<usize>,
    /// "CPUStartTime" is seconds by default but milliseconds with `--qpc_time_ms`-style
    /// options; the unit is settled from the data (see `settle_time_unit`).
    time_unit_ambiguous: bool,
}

/// Incremental parser: feed it lines as they arrive.
#[derive(Debug, Default)]
pub struct CsvParser {
    cols: Option<Columns>,
    /// Running clock for files that have no time column.
    clock_ms: f64,
    prev_raw_time: Option<f64>,
    pub skipped_lines: usize,
}

/// Longest line the parser will look at. PresentMon rows are a few hundred bytes.
pub const MAX_LINE_LEN: usize = 8192;

fn split(line: &str) -> Vec<&str> {
    // PresentMon never quotes fields; application names cannot contain commas on Windows paths' file names it emits.
    line.split(',').map(|f| f.trim()).collect()
}

fn num(fields: &[&str], col: Option<usize>) -> Option<f64> {
    let s = fields.get(col?)?;
    if s.is_empty() || s.eq_ignore_ascii_case("NA") {
        return None;
    }
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

impl CsvParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_header(&self) -> bool {
        self.cols.is_some()
    }

    fn parse_header(line: &str) -> Option<Columns> {
        let names: Vec<String> = split(line).iter().map(|s| s.to_ascii_lowercase()).collect();
        let find = |aliases: &[&str]| aliases.iter().find_map(|a| names.iter().position(|n| n == a));
        let mut c = Columns {
            frame_time: find(&["frametime", "msbetweenappstart"]),
            between_presents: find(&["msbetweenpresents"]),
            displayed_time: find(&["displayedtime", "msdisplaytime", "msbetweendisplaychange"]),
            cpu_busy: find(&["cpubusy", "mscpubusy"]),
            cpu_wait: find(&["cpuwait", "mscpuwait"]),
            gpu_busy: find(&["gpubusy", "msgpubusy", "msgpuactive"]),
            gpu_wait: find(&["gpuwait", "msgpuwait"]),
            gpu_time: find(&["gputime", "msgputime"]),
            display_latency: find(&["displaylatency", "msuntildisplayed"]),
            dropped: find(&["dropped"]),
            frame_type: find(&["frametype"]),
            allows_tearing: find(&["allowstearing"]),
            process_id: find(&["processid"]),
            application: find(&["application"]),
            time: None,
            time_unit_ambiguous: false,
        };
        c.time = find(&["cpustarttime", "cpustarttimeinms", "timeinms", "cpustartqpctime", "cpustartqpctimeinms"])
            .map(|i| (i, 1.0))
            .or_else(|| find(&["cpustarttimeinseconds", "timeinseconds"]).map(|i| (i, 1000.0)));
        // 2.0-style files call the start time "CPUStartTime" but give seconds when the default unit is used.
        if let Some((i, _)) = c.time {
            if names[i] == "cpustarttime" {
                c.time = Some((i, 1000.0));
                c.time_unit_ambiguous = true;
            }
        }
        (c.application.is_some() && (c.frame_time.is_some() || c.between_presents.is_some() || (c.cpu_busy.is_some() && c.cpu_wait.is_some()))).then_some(c)
    }

    /// Parse one line. `Ok(None)` for the header, blank lines and rows that
    /// carry no frame; `Err` only when the first line is not a PresentMon header.
    pub fn line(&mut self, line: &str) -> anyhow::Result<Option<(Option<u32>, Frame)>> {
        let line = line.trim_end_matches(['\r', '\n']).trim_start_matches('\u{feff}');
        if line.trim().is_empty() {
            return Ok(None);
        }
        if line.len() > MAX_LINE_LEN {
            self.skipped_lines += 1;
            return Ok(None);
        }
        if self.cols.is_none() {
            self.cols = Some(Self::parse_header(line).ok_or_else(|| anyhow::anyhow!("This is not PresentMon CSV output (no recognised header)"))?);
            return Ok(None);
        }
        let cols = self.cols.clone().expect("header parsed above");
        let cols = &cols;
        let f = split(line);
        // PresentMon repeats the header when a new process appears in some modes.
        if f.first().map(|s| s.eq_ignore_ascii_case("Application")).unwrap_or(false) {
            return Ok(None);
        }
        let cpu_busy = num(&f, cols.cpu_busy);
        let cpu_wait = num(&f, cols.cpu_wait);
        let frame_time = num(&f, cols.frame_time)
            .or_else(|| Some(cpu_busy? + cpu_wait?))
            .or_else(|| num(&f, cols.between_presents));
        let Some(frame_time) = frame_time.filter(|v| *v > 0.0) else {
            self.skipped_lines += 1;
            return Ok(None);
        };
        let displayed = num(&f, cols.displayed_time).filter(|v| *v > 0.0);
        let dropped = match cols.dropped {
            Some(_) => num(&f, cols.dropped).map(|v| v != 0.0).unwrap_or(false),
            None => cols.displayed_time.is_some() && displayed.is_none(),
        };
        let raw_time = cols.time.and_then(|(i, _)| num(&f, Some(i)));
        let mut mul = cols.time.map(|(_, m)| m).unwrap_or(1.0);
        // Settle an ambiguous unit: consecutive start times differ by about one
        // frame time, so the raw difference tells seconds from milliseconds.
        if cols.time_unit_ambiguous {
            if let (Some(prev), Some(now)) = (self.prev_raw_time, raw_time) {
                let delta = now - prev;
                if delta > 0.0 {
                    mul = if frame_time / delta > 30.0 { 1000.0 } else { 1.0 };
                    if let Some(c) = self.cols.as_mut() {
                        c.time = c.time.map(|(i, _)| (i, mul));
                        c.time_unit_ambiguous = false;
                    }
                }
            }
        }
        self.prev_raw_time = raw_time;
        let cols = self.cols.as_ref().expect("header parsed above");
        let time_ms = match raw_time {
            Some(t) => t * mul,
            None => self.clock_ms,
        };
        self.clock_ms = time_ms + frame_time;
        let generated = cols
            .frame_type
            .and_then(|i| f.get(i))
            .map(|t| !(t.is_empty() || t.eq_ignore_ascii_case("Application") || t.eq_ignore_ascii_case("NotSet") || t.eq_ignore_ascii_case("Unspecified") || t.eq_ignore_ascii_case("NA")))
            .unwrap_or(false);
        let pid = cols.process_id.and_then(|i| f.get(i)).and_then(|s| s.parse::<u32>().ok());
        Ok(Some((
            pid,
            Frame {
                time_ms,
                frame_time_ms: frame_time,
                displayed_time_ms: if dropped { None } else { displayed },
                cpu_busy_ms: cpu_busy,
                cpu_wait_ms: cpu_wait,
                gpu_busy_ms: num(&f, cols.gpu_busy),
                gpu_wait_ms: num(&f, cols.gpu_wait),
                gpu_time_ms: num(&f, cols.gpu_time),
                display_latency_ms: num(&f, cols.display_latency),
                dropped,
                generated,
                allows_tearing: num(&f, cols.allows_tearing).map(|v| v != 0.0),
            },
        )))
    }
}

/// Parse a whole CSV document. Frames of other processes are kept only when
/// `pid` is `None`.
pub fn parse(text: &str, pid: Option<u32>) -> anyhow::Result<Vec<Frame>> {
    let mut p = CsvParser::new();
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some((row_pid, frame)) = p.line(line)? {
            if pid.is_none() || row_pid == pid {
                out.push(frame);
            }
        }
    }
    if !p.has_header() {
        anyhow::bail!("The file is empty");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const V2: &str = "Application,ProcessID,SwapChainAddress,PresentRuntime,SyncInterval,PresentFlags,AllowsTearing,PresentMode,FrameType,CPUStartTime,FrameTime,CPUBusy,CPUWait,GPULatency,GPUTime,GPUBusy,GPUWait,VideoBusy,DisplayLatency,DisplayedTime,AnimationError,AnimationTime,MsFlipDelay,AllInputToPhotonLatency,ClickToPhotonLatency\n\
game.exe,4242,0x1,DXGI,0,512,1,Hardware: Independent Flip,Application,0.0000,6.9000,5.1000,1.8000,0.4000,6.2000,6.0000,0.2000,0.0000,14.1000,6.9100,0.0100,0.0000,0.0000,NA,NA\n\
game.exe,4242,0x1,DXGI,0,512,1,Hardware: Independent Flip,Application,0.0069,7.1000,5.0000,2.1000,0.4000,6.3000,6.1000,0.2000,0.0000,14.3000,NA,NA,0.0069,0.0000,NA,NA\n\
other.exe,7,0x2,DXGI,1,0,0,Composed: Flip,Application,0.0100,16.6000,2.0000,14.6000,0.3000,3.0000,2.9000,0.1000,0.0000,20.0000,16.6000,0.0000,0.0100,0.0000,NA,NA\n";

    const V3: &str = "Application,ProcessID,SwapChainAddress,PresentRuntime,SyncInterval,PresentFlags,AllowsTearing,PresentMode,FrameType,TimeInSeconds,MsBetweenSimulationStart,MsBetweenPresents,MsBetweenDisplayChange,MsInPresentAPI,MsRenderPresentLatency,MsUntilDisplayed,MsPCLatency,CPUStartTimeInSeconds,MsBetweenAppStart,MsCPUBusy,MsCPUWait,MsGPULatency,MsGPUTime,MsGPUBusy,MsGPUWait,MsVideoBusy,MsAnimationError,AnimationTime,MsFlipDelay\n\
game.exe,9,0x1,DXGI,0,512,1,Hardware: Independent Flip,Application,1.5000,NA,8.3000,8.3300,0.2000,7.0000,12.0000,NA,1.4990,8.3100,6.0000,2.3100,0.5000,7.5000,7.2000,0.3000,0.0000,0.0200,1.4990,0.0000\n\
game.exe,9,0x1,DXGI,0,512,1,Hardware: Independent Flip,Intel XeSS-FG,1.5083,NA,8.3000,8.3300,0.2000,7.0000,12.0000,NA,1.5073,8.3000,6.0000,2.3000,0.5000,7.5000,7.2000,0.3000,0.0000,0.0200,1.5073,0.0000\n";

    const V1: &str = "Application,ProcessID,SwapChainAddress,Runtime,SyncInterval,PresentFlags,Dropped,TimeInSeconds,msInPresentAPI,msBetweenPresents,AllowsTearing,PresentMode,msUntilRenderComplete,msUntilDisplayed,msBetweenDisplayChange,msUntilRenderStart,msGPUActive\n\
old.exe,5,0x1,DXGI,1,0,0,0.5000,0.3,16.7,0,Composed: Flip,10.0,25.0,16.7,1.0,9.5\n\
old.exe,5,0x1,DXGI,1,0,1,0.5167,0.3,16.6,0,Composed: Flip,10.0,0.0,0.0,1.0,9.4\n";

    #[test]
    fn parses_v2_and_filters_by_pid() {
        let frames = parse(V2, Some(4242)).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].frame_time_ms, 6.9);
        assert_eq!(frames[0].cpu_busy_ms, Some(5.1));
        assert_eq!(frames[0].gpu_busy_ms, Some(6.0));
        assert_eq!(frames[0].displayed_time_ms, Some(6.91));
        assert_eq!(frames[0].allows_tearing, Some(true));
        assert!(!frames[0].dropped);
        // "NA" displayed time = the frame never reached the screen.
        assert!(frames[1].dropped && frames[1].displayed_time_ms.is_none());
        assert!((frames[1].time_ms - 6.9).abs() < 1e-9);
        assert_eq!(parse(V2, None).unwrap().len(), 3);
    }

    #[test]
    fn parses_current_header_and_flags_generated_frames() {
        let frames = parse(V3, Some(9)).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].frame_time_ms, 8.31);
        assert_eq!(frames[0].cpu_wait_ms, Some(2.31));
        assert_eq!(frames[0].display_latency_ms, Some(12.0));
        assert!((frames[0].time_ms - 1499.0).abs() < 1e-6);
        assert!(!frames[0].generated && frames[1].generated);
    }

    #[test]
    fn parses_v1_with_dropped_column() {
        let frames = parse(V1, None).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].frame_time_ms, 16.7);
        assert_eq!(frames[0].gpu_busy_ms, Some(9.5));
        assert!(!frames[0].dropped && frames[1].dropped);
        assert!((frames[0].time_ms - 500.0).abs() < 1e-9);
    }

    #[test]
    fn start_time_in_milliseconds_is_detected() {
        let ms = V2.replace(",0.0000,6.9000,", ",0.0000,6.9000,").replace(",0.0069,7.1000,", ",6.9000,7.1000,").replace(",0.0100,16.6000,", ",10.0000,16.6000,");
        let frames = parse(&ms, Some(4242)).unwrap();
        assert!((frames[1].time_ms - 6.9).abs() < 1e-9);
    }

    #[test]
    fn rejects_non_presentmon_input() {
        assert!(parse("name,value\na,1\n", None).is_err());
        assert!(parse("", None).is_err());
        assert!(parse("<html>", None).is_err());
    }

    #[test]
    fn bad_rows_are_skipped_not_fatal() {
        let mut p = CsvParser::new();
        let mut lines = V2.lines();
        assert!(p.line(lines.next().unwrap()).unwrap().is_none());
        assert!(p.line("game.exe,4242,garbage").unwrap().is_none());
        assert!(p.line(&"x".repeat(MAX_LINE_LEN + 1)).unwrap().is_none());
        assert!(p.line("").unwrap().is_none());
        assert_eq!(p.skipped_lines, 2);
        assert!(p.line(lines.next().unwrap()).unwrap().is_some());
    }
}
