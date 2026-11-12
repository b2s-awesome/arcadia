//! Safe wrappers over the documented Win32 calls FrameForge uses to read and
//! change system state: power schemes, process priority, window close
//! requests, elevation. Windows only; every other platform uses the mocks.

use std::ffi::c_void;
use windows::core::GUID;
use windows::Win32::Foundation::*;
use windows::Win32::System::Power::*;
use windows::Win32::System::Threading::*;

fn win32(err: WIN32_ERROR, what: &str) -> anyhow::Result<()> {
    if err == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(anyhow::anyhow!("{what} failed: {}", windows::core::Error::from(err.to_hresult()).message()))
    }
}

pub fn guid_to_string(g: &GUID) -> String {
    format!("{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}", g.data1, g.data2, g.data3, g.data4[0], g.data4[1], g.data4[2], g.data4[3], g.data4[4], g.data4[5], g.data4[6], g.data4[7])
}

pub fn parse_guid(s: &str) -> anyhow::Result<GUID> {
    let hex: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() != 32 {
        anyhow::bail!("'{s}' is not a power scheme id");
    }
    Ok(GUID::from_u128(u128::from_str_radix(&hex, 16)?))
}

pub fn active_power_scheme() -> anyhow::Result<GUID> {
    let mut p: *mut GUID = std::ptr::null_mut();
    // SAFETY: valid out-pointer; the returned GUID is freed with LocalFree below.
    win32(unsafe { PowerGetActiveScheme(None, &mut p) }, "Reading the active power plan")?;
    if p.is_null() {
        anyhow::bail!("Windows returned no active power plan");
    }
    // SAFETY: p points to a GUID allocated by the power API.
    let g = unsafe { *p };
    // SAFETY: allocated with LocalAlloc by PowerGetActiveScheme.
    unsafe { LocalFree(Some(HLOCAL(p as *mut c_void))) };
    Ok(g)
}

fn scheme_name(g: &GUID) -> String {
    let mut size = 0u32;
    // SAFETY: size query with no buffer.
    let _ = unsafe { PowerReadFriendlyName(None, Some(g), None, None, None, &mut size) };
    if size == 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (size as usize).div_ceil(2)];
    // SAFETY: buffer of `size` bytes.
    if unsafe { PowerReadFriendlyName(None, Some(g), None, None, Some(buf.as_mut_ptr().cast()), &mut size) } != ERROR_SUCCESS {
        return String::new();
    }
    crate::frameforge::ffi::wide_to_string(&buf)
}

/// Every power scheme on the machine: `(guid, name, active)`.
pub fn power_schemes() -> Vec<(String, String, bool)> {
    let active = active_power_scheme().ok();
    let mut out = Vec::new();
    for index in 0..64u32 {
        let mut g = GUID::zeroed();
        let mut size = std::mem::size_of::<GUID>() as u32;
        // SAFETY: buffer is exactly one GUID.
        let st = unsafe { PowerEnumerate(None, None, None, ACCESS_SCHEME, index, Some((&mut g as *mut GUID).cast()), &mut size) };
        if st != ERROR_SUCCESS {
            break;
        }
        out.push((guid_to_string(&g), scheme_name(&g), Some(g) == active));
    }
    out
}

pub fn set_active_power_scheme(guid: &str) -> anyhow::Result<()> {
    let g = parse_guid(guid)?;
    // SAFETY: valid GUID pointer.
    win32(unsafe { PowerSetActiveScheme(None, Some(&g)) }, "Switching the power plan")
}

