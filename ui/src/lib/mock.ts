// A fake backend so the UI can be developed and previewed in a browser.
import type { Entry, Game, RuleConfig, Settings } from "./types";

const listeners = new Map<string, Set<(p: unknown) => void>>();
export function mockListen(event: string, cb: (p: unknown) => void): () => void {
  if (!listeners.has(event)) listeners.set(event, new Set());
  listeners.get(event)!.add(cb);
  return () => listeners.get(event)!.delete(cb);
}
function emit(event: string, p: unknown) {
  listeners.get(event)?.forEach((cb) => cb(p));
}

const steam = (id: number, name: string): Game => ({
  id: `steam:${id}`,
  name,
  source: "steam",
  install_dir: `D:\\SteamLibrary\\steamapps\\common\\${name}`,
  launch: { kind: "uri", uri: `steam://rungameid/${id}` },
  cover: `https://cdn.cloudflare.steamstatic.com/steam/apps/${id}/library_600x900.jpg`,
  hero: `https://cdn.cloudflare.steamstatic.com/steam/apps/${id}/library_hero.jpg`,
});
const other = (source: Game["source"], id: string, name: string): Game => ({
  id: `${source}:${id}`,
  name,
  source,
  install_dir: null,
  launch: { kind: "uri", uri: "" },
  cover: null,
  hero: null,
});

const games: Game[] = [
  { id: "minecraft:launcher", name: "Minecraft", source: "minecraft", install_dir: null, launch: { kind: "minecraft" }, cover: null, hero: null },
  { id: "roblox:player", name: "Roblox", source: "roblox", install_dir: null, launch: { kind: "roblox" }, cover: null, hero: null },
  steam(1086940, "Baldur's Gate 3"),
  other("battlenet", "prometheus", "Overwatch"),
  steam(730, "Counter-Strike 2"),
  other("epic", "Fortnite", "Fortnite"),
  steam(1145360, "Hades"),
  other("battlenet", "fenris", "Diablo IV"),
  steam(413150, "Stardew Valley"),
  other("gog", "1207664643", "The Witcher 3: Wild Hunt"),
  other("epic", "Rocket", "Rocket League"),
  other("ubisoft", "5271", "Rainbow Six Siege"),
  steam(1245620, "Elden Ring"),
  other("ea", "apex", "Apex Legends"),
  steam(367520, "Hollow Knight"),
];

let settings: Settings = {
  accent: "#ffb38a",
  run_on_boot: false,
  minimize_to_tray: true,
  roblox_channel: "production",
  roblox_pinned_version: null,
  roblox_auto_update: true,
  roblox_multi_instance: true,
  roblox_fps_cap: 240,
  roblox_priority: "AboveNormal",
  roblox_custom_font: null,
  roblox_apply_mods: true,
  fast_flags: { FFlagDebugGraphicsPreferD3D11: "True", FIntDebugForceMSAASamples: "0" },
  proxy_enabled: true,
  proxy_port: 58443,
  proxy_clear_cache_on_launch: true,
  proxy_clear_cache_on_exit: true,
  proxy_fflag_overrides: {},
  proxy_fflags_risk_accepted: false,
  proxy_inspector_enabled: true,
  hide_on_game_launch: false,
  minecraft_open_mode: "page",
  minecraft_client_id: "",
  curseforge_api_key: "",
  minecraft_default_memory_mb: 4096,
  ui: (() => {
    const q = new URLSearchParams(location.search);
    const preset = q.get("look");
    const addons = q.get("addons");
    const base = preset ? { brand: { name: q.get("name") ?? "Arcadia", tagline: q.get("tag") ?? "Games and Roblox" }, look: { preset, ...(q.get("wall") ? { wallpaper: `builtin:${q.get("wall")}` } : {}) }, saved: [
      { name: "Gruvbox night", look: { preset: "gruvbox" } },
      { name: "Glass", look: { preset: "frost" } },
      { name: "Zen", look: { preset: "obsidian" } },
    ], applied: "Gruvbox night" } : null;
    return addons ? { ...(base ?? {}), addons: { installed: addons.split(",") } } : base;
  })(),
};

