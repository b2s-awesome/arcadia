//! NVAPI, loaded from the installed NVIDIA driver (`nvapi64.dll`).
//!
//! NVAPI exports a single symbol, `nvapi_QueryInterface`, which maps a
//! function id to a function pointer. The ids below are copied from
//! `nvapi_interface.h` in NVIDIA's MIT-licensed SDK
//! (<https://github.com/NVIDIA/nvapi>); the structures come from the same
//! SDK through bindgen (`nvapi_types.rs`).
//!
//! Only documented functions are resolved. A function the installed driver
//! does not implement resolves to `None` and its caller reports
//! "not supported by this driver".

use super::nvapi_types::*;
use std::ffi::c_void;

pub type Status = i32;
pub const NVAPI_OK: Status = 0;
pub const NVAPI_ERROR: Status = -1;
pub const NVAPI_LIBRARY_NOT_FOUND: Status = -2;
pub const NVAPI_NO_IMPLEMENTATION: Status = -3;
pub const NVAPI_API_NOT_INITIALIZED: Status = -4;
pub const NVAPI_NVIDIA_DEVICE_NOT_FOUND: Status = -6;
pub const NVAPI_END_ENUMERATION: Status = -7;
pub const NVAPI_NOT_SUPPORTED: Status = -104;
pub const NVAPI_INVALID_USER_PRIVILEGE: Status = -137;
pub const NVAPI_SETTING_NOT_FOUND: Status = -160;
pub const NVAPI_PROFILE_NOT_FOUND: Status = -163;
pub const NVAPI_PROFILE_NAME_IN_USE: Status = -164;
pub const NVAPI_EXECUTABLE_ALREADY_IN_USE: Status = -167;
pub const NVAPI_ACCESS_DENIED: Status = -175;

/// `MAKE_NVAPI_VERSION(type, ver)`.
pub const fn make_version<T>(ver: u32) -> u32 {
    (std::mem::size_of::<T>() as u32) | (ver << 16)
}

pub const NVDRS_SETTING_VER: u32 = make_version::<NVDRS_SETTING_V1>(1);
pub const NVDRS_SETTING_VALUES_VER: u32 = make_version::<NVDRS_SETTING_VALUES>(1);
pub const NVDRS_APPLICATION_VER_V4: u32 = make_version::<NVDRS_APPLICATION_V4>(4);
pub const NVDRS_PROFILE_VER: u32 = make_version::<NVDRS_PROFILE_V1>(1);
pub const NV_GPU_THERMAL_SETTINGS_VER: u32 = make_version::<NV_GPU_THERMAL_SETTINGS_V2>(2);
pub const NV_GPU_DYNAMIC_PSTATES_INFO_EX_VER: u32 = make_version::<NV_GPU_DYNAMIC_PSTATES_INFO_EX>(1);
pub const NV_GPU_CLOCK_FREQUENCIES_VER: u32 = make_version::<NV_GPU_CLOCK_FREQUENCIES_V2>(3);
pub const NV_GPU_MEMORY_INFO_EX_VER: u32 = make_version::<NV_GPU_MEMORY_INFO_EX_V1>(1);
pub const NV_GET_VRR_INFO_VER: u32 = make_version::<NV_GET_VRR_INFO_V1>(1);

pub const NVAPI_GPU_PUBLIC_CLOCK_GRAPHICS: usize = 0;
pub const NVAPI_GPU_PUBLIC_CLOCK_MEMORY: usize = 4;
pub const NVAPI_GPU_UTILIZATION_DOMAIN_GPU: usize = 0;
pub const NV_GPU_PERF_DECREASE_REASON_THERMAL_PROTECTION: u32 = 0x1;
pub const NV_GPU_PERF_DECREASE_REASON_POWER_CONTROL: u32 = 0x2;

type Gpu = NvPhysicalGpuHandle;
type Session = NvDRSSessionHandle;
type Profile = NvDRSProfileHandle;
type WStr = *mut NvAPI_UnicodeString;

macro_rules! nvapi_functions {
    ($( $name:ident = $id:literal : fn($($arg:ty),*) ; )*) => {
        /// Function pointers resolved from the driver. `None` = not implemented by this driver.
        #[allow(non_snake_case)]
        pub struct NvApi {
            _lib: libloading::Library,
            $( pub $name: Option<unsafe extern "C" fn($($arg),*) -> Status>, )*
        }

        impl NvApi {
            #[allow(non_snake_case)]
            unsafe fn resolve(lib: libloading::Library) -> Result<Self, String> {
                let query: libloading::Symbol<unsafe extern "C" fn(u32) -> *mut c_void> = unsafe {
                    lib.get(b"nvapi_QueryInterface\0").map_err(|e| format!("nvapi_QueryInterface is missing: {e}"))?
                };
                $(
                    let $name = {
                        let p = unsafe { query($id) };
                        // SAFETY: a non-null pointer returned for this id is the
                        // function with the signature given in nvapi.h.
                        (!p.is_null()).then(|| unsafe { std::mem::transmute::<*mut c_void, unsafe extern "C" fn($($arg),*) -> Status>(p) })
                    };
                )*
                drop(query);
                Ok(Self { _lib: lib, $( $name, )* })
            }
        }
    };
}

