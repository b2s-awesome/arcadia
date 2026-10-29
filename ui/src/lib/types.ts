export type Source = "roblox" | "minecraft" | "steam" | "epic" | "battlenet" | "gog" | "ea" | "ubisoft" | "custom";

export const SOURCE_LABEL: Record<Source, string> = {
  roblox: "Roblox",
  minecraft: "Minecraft",
  steam: "Steam",
  epic: "Epic Games",
  battlenet: "Battle.net",
  gog: "GOG",
  ea: "EA app",
  ubisoft: "Ubisoft Connect",
  custom: "Added by you",
};

export type LaunchTarget =
  | { kind: "uri"; uri: string }
  | { kind: "exe"; path: string; args: string[]; cwd: string | null }
  | { kind: "roblox" }
  | { kind: "minecraft" };

export interface Game {
  id: string;
  name: string;
  source: Source;
  install_dir: string | null;
  launch: LaunchTarget;
  cover: string | null;
  hero: string | null;
}

export interface GameMeta {
  favourite: boolean;
  hidden: boolean;
  collections: string[];
  notes: string;
  launchArgs: string;
  before: string;
  after: string;
  cover: string | null;
  hero: string | null;
  logo: string | null;
  playtime: number;
  sessions: number;
  lastPlayed: number | null;
  beat: { main: number | null; extra: number | null; complete: number | null };
  rename: string | null;
}
export interface GameCollection { name: string; icon: string }
export interface LibraryMeta {
  meta: { games: Record<string, Partial<GameMeta>>; collections: GameCollection[] };
  /** id → [minutes, last played] from Steam. */
  steam: Record<string, [number, number]>;
  /** id → [seconds, last played] from Arcadia's Roblox and Minecraft trackers. */
  arcadia: Record<string, [number, number]>;
}
export interface SgdbFound { id: number; name: string; year: number | null }
export interface SgdbArt { url: string; thumb: string; width: number; height: number; style: string }

export interface ScanReport {
  games: Game[];
  launchers: Source[];
  warnings: string[];
}

export type FlagValue = string | number | boolean;

export interface Settings {
  accent: string;
  run_on_boot: boolean;
  minimize_to_tray: boolean;
  roblox_channel: string;
  roblox_pinned_version: string | null;
  roblox_auto_update: boolean;
  roblox_multi_instance: boolean;
  roblox_fps_cap: number;
  roblox_priority: "Normal" | "AboveNormal" | "High" | "BelowNormal";
  roblox_custom_font: string | null;
  roblox_apply_mods: boolean;
  fast_flags: Record<string, FlagValue>;
  roblox_flag_profiles: Record<string, Record<string, FlagValue>>;
  roblox_flag_profile: string;
  roblox_auto_rejoin: boolean;
  roblox_rejoin_delay: number;
  roblox_rejoin_same_server: boolean;
  roblox_mod_studio: ModStudio;
  roblox_mod_presets: Record<string, ModStudio>;
  proxy_enabled: boolean;
  proxy_port: number;
  proxy_clear_cache_on_launch: boolean;
  proxy_clear_cache_on_exit: boolean;
  proxy_fflag_overrides: Record<string, FlagValue>;
  proxy_fflags_risk_accepted: boolean;
  proxy_inspector_enabled: boolean;
  ui: unknown;
  hide_on_game_launch: boolean;
  minecraft_open_mode: "page" | "window";
  minecraft_client_id: string;
  curseforge_api_key: string;
  minecraft_default_memory_mb: number;
  steamgriddb_key: string;
  track_playtime: boolean;
}

export interface RobloxStatus {
  installedVersion: string | null;
  channel: string | null;
  protocolRegistered: boolean;
  runningSessions: number;
}

export type Progress =
  | { stage: "checking" }
  | { stage: "downloading"; package: string; done: number; total: number }
  | { stage: "extracting"; package: string; index: number; count: number }
  | { stage: "finalizing" }
  | { stage: "ready"; version: string };

export type Activity =
  | { type: "joining"; place_id: number; job_id: string }
  | { type: "serverIp"; ip: string }
  | { type: "disconnected"; reason: number | null }
  | { type: "left" };

export interface ModStudio {
  /** "" = Roblox's own, a built-in style, or "custom". */
  cursor: string;
  cursor_colour: string;
  cursor_scale: number;
  cursor_arrow: string | null;
  cursor_far: string | null;
  shiftlock: string;
  shiftlock_file: string | null;
  death_sound: string | null;
}

export interface RobloxAccount {
  id: number;
  name: string;
  displayName: string;
  avatar: string | null;
  note: string;
  added: number;
  lastUsed: number | null;
}
export interface RobloxAccounts {
  list: RobloxAccount[];
  default: number | null;
}

