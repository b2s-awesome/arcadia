// Browser-only preview data for the Performance section.
//
// This file is used ONLY when the UI runs in a plain browser (design work,
// UI tests). Inside Arcadia every `ff_*` call goes to the Rust backend and
// none of these values exist. The preview machine names itself as such.
import { optMock } from "./ffmockOpt";
import { nvMock } from "./ffmockNv";
import { gpuMock } from "./ffmockGpu";
import { profileMock } from "./ffmockProfiles";
import type { FfConfig, FrameStats, GpuDevice, GpuSample, MonitorSnapshot, Overview, ProcessInfo, Sample } from "./fftypes";

const GB = 1024 ** 3;
const preview = typeof location !== "undefined" ? new URLSearchParams(location.search) : new URLSearchParams();
/** `?ffgpu=amd|intel|none` and `?ffpm=0` switch the preview machine, for UI tests. */
const gpuMode = preview.get("ffgpu") ?? "nvidia";
const pmReady = preview.get("ffpm") !== "0";

const nvidia: GpuDevice = {
  id: "10de:2782:0:1", vendor: "nvidia", name: "Preview GeForce RTX 4070 Ti", vendorId: 0x10de, deviceId: 0x2782, subsystemId: 0, revision: 0xa1, luid: "0:1",
  kind: "discrete", kindSource: "vendor API", driverVersion: "32.0.15.6614", driverVersionVendor: "566.14", driverBranch: "r565_00", vramBytes: 12 * GB, sharedBytes: 16 * GB,
  outputs: ["\\\\.\\DISPLAY1"], features: ["Driver profiles (DRS)"], api: { state: "available", api: "NVAPI", version: "driver 566.14" },
};
const amd: GpuDevice = {
  ...nvidia, id: "1002:747e:0:2", vendor: "amd", name: "Preview Radeon RX 7800 XT", vendorId: 0x1002, deviceId: 0x747e, driverVersionVendor: null, driverBranch: null, features: [],
  vramBytes: 16 * GB, api: { state: "unavailable", api: "ADLX", reason: "This build of Arcadia was compiled without AMD ADLX support (the `adlx` build feature)." },
};
const intel: GpuDevice = {
  ...nvidia, id: "8086:56a0:0:3", vendor: "intel", name: "Preview Arc A770", vendorId: 0x8086, deviceId: 0x56a0, driverVersionVendor: null, driverBranch: null, features: [],
  vramBytes: 16 * GB, api: { state: "available", api: "IGCL", version: "1.1" },
};
const igpu: GpuDevice = {
  ...intel, id: "8086:a780:0:4", name: "Preview UHD Graphics 770", deviceId: 0xa780, kind: "integrated", kindSource: "heuristic", vramBytes: 128 * 1024 ** 2, outputs: [],
  api: { state: "unavailable", api: "IGCL", reason: "ControlLib.dll was not found. It ships with current Intel Arc and Iris Xe graphics drivers." },
};
const gpus: GpuDevice[] = gpuMode === "none" ? [] : gpuMode === "amd" ? [amd] : gpuMode === "intel" ? [intel] : [nvidia, igpu];

export const ffMockGpus = gpus;

let config: FfConfig = {
  schema: 2, presentmonCli: null, npiExe: null,
  monitorTiles: ["fps", "frameTimeGraph", "avgFps", "low1", "low01", "frameTime", "consistency", "cpuBusy", "gpuBusy", "gpuUtil", "cpuUtil", "gpuTemp", "gpuPower", "vram", "ram"],
  optimizerLevel: 1, expertAcknowledged: false, autoProfiles: false, autoMonitor: false, restoreOnExit: true, nvidiaFavourites: [],
};

