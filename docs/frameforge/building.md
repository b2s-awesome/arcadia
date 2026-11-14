# Building, testing and troubleshooting

## Build

FrameForge builds with the rest of Arcadia; see the main README ("Build it (Windows)"). No extra step is needed for the default build.

### AMD ADLX support (optional feature)

AMD's SDK licence does not allow shipping its headers as source, so ADLX bindings are generated on your machine:

1. Get the SDK: `git clone https://github.com/GPUOpen-LibrariesAndSDKs/ADLX`.
2. Install LLVM (bindgen needs `libclang`); set `LIBCLANG_PATH` if it is not found.
3. Build with the feature:

```powershell
$env:ADLX_SDK_DIR = "C:\path\to\ADLX"
npm run tauri build -- --features adlx
```

Without the feature the app builds and runs; the AMD card is detected, and GPU Control says ADLX support was not compiled in.

### Runtime requirements (nothing is bundled)

| Feature | Needs |
|---|---|
| NVIDIA readings and driver profiles | NVIDIA driver (`nvapi64.dll`) |
| AMD readings and controls | AMD Software driver (`amdadlx64.dll`) and a build with `adlx` |
| Intel readings and controls | Intel graphics driver (`ControlLib.dll`) |
| Live Monitor and benchmarks | Intel PresentMon installed (service), or the PresentMon console application chosen in Live Monitor |

## Windows permissions

Arcadia runs as the signed-in user and does not ask for elevation.

- Power plans, per-user registry settings, startup entries under the user's `Run` key and the user's own processes need no elevation.
- NVIDIA DRS, ADLX and IGCL calls work for a standard user through the vendor's driver service.
- PresentMon: the PresentMon **service** does the privileged tracing, so Arcadia stays unprivileged. The **console application** uses ETW directly and needs to run elevated or by a member of the "Performance Log Users" group; if it cannot start a trace, Live Monitor shows its error.
- Processes owned by other users or by the system are protected and never touched.

## Tests

```sh
cargo test -p arcadia-core            # core, including all FrameForge unit tests
npm run check --prefix ui             # svelte-check, TypeScript
npm run build --prefix ui
npm run test:ui --prefix ui           # browser checks (needs Chrome, Chromium or Edge; set CHROME_PATH if it is not found)
```

What the unit tests cover: frame statistics; PresentMon CSV parsing and replay; hardware model; GPU registry and control validation; diagnostics rules; the optimiser (apply, verify, rollback on failure, revert, history, level gating, Realtime refusal, protected processes); `.nip` parse / write / compare with fixtures; DRS value encoding; NVIDIA profile editing, import and snapshot restore against a mock driver; Intel control mapping; presets; game profiles and the launch lifecycle; benchmark summary, comparison flags and export.

The tests use mocks and a temporary data folder. They never change the machine they run on.

What the UI checks cover: every tab renders from backend data; all four themes; missing hardware and missing PresentMon are explained without fake numbers; optimiser select / confirm / apply / revert; Expert acknowledgement; protected processes have no controls; NVIDIA edit / review / apply / restore, invalid values, import dialog; GPU Control per vendor, level locks and presets; game profile create / edit / duplicate / apply / revert with automation off by default; benchmark record / compare.

Not covered by any automated test: real drivers, real PresentMon, real Windows settings. See the verification status in the [overview](README.md).

To exercise the `.nip` reader on your own Profile Inspector exports:

```sh
ARCADIA_NIP_SAMPLES=/path/to/folder cargo test -p arcadia-core nip_samples -- --nocapture
```

### Checking the Windows build from Linux

`cargo check --target x86_64-pc-windows-gnu -p arcadia-core` (and `-p arcadia`) type-checks the Windows code without running it. It needs a MinGW-capable C compiler for the few C dependencies.

## Troubleshooting

| Symptom | Cause and fix |
|---|---|
| Live Monitor says PresentMon is not set up | Install Intel PresentMon (the installer adds the service), or choose `PresentMon-x.y.z-x64.exe` with "Choose console application". Arcadia does not download it. |
| "PresentMonAPI2.dll and the PresentMon Service are different versions" | Reinstall PresentMon so the service and its middleware match. |
| No displayed FPS, only presented | The source reported no display timing for that game (for example some exclusive or protected presentation paths). Presented FPS is shown and labelled. |
| No 0.1% low | Fewer than 3000 frames so far. Keep measuring. |
| A GPU reading shows "not reported" | The vendor API did not return it for this card or driver. Nothing is estimated. |
| AMD card: "compiled without AMD ADLX support" | Build with the `adlx` feature (above). |
| NVIDIA Profiles: a setting is read-only and flagged | It is an encrypted predefined setting. FrameForge shows it raw and does not change it. |
| NVIDIA apply says "not everything read back as expected" | The driver stored a different value. The differences are listed; the snapshot taken before the change is under Backups. |
| A profile did not apply automatically | Check, in order: "Apply profiles when their game starts" is on; the profile is switched on with "Apply automatically"; it is not marked "Not reviewed"; the executable name matches the running process. Recent activity says what was left out and why. |
| An optimiser action shows "not applicable" | The reason is shown on the row (for example no second GPU, or the setting is already at the target). |
| A revert failed | The entry stays in History with the error. Retry, or set the value by hand using the "before" value shown. |
