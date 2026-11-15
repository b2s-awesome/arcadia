# Hardware and vendor integrations

Vendor libraries are loaded at run time from the installed driver or service. Arcadia ships no vendor binary. When a library is missing, the adapter reports "unavailable" with the reason, and the UI shows that instead of controls or numbers.

## Hardware detection

| Data | Source |
|---|---|
| CPU, memory totals, OS | `sysinfo`, Windows version APIs |
| Memory modules | WMI `Win32_PhysicalMemory` |
| Drives and disk type | WMI storage namespace (`MSFT_PhysicalDisk`, `MSFT_Partition`) |
| Displays, refresh rates, HDR | `QueryDisplayConfig`, `DisplayConfigGetDeviceInfo`, `EnumDisplaySettingsW` |
| GPUs | DXGI adapter enumeration plus the display-class registry key; vendor adapters add detail |
| Power plans | `PowerEnumerate`, `PowerGetActiveScheme` |
| Startup apps | The documented `Run` registry keys |
| Per-process GPU use | Windows GPU performance counters |

A value that cannot be read is `None` and is shown as "not reported". Nothing is guessed or hardcoded.

## GPU abstraction

`gpu::VendorAdapter` is the single interface: `status`, `enrich` (driver version, features), `telemetry`, `controls`, `set_control`, `vrr`. `GpuRegistry` picks the adapter by PCI vendor id. A `GpuControl` carries its kind (toggle, range with driver-reported limits, choice, read-only), current and default value, whether the driver supports it and why not, a risk and an optimiser level. `validate_control` refuses any value outside what the driver reported before it reaches the driver.

## NVIDIA

**NVAPI** (`nvapi64.dll`, loaded through `nvapi_QueryInterface`): GPU identification, driver version and branch, utilisation, temperature, clocks, memory, G-SYNC capability where exposed. The public NVAPI has no setters for power limit, fan curve or clock offsets, so FrameForge shows those as unsupported and explains why. It does not use private interfaces.

**DRS (driver settings)**, the same database NVIDIA Profile Inspector edits:

- Enumerate profiles (global, predefined, user), their applications and settings.
- Read each setting's current and predefined value, type (DWORD, QWORD via binary, wide string, ANSI string, binary) and origin (user, NVIDIA predefined, inherited from global, driver default).
- Change, restore to default or delete settings; restore a whole profile; create and delete user profiles; add and remove applications.
- Save, then read back and compare ("verified").
- Names and value lists come from, in order: NVIDIA's `NvApiDriverSettings.h` (MIT), Profile Inspector's `CustomSettingNames.xml` (MIT, embedded with its licence), the driver's own `EnumAvailableSettingIds` / `GetSettingNameFromId` / `EnumAvailableSettingValues`, and optionally a `Reference.xml` the user points to at run time. Each name shows its source; unknown settings are shown by id and marked undocumented.

Limits, stated in the UI:

- Predefined settings NVIDIA stores encrypted are shown raw, flagged and read-only. FrameForge does not decrypt them.
- NVIDIA-predefined applications are never removed by an import.
- `NvAPI_DRS_SaveSettingsToFile` writes a binary database, so "does this profile hold internal settings" cannot be answered from it; the UI says so.

Every DRS change takes a `.nip` snapshot of the affected profiles first. The snapshot list (with reason, differences and verified flag) is the change record; restoring a snapshot is a replace-import of it.

### `.nip` compatibility

Format, checked byte for byte against real Profile Inspector exports (round trip is identical):

- UTF-8 with BOM, XML declaration says `utf-16`, CR LF, two-space indent, no final newline. UTF-16 input is read too.
- Root `ArrayOfProfile`; each `Profile` has `ProfileName`, `Executeables/string`, `Settings/ProfileSetting` (`SettingNameInfo`, `SettingID`, `SettingValue`, `ValueType`) and optionally `ExecutableFindFiles`.
- `ValueType`: `Dword`, `AnsiString`, `String`, `Binary` (base64), `Qword`.
- Unknown elements and unknown value types are kept and written back unchanged, so a file from a newer Profile Inspector is not damaged.

Import shows file against driver for every setting and asks before writing. Modes: replace or merge. Export covers chosen profiles or all customised ones. Two files can be compared without a driver.

### Profile Inspector fallback

Optional. The user points FrameForge at their own `nvidiaProfileInspector.exe`; the path is validated and the tool is started directly (no shell) with only the documented arguments: a `.nip` path with `-silentImport`, or `-exportCustomized`. Arcadia does not download or bundle the tool.

## AMD (ADLX)

The ADLX SDK licence allows redistribution in object-code form only, so no ADLX header-derived source is in this repository. ADLX support is the cargo feature `adlx`: `crates/core/build.rs` generates the bindings at build time from an SDK checkout named by `ADLX_SDK_DIR` (see [building](building.md)). Without the feature the AMD adapter reports that the build has no ADLX support.

With the feature: identification and driver version, telemetry from the performance-monitoring service, and these controls when the driver reports them as supported: Anti-Lag, Chill, Boost, Enhanced Sync, Image Sharpening and sharpness, Frame Rate Target Control and its FPS, Wait for Vertical Refresh, Radeon Super Resolution. All are Advanced level. No tuning (clock, voltage, fan) interface is used.

## Intel (IGCL)

`ControlLib.dll` from the Intel graphics driver. Telemetry through the power and temperature telemetry call. Controls are built from `ctlGetSupported3DCapabilities` only: frame pacing, frame limit, anisotropic filtering, CMAA, texture filtering quality, adaptive tessellation, sharpening, flip mode, low latency, windowed VRR. A feature the driver does not list does not appear as available. Sustained power limit and temperature limit are Expert level and use the driver's own range and units, unconverted.

## PresentMon

Frame data comes from Intel PresentMon, in this order:

1. **PresentMon Service** through `PresentMonAPI2.dll`. The DLL path is read from the registry value the PresentMon installer writes (`HKLM\SOFTWARE\INTEL\PresentMon\Service`, `sharedMiddlewarePath`), so the middleware always matches the installed service. The API major version is checked, and only metrics the service's introspection reports as available are requested.
2. **PresentMon console application**, chosen by the user. The path is validated and the program is started directly with fixed arguments; its CSV output is parsed.

If neither is present, Live Monitor shows setup steps and no numbers. Arcadia never downloads PresentMon and never ships a copy of the DLL.

Per frame: frame time, displayed time, CPU busy / wait, GPU busy / wait, display latency, dropped and generated flags, tearing. Displayed FPS uses frames as shown on screen; when the source has no display timing, presented FPS is shown and labelled.

## Arcadia GPU presets

A preset is a JSON file with a vendor and a list of that vendor's setting ids and values (`drs.0x…` typed driver values for NVIDIA, `adlx.*` or `igcl.*` control ids with numbers). The container is shared; the contents are not interchangeable. A preset is applied only to a GPU of its own vendor, each setting is checked against the driver first, and what cannot be applied is listed with the reason. AMD and Intel presets go through the optimiser (recorded, revertible); NVIDIA presets go through DRS with a snapshot. A preset is not a `.nip` file.