nvapi_functions! {
    Initialize = 0x0150e828: fn();
    Unload = 0xd22bdd7e: fn();
    GetErrorMessage = 0x6c2d048c: fn(Status, *mut NvAPI_ShortString);
    GetInterfaceVersionString = 0x01053fa5: fn(*mut NvAPI_ShortString);
    SYS_GetDriverAndBranchVersion = 0x2926aaad: fn(*mut u32, *mut NvAPI_ShortString);
    EnumPhysicalGPUs = 0xe5ac921f: fn(*mut [Gpu; 64], *mut u32);
    GPU_GetFullName = 0xceee8e9f: fn(Gpu, *mut NvAPI_ShortString);
    GPU_GetPCIIdentifiers = 0x2ddfb66e: fn(Gpu, *mut u32, *mut u32, *mut u32, *mut u32);
    GPU_GetGPUType = 0xc33baeb1: fn(Gpu, *mut i32);
    GPU_GetSystemType = 0xbaaabfcc: fn(Gpu, *mut i32);
    GPU_GetVbiosVersionString = 0xa561fd7d: fn(Gpu, *mut NvAPI_ShortString);
    GPU_GetTachReading = 0x5f608315: fn(Gpu, *mut u32);
    GPU_GetPerfDecreaseInfo = 0x7f7f4600: fn(Gpu, *mut u32);
    GPU_GetDynamicPstatesInfoEx = 0x60ded2ed: fn(Gpu, *mut NV_GPU_DYNAMIC_PSTATES_INFO_EX);
    GPU_GetThermalSettings = 0xe3640a56: fn(Gpu, u32, *mut NV_GPU_THERMAL_SETTINGS_V2);
    GPU_GetAllClockFrequencies = 0xdcb616c3: fn(Gpu, *mut NV_GPU_CLOCK_FREQUENCIES_V2);
    GPU_GetMemoryInfoEx = 0xc0599498: fn(Gpu, *mut NV_GPU_MEMORY_INFO_EX_V1);
    DISP_GetDisplayIdByDisplayName = 0xae457190: fn(*const std::ffi::c_char, *mut u32);
    Disp_GetVRRInfo = 0xdf8fda57: fn(u32, *mut NV_GET_VRR_INFO_V1);

    DRS_CreateSession = 0x0694d52e: fn(*mut Session);
    DRS_DestroySession = 0xdad9cff8: fn(Session);
    DRS_LoadSettings = 0x375dbd6b: fn(Session);
    DRS_SaveSettings = 0xfcbc7e14: fn(Session);
    DRS_SaveSettingsToFile = 0x2be25df8: fn(Session, WStr);
    DRS_LoadSettingsFromFile = 0xd3ede889: fn(Session, WStr);
    DRS_CreateProfile = 0xcc176068: fn(Session, *mut NVDRS_PROFILE_V1, *mut Profile);
    DRS_DeleteProfile = 0x17093206: fn(Session, Profile);
    DRS_GetCurrentGlobalProfile = 0x617bff9f: fn(Session, *mut Profile);
    DRS_GetBaseProfile = 0xda8466a0: fn(Session, *mut Profile);
    DRS_GetProfileInfo = 0x61cd6fd6: fn(Session, Profile, *mut NVDRS_PROFILE_V1);
    DRS_FindProfileByName = 0x7e4a9a0b: fn(Session, WStr, *mut Profile);
    DRS_EnumProfiles = 0xbc371ee0: fn(Session, u32, *mut Profile);
    DRS_GetNumProfiles = 0x1dae4fbc: fn(Session, *mut u32);
    DRS_CreateApplication = 0x4347a9de: fn(Session, Profile, *mut NVDRS_APPLICATION_V4);
    DRS_DeleteApplicationEx = 0xc5ea85a1: fn(Session, Profile, *mut NVDRS_APPLICATION_V4);
    DRS_DeleteApplication = 0x2c694bc6: fn(Session, Profile, WStr);
    DRS_EnumApplications = 0x7fa2173a: fn(Session, Profile, u32, *mut u32, *mut NVDRS_APPLICATION_V4);
    DRS_FindApplicationByName = 0xeee566b2: fn(Session, WStr, *mut Profile, *mut NVDRS_APPLICATION_V4);
    DRS_SetSetting = 0x577dd202: fn(Session, Profile, *mut NVDRS_SETTING_V1);
    DRS_GetSetting = 0x73bf8338: fn(Session, Profile, u32, *mut NVDRS_SETTING_V1);
    DRS_EnumSettings = 0xae3039da: fn(Session, Profile, u32, *mut u32, *mut NVDRS_SETTING_V1);
    DRS_EnumAvailableSettingIds = 0xf020614a: fn(*mut u32, *mut u32);
    DRS_EnumAvailableSettingValues = 0x2ec39f90: fn(u32, *mut u32, *mut NVDRS_SETTING_VALUES);
    DRS_GetSettingNameFromId = 0xd61cbe6e: fn(u32, WStr);
    DRS_DeleteProfileSetting = 0xe4a26362: fn(Session, Profile, u32);
    DRS_RestoreProfileDefault = 0xfa5f6134: fn(Session, Profile);
    DRS_RestoreProfileDefaultSetting = 0x53f0381e: fn(Session, Profile, u32);
}