let rules: RuleConfig = {
  profiles: [
    {
      name: "Phantom Forces skins",
      enabled: true,
      rules: [
        { id: "r1", name: "Night sky", enabled: true, assetIds: [6444884337, 6444884785], action: { type: "file", path: "C:\\Users\\you\\AppData\\Local\\Arcadia\\proxy\\files\\night-sky.png" } },
        { id: "r2", name: "Cleaner scope", enabled: true, assetIds: [5765497841], action: { type: "replace", withId: 13541203 } },
        { id: "r3", name: "Hide muzzle flash", enabled: false, assetIds: [1047066405, 1047066543], action: { type: "remove" } },
      ],
    },
    { name: "Default", enabled: true, rules: [] },
  ],
};

let entryId = 1;
const entries: Entry[] = [];
function fakeEntry(e: Partial<Entry>): Entry {
  return {
    id: entryId++, timeMs: Date.now(), method: "POST", url: "https://assetdelivery.roblox.com/v1/assets/batch",
    status: 200, reqBytes: 412, respBytes: 1630, durationMs: 84, kind: "passthrough", note: "", ...e,
  };
}
entries.push(
  fakeEntry({ method: "CONNECT", url: "apis.roblox.com:443", kind: "tunnel", note: "not decrypted", durationMs: 30211 }),
  fakeEntry({ method: "GET", url: "https://clientsettingscdn.roblox.com/v2/settings/application/PCDesktopClient", respBytes: 48211 }),
  fakeEntry({ kind: "modified", note: "replaced 1, file 2", reqBody: '[{"assetId":13541203,"requestId":"0"}]', respBody: '[{"requestId":"0","location":"https://c3.rbxcdn.com/..."}]' }),
  fakeEntry({ method: "GET", url: "https://assetdelivery.roblox.com/__arcadia/file/6444884337", kind: "served-file", note: "asset 6444884337 ← night-sky.png", respBytes: 284112 }),
  fakeEntry({ method: "GET", url: "https://assetdelivery.roblox.com/v1/asset/?id=1047066405", kind: "blocked", status: 404, note: "removed 1047066405", respBytes: 0 }),
);

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const addonStore = new Map<string, unknown>();
const mockSecrets = new Set<string>();

/** Stand-in artwork for a pretend Discord lookup. */
function mockImage(url: string): string {
  const svg = url === "mock:banner"
    ? `<svg xmlns="http://www.w3.org/2000/svg" width="680" height="240"><defs><linearGradient id="g" x1="0" x2="1" y1="0" y2="1"><stop offset="0" stop-color="#1f1147"/><stop offset=".55" stop-color="#6a2c91"/><stop offset="1" stop-color="#f08a5d"/></linearGradient></defs><rect width="680" height="240" fill="url(#g)"/><circle cx="560" cy="60" r="70" fill="#ffd28a" opacity=".55"/><path d="M0 200 Q170 140 340 190 T680 170 V240 H0z" fill="#12082b" opacity=".7"/></svg>`
    : `<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256"><rect width="256" height="256" fill="#9ad1d4"/><circle cx="128" cy="104" r="52" fill="#f6e7d8"/><path d="M40 256 Q128 150 216 256z" fill="#2f6f73"/><circle cx="108" cy="100" r="7" fill="#1b1b1b"/><circle cx="148" cy="100" r="7" fill="#1b1b1b"/><path d="M80 70 Q128 20 176 70 Q150 52 128 56 Q104 52 80 70z" fill="#e85d75"/></svg>`;
  return `data:image/svg+xml;base64,${btoa(svg)}`;
}

