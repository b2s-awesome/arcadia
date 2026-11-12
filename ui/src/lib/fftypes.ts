// Types for FrameForge (the Performance section). They mirror
// crates/core/src/frameforge; a value typed `T | null` is one the machine may
// not expose, and the UI shows it as unavailable instead of inventing it.

export type Vendor = "nvidia" | "amd" | "intel" | "other";
export type GpuKind = "discrete" | "integrated" | "software" | "unknown";

export type ApiStatus =
  | { state: "available"; api: string; version: string | null }
  | { state: "unavailable"; api: string; reason: string }
  | { state: "notApplicable" };

export interface GpuDevice {
  id: string;
  vendor: Vendor;
  name: string;
  vendorId: number;
  deviceId: number;
  subsystemId: number;
  revision: number;
  luid: string | null;
  kind: GpuKind;
  kindSource: string;
  driverVersion: string | null;
  driverVersionVendor: string | null;
  driverBranch: string | null;
  vramBytes: number | null;
  sharedBytes: number | null;
  outputs: string[];
  features: string[];
  api: ApiStatus;
}

export interface GpuTelemetry {
  utilizationPct: number | null;
  vramUsedBytes: number | null;
  vramTotalBytes: number | null;
  temperatureC: number | null;
  hotspotC: number | null;
  powerW: number | null;
  fanRpm: number | null;
  fanPct: number | null;
  coreClockMhz: number | null;
  memoryClockMhz: number | null;
  voltageV: number | null;
  thermalLimited: boolean | null;
  powerLimited: boolean | null;
  sources: Record<string, string>;
  unavailable: Record<string, string>;
}

export interface GpuSample extends GpuTelemetry {
  id: string;
  name: string;
}

export interface Sample {
  atMs: number;
  cpuPct: number | null;
  cpuMaxCorePct: number | null;
  cpuFrequencyMhz: number | null;
  cpuTemperatureC: number | null;
  cpuTemperatureNote: string | null;
  ramTotalBytes: number;
  ramUsedBytes: number;
  ramAvailableBytes: number;
  ramPct: number | null;
  swapUsedBytes: number;
  swapTotalBytes: number;
  gpus: GpuSample[];
}

export interface CpuInfo {
  manufacturer: string;
  model: string;
  architecture: string;
  physicalCores: number | null;
  logicalCores: number;
  baseFrequencyMhz: number | null;
}

export interface MemoryModule {
  capacityBytes: number;
  kind: string | null;
  speedMhz: number | null;
  manufacturer: string | null;
  partNumber: string | null;
  slot: string | null;
}

export interface DriveInfo {
  mount: string;
  label: string;
  fileSystem: string;
  kind: string | null;
  bus: string | null;
  model: string | null;
  totalBytes: number;
  freeBytes: number;
  removable: boolean;
}

export interface DisplayInfo {
  device: string;
  name: string;
  primary: boolean;
  width: number;
  height: number;
  nativeWidth: number | null;
  nativeHeight: number | null;
  refreshHz: number | null;
  availableRefreshHz: number[];
  hdrSupported: boolean | null;
  hdrEnabled: boolean | null;
  vrrSupported: boolean | null;
  vrrNote: string | null;
  bitsPerColor: number | null;
  adapter: string | null;
}

export interface PowerPlan { guid: string; name: string; active: boolean }
export interface StartupApp { name: string; command: string; scope: string; enabled: boolean }

export interface GamingConfig {
  gameMode: boolean | null;
  backgroundRecording: boolean | null;
  captureEnabled: boolean | null;
  hags: boolean | null;
  windowedOptimizations: boolean | null;
  systemVrr: boolean | null;
  tearingSupported: boolean | null;
  onBattery: boolean | null;
}

export interface SystemInfo {
  osName: string;
  osVersion: string;
  osBuild: string | null;
  hostName: string | null;
  isWindows: boolean;
  isElevated: boolean;
  powerPlans: PowerPlan[];
  startupApps: StartupApp[];
  gaming: GamingConfig;
}

export interface Inventory {
  cpu: CpuInfo;
  gpus: GpuDevice[];
  memory: { totalBytes: number; modules: MemoryModule[]; modulesNote: string | null };
  drives: DriveInfo[];
  displays: DisplayInfo[];
  system: SystemInfo;
  warnings: string[];
  scannedAt: number;
}

export interface VendorApi { vendor: Vendor; api: string; status: ApiStatus; hardwarePresent: boolean }

export interface PresentMonAvailability {
  serviceReady: boolean;
  serviceDetail: string;
  cliReady: boolean;
  cliDetail: string;
  willUse: "service" | "cli" | "replay" | null;
  setupSteps: string[];
  downloadUrl: string;
}

export interface Overview { inventory: Inventory; vendorApis: VendorApi[]; presentmon: PresentMonAvailability }

