// Every call into the Rust side goes through here. Outside Tauri (plain
// browser during design work) a mock backend answers instead.
import type {
  DeviceCode, FoundInstance, Hit, ImportResult, Incident, LoaderVersion, McAccount, McInstance, McLoader, McVersion, ModFile, SecurityReport, Verified,
  Entry, Game, LaunchTarget, Preset, ProxyStatus, RobloxStatus, RuleConfig, ScanReport, Settings, RulesImported } from "./types";
import type {
  ActionView, ApplyRequest, Diagnosis, FfConfig, MonitorSnapshot, NipDiff, NipFile, NpiRun, NvApplyOutcome, NvBackup, NvChange, NvDiffLine, NvImportOutcome,
  NvImportPreview, NvProfileDetail, NvProfileSummary, NvStatus, OptSession, Overview, PresentMonAvailability, ProcessInfo, Sample,
  GpuControlsView, GpuPreset, PresetCheck, PresetOutcome,
  GameProfile, ProfileView, GameChoice, ProfileRun, LifecycleEvent, BenchSession, BenchComparison,
} from "./fftypes";
import { mockInvoke, mockListen } from "./mock";

const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!inTauri) return mockInvoke(cmd, args) as Promise<T>;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

export async function on<T>(event: string, cb: (payload: T) => void): Promise<() => void> {
  if (!inTauri) return mockListen(event, cb as (p: unknown) => void);
  const { listen } = await import("@tauri-apps/api/event");
  return listen<T>(event, (e) => cb(e.payload));
}

export async function pickFile(filters?: { name: string; extensions: string[] }[]): Promise<string | null> {
  if (!inTauri) return "C:\\Users\\you\\Pictures\\example.png";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ multiple: false, directory: false, filters });
  return typeof r === "string" ? r : null;
}

/** URL the webview can load for a local file (wallpapers). */
export function fileUrl(path: string): string {
  if (!inTauri) return path;
  const internals = (window as unknown as { __TAURI_INTERNALS__: { convertFileSrc: (p: string, proto: string) => string } }).__TAURI_INTERNALS__;
  return internals.convertFileSrc(path, "asset");
}

export const win = {
  async minimize() {
    if (!inTauri) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().minimize();
  },
  async toggleMaximize() {
    if (!inTauri) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().toggleMaximize();
  },
  async setTitle(title: string) {
    document.title = title;
    if (!inTauri) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().setTitle(title);
  },
  async close() {
    if (!inTauri) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().close();
  },
};

export const api = {
  getSettings: () => call<Settings>("get_settings"),
  saveSettings: (settings: Settings) => call<void>("save_settings", { settings }),

  scanLibrary: () => call<ScanReport>("scan_library"),
  launchGame: (id: string) => call<void>("launch_game", { id }),
  addCustomGame: (name: string, target: LaunchTarget, cover: string | null) =>
    call<void>("add_custom_game", { name, target, cover }),
  removeCustomGame: (id: string) => call<void>("remove_custom_game", { id }),

  robloxStatus: () => call<RobloxStatus>("roblox_status"),
  robloxLatest: (channel: string) => call<{ version: string; clientVersionUpload: string }>("roblox_latest", { channel }),
  robloxLaunch: (uri: string | null = null) => call<string>("roblox_launch", { uri }),
  robloxReinstall: () => call<void>("roblox_reinstall"),
  registerProtocol: () => call<void>("roblox_register_protocol"),
  clearCache: (kill: boolean) => call<{ removed: number; locked: number }>("clear_roblox_cache", { kill }),
  fflagPresets: () => call<Preset[]>("fflag_presets"),
  parseFflags: (json: string) => call<Record<string, string>>("parse_fflags", { json }),
  openFolder: (which: "mods" | "roblox" | "proxy" | "data") => call<void>("open_folder", { which }),

  proxyStatus: () => call<ProxyStatus>("proxy_status"),
  proxyStart: () => call<number>("proxy_start"),
  proxyStop: () => call<void>("proxy_stop"),
  rulesGet: () => call<RuleConfig>("rules_get"),
  rulesSave: (config: RuleConfig) => call<void>("rules_save", { config }),
  rulesImport: (json: string) => call<RulesImported>("rules_import", { json }),
  rulesImportFile: (path: string) => call<RulesImported>("rules_import_file", { path }),
  rulesImportFleasion: () => call<RulesImported>("rules_import_fleasion"),
  rulesExport: (profile: string | null) => call<string>("rules_export", { profile }),
  importFile: (src: string) => call<string>("proxy_import_file", { src }),
  parseIds: (text: string) => call<number[]>("parse_asset_ids", { text }),
  inspectorSnapshot: () => call<Entry[]>("inspector_snapshot"),
  inspectorClear: () => call<void>("inspector_clear"),
  setWallpaper: (src: string) => call<string>("set_wallpaper", { src }),
};

