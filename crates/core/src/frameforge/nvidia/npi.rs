//! Optional fallback: hand a `.nip` file to an NVIDIA Profile Inspector the
//! user installed themselves.
//!
//! Arcadia's own DRS layer is the normal path. This exists for the cases it
//! cannot cover (for example a driver whose NVAPI Arcadia cannot load).
//! Rules:
//! * the executable is one the user picked; Arcadia never downloads it;
//! * its path is validated every time before it is started;
//! * only the command-line options documented in Profile Inspector's README
//!   are used: `<file>.nip`, `-silentImport` and `-exportCustomized`;
//! * arguments are passed as a list, never through a shell.
//!
//! `-silentImport` replaces the profiles named in the file (that is what the
//! tool does in silent mode). `-exportCustomized` exports **all** customised
//! profiles to a time-stamped file next to the executable; it cannot export
//! one chosen profile.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long Arcadia waits for a silent import before reporting that the tool is still running.
pub const IMPORT_TIMEOUT: Duration = Duration::from_secs(120);

/// Check a user-chosen Profile Inspector executable.
pub fn validate_exe(path: &Path) -> Result<PathBuf, String> {
    let text = path.to_string_lossy();
    if !path.is_absolute() || text.starts_with(r"\\") || path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err("Choose the full path to nvidiaProfileInspector.exe on this PC".into());
    }
    if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("exe")) != Some(true) {
        return Err("That is not an .exe file".into());
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_ascii_lowercase();
    if !stem.contains("nvidiaprofileinspector") {
        return Err("The file is not named nvidiaProfileInspector.exe".into());
    }
    if !path.is_file() {
        return Err("That file does not exist".into());
    }
    Ok(path.to_path_buf())
}

/// A `.nip` file to hand over: absolute, existing, with the right extension.
pub fn validate_nip(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err("The profile file needs a full path".into());
    }
    if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("nip")) != Some(true) {
        return Err("The profile file must end in .nip".into());
    }
    if !path.is_file() {
        return Err("The profile file does not exist".into());
    }
    Ok(path.to_path_buf())
}

pub fn import_args(files: &[PathBuf]) -> Vec<OsString> {
    let mut a: Vec<OsString> = vec!["-silentImport".into()];
    a.extend(files.iter().map(|f| f.as_os_str().to_os_string()));
    a
}

pub fn export_customized_args() -> Vec<OsString> {
    vec!["-exportCustomized".into()]
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NpiRun {
    /// The tool exited by itself within the wait.
    pub finished: bool,
    pub exit_code: Option<i32>,
    pub message: String,
}

fn run(exe: &Path, args: Vec<OsString>, done: &str) -> Result<NpiRun, String> {
    let exe = validate_exe(exe)?;
    let mut child = std::process::Command::new(&exe)
        .args(args)
        .current_dir(exe.parent().unwrap_or(Path::new(".")))
        .stdin(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("NVIDIA Profile Inspector could not be started: {e}"))?;
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return Ok(NpiRun { finished: true, exit_code: status.code(), message: if status.success() { done.to_string() } else { format!("NVIDIA Profile Inspector exited with code {}", status.code().unwrap_or(-1)) } });
            }
            Ok(None) if started.elapsed() > IMPORT_TIMEOUT => {
                return Ok(NpiRun { finished: false, exit_code: None, message: "NVIDIA Profile Inspector is still open. Finish there; Arcadia did not wait for it.".into() });
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(200)),
            Err(e) => return Err(format!("Waiting for NVIDIA Profile Inspector failed: {e}")),
        }
    }
}

/// Import `.nip` files with the user's Profile Inspector (replaces the profiles named in them).
pub fn import_with(exe: &Path, files: &[PathBuf]) -> Result<NpiRun, String> {
    if files.is_empty() {
        return Err("No profile file was given".into());
    }
    let files: Vec<PathBuf> = files.iter().map(|f| validate_nip(f)).collect::<Result<_, _>>()?;
    run(exe, import_args(&files), "NVIDIA Profile Inspector imported the file. It does not report details back to Arcadia; refresh the profile list to see the result.")
}

/// Ask the user's Profile Inspector to export all customised profiles next to its executable.
pub fn export_customized_with(exe: &Path) -> Result<NpiRun, String> {
    run(exe, export_customized_args(), "NVIDIA Profile Inspector wrote a time-stamped .nip file with all customised profiles next to its executable.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_plausible_local_executable_is_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("nvidiaProfileInspector.exe");
        std::fs::write(&good, b"MZ").unwrap();
        assert!(validate_exe(&good).is_ok());
        assert!(validate_exe(Path::new("nvidiaProfileInspector.exe")).is_err());
        assert!(validate_exe(&dir.path().join("missing").join("nvidiaProfileInspector.exe")).is_err());
        let other = dir.path().join("cmd.exe");
        std::fs::write(&other, b"MZ").unwrap();
        assert!(validate_exe(&other).unwrap_err().contains("not named"));
        let script = dir.path().join("nvidiaProfileInspector.bat");
        std::fs::write(&script, b"").unwrap();
        assert!(validate_exe(&script).is_err());
        assert!(validate_exe(&dir.path().join("..").join("nvidiaProfileInspector.exe")).is_err());
        assert!(validate_exe(Path::new(r"\\server\share\nvidiaProfileInspector.exe")).is_err());
    }

    #[test]
    fn arguments_are_the_documented_ones_and_stay_separate() {
        let files = vec![PathBuf::from("/tmp/a b.nip"), PathBuf::from("/tmp/-silent & calc.nip")];
        let a = import_args(&files);
        assert_eq!(a.len(), 3);
        assert_eq!(a[0], "-silentImport");
        // A file name with spaces or shell characters is still one argument.
        assert_eq!(a[2], "/tmp/-silent & calc.nip");
        assert_eq!(export_customized_args(), vec![OsString::from("-exportCustomized")]);
    }

    #[test]
    fn profile_files_are_checked_before_handover() {
        let dir = tempfile::tempdir().unwrap();
        let nip = dir.path().join("p.nip");
        std::fs::write(&nip, b"x").unwrap();
        assert!(validate_nip(&nip).is_ok());
        assert!(validate_nip(&dir.path().join("p.exe")).is_err());
        assert!(validate_nip(Path::new("p.nip")).is_err());
        assert!(import_with(&dir.path().join("nvidiaProfileInspector.exe"), &[]).is_err());
    }
}