export interface FrameStats {
  frames: number;
  durationMs: number;
  avgFps: number | null;
  avgFrameTimeMs: number | null;
  minFrameTimeMs: number | null;
  maxFrameTimeMs: number | null;
  p50Ms: number | null;
  p90Ms: number | null;
  p95Ms: number | null;
  p99Ms: number | null;
  p999Ms: number | null;
  low1pctFps: number | null;
  low01pctFps: number | null;
  p1Fps: number | null;
  p01Fps: number | null;
  stddevMs: number | null;
  consistencyPct: number | null;
  stutterFrames: number | null;
  notes: string[];
}

export type MonitorState = "idle" | "running" | "ended" | "error";

export interface MonitorSnapshot {
  state: MonitorState;
  pid: number | null;
  process: string;
  source: "service" | "cli" | "replay" | null;
  sourceDetail: string;
  error: string | null;
  elapsedMs: number;
  totalFrames: number;
  displayedFpsNow: number | null;
  presentedFpsNow: number | null;
  displayed: FrameStats;
  presented: FrameStats;
  latestFrameTimeMs: number | null;
  cpuBusyMs: number | null;
  cpuWaitMs: number | null;
  gpuBusyMs: number | null;
  gpuWaitMs: number | null;
  displayLatencyMs: number | null;
  droppedPct: number | null;
  generatedPct: number | null;
  frameTimesMs: number[];
  telemetry: Sample | null;
  telemetryHistory: Sample[];
  recording: boolean;
  recordedFrames: number;
  notes: string[];
}

export type ProcessCategory = "game" | "launcher" | "browser" | "overlay" | "chat" | "protected" | "app";

export interface ProcessInfo {
  pid: number;
  name: string;
  exe: string | null;
  cpuPct: number;
  memoryBytes: number;
  gpuPct: number | null;
  owner: string | null;
  category: ProcessCategory;
  gameId: string | null;
  gameName: string | null;
  relevance: string;
  closable: boolean;
  protectedReason: string | null;
}

export interface FfConfig {
  schema: number;
  presentmonCli: string | null;
  npiExe: string | null;
  monitorTiles: string[];
  optimizerLevel: number;
  expertAcknowledged: boolean;
  autoProfiles: boolean;
  autoMonitor: boolean;
  restoreOnExit: boolean;
  nvidiaFavourites: number[];
}

// ── diagnostics and optimiser ────────────────────────────────────────────

export type Risk = "low" | "medium" | "high";
export type Restart = "none" | "game" | "signOut" | "windows";

export interface Finding {
  id: string;
  title: string;
  severity: "info" | "notice" | "warning";
  confidence: "low" | "medium" | "high";
  evidence: string[];
  why: string;
  recommendation: string;
  tradeOff: string;
  risk: Risk;
  restart: Restart;
  reversible: boolean;
  actions: string[];
  settingsUri: string | null;
}

export interface FrameEvidence {
  process: string;
  stats: FrameStats;
  cpuBusyMs: number | null;
  cpuWaitMs: number | null;
  gpuBusyMs: number | null;
  gpuWaitMs: number | null;
  droppedPct: number | null;
  generatedPct: number | null;
}

export interface Diagnosis { generatedAt: number; findings: Finding[]; report: string; frames: FrameEvidence | null; basis: string }

export type ActionCategory = "power" | "windows" | "startup" | "process" | "graphics" | "driver";

export interface ActionView {
  id: string;
  name: string;
  description: string;
  category: ActionCategory;
  risk: Risk;
  level: number;
  restart: Restart;
  reversible: boolean;
  temporary: boolean;
  tradeOff: string;
  source: string;
  hardware: string;
  os: string;
  key: string;
  params: unknown;
  label: string | null;
  applicable: boolean;
  reason: string | null;
  current: string | null;
  proposed: string | null;
  state: "available" | "applied" | "alreadySet" | "notApplicable";
  sessionId: string | null;
  changeId: number | null;
}

export interface ApplyRequest { id: string; params: unknown }

export type ChangeStatus = "pending" | "applied" | "skipped" | "failed" | "rolledBack" | "reverted" | "revertFailed";

export interface OptChange {
  id: number;
  actionId: string;
  key: string;
  name: string;
  category: ActionCategory;
  risk: Risk;
  level: number;
  restart: Restart;
  reversible: boolean;
  temporary: boolean;
  params: unknown;
  beforeText: string;
  proposedText: string;
  afterText: string | null;
  status: ChangeStatus;
  error: string | null;
  appliedAt: number;
  revertedAt: number | null;
}

export type SessionStatus = "inProgress" | "applied" | "failed" | "failedDirty" | "partlyReverted" | "reverted";

export interface OptSession {
  schema: number;
  id: string;
  label: string;
  startedAt: number;
  level: number;
  status: SessionStatus;
  error: string | null;
  changes: OptChange[];
}

export const RESTART_TEXT: Record<Restart, string> = { none: "", game: "Restart the game", signOut: "Sign out and in", windows: "Restart Windows" };
export const LEVEL_NAME = ["", "Safe", "Advanced", "Expert"];

export const VENDOR_LABEL: Record<Vendor, string> = { nvidia: "NVIDIA", amd: "AMD", intel: "Intel", other: "Other" };