export async function mockInvoke(cmd: string, args: Record<string, unknown> = {}): Promise<unknown> {
  await sleep(60);
  switch (cmd) {
    case "addon_get": return structuredClone(addonStore.get(`${args.addon}/${args.key}`) ?? null);
    case "addon_set": {
      const k = `${args.addon}/${args.key}`;
      if (args.value == null) addonStore.delete(k); else addonStore.set(k, structuredClone(args.value));
      return;
    }
    case "addon_keys": return [...addonStore.keys()].filter((k) => k.startsWith(`${args.addon}/`)).map((k) => k.split("/")[1]).sort();
    case "addon_clear": for (const k of [...addonStore.keys()]) if (k.startsWith(`${args.addon}/`)) addonStore.delete(k); mockSecrets.clear(); return;
    case "addon_secret_set": if (args.value) mockSecrets.add(args.name as string); else mockSecrets.delete(args.name as string); return;
    case "addon_secrets": return [...mockSecrets];
    case "discord_lookup": {
      await sleep(500);
      const q = String(args.query).trim().replace(/^@/, "").toLowerCase();
      if (!/^\d{17,20}$/.test(q) && !mockSecrets.has("bot_token")) {
        throw "Finding someone by username needs a bot token (Discord doesn't offer a public username search). Add one under “Import settings”, or paste a user ID.";
      }
      return {
        id: "175928847299117063", username: /^\d+$/.test(q) ? "sample.person" : q, displayName: "Sample Person",
        avatarUrl: "mock:avatar", avatarAnimated: false, bannerUrl: "mock:banner", bannerAnimated: false,
        accentColor: "#2b2d6e", decorationUrl: null, createdAt: 1462015105796, bot: false, source: "discord", notes: [],
      };
    }
    case "discord_image": return mockImage(String(args.url));
    case "window_effects_status": {
      const ui = (settings.ui ?? {}) as { addons?: { installed?: string[] }; seeThrough?: { enabled?: boolean } };
      const wanted = !!ui.addons?.installed?.includes("see-through") && ui.seeThrough?.enabled !== false;
      return { active: new URLSearchParams(location.search).has("fxactive") && wanted, wanted, build: 22631, os: "windows" };
    }
    case "window_effect_set": return;
    case "app_restart": location.reload(); return;
    case "get_settings": return structuredClone(settings);
    case "save_settings": settings = args.settings as Settings; return;
    case "scan_library": return { games, launchers: ["minecraft", "roblox", "steam", "epic", "battlenet", "gog", "ea", "ubisoft"], warnings: [] };
    case "launch_game": return;
    case "roblox_status": return { installedVersion: "version-6d7e1c9a4b2f4e31", channel: "production", protocolRegistered: true, runningSessions: 0 };
    case "roblox_latest": return { version: "0.698.0.6980712", clientVersionUpload: "version-6d7e1c9a4b2f4e31" };
    case "roblox_launch": {
      const stages = [
        { stage: "checking" },
        { stage: "downloading", package: "RobloxApp.zip", done: 18_000_000, total: 36_000_000 },
        { stage: "extracting", package: "content-textures2.zip", index: 9, count: 21 },
        { stage: "ready", version: "version-6d7e1c9a4b2f4e31" },
      ];
      for (const s of stages) { emit("roblox-progress", s); await sleep(500); }
      emit("roblox-launched", "version-6d7e1c9a4b2f4e31");
      await sleep(400);
      emit("roblox-activity", { type: "joining", place_id: 292439477, job_id: "4f1c9a3e-2b7d-4e8f-9a10-1234567890ab" });
      emit("roblox-activity", { type: "serverIp", ip: "128.116.44.12" });
      return "version-6d7e1c9a4b2f4e31";
    }
    case "clear_roblox_cache": return { removed: 3, locked: 0 };
    case "fflag_presets": return [
      { id: "dx11", label: "Force Direct3D 11", description: "Use the D3D11 renderer.", flags: { FFlagDebugGraphicsPreferD3D11: "True" } },
      { id: "vulkan", label: "Force Vulkan", description: "Use the Vulkan renderer.", flags: { FFlagDebugGraphicsPreferVulkan: "True" } },
      { id: "opengl", label: "Force OpenGL", description: "Use the OpenGL renderer.", flags: { FFlagDebugGraphicsPreferOpenGL: "True" } },
      { id: "no-grass", label: "Disable terrain grass", description: "Hides grass decoration for clarity and FPS.", flags: { FIntFRMMinGrassDistance: "0", FIntFRMMaxGrassDistance: "0" } },
      { id: "msaa-off", label: "Disable MSAA", description: "Turns off multisample anti-aliasing.", flags: { FIntDebugForceMSAASamples: "0" } },
    ];
    case "parse_fflags": return JSON.parse(args.json as string);
    case "proxy_status": return { running: true, port: 58443, stats: { requests: 1284, replaced: 37, removed: 12, servedFiles: 4, fflagInjections: 0 }, caPath: "" };
    case "proxy_start": return 58443;
    case "rules_get": return structuredClone(rules);
    case "rules_save": rules = args.config as RuleConfig; return;
    case "rules_export": return JSON.stringify(rules, null, 2);
    case "rules_import":
    case "rules_import_file":
    case "rules_import_fleasion":
      return { config: structuredClone(rules), report: { profiles: ["Imported"], rules: 3, skippedTargets: 1, skippedRules: 0, missingFiles: [] } };
    case "proxy_import_file": return "C:\\Users\\you\\AppData\\Local\\Arcadia\\proxy\\files\\example.png";
    case "set_wallpaper": return "builtin:aurora";
    case "parse_asset_ids": return ((args.text as string).match(/\d{3,}/g) ?? []).map(Number);
    case "inspector_snapshot": return entries;
    // ── Minecraft ──
    case "mc_instances": {
      const now = Math.floor(Date.now() / 1000);
      return [
        { id: "a1", name: "Fabulously Optimized", mcVersion: "1.21.4", loader: "fabric", loaderVersion: "0.16.10", versionId: null, memoryMb: 4096, javaArgs: "", created: now - 864000, lastPlayed: now - 7200, playtimeSecs: 154800, source: { platform: "modrinth", name: "Fabulously Optimized" }, manualDownloads: [] },
        { id: "a2", name: "All the Mods 10", mcVersion: "1.21.1", loader: "neoforge", loaderVersion: "21.1.93", versionId: null, memoryMb: 8192, javaArgs: "", created: now - 400000, lastPlayed: now - 172800, playtimeSecs: 41200, source: { platform: "curseforge" }, manualDownloads: [{ name: "Example Mod", fileName: "examplemod-1.21.1.jar", pageUrl: "https://www.curseforge.com/minecraft/mc-mods/", folder: "mods" }] },
        { id: "a3", name: "RLCraft", mcVersion: "1.12.2", loader: "forge", loaderVersion: "14.23.5.2860", versionId: null, memoryMb: 6144, javaArgs: "", created: now - 2000000, lastPlayed: now - 1500000, playtimeSecs: 9000, source: null, manualDownloads: [] },
        { id: "a4", name: "Vanilla 1.21.4", mcVersion: "1.21.4", loader: "vanilla", loaderVersion: null, versionId: null, memoryMb: 3072, javaArgs: "", created: now - 50000, lastPlayed: null, playtimeSecs: 0, source: null, manualDownloads: [] },
      ];
    }
    case "mc_running": return [];
    case "mc_accounts": return [{ uuid: "069a79f444e94726a5befca90e38aaf5", name: "Notch", xuid: "", active: true }];
    case "mc_mods": return [
      { fileName: "sodium-fabric-0.6.5+mc1.21.4.jar", enabled: true, size: 1203456, meta: { id: "sodium", name: "Sodium", version: "0.6.5", loader: "fabric", description: "" } },
      { fileName: "lithium-fabric-0.14.3.jar", enabled: true, size: 702311, meta: { id: "lithium", name: "Lithium", version: "0.14.3", loader: "fabric", description: "" } },
      { fileName: "iris-fabric-1.8.5.jar", enabled: true, size: 2911000, meta: { id: "iris", name: "Iris Shaders", version: "1.8.5", loader: "fabric", description: "" } },
      { fileName: "journeymap-1.21.4-6.0.0.jar.disabled", enabled: false, size: 6650000, meta: { id: "journeymap", name: "JourneyMap", version: "6.0.0", loader: "fabric", description: "" } },
      { fileName: "fabric-api-0.115.0+1.21.4.jar", enabled: true, size: 2400000, meta: { id: "fabric-api", name: "Fabric API", version: "0.115.0", loader: "fabric", description: "" } },
      { fileName: "mystery-mod.jar", enabled: true, size: 42000, meta: null },
    ];
    case "mc_verify": return [
      { fileName: "sodium-fabric-0.6.5+mc1.21.4.jar", source: "modrinth", project: "Sodium" },
      { fileName: "lithium-fabric-0.14.3.jar", source: "modrinth", project: "Lithium" },
      { fileName: "iris-fabric-1.8.5.jar", source: "modrinth", project: "Iris" },
      { fileName: "journeymap-1.21.4-6.0.0.jar.disabled", source: "curseforge", project: "JourneyMap" },
      { fileName: "fabric-api-0.115.0+1.21.4.jar", source: "modrinth", project: "Fabric API" },
      { fileName: "mystery-mod.jar", source: "unknown", project: null },
    ];
    case "mc_search": return [
      { id: "1", slug: "fo", title: "Fabulously Optimized", description: "Improved performance and graphics, without changing how the game plays.", author: "robotkoer", iconUrl: null, downloads: 8_400_000, source: "modrinth" },
      { id: "2", slug: "cobblemon", title: "Cobblemon Official Modpack", description: "The official modpack for Cobblemon, a Pokémon mod for Minecraft.", author: "Cobblemon", iconUrl: null, downloads: 2_100_000, source: "modrinth" },
      { id: "3", slug: "simply", title: "Simply Optimized", description: "A lightweight, vanilla-friendly performance pack.", author: "Sitroon", iconUrl: null, downloads: 1_300_000, source: "modrinth" },
      { id: "4", slug: "additive", title: "Additive", description: "Vanilla, but more: a curated set of content mods that fit in.", author: "Treeways", iconUrl: null, downloads: 980_000, source: "modrinth" },
    ];
    case "mc_versions": return [{ id: "1.21.4", type: "release", url: "" }, { id: "1.21.1", type: "release", url: "" }, { id: "1.20.1", type: "release", url: "" }];
    case "mc_loader_versions": return [{ version: "0.16.10", stable: true }, { version: "0.16.9", stable: true }];
    case "mc_discover": return [
      { launcher: "Prism Launcher", name: "Create: Above and Beyond", path: "C:\\Users\\you\\AppData\\Roaming\\PrismLauncher\\instances\\Create Above and Beyond" },
      { launcher: "CurseForge app", name: "Better MC [FORGE] - BMC4", path: "C:\\Users\\you\\curseforge\\minecraft\\Instances\\Better MC [FORGE] - BMC4" },
    ];
    case "mc_incidents": return [
      { id: "fractureiser", name: "fractureiser", date: "June 2023", kind: "Malware in uploaded mods", summary: "Malicious code was added to several mods and plugins uploaded to CurseForge and BukkitDev, some of which were in popular modpacks.", whatToDo: "Run the leftover-file check. If anything is found, run a full antivirus scan and change your passwords from another device.", links: ["https://github.com/fractureiser-investigation/fractureiser"] },
      { id: "bleedingpipe", name: "BleedingPipe", date: "July 2023", kind: "Remote code execution in mods", summary: "Many mods, mostly for Forge 1.7.10 and 1.12.2, let a malicious server or player run code on your computer.", whatToDo: "Update the listed mods. On Forge 1.7.2 to 1.12.2, also install PipeBlocker.", links: ["https://blog.mmpa.info/posts/bleeding-pipe/"] },
      { id: "log4shell", name: "Log4Shell", date: "December 2021", kind: "Remote code execution in the game itself", summary: "A flaw in Log4j let a chat message run code on clients and servers for Minecraft 1.7 to 1.18.", whatToDo: "Nothing: Arcadia always applies Mojang's fix.", links: ["https://www.minecraft.net/en-us/article/important-message--security-vulnerability-java-edition"] },
    ];
    case "mc_security_scan": return {
      findings: [{ incident: "bleedingpipe", instanceId: "a3", instanceName: "RLCraft", modName: "Astral Sorcery", fileName: "astralsorcery-1.12.2-1.10.27.jar", note: "Versions 1.9.1 and earlier." }],
      leftovers: [],
      pipeblockerAdvised: ["RLCraft"],
      checkedInstances: 4,
    };
    default: return;
  }
}