/// Copy an existing scheme under a new name and return the copy's id.
pub fn duplicate_power_scheme(source: &str, name: &str) -> anyhow::Result<String> {
    let src = parse_guid(source)?;
    let mut p: *mut GUID = std::ptr::null_mut();
    // SAFETY: valid in/out pointers; a null destination asks Windows to allocate a new GUID.
    win32(unsafe { PowerDuplicateScheme(None, &src, &mut p) }, "Creating the power plan")?;
    if p.is_null() {
        anyhow::bail!("Windows did not return the new power plan");
    }
    // SAFETY: p points to a GUID allocated by the power API.
    let g = unsafe { *p };
    // SAFETY: allocated with LocalAlloc by PowerDuplicateScheme.
    unsafe { LocalFree(Some(HLOCAL(p as *mut c_void))) };
    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    // SAFETY: the buffer is the UTF-16 name including its terminator, as bytes.
    let bytes = unsafe { std::slice::from_raw_parts(wide.as_ptr().cast::<u8>(), wide.len() * 2) };
    // SAFETY: valid GUID and byte buffer.
    let _ = unsafe { PowerWriteFriendlyName(None, &g, None, None, bytes) };
    Ok(guid_to_string(&g))
}

pub fn delete_power_scheme(guid: &str) -> anyhow::Result<()> {
    let g = parse_guid(guid)?;
    // SAFETY: valid GUID pointer.
    win32(unsafe { PowerDeleteScheme(None, &g) }, "Removing the power plan")
}

/// Read an AC setting of a scheme (subgroup + setting GUIDs as strings).
pub fn read_ac_value(scheme: &str, subgroup: &str, setting: &str) -> anyhow::Result<u32> {
    let (s, sub, set) = (parse_guid(scheme)?, parse_guid(subgroup)?, parse_guid(setting)?);
    let mut v = 0u32;
    // SAFETY: valid GUID pointers and out-pointer.
    win32(unsafe { PowerReadACValueIndex(None, Some(&s), Some(&sub), Some(&set), &mut v) }, "Reading a power setting")?;
    Ok(v)
}

pub fn write_ac_value(scheme: &str, subgroup: &str, setting: &str, value: u32) -> anyhow::Result<()> {
    let (s, sub, set) = (parse_guid(scheme)?, parse_guid(subgroup)?, parse_guid(setting)?);
    // SAFETY: valid GUID pointers.
    win32(unsafe { PowerWriteACValueIndex(None, &s, Some(&sub), Some(&set), value) }, "Writing a power setting")
}

pub fn on_battery() -> Option<bool> {
    let mut st = SYSTEM_POWER_STATUS::default();
    // SAFETY: valid out-pointer.
    unsafe { GetSystemPowerStatus(&mut st) }.ok()?;
    match st.ACLineStatus {
        0 => Some(true),
        1 => Some(false),
        _ => None,
    }
}

pub fn is_elevated() -> bool {
    use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    let mut token = HANDLE::default();
    // SAFETY: pseudo-handle for the current process and a valid out-pointer.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.is_err() {
        return false;
    }
    let mut elev = TOKEN_ELEVATION::default();
    let mut len = 0u32;
    // SAFETY: buffer is one TOKEN_ELEVATION.
    let ok = unsafe { GetTokenInformation(token, TokenElevation, Some((&mut elev as *mut TOKEN_ELEVATION).cast()), std::mem::size_of::<TOKEN_ELEVATION>() as u32, &mut len) }.is_ok();
    // SAFETY: handle opened above.
    let _ = unsafe { CloseHandle(token) };
    ok && elev.TokenIsElevated != 0
}

/// Priority classes FrameForge is allowed to use. REALTIME is never offered.
pub fn priority_from_name(name: &str) -> Option<PROCESS_CREATION_FLAGS> {
    Some(match name {
        "Idle" => IDLE_PRIORITY_CLASS,
        "BelowNormal" => BELOW_NORMAL_PRIORITY_CLASS,
        "Normal" => NORMAL_PRIORITY_CLASS,
        "AboveNormal" => ABOVE_NORMAL_PRIORITY_CLASS,
        "High" => HIGH_PRIORITY_CLASS,
        _ => return None,
    })
}

fn priority_name(class: u32) -> String {
    match PROCESS_CREATION_FLAGS(class) {
        IDLE_PRIORITY_CLASS => "Idle",
        BELOW_NORMAL_PRIORITY_CLASS => "BelowNormal",
        NORMAL_PRIORITY_CLASS => "Normal",
        ABOVE_NORMAL_PRIORITY_CLASS => "AboveNormal",
        HIGH_PRIORITY_CLASS => "High",
        REALTIME_PRIORITY_CLASS => "Realtime",
        _ => "Unknown",
    }
    .to_string()
}

