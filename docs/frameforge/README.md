# FrameForge

FrameForge is Arcadia's performance suite. It lives in the **Performance** section of the app:

| Tab | What it does |
|---|---|
| Dashboard | Hardware, drivers, displays, vendor API status, live readings |
| Optimizer | Analysis of what limits performance; tiered, reversible system changes; background apps; history |
| Live Monitor | Frame rate, frame times and hardware load for one game, measured by PresentMon |
| Game Profiles | Per-game sets of changes, applied by hand or when the game starts |
| GPU Control | Driver controls each vendor's API offers; Arcadia GPU presets |
| NVIDIA Profiles | NVIDIA driver profile editor with `.nip` import, export, compare and snapshots |
| Benchmark History | Recorded runs, comparison with reliability warnings, CSV / JSON export |

## Documents

- [Architecture](architecture.md): modules, data flow, where things are stored
- [Hardware and vendor integrations](vendor-apis.md): detection, NVIDIA (NVAPI, DRS, `.nip`), AMD (ADLX), Intel (IGCL), PresentMon
- [Safety and rollback](safety.md): what the optimiser may and may not do, levels, records, revert, game profiles
- [Measuring](measuring.md): frame statistics, diagnostics rules, benchmarks and comparisons
- [Building, testing and troubleshooting](building.md): build requirements, Windows permissions, tests, common problems
- [References and third-party notices](references.md)

## Verification status

Read this before relying on any vendor feature.

- **Run and passing on Linux, against mocks:** the Rust unit tests (`cargo test -p arcadia-core`) and the browser UI checks (`npm run test:ui --prefix ui`). These cover the logic: parsing, validation, the optimiser's record / apply / verify / revert flow, profile matching, benchmark statistics, UI behaviour.
- **Type-checked for Windows, never executed:** every Windows and vendor code path (NVAPI, DRS, IGCL, ADLX, PresentMon, power plans, registry, WMI, process control). The code compiles for `x86_64-pc-windows-gnu`; that proves the calls are well-formed against the bindings, not that a driver accepts them.
- **Not yet run on Windows or on real hardware:** all of the above. Until someone runs the app on Windows with NVIDIA, AMD and Intel GPUs, treat hardware features as untested.

The UI never substitutes sample data inside the app. When an API is missing it says so and shows no numbers. Sample data exists only in the browser preview (`npm run dev`), where it is labelled as a preview.
