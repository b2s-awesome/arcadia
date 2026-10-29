// Every call into the Rust side goes through here. Outside Tauri (plain
// browser during design work) a mock backend answers instead.
import type {
  DeviceCode, FoundInstance, Hit, McPack, McWorld, McBackup, McShot, McSavedServer, McServerStatus, McUpdate, McLogFile, McCrashInfo, McSkin, McProfile, McExportOptions, ImportResult, Incident, LoaderVersion, McAccount, McInstance, McLoader, McVersion, ModFile, SecurityReport, Verified,
  Entry, Game, GameMeta, GameCollection, LibraryMeta, SgdbFound, SgdbArt, LaunchTarget, Preset, ProxyStatus, RobloxStatus, RobloxAccount, RobloxAccounts, RobloxGame, RobloxHistory, RobloxServer, RuleConfig, ScanReport, Settings, RulesImported,
  MusicPlaylist, MusicScanReport, MusicStatus, QueueView, RepeatMode, Track, ShuffleMode, ListenStats, Lyrics,
  DownloadJob, OnlineFound, OnlineItem, OnlineSource, StreamingConfig, StreamingStatus, SyncedSource } from "./types";
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

export async function pickSave(defaultPath: string, filters?: { name: string; extensions: string[] }[]): Promise<string | null> {
  if (!inTauri) return `C:\\Users\\you\\Downloads\\${defaultPath}`;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return (await save({ defaultPath, filters })) ?? null;
}

export async function pickFiles(filters?: { name: string; extensions: string[] }[]): Promise<string[]> {
  if (!inTauri) return ["C:\\Users\\you\\Downloads\\Faithful 32x.zip"];
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ multiple: true, directory: false, filters });
  return Array.isArray(r) ? r : typeof r === "string" ? [r] : [];
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

export interface BackupManifest { app: string; created: number; computer: string; parts: string[]; keys: boolean; files: number }