pub fn get_priority(pid: u32) -> anyhow::Result<String> {
    // SAFETY: OpenProcess with query rights; handle closed below.
    let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.map_err(|e| anyhow::anyhow!("Process {pid} can't be opened: {}", e.message()))?;
    // SAFETY: valid process handle.
    let class = unsafe { GetPriorityClass(h) };
    // SAFETY: handle opened above.
    let _ = unsafe { CloseHandle(h) };
    if class == 0 {
        anyhow::bail!("Windows would not report the priority of process {pid}");
    }
    Ok(priority_name(class))
}

/// Is the process still running (opened and not yet exited)?
pub fn is_running(pid: u32) -> bool {
    // SAFETY: OpenProcess with query rights; handle closed below.
    let Ok(h) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else { return false };
    let mut code = 0u32;
    // SAFETY: valid process handle and out-pointer.
    let ok = unsafe { GetExitCodeProcess(h, &mut code) }.is_ok();
    // SAFETY: handle opened above.
    let _ = unsafe { CloseHandle(h) };
    // 259 = STILL_ACTIVE.
    ok && code == 259
}

pub fn set_priority(pid: u32, name: &str) -> anyhow::Result<()> {
    let class = priority_from_name(name).ok_or_else(|| anyhow::anyhow!("'{name}' is not a priority Arcadia will set"))?;
    // SAFETY: OpenProcess with set-information rights; handle closed below.
    let h = unsafe { OpenProcess(PROCESS_SET_INFORMATION, false, pid) }.map_err(|e| anyhow::anyhow!("Process {pid} can't be changed: {}", e.message()))?;
    // SAFETY: valid process handle and a class from the allow-list above.
    let r = unsafe { SetPriorityClass(h, class) };
    // SAFETY: handle opened above.
    let _ = unsafe { CloseHandle(h) };
    r.map_err(|e| anyhow::anyhow!("Setting the priority failed: {}", e.message()))
}

/// Ask every top-level window of a process to close (WM_CLOSE), the same as
/// clicking its close button. Returns how many windows were asked.
pub fn request_close(pid: u32) -> u32 {
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE};
    struct Ctx {
        pid: u32,
        asked: u32,
    }
    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
        // SAFETY: lparam is the &mut Ctx passed to EnumWindows below and outlives the enumeration.
        let ctx = unsafe { &mut *(lparam.0 as *mut Ctx) };
        let mut owner = 0u32;
        // SAFETY: hwnd comes from EnumWindows.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut owner)) };
        // SAFETY: hwnd comes from EnumWindows.
        if owner == ctx.pid && unsafe { IsWindowVisible(hwnd) }.as_bool() && unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) }.is_ok() {
            ctx.asked += 1;
        }
        true.into()
    }
    let mut ctx = Ctx { pid, asked: 0 };
    // SAFETY: callback and context are valid for the duration of the call.
    let _ = unsafe { EnumWindows(Some(each), LPARAM(&mut ctx as *mut Ctx as isize)) };
    ctx.asked
}

/// Force-end a process. Only called after the user approved it for that process.
pub fn terminate(pid: u32) -> anyhow::Result<()> {
    // SAFETY: OpenProcess with terminate rights; handle closed below.
    let h = unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) }.map_err(|e| anyhow::anyhow!("Process {pid} can't be ended: {}", e.message()))?;
    // SAFETY: valid process handle.
    let r = unsafe { TerminateProcess(h, 1) };
    // SAFETY: handle opened above.
    let _ = unsafe { CloseHandle(h) };
    r.map_err(|e| anyhow::anyhow!("Ending the process failed: {}", e.message()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_round_trip() {
        let s = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";
        assert_eq!(guid_to_string(&parse_guid(s).unwrap()), s);
        assert!(parse_guid("nope").is_err());
    }

    #[test]
    fn realtime_is_never_settable() {
        assert!(priority_from_name("Realtime").is_none());
        assert!(priority_from_name("High").is_some());
    }
}