/** Minecraft launcher. */
export const mc = {
  versions: (snapshots = false) => call<McVersion[]>("mc_versions", { snapshots }),
  loaderVersions: (loader: McLoader, mc: string) => call<LoaderVersion[]>("mc_loader_versions", { loader, mc }),
  instances: () => call<McInstance[]>("mc_instances"),
  create: (name: string, mcVersion: string, loader: McLoader, loaderVersion: string | null, memoryMb: number) =>
    call<McInstance>("mc_create", { name, mc: mcVersion, loader, loaderVersion, memoryMb }),
  update: (id: string, name: string, memoryMb: number, javaArgs: string) => call<McInstance>("mc_update", { id, name, memoryMb, javaArgs }),
  remove: (id: string) => call<void>("mc_delete", { id }),
  duplicate: (id: string, name: string) => call<McInstance>("mc_duplicate", { id, name }),
  mods: (id: string) => call<ModFile[]>("mc_mods", { id }),
  toggleMod: (id: string, file: string, enabled: boolean) => call<string>("mc_mod_toggle", { id, file, enabled }),
  removeMod: (id: string, file: string) => call<void>("mc_mod_remove", { id, file }),
  openFolder: (id: string, sub: string | null = null) => call<void>("mc_open_folder", { id, sub }),
  search: (source: string, query: string, kind: string, instanceId: string | null) => call<Hit[]>("mc_search", { source, query, kind, instanceId }),
  install: (source: string, projectId: string, kind: string, instanceId: string) =>
    call<{ fileName: string; dependenciesAdded: string[] }>("mc_install", { source, projectId, kind, instanceId }),
  installPack: (source: string, projectId: string) => call<ImportResult>("mc_install_pack", { source, projectId }),
  importPath: (path: string) => call<ImportResult>("mc_import", { path }),
  discover: () => call<FoundInstance[]>("mc_discover"),
  accounts: () => call<McAccount[]>("mc_accounts"),
  loginStart: () => call<DeviceCode>("mc_login_start"),
  loginFinish: (code: DeviceCode) => call<McAccount>("mc_login_finish", { code }),
  selectAccount: (uuid: string) => call<void>("mc_account_select", { uuid }),
  removeAccount: (uuid: string) => call<void>("mc_account_remove", { uuid }),
  play: (id: string) => call<void>("mc_play", { id }),
  running: () => call<string[]>("mc_running"),
  kill: (id: string) => call<void>("mc_kill", { id }),
  incidents: () => call<Incident[]>("mc_incidents"),
  securityScan: () => call<SecurityReport>("mc_security_scan"),
  verify: (id: string) => call<Verified[]>("mc_verify", { id }),
  openLauncher: () => call<void>("open_minecraft_launcher"),
  popOut: () => call<void>("minecraft_pop_out"),
  dock: () => call<void>("minecraft_dock"),
  createShortcut: (name: string) => call<string[]>("create_minecraft_shortcut", { name }),
};

/** FrameForge: the Performance section. */
export const ff = {
  overview: (rescan = false) => call<Overview>("ff_overview", { rescan }),
  telemetry: () => call<Sample>("ff_telemetry"),
  processes: () => call<ProcessInfo[]>("ff_processes"),
  runningGame: () => call<ProcessInfo | null>("ff_running_game"),
  presentmon: () => call<PresentMonAvailability>("ff_presentmon"),
  configGet: () => call<FfConfig>("ff_config_get"),
  configSet: (config: FfConfig) => call<FfConfig>("ff_config_set", { config }),
  monitorStart: (pid: number, name: string) => call<void>("ff_monitor_start", { pid, name }),
  monitorStop: () => call<void>("ff_monitor_stop"),
  monitorSnapshot: () => call<MonitorSnapshot>("ff_monitor_snapshot"),
  diagnose: () => call<Diagnosis>("ff_diagnose"),
  optList: () => call<ActionView[]>("ff_opt_list"),
  optApply: (requests: ApplyRequest[], level: number, label: string) => call<OptSession>("ff_opt_apply", { requests, level, label }),
  optRevertChange: (sessionId: string, changeId: number) => call<OptSession>("ff_opt_revert_change", { sessionId, changeId }),
  optRevertSession: (sessionId: string) => call<OptSession>("ff_opt_revert_session", { sessionId }),
  optHistory: () => call<OptSession[]>("ff_opt_history"),
  openSettings: (uri: string) => call<void>("ff_open_settings", { uri }),
  gpuControls: () => call<GpuControlsView[]>("ff_gpu_controls"),
  presets: () => call<GpuPreset[]>("ff_presets"),
  presetCapture: (gpu: string, name: string, gameExe: string | null, notes: string) => call<GpuPreset>("ff_preset_capture", { gpu, name, gameExe, notes }),
  presetCheck: (id: string) => call<PresetCheck>("ff_preset_check", { id }),
  presetApply: (id: string, level: number) => call<PresetOutcome>("ff_preset_apply", { id, level }),
  presetImport: (path: string) => call<GpuPreset>("ff_preset_import", { path }),
  presetExport: (id: string, path: string) => call<void>("ff_preset_export", { id, path }),
  presetDelete: (id: string) => call<void>("ff_preset_delete", { id }),
  profiles: () => call<ProfileView[]>("ff_profiles"),
  profileGames: () => call<GameChoice[]>("ff_profile_games"),
  profileSave: (profile: GameProfile, level: number) => call<GameProfile>("ff_profile_save", { profile, level }),
  profileDelete: (id: string) => call<void>("ff_profile_delete", { id }),
  profileDuplicate: (id: string) => call<GameProfile>("ff_profile_duplicate", { id }),
  profileApply: (id: string, level: number) => call<ProfileRun>("ff_profile_apply", { id, level }),
  profileRevert: (id: string) => call<string[]>("ff_profile_revert", { id }),
  profileExport: (id: string, path: string) => call<void>("ff_profile_export", { id, path }),
  profileImport: (path: string) => call<GameProfile>("ff_profile_import", { path }),
  profileEvents: () => call<LifecycleEvent[]>("ff_profile_events"),
  benchStart: (label: string) => call<void>("ff_bench_start", { label }),
  benchStop: () => call<BenchSession>("ff_bench_stop"),
  benchList: () => call<BenchSession[]>("ff_bench_list"),
  benchRename: (id: string, label: string) => call<BenchSession>("ff_bench_rename", { id, label }),
  benchDelete: (id: string) => call<void>("ff_bench_delete", { id }),
  benchCompare: (ids: string[]) => call<BenchComparison>("ff_bench_compare", { ids }),
  benchExport: (ids: string[], path: string) => call<void>("ff_bench_export", { ids, path }),
};