function overview(): Overview {
  return {
    inventory: {
      cpu: { manufacturer: "AMD", model: "Preview Ryzen 7 7800X3D 8-Core Processor", architecture: "x86_64", physicalCores: 8, logicalCores: 16, baseFrequencyMhz: 4200 },
      gpus,
      memory: {
        totalBytes: 32 * GB,
        modules: [
          { capacityBytes: 16 * GB, kind: "DDR5", speedMhz: 6000, manufacturer: "Preview", partNumber: "PV-16G", slot: "DIMM_A2" },
          { capacityBytes: 16 * GB, kind: "DDR5", speedMhz: 6000, manufacturer: "Preview", partNumber: "PV-16G", slot: "DIMM_B2" },
        ],
        modulesNote: null,
      },
      drives: [
        { mount: "C:\\", label: "Windows", fileSystem: "NTFS", kind: "SSD", bus: "NVMe", model: "Preview NVMe 1TB", totalBytes: 931 * GB, freeBytes: 212 * GB, removable: false },
        { mount: "D:\\", label: "Games", fileSystem: "NTFS", kind: "SSD", bus: "NVMe", model: "Preview NVMe 2TB", totalBytes: 1863 * GB, freeBytes: 97 * GB, removable: false },
      ],
      displays: gpus.length ? [{
        device: "\\\\.\\DISPLAY1", name: "Preview 27\" monitor", primary: true, width: 2560, height: 1440, nativeWidth: 2560, nativeHeight: 1440, refreshHz: 60,
        availableRefreshHz: [60, 120, 144, 165], hdrSupported: true, hdrEnabled: false, vrrSupported: gpuMode === "nvidia" ? true : null,
        vrrNote: gpuMode === "nvidia" ? null : "No vendor API reported variable refresh for this display.", bitsPerColor: 10, adapter: gpus[0].name,
      }] : [],
      system: {
        osName: "Windows 11 (browser preview)", osVersion: "24H2", osBuild: "26100", hostName: "PREVIEW", isWindows: true, isElevated: false,
        powerPlans: [
          { guid: "381b4222-f694-41f0-9685-ff5bb260df2e", name: "Balanced", active: true },
          { guid: "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c", name: "High performance", active: false },
        ],
        startupApps: [
          { name: "Discord", command: "\"C:\\Users\\you\\AppData\\Local\\Discord\\Update.exe\" --processStart Discord.exe", scope: "HKCU", enabled: true },
          { name: "Steam", command: "\"C:\\Program Files (x86)\\Steam\\steam.exe\" -silent", scope: "HKCU", enabled: true },
          { name: "SecurityHealth", command: "%windir%\\system32\\SecurityHealthSystray.exe", scope: "HKLM", enabled: true },
        ],
        gaming: { gameMode: true, backgroundRecording: true, captureEnabled: true, hags: true, windowedOptimizations: false, systemVrr: null, tearingSupported: true, onBattery: false },
      },
      warnings: gpus.length ? [] : ["No graphics adapter was found."],
      scannedAt: Math.floor(Date.now() / 1000),
    },
    vendorApis: [
      { vendor: "nvidia", api: "NVAPI", status: gpuMode === "nvidia" ? nvidia.api : { state: "unavailable", api: "NVAPI", reason: "nvapi64.dll was not found. Install the NVIDIA graphics driver." }, hardwarePresent: gpuMode === "nvidia" },
      { vendor: "amd", api: "ADLX", status: amd.api, hardwarePresent: gpuMode === "amd" },
      { vendor: "intel", api: "IGCL", status: gpuMode === "intel" ? intel.api : igpu.api, hardwarePresent: gpuMode === "intel" || gpuMode === "nvidia" },
    ],
    presentmon: presentmon(),
  };
}

function presentmon() {
  return pmReady
    ? { serviceReady: true, serviceDetail: "PresentMon Service, API 3.1.0 (preview)", cliReady: false, cliDetail: "No PresentMon console application has been chosen", willUse: "service" as const, setupSteps: [], downloadUrl: "https://github.com/GameTechDev/PresentMon/releases" }
    : {
        serviceReady: false, serviceDetail: "The PresentMon Service is not installed (version 2.3.1 or newer is needed).", cliReady: false, cliDetail: "No PresentMon console application has been chosen", willUse: null,
        setupSteps: [
          "Download the PresentMon installer (PresentMon 2.3.1 or newer) from Intel's GitHub releases page.",
          "Run the installer. It adds the PresentMon Service, which measures frames for any game.",
          "Come back here and press Check again. Arcadia finds the service by itself.",
        ],
        downloadUrl: "https://github.com/GameTechDev/PresentMon/releases",
      };
}

let t = 0;
const wobble = (base: number, amp: number, speed = 1) => base + Math.sin(t * speed) * amp + (Math.random() - 0.5) * amp * 0.4;

function gpuSample(g: GpuDevice): GpuSample {
  const full = g.api.state === "available";
  const unavailable: Record<string, string> = {};
  const why = g.api.state === "unavailable" ? `${g.api.api} unavailable: ${g.api.reason}` : "Not exposed by the driver or Windows for this adapter";
  if (!full) for (const m of ["temperature", "power", "fanRpm", "fanPct", "coreClock", "memoryClock"]) unavailable[m] = why;
  else { unavailable.power = "Not part of public NVAPI; shown when the PresentMon service reports it"; unavailable.fanPct = unavailable.power; }
  const util = g.kind === "integrated" ? wobble(4, 2) : wobble(96, 3);
  return {
    id: g.id, name: g.name, utilizationPct: Math.max(0, Math.min(100, util)), vramUsedBytes: g.kind === "integrated" ? null : Math.round(wobble(9.8, 0.2, 0.3) * GB), vramTotalBytes: g.vramBytes,
    temperatureC: full ? wobble(71, 1.5, 0.2) : null, hotspotC: null, powerW: null, fanRpm: full && g.vendor === "nvidia" ? wobble(1650, 40, 0.2) : null, fanPct: null,
    coreClockMhz: full ? wobble(2610, 15) : null, memoryClockMhz: full ? 10501 : null, voltageV: null, thermalLimited: full ? false : null, powerLimited: full ? false : null,
    sources: full ? { utilization: g.api.state === "available" ? g.api.api : "Windows", temperature: "NVAPI" } : { utilization: "Windows" },
    unavailable: g.kind === "integrated" ? { ...unavailable, vramUsed: "Not exposed by the driver or Windows for this adapter" } : unavailable,
  };
}