export const api = {
  getSettings: () => call<Settings>("get_settings"),
  backupParts: () => call<{ id: string; label: string }[]>("backup_parts"),
  backupExport: (dest: string, parts: string[], keys: boolean) => call<BackupManifest>("backup_export", { dest, parts, keys }),
  backupInspect: (path: string) => call<BackupManifest>("backup_inspect", { path }),
  backupRestore: (path: string, parts: string[]) => call<number>("backup_restore", { path, parts }),
  syncNow: () => call<BackupManifest>("sync_now"),
  syncCheck: () => call<BackupManifest | null>("sync_check"),
  syncRestore: () => call<number>("sync_restore"),
  saveSettings: (settings: Settings) => call<void>("save_settings", { settings }),

  scanLibrary: () => call<ScanReport>("scan_library"),
  launchGame: (id: string) => call<void>("launch_game", { id }),
  libraryMeta: () => call<LibraryMeta>("library_meta"),
  libraryMetaSet: (id: string, meta: GameMeta) => call<void>("library_meta_set", { id, meta }),
  libraryCollectionsSet: (list: GameCollection[]) => call<void>("library_collections_set", { list }),
  libraryArtImport: (path: string) => call<string>("library_art_import", { path }),
  libraryArtSearch: (term: string) => call<SgdbFound[]>("library_art_search", { term }),
  libraryArt: (kind: "cover" | "hero" | "logo", id: number, steam: boolean) => call<SgdbArt[]>("library_art", { kind, id, steam }),
  libraryBigPicture: (on: boolean) => call<void>("library_big_picture", { on }),
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
  robloxPlay: (account: number | null, place: number | null = null, job: string | null = null) => call<string>("roblox_play", { account, place, job }),
  robloxRejoinCancel: () => call<void>("roblox_rejoin_cancel"),
  robloxAccounts: () => call<RobloxAccounts>("roblox_accounts"),
  robloxAccountLogin: () => call<RobloxAccount>("roblox_account_login"),
  robloxAccountCancelLogin: () => call<void>("roblox_account_cancel_login"),
  robloxAccountRemove: (id: number) => call<void>("roblox_account_remove", { id }),
  robloxAccountDefault: (id: number | null) => call<void>("roblox_account_default", { id }),
  robloxAccountNote: (id: number, note: string) => call<void>("roblox_account_note", { id, note }),
  robloxAccountReorder: (ids: number[]) => call<void>("roblox_account_reorder", { ids }),
  robloxAccountCheck: (id: number) => call<boolean>("roblox_account_check", { id }),
  robloxServers: (place: number, cursor: string | null = null) => call<{ servers: RobloxServer[]; next: string | null }>("roblox_servers", { place, cursor }),
  robloxServerRegion: (place: number, job: string) => call<string | null>("roblox_server_region", { place, job }),
  robloxGames: (places: number[]) => call<RobloxGame[]>("roblox_games", { places }),
  robloxPlaceFrom: (text: string) => call<number | null>("roblox_place_from", { text }),
  robloxHistory: () => call<RobloxHistory>("roblox_history"),
  robloxHistoryAdd: (place: number) => call<void>("roblox_history_add", { place }),
  robloxHistoryFavourite: (place: number, on: boolean) => call<void>("roblox_history_favourite", { place, on }),
  robloxHistoryForget: (place: number) => call<void>("roblox_history_forget", { place }),
  robloxHistoryClear: () => call<void>("roblox_history_clear"),
  robloxShortcut: (place: number, name: string, account: number | null) => call<string>("roblox_shortcut", { place, name, account }),
  robloxCursorPreview: (style: string, colour: string, scale: number) => call<string>("roblox_cursor_preview", { style, colour, scale }),
  robloxFlagAllowlist: () => call<string[]>("roblox_flag_allowlist"),
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
  play: (id: string, join: string | null = null) => call<void>("mc_play", { id, join }),
  packs: (id: string, kind: "resourcepacks" | "shaderpacks") => call<McPack[]>("mc_packs", { id, kind }),
  packToggle: (id: string, kind: string, file: string, on: boolean) => call<string>("mc_pack_toggle", { id, kind, file, on }),
  packRemove: (id: string, kind: string, file: string) => call<void>("mc_pack_remove", { id, kind, file }),
  packsAdd: (id: string, kind: string, files: string[]) => call<number>("mc_packs_add", { id, kind, files }),
  worlds: (id: string) => call<McWorld[]>("mc_worlds", { id }),
  worldBackup: (id: string, folder: string) => call<McBackup>("mc_world_backup", { id, folder }),
  worldRestore: (id: string, folder: string, file: string) => call<void>("mc_world_restore", { id, folder, file }),
  worldBackupDelete: (id: string, folder: string, file: string) => call<void>("mc_world_backup_delete", { id, folder, file }),
  worldDelete: (id: string, folder: string, keepBackup: boolean) => call<void>("mc_world_delete", { id, folder, keepBackup }),
  screenshots: (id: string) => call<McShot[]>("mc_screenshots", { id }),
  screenshotDelete: (id: string, file: string) => call<void>("mc_screenshot_delete", { id, file }),
  servers: (id: string) => call<McSavedServer[]>("mc_servers", { id }),
  serverAdd: (id: string, name: string, address: string) => call<void>("mc_server_add", { id, name, address }),
  serverRemove: (id: string, address: string) => call<void>("mc_server_remove", { id, address }),
  serverPing: (address: string) => call<McServerStatus>("mc_server_ping", { address }),
  updates: (id: string) => call<McUpdate[]>("mc_updates", { id }),
  updateApply: (id: string, update: McUpdate) => call<string>("mc_update_apply", { id, update }),
  exportPack: (id: string, dest: string, options: McExportOptions) => call<{ path: string; linked: number; packed: number }>("mc_export", { id, dest, options }),
  javaRuntimes: () => call<{ id: string; label: string }[]>("mc_java_runtimes"),
  setLaunch: (id: string, java: string, javaPath: string | null, autoJoin: string) => call<void>("mc_set_launch", { id, java, javaPath, autoJoin }),
  logs: (id: string) => call<McLogFile[]>("mc_logs", { id }),
  logRead: (id: string, path: string) => call<string>("mc_log_read", { id, path }),
  explain: (id: string, path: string | null = null) => call<McCrashInfo | null>("mc_explain", { id, path }),
  skins: () => call<McSkin[]>("mc_skins"),
  skinAddFile: (path: string) => call<McSkin>("mc_skin_add_file", { path }),
  skinAddPlayer: (name: string) => call<McSkin>("mc_skin_add_player", { name }),
  skinEdit: (id: string, name: string, variant: string) => call<void>("mc_skin_edit", { id, name, variant }),
  skinRemove: (id: string) => call<void>("mc_skin_remove", { id }),
  profile: () => call<McProfile>("mc_profile"),
  skinWear: (id: string) => call<McProfile>("mc_skin_wear", { id }),
  capeSet: (cape: string | null) => call<McProfile>("mc_cape_set", { cape }),
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

/** One looked-up Discord account (see crates/core/src/addons/discord.rs). */
export interface DiscordFound {
  id: string;
  username: string;
  displayName: string | null;
  avatarUrl: string;
  avatarAnimated: boolean;
  bannerUrl: string | null;
  bannerAnimated: boolean;
  accentColor: string | null;
  decorationUrl: string | null;
  createdAt: number | null;
  bot: boolean;
  source: "discord" | "lanyard";
  notes: string[];
}

/** Marketplace addons: sandboxed storage and their backends. */
export const addons = {
  get: <T>(addon: string, key: string) => call<T | null>("addon_get", { addon, key }),
  set: (addon: string, key: string, value: unknown) => call<void>("addon_set", { addon, key, value }),
  keys: (addon: string) => call<string[]>("addon_keys", { addon }),
  clear: (addon: string) => call<void>("addon_clear", { addon }),
  setSecret: (addon: string, name: string, value: string | null) => call<void>("addon_secret_set", { addon, name, value }),
  secrets: (addon: string) => call<string[]>("addon_secrets", { addon }),
  discordLookup: (query: string) => call<DiscordFound>("discord_lookup", { query }),
  discordImage: (url: string) => call<string>("discord_image", { url }),
};

export interface EffectsStatus {
  active: boolean;
  wanted: boolean;
  build: number | null;
  os: string;
}

export const windowFx = {
  status: () => call<EffectsStatus>("window_effects_status"),
  set: (effect: string | null, color: [number, number, number, number] | null) => call<void>("window_effect_set", { effect, color }),
  restart: () => call<void>("app_restart"),
};

/** This webview's window label ("main" or "minecraft"). */
export async function windowLabel(): Promise<string> {
  if (!inTauri) return "main";
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  return getCurrentWindow().label;
}

/** The music player. */
export const music = {
  library: () => call<Track[]>("music_library"),
  folders: () => call<string[]>("music_folders"),
  setFolders: (folders: string[]) => call<void>("music_set_folders", { folders }),
  scan: () => call<MusicScanReport>("music_scan"),
  favourite: (id: number, on: boolean) => call<void>("music_favourite", { id, on }),
  status: () => call<MusicStatus>("music_status"),
  play: (ids: number[], start = 0) => call<void>("music_play", { ids, start }),
  pause: (paused: boolean | null = null) => call<void>("music_pause", { paused }),
  next: () => call<void>("music_next"),
  prev: () => call<void>("music_prev"),
  seek: (secs: number) => call<void>("music_seek", { secs }),
  volume: (volume: number) => call<void>("music_volume", { volume }),
  shuffle: (on: boolean) => call<void>("music_shuffle", { on }),
  repeat: (mode: RepeatMode) => call<void>("music_repeat", { mode }),
  devices: () => call<string[]>("music_devices"),
  setDevice: (name: string | null) => call<void>("music_set_device", { name }),
  queue: () => call<QueueView>("music_queue"),
  queueAdd: (ids: number[], next: boolean) => call<void>("music_queue_add", { ids, next }),
  queueRemove: (index: number) => call<void>("music_queue_remove", { index }),
  queueMove: (from: number, to: number) => call<void>("music_queue_move", { from, to }),
  queueJump: (index: number) => call<void>("music_queue_jump", { index }),
  queueClear: () => call<void>("music_queue_clear"),
  playlists: () => call<MusicPlaylist[]>("music_playlists"),
  playlistItems: (id: number) => call<number[]>("music_playlist_items", { id }),
  playlistCreate: (name: string, ids: number[]) => call<number>("music_playlist_create", { name, ids }),
  playlistRename: (id: number, name: string) => call<void>("music_playlist_rename", { id, name }),
  playlistDelete: (id: number) => call<void>("music_playlist_delete", { id }),
  playlistSet: (id: number, ids: number[]) => call<void>("music_playlist_set", { id, ids }),
  playlistAdd: (id: number, ids: number[]) => call<void>("music_playlist_add", { id, ids }),
  reloadRules: () => call<void>("music_reload_rules"),
  shuffleMode: (mode: ShuffleMode) => call<void>("music_shuffle_mode", { mode }),
  loop: (a: number | null, b: number | null) => call<void>("music_loop", { a, b }),
  spectrum: (bands: number) => call<number[]>("music_spectrum", { bands }),
  spectrumStop: () => call<void>("music_spectrum_stop"),
  queueTool: (tool: "dedupe" | "clearPlayed" | "shuffleRest") => call<void>("music_queue_tool", { tool }),
  stats: (since: number | null, days = 182) => call<ListenStats>("music_stats", { since, tz: new Date().getTimezoneOffset(), days }),
};

/** Lyrics Plus addon. */
export const lyricsApi = {
  get: (id: number, online: boolean, sidecar: boolean) => call<Lyrics | null>("lyrics_get", { id, online, sidecar }),
  save: (id: number, lrc: string, sidecar: boolean) => call<Lyrics>("lyrics_save", { id, lrc, sidecar }),
  forget: (id: number) => call<void>("lyrics_forget", { id }),
  translate: (id: number, from: string, to: string, lines: string[], email: string | null) => call<string[]>("lyrics_translate", { id, from, to, lines, email }),
  window: (kind: "popup" | "bar", open: boolean, top = false) => call<void>("lyrics_window", { kind, open, top }),
};

/** Streaming & downloads addon. */
export const streaming = {
  status: () => call<StreamingStatus>("streaming_status"),
  install: (tool: "yt-dlp" | "ffmpeg") => call<void>("streaming_install", { tool }),
  update: () => call<string>("streaming_update"),
  setConfig: (config: StreamingConfig) => call<void>("streaming_set_config", { config }),
  clearCache: () => call<number>("streaming_clear_cache"),
  search: (source: OnlineSource, query: string) => call<OnlineFound>("streaming_search", { source, query }),
  resolve: (url: string) => call<OnlineFound>("streaming_resolve", { url }),
  tracks: (items: OnlineItem[]) => call<number[]>("streaming_tracks", { items }),
  jobs: () => call<DownloadJob[]>("streaming_jobs"),
  download: (ids: number[]) => call<number>("streaming_download", { ids }),
  cancel: (id: number) => call<void>("streaming_cancel", { id }),
  retry: (id: number) => call<void>("streaming_retry", { id }),
  clearFinished: () => call<void>("streaming_clear_finished"),
  sources: () => call<SyncedSource[]>("streaming_sources"),
  import: (url: string, name: string | null, download: boolean) => call<number>("streaming_import", { url, name, download }),
  sync: (playlistId: number) => call<number>("streaming_sync", { playlistId }),
  unlink: (playlistId: number) => call<void>("streaming_unlink", { playlistId }),
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