export interface RobloxVisit {
  placeId: number;
  jobId: string;
  session: number;
  account: number | null;
  started: number;
  ended: number | null;
  region: string | null;
}
export interface RobloxPlayed {
  placeId: number;
  name: string;
  creator: string;
  icon: string | null;
  seconds: number;
  sessions: number;
  last: number;
  favourite: boolean;
}
export interface RobloxHistory {
  visits: RobloxVisit[];
  games: RobloxPlayed[];
}
export interface RobloxServer {
  id: string;
  playing: number;
  maxPlayers: number;
  ping: number | null;
  fps: number | null;
}
export interface RobloxGame {
  placeId: number;
  universeId: number;
  name: string;
  creator: string;
  icon: string | null;
  playing: number;
}
export interface RejoinEvent {
  state: "waiting" | "cancelled" | "joining" | "done" | "failed" | "limit";
  placeId: number;
  seconds: number;
  message: string | null;
}

export interface Preset {
  id: string;
  label: string;
  description: string;
  flags: Record<string, FlagValue>;
}

export type Action =
  | { type: "replace"; withId: number }
  | { type: "file"; path: string }
  | { type: "link"; url: string }
  | { type: "remove" };

export interface Rule {
  id: string;
  name: string;
  enabled: boolean;
  assetIds: number[];
  action: Action;
}

export interface Profile {
  name: string;
  enabled: boolean;
  rules: Rule[];
}

export interface ImportReport {
  profiles: string[];
  rules: number;
  skippedTargets: number;
  skippedRules: number;
  missingFiles: string[];
}

export interface RulesImported {
  config: RuleConfig;
  report: ImportReport;
}

export interface RuleConfig {
  profiles: Profile[];
}

export interface Stats {
  requests: number;
  replaced: number;
  removed: number;
  servedFiles: number;
  fflagInjections: number;
}

export interface ProxyStatus {
  running: boolean;
  port: number | null;
  stats: Stats | null;
  caPath: string;
}

export interface Entry {
  id: number;
  timeMs: number;
  method: string;
  url: string;
  status: number;
  reqBytes: number;
  respBytes: number;
  durationMs: number;
  kind: "tunnel" | "passthrough" | "modified" | "served-file" | "blocked" | "error";
  note: string;
  reqBody?: string;
  respBody?: string;
}

// ───────────────────────── Minecraft ─────────────────────────

export type McLoader = "vanilla" | "fabric" | "neoforge" | "forge";
export const LOADER_LABEL: Record<McLoader, string> = { vanilla: "Vanilla", fabric: "Fabric", neoforge: "NeoForge", forge: "Forge" };

export interface McVersion { id: string; type: string; url: string; releaseTime?: string }
export interface LoaderVersion { version: string; stable: boolean }

export interface ManualDownload { name: string; fileName: string; pageUrl: string; folder: string }
export interface McInstance {
  id: string;
  name: string;
  mcVersion: string;
  loader: McLoader;
  loaderVersion: string | null;
  versionId: string | null;
  memoryMb: number;
  javaArgs: string;
  created: number;
  lastPlayed: number | null;
  playtimeSecs: number;
  source: { platform: string; projectId?: string | null; versionId?: string | null; name?: string | null } | null;
  manualDownloads: ManualDownload[];
  /** "" = what this version asks for; a Mojang runtime id; or "custom". */
  java: string;
  javaPath: string | null;
  autoJoin: string;
}

export interface McPack { fileName: string; name: string; enabled: boolean; folder: boolean; size: number; description: string | null; icon: string | null }
export interface McBackup { file: string; created: number; size: number }
export interface McWorld {
  folder: string; name: string; lastPlayed: number | null; gameMode: number | null; hardcore: boolean; cheats: boolean;
  version: string | null; seed: number | null; size: number; icon: string | null; backups: McBackup[];
}
export interface McShot { file: string; path: string; taken: number; size: number }
export interface McSavedServer { name: string; address: string; icon: string | null }
export interface McServerStatus { online: number; max: number; version: string; motd: string; icon: string | null; pingMs: number }
export interface McUpdate {
  folder: "mods" | "resourcepacks" | "shaderpacks"; fileName: string; projectId: string; title: string; icon: string | null;
  current: string; latest: string; latestId: string; latestFile: string;
}
export interface McLogFile { path: string; kind: "log" | "crash" | "jvm"; modified: number; size: number }
export interface McDiagnosis { kind: string; title: string; detail: string; fix: string; mods: string[] }
export interface McCrashInfo { instance: string; path: string; diagnoses: McDiagnosis[]; lastError: string | null }
export interface McSkin { id: string; name: string; variant: "classic" | "slim"; added: number; data: string }
export interface McCape { id: string; alias: string; active: boolean; data: string | null }
export interface McProfile { name: string; skin: string | null; variant: "classic" | "slim"; capes: McCape[] }
export interface McExportOptions {
  linkDownloads: boolean; includeConfig: boolean; includeWorlds: boolean; includeOptions: boolean;
  includeResourcepacks: boolean; includeShaderpacks: boolean; version: string;
}

export interface ModFile {
  fileName: string;
  enabled: boolean;
  size: number;
  meta: { id: string; name: string; version: string; loader: string; description: string } | null;
}

export interface Hit { id: string; slug: string; title: string; description: string; author: string; iconUrl: string | null; downloads: number; source: "modrinth" | "curseforge" }
export interface ImportResult { instance: McInstance; warnings: string[] }
export interface FoundInstance { launcher: string; name: string; path: string }
export interface McProgress { task: string; message: string; done: number; total: number }