function sample(): Sample {
  t += 0.35;
  const used = wobble(19.5, 0.3, 0.2) * GB;
  return {
    atMs: Date.now(), cpuPct: wobble(38, 8), cpuMaxCorePct: wobble(82, 9), cpuFrequencyMhz: 4850, cpuTemperatureC: null,
    cpuTemperatureNote: "Windows has no general CPU temperature interface; shown when the PresentMon service reports it.",
    ramTotalBytes: 32 * GB, ramUsedBytes: used, ramAvailableBytes: 32 * GB - used, ramPct: (used / (32 * GB)) * 100, swapUsedBytes: 2 * GB, swapTotalBytes: 8 * GB,
    gpus: gpus.map(gpuSample),
  };
}

const proc = (pid: number, name: string, category: ProcessInfo["category"], cpuPct: number, memGb: number, extra: Partial<ProcessInfo> = {}): ProcessInfo => ({
  pid, name, exe: `C:\\Preview\\${name}`, cpuPct, memoryBytes: memGb * GB, gpuPct: null, owner: "you", category, gameId: null, gameName: null, relevance: "",
  closable: category !== "protected" && category !== "game", protectedReason: category === "protected" ? "Windows, driver, security or anti-cheat component" : null, ...extra,
});
export const ffMockProcesses: ProcessInfo[] = [
  proc(9120, "Cyberpunk2077.exe", "game", 31, 9.4, { gpuPct: 93, gameId: "steam:1091500", gameName: "Cyberpunk 2077", exe: "D:\\SteamLibrary\\steamapps\\common\\Cyberpunk 2077\\bin\\x64\\Cyberpunk2077.exe" }),
  proc(4412, "chrome.exe", "browser", 6, 3.1, { relevance: "using 6% of the CPU, holding 3.1 GB of memory" }),
  proc(5210, "Discord.exe", "chat", 1, 0.6),
  proc(6120, "obs64.exe", "overlay", 4, 0.9, { gpuPct: 5, relevance: "using 5% of the GPU, draws an overlay or records the screen" }),
  proc(3020, "steam.exe", "launcher", 0.4, 0.3),
  proc(1180, "MsMpEng.exe", "protected", 0.8, 0.4, { owner: "SYSTEM" }),
  proc(980, "dwm.exe", "protected", 1.2, 0.2, { owner: "DWM-1" }),
  proc(7702, "notepad.exe", "app", 0, 0.05),
];

// ── monitor ──────────────────────────────────────────────────────────────

let monitor: { pid: number; name: string; started: number; frames: number[]; history: Sample[] } | null = null;
/** Frame count when the preview benchmark recording began; null when not recording. */
let recordFrom: number | null = null;
let recordStarted = 0;

function stats(frames: number[]): FrameStats {
  const n = frames.length;
  const empty: FrameStats = { frames: n, durationMs: 0, avgFps: null, avgFrameTimeMs: null, minFrameTimeMs: null, maxFrameTimeMs: null, p50Ms: null, p90Ms: null, p95Ms: null, p99Ms: null, p999Ms: null, low1pctFps: null, low01pctFps: null, p1Fps: null, p01Fps: null, stddevMs: null, consistencyPct: null, stutterFrames: null, notes: [] };
  if (n < 10) return { ...empty, notes: [`Not enough frames yet (${n} of 10)`] };
  const s = [...frames].sort((a, b) => a - b);
  const sum = s.reduce((a, b) => a + b, 0);
  const pct = (p: number) => { const r = (p / 100) * (n - 1); const lo = Math.floor(r); return s[lo] + (s[Math.ceil(r)] - s[lo]) * (r - lo); };
  const low = (perMille: number) => { const k = Math.floor((n * perMille) / 1000); if (k < 3) return null; const part = s.slice(n - k).reduce((a, b) => a + b, 0); return (1000 * k) / part; };
  const p50 = pct(50), p99 = pct(99);
  const out: FrameStats = { ...empty, durationMs: sum, avgFps: (1000 * n) / sum, avgFrameTimeMs: sum / n, minFrameTimeMs: s[0], maxFrameTimeMs: s[n - 1], p50Ms: p50, p90Ms: pct(90), p95Ms: pct(95), p99Ms: p99, p999Ms: pct(99.9), low1pctFps: low(10), low01pctFps: low(1), stddevMs: 0, consistencyPct: Math.min(100, (p50 / p99) * 100), stutterFrames: s.filter((x) => x > Math.max(p50 * 2, p50 + 4)).length };
  if (out.low1pctFps === null) out.notes.push(`1% low needs 300 frames, have ${n}`);
  if (out.low01pctFps === null) out.notes.push(`0.1% low needs 3000 frames, have ${n}`);
  return out;
}