// SAFETY: the table only holds function pointers into a DLL that stays
// loaded for the life of the struct. NVAPI itself is documented as callable
// from any thread; callers serialise DRS sessions with their own lock.
unsafe impl Send for NvApi {}
unsafe impl Sync for NvApi {}

impl NvApi {
    /// Load `nvapi64.dll` and call `NvAPI_Initialize`. The error string is
    /// written for the UI.
    pub fn load() -> Result<Self, String> {
        if cfg!(not(all(windows, target_pointer_width = "64"))) {
            return Err("NVAPI is only available on 64-bit Windows".into());
        }
        // SAFETY: loading the driver's own DLL by bare name from the system search path.
        let lib = unsafe { libloading::Library::new("nvapi64.dll") }
            .map_err(|_| "nvapi64.dll was not found. Install the NVIDIA graphics driver.".to_string())?;
        let api = unsafe { Self::resolve(lib)? };
        let init = api.Initialize.ok_or("This NVIDIA driver does not implement NvAPI_Initialize")?;
        // SAFETY: no arguments; documented as the first call into NVAPI.
        let st = unsafe { init() };
        if st != NVAPI_OK {
            return Err(match st {
                NVAPI_NVIDIA_DEVICE_NOT_FOUND => "No NVIDIA GPU with a working driver was found".into(),
                NVAPI_LIBRARY_NOT_FOUND => "The NVIDIA driver's support library could not be loaded".into(),
                other => format!("NvAPI_Initialize failed: {}", api.error_message(other)),
            });
        }
        Ok(api)
    }

    /// NVAPI's own text for a status code, with the numeric code appended.
    pub fn error_message(&self, status: Status) -> String {
        let mut buf: NvAPI_ShortString = [0; 64];
        let text = match self.GetErrorMessage {
            // SAFETY: buffer is the 64-char NvAPI_ShortString the function expects.
            Some(f) if unsafe { f(status, &mut buf) } == NVAPI_OK => super::cchars_to_string(&buf),
            _ => String::new(),
        };
        if text.is_empty() {
            format!("NVAPI status {status}")
        } else {
            format!("{text} ({status})")
        }
    }
}

impl Drop for NvApi {
    fn drop(&mut self) {
        if let Some(unload) = self.Unload {
            // SAFETY: balanced with the successful Initialize in `load`.
            unsafe { unload() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The version words must match what the C macro produces for the
    /// official structures (sizes are asserted by bindgen's layout checks).
    #[test]
    fn version_words_match_sdk_layouts() {
        assert_eq!(NVDRS_SETTING_VER, 12320 | (1 << 16));
        assert_eq!(NVDRS_APPLICATION_VER_V4, 20492 | (4 << 16));
        assert_eq!(NVDRS_PROFILE_VER, 4116 | (1 << 16));
        assert_eq!(NVDRS_SETTING_VALUES_VER, 414112 | (1 << 16));
        assert_eq!(NV_GPU_THERMAL_SETTINGS_VER, 68 | (2 << 16));
        assert_eq!(NV_GPU_DYNAMIC_PSTATES_INFO_EX_VER, 72 | (1 << 16));
        assert_eq!(NV_GPU_CLOCK_FREQUENCIES_VER, 264 | (3 << 16));
        assert_eq!(NV_GPU_MEMORY_INFO_EX_VER, 80 | (1 << 16));
    }

    #[test]
    fn missing_driver_is_an_error_not_a_crash() {
        // On a machine without the NVIDIA driver (and on every non-Windows
        // build machine) loading must fail cleanly with a readable reason.
        if let Err(e) = NvApi::load() {
            assert!(!e.is_empty());
        }
    }
}

/// One process-wide NVAPI instance, shared by telemetry and the DRS layer.
pub fn shared() -> Result<std::sync::Arc<NvApi>, String> {
    static API: std::sync::OnceLock<Result<std::sync::Arc<NvApi>, String>> = std::sync::OnceLock::new();
    API.get_or_init(|| NvApi::load().map(std::sync::Arc::new)).clone()
}
