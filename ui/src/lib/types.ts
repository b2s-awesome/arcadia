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
  | { type: "left" };

export interface Preset {
  id: string;
  label: string;
  description: string;
  flags: Record<string, FlagValue>;
}

export type Action = { type: "replace"; withId: number } | { type: "file"; path: string } | { type: "remove" };

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