export interface McAccount { uuid: string; name: string; xuid: string; active: boolean }
export interface DeviceCode { userCode: string; deviceCode: string; verificationUri: string; expiresIn: number; interval: number; message: string }

export interface Incident { id: string; name: string; date: string; kind: string; summary: string; whatToDo: string; links: string[] }
export interface Finding { incident: string; instanceId: string; instanceName: string; modName: string; fileName: string; note: string }
export interface Leftover { path: string; what: string }
export interface SecurityReport { findings: Finding[]; leftovers: Leftover[]; pipeblockerAdvised: string[]; checkedInstances: number }
export interface Verified { fileName: string; source: "modrinth" | "curseforge" | "unknown"; project: string | null }

// ── Music ──────────────────────────────────────────────────────
export interface Track {
  id: number;
  path: string;
  title: string;
  artist: string;
  albumArtist: string;
  album: string;
  genre: string;
  year: number | null;
  trackNo: number | null;
  discNo: number | null;
  duration: number;
  sampleRate: number | null;
  bitDepth: number | null;
  channels: number | null;
  bitrate: number | null;
  format: string;
  art: string | null;
  artColor: string | null;
  addedAt: number;
  playCount: number;
  lastPlayed: number | null;
  favourite: boolean;
  /** "local", "stream" (online, cached) or "download". */
  origin: "local" | "stream" | "download";
  sourceUrl: string | null;
  /** Marked explicit in its tags; null until checked. */
  explicit: boolean | null;
}
export interface MusicPlaylist { id: number; name: string; count: number; duration: number; updated: number; arts: string[] }
export interface OutputInfo { device: string; rate: number; channels: number }
export type RepeatMode = "off" | "all" | "one";
export interface MusicStatus {
  track: Track | null;
  playing: boolean;
  pos: number;
  duration: number;
  volume: number;
  shuffle: boolean;
  repeat: RepeatMode;
  queueIndex: number | null;
  queueLength: number;
  output: OutputInfo | null;
  outputError: string | null;
  device: string | null;
  /** Fetching the current online song. */
  loading: boolean;
  shuffleMode: ShuffleMode;
  /** Loopy Loop: the A–B loop, in seconds. */
  loopAb: [number, number] | null;
}
export type ShuffleMode = "random" | "spreadArtists" | "fewerRepeats" | "lessPlayed";

// ── Stats addon ────────────────────────────────────────────────
export interface StatArt { art: string | null; artColor: string | null }
export interface TopArtist extends StatArt { name: string; plays: number; listened: number }
export interface TopSong extends StatArt { id: number | null; title: string; artist: string; album: string; plays: number; skips: number; avg: number }
export interface TopAlbum extends StatArt { album: string; artist: string; plays: number }
export interface ListenStats {
  listened: number;
  previous: number | null;
  plays: number;
  skips: number;
  songs: number;
  newSongs: number;
  topArtists: TopArtist[];
  topSongs: TopSong[];
  topAlbums: TopAlbum[];
  hours: number[];
  days: number[];
  sources: { key: string; listened: number }[];
  formats: { key: string; listened: number }[];
  streak: number;
  busiestDay: { at: number; listened: number } | null;
  today: number;
}

// ── Lyrics Plus addon ─────────────────────────────────────────
export interface LyricWord { t: number; text: string }
export interface LyricLine { t: number | null; text: string; words?: LyricWord[] }
export interface Lyrics { synced: boolean; words: boolean; source: "file" | "tags" | "lrclib" | "yours"; lines: LyricLine[]; instrumental: boolean }
export interface QueueView { index: number | null; tracks: Track[] }
export interface MusicScanReport { added: number; updated: number; removed: number; total: number; missingFolders: string[]; errors: string[] }
export interface MusicScanProgress { done: number; total: number; current: string }

// ── Streaming & downloads addon ────────────────────────────────
export type OnlineSource = "youtube-music" | "soundcloud";
export interface OnlineItem { url: string; title: string; artist: string; album: string | null; duration: number | null; thumbnail: string | null; site: string }
export interface OnlineFound { playlist: string | null; items: OnlineItem[] }
export type DownloadFormat = "m4a" | "mp3" | "flac";
export interface StreamingConfig { format: DownloadFormat; scOriginal: boolean; downloadDir: string; cacheLimitMb: number; ytdlpPath: string; ffmpegPath: string }
export interface ToolInfo { path: string; version: string | null; managed: boolean }
export interface StreamingStatus { ytdlp: ToolInfo | null; ffmpeg: ToolInfo | null; config: StreamingConfig; downloadDir: string; cacheBytes: number; windows: boolean }
export interface FetchProgress { percent: number; speed: string; eta: string; stage: string }
export interface DownloadJob { id: number; trackId: number; title: string; artist: string; url: string; state: "queued" | "running" | "done" | "failed" | "cancelled"; progress: FetchProgress; error: string | null; path: string | null }
export interface SyncedSource { playlistId: number; url: string; download: boolean; lastSync: number }
export interface InstallProgress { tool: "yt-dlp" | "ffmpeg"; done: number; total: number; stage: string }