function snapshot(): MonitorSnapshot {
  const idle = stats([]);
  if (!monitor) return { state: "idle", pid: null, process: "", source: null, sourceDetail: "", error: null, elapsedMs: 0, totalFrames: 0, displayedFpsNow: null, presentedFpsNow: null, displayed: idle, presented: idle, latestFrameTimeMs: null, cpuBusyMs: null, cpuWaitMs: null, gpuBusyMs: null, gpuWaitMs: null, displayLatencyMs: null, droppedPct: null, generatedPct: null, frameTimesMs: [], telemetry: null, telemetryHistory: [], recording: false, recordedFrames: 0, notes: [] };
  for (let i = 0; i < 36; i++) monitor.frames.push(Math.random() < 0.01 ? wobble(31, 4) : wobble(13.6, 1.2, 3));
  if (monitor.frames.length > 2200) monitor.frames.splice(0, monitor.frames.length - 2200);
  monitor.history.push(sample());
  if (monitor.history.length > 120) monitor.history.shift();
  const st = stats(monitor.frames);
  const last = monitor.frames.slice(-70);
  const now = last.length / (last.reduce((a, b) => a + b, 0) / 1000);
  return {
    state: "running", pid: monitor.pid, process: monitor.name, source: "service", sourceDetail: "PresentMon Service, API 3.1.0 (preview)", error: null, elapsedMs: Date.now() - monitor.started,
    totalFrames: monitor.frames.length, displayedFpsNow: now, presentedFpsNow: now, displayed: st, presented: st, latestFrameTimeMs: last[last.length - 1], cpuBusyMs: 6.1, cpuWaitMs: 7.5, gpuBusyMs: 13.1, gpuWaitMs: 0.5,
    displayLatencyMs: 34, droppedPct: 0.2, generatedPct: 0, frameTimesMs: monitor.frames.slice(-240), telemetry: monitor.history[monitor.history.length - 1], telemetryHistory: monitor.history, recording: recordFrom !== null, recordedFrames: recordFrom === null ? 0 : Math.round((Date.now() - recordStarted) * 0.0735), notes: [],
  };
}

const gpuHandlers = gpuMock(gpus, optMock.ff_opt_apply);
export const ffMockExtra: Record<string, (args: Record<string, unknown>) => unknown> = {
  ...optMock, ...nvMock, ...gpuHandlers,
  ...profileMock({
    processes: () => ffMockProcesses,
    presets: () => gpuHandlers.ff_presets({}) as { id: string; name: string }[],
    actions: () => optMock.ff_opt_list({}) as { id: string; params: unknown; name: string; label: string | null; level: number; state: string }[],
    monitor: () => (monitor ? { pid: monitor.pid, name: monitor.name } : null),
    recording: () => recordFrom !== null,
    record: (on) => { recordFrom = on && monitor ? monitor.frames.length : null; if (on) recordStarted = Date.now(); },
    recordedMs: () => Date.now() - recordStarted,
    stats,
    inventory: () => overview().inventory,
  }),
};

export function ffMock(cmd: string, args: Record<string, unknown>): unknown {
  switch (cmd) {
    case "ff_overview": return overview();
    case "ff_telemetry": return sample();
    case "ff_processes": return ffMockProcesses;
    case "ff_running_game": return ffMockProcesses[0];
    case "ff_presentmon": return presentmon();
    case "ff_config_get": return structuredClone(config);
    case "ff_config_set": config = JSON.parse(JSON.stringify(args.config)) as FfConfig; return structuredClone(config);
    case "ff_monitor_start": monitor = { pid: args.pid as number, name: args.name as string, started: Date.now(), frames: [], history: [] }; return;
    case "ff_monitor_stop": if (recordFrom !== null) ffMockExtra.ff_bench_stop({}); monitor = null; return;
    case "ff_monitor_snapshot": return snapshot();
    default: {
      const f = ffMockExtra[cmd];
      if (f) return f(args);
      throw new Error(`The browser preview has no answer for ${cmd}`);
    }
  }
}