/** NVIDIA driver profiles. Every write goes through snapshot, apply, save, read-back in the backend. */
export const nv = {
  status: () => call<NvStatus>("ff_nv_status"),
  profiles: (refresh = false) => call<NvProfileSummary[]>("ff_nv_profiles", { refresh }),
  profile: (name: string) => call<NvProfileDetail>("ff_nv_profile", { name }),
  plan: (profile: string, changes: NvChange[]) => call<NvDiffLine[]>("ff_nv_plan", { profile, changes }),
  apply: (profile: string, changes: NvChange[]) => call<NvApplyOutcome>("ff_nv_apply", { profile, changes }),
  createProfile: (name: string, exe: string | null) => call<NvApplyOutcome>("ff_nv_create_profile", { name, exe }),
  deleteProfile: (name: string) => call<NvApplyOutcome>("ff_nv_delete_profile", { name }),
  exportTo: (path: string, names: string[]) => call<number>("ff_nv_export", { path, names }),
  importPreview: (path: string) => call<NvImportPreview>("ff_nv_import_preview", { path }),
  preview: (file: NipFile) => call<NvImportPreview>("ff_nv_preview", { file }),
  importFile: (file: NipFile, only: string[], mode: "replace" | "merge", label: string) => call<NvImportOutcome>("ff_nv_import", { file, only, mode, label }),
  saveFile: (path: string, file: NipFile) => call<void>("ff_nv_save_file", { path, file }),
  compare: (a: string, b: string) => call<NipDiff>("ff_nv_compare", { a, b }),
  backups: () => call<NvBackup[]>("ff_nv_backups"),
  backupNow: (names: string[], label: string) => call<NvBackup>("ff_nv_backup_now", { names, label }),
  backupContents: (id: string) => call<NvImportPreview>("ff_nv_backup_contents", { id }),
  restoreBackup: (id: string) => call<NvImportOutcome>("ff_nv_restore_backup", { id }),
  deleteBackup: (id: string) => call<void>("ff_nv_delete_backup", { id }),
  saveDatabase: (path: string) => call<void>("ff_nv_save_database", { path }),
  npiImport: (path: string) => call<NpiRun>("ff_nv_npi_import", { path }),
  npiExport: () => call<NpiRun>("ff_nv_npi_export"),
  setNpi: (path: string | null) => call<FfConfig>("ff_nv_set_npi", { path }),
  loadReference: () => call<number>("ff_nv_load_reference"),
};

/** Ask where to save a file. Returns null when the dialog is cancelled. */
export async function pickSavePath(defaultName: string, filters: { name: string; extensions: string[] }[]): Promise<string | null> {
  if (!inTauri) return `C:\\Users\\you\\Documents\\${defaultName}`;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return (await save({ defaultPath: defaultName, filters })) ?? null;
}

export async function pickFolder(): Promise<string | null> {
  if (!inTauri) return "C:\\Users\\you\\AppData\\Roaming\\PrismLauncher\\instances\\Fabulously Optimized";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ multiple: false, directory: true });
  return typeof r === "string" ? r : null;
}

export async function openUrl(url: string) {
  await call<void>("open_url", { url });
}

export type { Game };
