//! PresentMon console application as a frame source.
//!
//! Arcadia never downloads or bundles the executable. The user chooses it;
//! the path is validated; it is started directly (no shell) with fixed,
//! documented arguments, and its CSV output is read from stdout.

use super::csv::CsvParser;
use super::{Frame, FrameSource, SourceKind};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

/// Check a user-supplied path really is a PresentMon console executable.
pub fn validate_exe(path: &Path) -> anyhow::Result<PathBuf> {
    if !path.is_absolute() {
        anyhow::bail!("Choose the full path to the PresentMon executable");
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_ascii_lowercase();
    if !(name.starts_with("presentmon") && name.ends_with(".exe")) {
        anyhow::bail!("That file is not a PresentMon executable (expected a name like PresentMon-2.3.1-x64.exe)");
    }
    let meta = std::fs::metadata(path).map_err(|_| anyhow::anyhow!("PresentMon was not found at that path"))?;
    if !meta.is_file() {
        anyhow::bail!("That path is not a file");
    }
    Ok(path.to_path_buf())
}

/// The fixed argument list. Every option is from PresentMon's README.
pub fn arguments(pid: u32) -> Vec<String> {
    vec![
        "--process_id".into(),
        pid.to_string(),
        "--output_stdout".into(),
        "--no_console_stats".into(),
        "--terminate_on_proc_exit".into(),
        "--stop_existing_session".into(),
        "--session_name".into(),
        "ArcadiaFrameForge".into(),
    ]
}

/// Reads CSV lines from any stream on a background thread.
pub struct CsvStream {
    rx: Receiver<Frame>,
    done: Arc<AtomicBool>,
    error: Arc<parking_lot::Mutex<Option<String>>>,
}

impl CsvStream {
    pub fn from_reader<R: Read + Send + 'static>(reader: R, pid: Option<u32>) -> Self {
        let (tx, rx) = channel();
        let done = Arc::new(AtomicBool::new(false));
        let error = Arc::new(parking_lot::Mutex::new(None));
        let (d, e) = (done.clone(), error.clone());
        std::thread::spawn(move || {
            let mut parser = CsvParser::new();
            for line in BufReader::new(reader).lines() {
                let Ok(line) = line else { break };
                match parser.line(&line) {
                    Ok(Some((row_pid, frame))) => {
                        if (pid.is_none() || row_pid == pid) && tx.send(frame).is_err() {
                            break;
                        }
                    }
                    Ok(None) => {}
                    Err(err) => {
                        *e.lock() = Some(err.to_string());
                        break;
                    }
                }
            }
            d.store(true, Ordering::Relaxed);
        });
        Self { rx, done, error }
    }

    pub fn drain(&self) -> Vec<Frame> {
        self.rx.try_iter().collect()
    }
    pub fn finished(&self) -> bool {
        self.done.load(Ordering::Relaxed)
    }
    pub fn error(&self) -> Option<String> {
        self.error.lock().clone()
    }
}

pub struct CliSource {
    child: std::process::Child,
    stream: CsvStream,
    stderr: Arc<parking_lot::Mutex<String>>,
    exe: PathBuf,
}

impl CliSource {
    pub fn spawn(exe: &Path, pid: u32) -> anyhow::Result<Self> {
        let exe = validate_exe(exe)?;
        let mut cmd = std::process::Command::new(&exe);
        cmd.args(arguments(pid)).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd.spawn().map_err(|e| anyhow::anyhow!("PresentMon could not be started: {e}"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("PresentMon gave no output stream"))?;
        let stderr_text = Arc::new(parking_lot::Mutex::new(String::new()));
        if let Some(mut err) = child.stderr.take() {
            let sink = stderr_text.clone();
            std::thread::spawn(move || {
                let mut buf = String::new();
                let _ = err.read_to_string(&mut buf);
                // Keep the tail only; this is shown in an error message.
                let tail: String = buf.chars().rev().take(600).collect::<String>().chars().rev().collect();
                *sink.lock() = tail;
            });
        }
        Ok(Self { child, stream: CsvStream::from_reader(stdout, Some(pid)), stderr: stderr_text, exe })
    }
}

impl FrameSource for CliSource {
    fn kind(&self) -> SourceKind {
        SourceKind::Cli
    }

    fn poll(&mut self) -> anyhow::Result<Vec<Frame>> {
        let frames = self.stream.drain();
        if let Some(e) = self.stream.error() {
            anyhow::bail!(e);
        }
        if frames.is_empty() && self.stream.finished() {
            if let Ok(Some(status)) = self.child.try_wait() {
                if !status.success() {
                    let err = self.stderr.lock().trim().to_string();
                    let hint = "PresentMon needs administrator rights, or your account in the \"Performance Log Users\" group (sign out and back in after adding it).";
                    anyhow::bail!(if err.is_empty() { format!("PresentMon stopped ({status}). {hint}") } else { format!("PresentMon stopped: {err}. {hint}") });
                }
            }
        }
        Ok(frames)
    }

    fn alive(&self) -> bool {
        !self.stream.finished()
    }

    fn detail(&self) -> String {
        format!("Console application: {}", self.exe.file_name().and_then(|n| n.to_str()).unwrap_or("PresentMon"))
    }
}

impl Drop for CliSource {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exe_validation() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("PresentMon-2.3.1-x64.exe");
        std::fs::write(&good, b"MZ").unwrap();
        assert!(validate_exe(&good).is_ok());
        let wrong_name = dir.path().join("totally-not.exe");
        std::fs::write(&wrong_name, b"MZ").unwrap();
        assert!(validate_exe(&wrong_name).unwrap_err().to_string().contains("not a PresentMon executable"));
        assert!(validate_exe(&dir.path().join("PresentMon.exe")).unwrap_err().to_string().contains("not found"));
        assert!(validate_exe(Path::new("PresentMon.exe")).unwrap_err().to_string().contains("full path"));
        // A folder named like the tool is not accepted either.
        let folder = dir.path().join("PresentMon-folder.exe");
        std::fs::create_dir(&folder).unwrap();
        assert!(validate_exe(&folder).is_err());
    }

    #[test]
    fn arguments_are_fixed_and_structured() {
        let a = arguments(4321);
        assert_eq!(&a[..2], ["--process_id", "4321"]);
        assert!(a.contains(&"--output_stdout".to_string()));
        assert!(a.iter().all(|s| !s.contains(' ') && !s.contains('&') && !s.contains('|')));
    }

    #[test]
    fn stream_parses_lines_as_they_arrive_and_filters_pid() {
        let csv = "Application,ProcessID,FrameTime,CPUBusy,CPUWait,DisplayedTime\n\
a.exe,1,10.0,6.0,4.0,10.0\n\
b.exe,2,20.0,6.0,14.0,20.0\n\
a.exe,1,11.0,6.0,5.0,NA\n";
        let s = CsvStream::from_reader(std::io::Cursor::new(csv.as_bytes().to_vec()), Some(1));
        let mut frames = Vec::new();
        for _ in 0..200 {
            frames.extend(s.drain());
            if s.finished() {
                frames.extend(s.drain());
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[1].frame_time_ms, 11.0);
        assert!(frames[1].dropped);
        assert!(s.error().is_none());
    }

    #[test]
    fn stream_reports_non_csv_output() {
        let s = CsvStream::from_reader(std::io::Cursor::new(b"error: access denied\n".to_vec()), None);
        for _ in 0..200 {
            if s.finished() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(s.error().unwrap().contains("not PresentMon CSV"));
    }
}
