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

// ── NVIDIA driver profiles ───────────────────────────────────────────────

export type NvKind = "dword" | "qword" | "wstring" | "ansi" | "binary";
/** A driver value as text: decimal for numbers, base64 for binary. */
export interface NvValue { type: NvKind; value: string }
export interface NvValueLabel { value: number; label: string }
export interface NvStatus { available: boolean; reason: string | null; driver: { version: string | null; branch: string | null }; knownSettings: number; referenceLoaded: boolean }
export type NvProfileKind = "global" | "predefined" | "user";
export interface NvProfileSummary { name: string; kind: NvProfileKind; executables: string[]; settingCount: number; customisedCount: number; userExecutables: number }
export type NvSettingState = "user" | "nvidia" | "inherited" | "default" | "unset";
export type NvNameSource = "nvidia" | "profileInspector" | "reference" | "driver" | "none";

export interface NvSetting {
  id: number;
  idHex: string;
  name: string;
  nameSource: NvNameSource;
  alternateNames: string[];
  group: string;
  description: string;
  kind: NvKind;
  kindLabel: string;
  value: NvValue | null;
  valueLabel: string | null;
  raw: string;
  default: NvValue | null;
  defaultLabel: string | null;
  state: NvSettingState;
  modified: boolean;
  options: NvValueLabel[];
  documented: boolean;
  exposed: boolean;
  common: boolean;
  hidden: boolean;
  internal: boolean;
  editable: boolean;
  readOnlyReason: string | null;
  note: string | null;
}

export interface NvApp { name: string; friendlyName: string; launcher: string; fileInFolder: string; commandLine: string; isPredefined: boolean }
export interface NvProfileDetail { summary: NvProfileSummary; apps: NvApp[]; settings: NvSetting[]; groups: string[]; internalScan: boolean }

export type NvChange =
  | { op: "set"; id: number; value: NvValue }
  | { op: "restore"; id: number }
  | { op: "restoreProfile" }
  | { op: "addApp"; exe: string; findFile?: string | null }
  | { op: "removeApp"; exe: string };

export interface NvDiffLine { profile: string; settingId: number | null; what: string; before: string; after: string; note: string | null }
export interface NvApplyOutcome { backupId: string; changes: NvDiffLine[]; verified: boolean; mismatches: string[] }

export interface NipSetting { id: number; nameInfo: string | null; valueType: string; rawValue: string; typeAbsent: boolean }
export interface NipProfile { name: string; executables: string[]; settings: NipSetting[]; findFiles: { executable: string | null; findFile: string | null }[]; extra: string[] }
export interface NipFile { profiles: NipProfile[]; encoding: string; hasFindFiles: boolean; warnings: string[] }

export interface NvPreviewSetting {
  id: number;
  idHex: string;
  name: string;
  nameSource: NvNameSource;
  documented: boolean;
  valueType: string;
  rawValue: string;
  label: string | null;
  problem: string | null;
  live: string | null;
  sameAsLive: boolean;
  options: NvValueLabel[];
}
export interface NvPreviewProfile { name: string; exists: boolean; kind: NvProfileKind | null; executables: string[]; conflicts: [string, string][]; settings: NvPreviewSetting[]; liveOnly: NvPreviewSetting[]; unknownElements: number }
export interface NvImportPreview { file: NipFile; profiles: NvPreviewProfile[]; warnings: string[]; driverAvailable: boolean; driverReason: string | null }
export interface NvImportResult { profile: string; created: boolean; ok: boolean; error: string | null; applied: number; skipped: { id: number; name: string; reason: string }[] }
export interface NvImportOutcome { backupId: string; profiles: NvImportResult[]; verified: boolean; mismatches: string[] }

export interface NvBackup {
  id: string;
  createdAt: number;
  label: string;
  reason: "edit" | "import" | "manual" | "restore" | "delete";
  profiles: string[];
  createdProfiles: string[];
  file: string;
  changes: NvDiffLine[];
  verified: boolean | null;
  note: string | null;
  driverVersion: string | null;
}

export interface NipSettingDiff { id: number; name: string; a: string | null; b: string | null; aType: string | null; bType: string | null }
export interface NipProfileDiff { name: string; status: "onlyA" | "onlyB" | "changed" | "same"; settings: NipSettingDiff[]; executablesOnlyA: string[]; executablesOnlyB: string[]; settingsSame: number }
export interface NipDiff { profiles: NipProfileDiff[]; identical: boolean }
export interface NpiRun { finished: boolean; exitCode: number | null; message: string }

export const NV_NAME_SOURCE: Record<NvNameSource, string> = {
  nvidia: "Named by NVIDIA's SDK",
  profileInspector: "Named by the NVIDIA Profile Inspector project",
  reference: "Named by your Profile Inspector Reference.xml",
  driver: "Named by the installed driver",
  none: "No known name",
};

export const VENDOR_LABEL: Record<Vendor, string> = { nvidia: "NVIDIA", amd: "AMD", intel: "Intel", other: "Other" };

// ── GPU controls and Arcadia presets ─────────────────────────────────────

export type ControlKind =
  | { type: "toggle" }
  | { type: "range"; min: number; max: number; step: number; unit: string }
  | { type: "choice"; options: { value: number; label: string }[] }
  | { type: "readOnly" };

/** One control a vendor API reports for a GPU. Limits come from the driver. */
export interface GpuControl {
  id: string;
  vendor: Vendor;
  name: string;
  description: string;
  category: string;
  kind: ControlKind;
  value: number | null;
  default: number | null;
  supported: boolean;
  unsupportedReason: string | null;
  risk: Risk;
  level: number;
  requiresRestart: boolean;
}

export interface GpuControlsView { gpu: GpuDevice; api: ApiStatus; controls: GpuControl[]; note: string | null }

export interface PresetSetting { id: string; value: number | NvValue; name: string }

/** An Arcadia preset: vendor-specific settings in a shared JSON container. */
export interface GpuPreset {
  schema: number;
  id: string;
  name: string;
  vendor: Vendor;
  gpuModel: string | null;
  driverVersion: string | null;
  minDriver: string | null;
  gameExe: string | null;
  settings: PresetSetting[];
  createdAt: number;
  createdWith: string;
  notes: string;
}

export interface PresetSettingCheck { id: string; name: string; current: string | null; target: string; status: "change" | "same" | "skip"; reason: string | null }
export interface PresetCheck { usable: boolean; problems: string[]; warnings: string[]; target: string; settings: PresetSettingCheck[]; level: number }
export interface PresetOutcome { applied: number; skipped: PresetSettingCheck[]; verified: boolean; message: string; sessionId: string | null; backupId: string | null }
