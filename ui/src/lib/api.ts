// Every call into the Rust side goes through here. Outside Tauri (plain
// browser during design work) a mock backend answers instead.
import type {
  DeviceCode, FoundInstance, Hit, ImportResult, Incident, LoaderVersion, McAccount, McInstance, McLoader, McVersion, ModFile, SecurityReport, Verified,
  Entry, Game, LaunchTarget, Preset, ProxyStatus, RobloxStatus, RuleConfig, ScanReport, Settings,
} from "./types";
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
  rulesImport: (json: string) => call<RuleConfig>("rules_import", { json }),
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
