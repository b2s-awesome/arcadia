// A fake backend so the UI can be developed and previewed in a browser.
import type { Entry, Game, RuleConfig, Settings } from "./types";
import { mockMusic } from "./mockMusic";

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


// ── Roblox+ sample data ──
const nowS = () => Math.floor(Date.now() / 1000);
let rbxAccounts = {
  list: [
    { id: 1, name: "skyline_main", displayName: "Skyline", avatar: null as string | null, note: "main", added: nowS() - 9e6, lastUsed: nowS() - 3600 },
    { id: 2, name: "skyline_alt", displayName: "Sky Alt", avatar: null as string | null, note: "trading", added: nowS() - 4e6, lastUsed: nowS() - 864000 },
    { id: 3, name: "afk_farmer_77", displayName: "Farmer", avatar: null as string | null, note: "", added: nowS() - 1e6, lastUsed: null as number | null },
  ],
  default: 1 as number | null,
};
const rbxGame = (placeId: number, name: string, creator: string, hours: number, sessions: number, agoH: number, favourite = false) =>
  ({ placeId, name, creator, icon: null as string | null, seconds: Math.round(hours * 3600), sessions, last: nowS() - agoH * 3600, favourite });
let rbxHistory = {
  games: [
    rbxGame(2753915549, "Blox Fruits", "Gamer Robot Inc", 61.5, 88, 2, true),
    rbxGame(920587237, "Adopt Me!", "Uplift Games", 23.2, 41, 26, true),
    rbxGame(6516141723, "DOORS", "LSPLASH", 9.8, 17, 50),
    rbxGame(4924922222, "Brookhaven RP", "Wolfpaq Games", 7.1, 12, 120),
    rbxGame(606849621, "Jailbreak", "Badimo", 4.4, 9, 300),
    rbxGame(142823291, "Murder Mystery 2", "Nikilis", 2.2, 6, 700),
  ],
  visits: [] as { placeId: number; jobId: string; session: number; account: number | null; started: number; ended: number | null; region: string | null }[],
};
{
  const regions = ["Frankfurt, Germany", "London, United Kingdom", "Ashburn, United States", "Amsterdam, Netherlands"];
  let t = nowS() - 2 * 3600;
  for (let i = 0; i < 14; i++) {
    const g = rbxHistory.games[(i * 7) % 5];
    const len = 600 + ((i * 937) % 4200);
    rbxHistory.visits.unshift({ placeId: g.placeId, jobId: `${(0x4f1c9a3e + i * 7919).toString(16)}-2b7d-4e8f-9a10-${(123456789 + i).toString(16).padStart(12, "0")}`, session: i, account: [1, 1, 2, null, 3][i % 5], started: t - len, ended: t, region: regions[i % 4] });
    t -= len + 3600 * (1 + (i % 3) * 7);
  }
}
function mockCursor(style: string, colour: string, scale: number): string {
  const k = scale / 100;
  const st = `fill="${colour}" stroke="rgba(10,10,12,.75)" stroke-width="${1.6 * k}"`;
  const shapes: Record<string, string> = {
    dot: `<circle cx="32" cy="32" r="${4 * k}" ${st}/>`,
    ring: `<circle cx="32" cy="32" r="${9 * k}" fill="none" stroke="rgba(10,10,12,.75)" stroke-width="${5 * k}"/><circle cx="32" cy="32" r="${9 * k}" fill="none" stroke="${colour}" stroke-width="${3.2 * k}"/>`,
    "ring-dot": `<circle cx="32" cy="32" r="${9 * k}" fill="none" stroke="rgba(10,10,12,.75)" stroke-width="${5 * k}"/><circle cx="32" cy="32" r="${9 * k}" fill="none" stroke="${colour}" stroke-width="${3.2 * k}"/><circle cx="32" cy="32" r="${2.2 * k}" ${st}/>`,
    cross: [[1, 0], [-1, 0], [0, 1], [0, -1]].map(([x, y]) => { const g = 3.5 * k, l = 7 * k; return `<rect x="${32 + (x ? x * (g + l / 2) - l / 2 : -1.2 * k)}" y="${32 + (y ? y * (g + l / 2) - l / 2 : -1.2 * k)}" width="${x ? l : 2.4 * k}" height="${y ? l : 2.4 * k}" ${st}/>`; }).join("") + `<circle cx="32" cy="32" r="${1.4 * k}" ${st}/>`,
    arrow: `<path transform="translate(32 32) scale(${k})" d="M0 0V19L4.6 14.8 8.2 22.4 11.4 21 7.9 13.6H13.6Z" ${st} stroke-linejoin="round"/>`,
  };
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64">${shapes[style] ?? shapes.dot}</svg>`;
  return "data:image/svg+xml;utf8," + encodeURIComponent(svg);
}


// ── Minecraft+ sample data ──
function mockShot(i: number): string {
  const hues = [200, 28, 120, 270, 340, 50, 180, 10, 230];
  const h = hues[i % hues.length];
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="320" height="180"><defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="hsl(${h} 70% 70%)"/><stop offset=".62" stop-color="hsl(${h + 20} 60% 82%)"/><stop offset=".62" stop-color="hsl(${(h + 120) % 360} 35% 40%)"/><stop offset="1" stop-color="hsl(${(h + 120) % 360} 35% 22%)"/></linearGradient></defs><rect width="320" height="180" fill="url(#g)"/><rect x="${40 + i * 23}" y="80" width="40" height="32" fill="hsl(${(h + 60) % 360} 30% 35%)"/><rect x="${180 - i * 9}" y="70" width="70" height="42" fill="hsl(${(h + 200) % 360} 25% 50%)"/></svg>`;
  return "data:image/svg+xml;utf8," + encodeURIComponent(svg);
}
function mockSkin(shirt: string, legs: string, hair = "#3b2a1e"): string {
  // A tiny 64×64 skin, drawn on a canvas so the 3D preview has real texture.
  if (typeof document === "undefined") return "";
  const c = document.createElement("canvas");
  c.width = c.height = 64;
  const g = c.getContext("2d")!;
  const r = (x: number, y: number, w: number, h: number, col: string) => { g.fillStyle = col; g.fillRect(x, y, w, h); };
  const skin = "#e0ac80";
  r(0, 8, 32, 8, skin); r(8, 0, 16, 8, skin); r(0, 8, 32, 2, hair); r(8, 0, 8, 8, hair); r(8, 8, 8, 2, hair);
  r(9, 12, 2, 1, "#fff"); r(13, 12, 2, 1, "#fff"); r(10, 12, 1, 1, "#2c4a8a"); r(13, 12, 1, 1, "#2c4a8a"); r(11, 14, 2, 1, "#9a5a44");
  r(16, 16, 24, 4, shirt); r(16, 20, 24, 12, shirt); r(40, 16, 16, 4, shirt); r(40, 20, 16, 12, shirt); r(44, 28, 4, 4, skin); r(32, 48, 16, 16, shirt); r(36, 60, 4, 4, skin);
  r(0, 16, 16, 16, legs); r(16, 48, 16, 16, legs); r(0, 28, 16, 4, "#3a3a3a"); r(16, 60, 16, 4, "#3a3a3a");
  return c.toDataURL("image/png");
}
let mcSkins: { id: string; name: string; variant: "classic" | "slim"; added: number; data: string }[] = [];
const mcProfile = { name: "Notch", skin: "" as string | null, variant: "classic" as "classic" | "slim", capes: [
  { id: "c1", alias: "Migrator", active: true, data: null as string | null },
  { id: "c2", alias: "Pan", active: false, data: null as string | null },
  { id: "c3", alias: "Common", active: false, data: null as string | null },
] };
if (typeof document !== "undefined") {
  mcSkins = [
    { id: "s1", name: "Red hoodie", variant: "classic", added: 1, data: mockSkin("#c0563b", "#2f3b5c") },
    { id: "s2", name: "Forest", variant: "slim", added: 2, data: mockSkin("#3f8a4f", "#4b3a2a", "#d8b45a") },
    { id: "s3", name: "Space", variant: "classic", added: 3, data: mockSkin("#2d3a8c", "#1c1c28", "#111") },
  ];
  mcProfile.skin = mcSkins[0].data;
}
function mockLog(path: string): string {
  if (path.startsWith("crash")) return `---- Minecraft Crash Report ----\n// Who set us up the TNT?\n\nTime: 2026-10-29 21:14:03\nDescription: Mod loading\n\njava.lang.IllegalStateException: Sodium Extra requires Sodium 0.6.0\n\tat net.fabricmc.loader.impl.FabricLoaderImpl.load(FabricLoaderImpl.java:212)\n\tat net.minecraft.client.main.Main.main(Main.java:221)\n\n-- System Details --\nMinecraft Version: 1.21.4\nJava Version: 21.0.3, Microsoft\nSuspected Mods: Iris (iris)\n`;
  return Array.from({ length: 60 }, (_, i) => `[19:${String(10 + Math.floor(i / 6)).padStart(2, "0")}:${String((i * 7) % 60).padStart(2, "0")}] [Render thread/${i % 17 === 5 ? "WARN" : i === 48 ? "ERROR" : "INFO"}]: ${["Loading 64 mods", "Reloading ResourceManager: vanilla, fabric, Faithful 32x", "Sound engine started", "Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas", "Connecting to mc.hypixel.net, 25565", "Missing sound for event: minecraft:item.goat_horn.play"][i % 6]}`).join("\n");
}


// ── Library+ sample data ──
const libMeta = {
  meta: {
    games: {
      "steam:1086940": { favourite: true, collections: ["Story", "Co-op"], notes: "Act 2: finish the Shadow-Cursed Lands before going to Baldur's Gate.\nSave before Ketheric!", playtime: 6 * 3600, sessions: 4, lastPlayed: Math.floor(Date.now() / 1000) - 86400, beat: { main: 75, extra: 110, complete: 150 }, launchArgs: "--skip-launcher" },
      "steam:730": { favourite: true, collections: ["Competitive"], launchArgs: "-novid -high", playtime: 3 * 3600, sessions: 9, lastPlayed: Math.floor(Date.now() / 1000) - 3600 * 5 },
      "battlenet:prometheus": { collections: ["Competitive"], playtime: 5400, sessions: 3, lastPlayed: Math.floor(Date.now() / 1000) - 86400 * 4 },
      "epic:Fortnite": { hidden: false, collections: ["Co-op"] },
    } as Record<string, Record<string, unknown>>,
    collections: [{ name: "Story", icon: "📖" }, { name: "Co-op", icon: "🤝" }, { name: "Competitive", icon: "🎯" }],
  },
  steam: { "steam:730": [61230, Math.floor(Date.now() / 1000) - 3600 * 5], "steam:1086940": [8940, Math.floor(Date.now() / 1000) - 86400] } as Record<string, [number, number]>,
  arcadia: { "roblox:player": [394000, Math.floor(Date.now() / 1000) - 7200], "minecraft:launcher": [208800, Math.floor(Date.now() / 1000) - 7200] } as Record<string, [number, number]>,
};
function mockArt(kind: string, i: number): string {
  const h = (i * 47) % 360;
  const [w, hgt] = kind === "hero" ? [960, 310] : kind === "logo" ? [400, 160] : [300, 450];
  const body = kind === "logo"
    ? `<text x="50%" y="58%" text-anchor="middle" font-family="Georgia" font-weight="700" font-size="58" fill="hsl(${h} 80% 75%)">TITLE</text>`
    : `<rect width="${w}" height="${hgt}" fill="url(#g)"/><circle cx="${w * 0.7}" cy="${hgt * 0.35}" r="${Math.min(w, hgt) * 0.22}" fill="hsl(${(h + 40) % 360} 80% 70% / .55)"/><text x="24" y="${hgt - 28}" font-family="Georgia" font-weight="700" font-size="${kind === "hero" ? 54 : 40}" fill="#fff" opacity=".9">Style ${i + 1}</text>`;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${hgt}"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="hsl(${h} 55% 45%)"/><stop offset="1" stop-color="hsl(${(h + 70) % 360} 60% 18%)"/></linearGradient></defs>${body}</svg>`;
  return "data:image/svg+xml;utf8," + encodeURIComponent(svg);
}

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
  roblox_flag_profiles: {
    "Competitive": { FFlagDebugGraphicsPreferD3D11: "True", FIntDebugForceMSAASamples: "0", DFIntTextureQualityOverride: "0", DFFlagTextureQualityOverrideEnabled: "True" },
    "Pretty": { FIntDebugForceMSAASamples: "4" },
  },
  roblox_flag_profile: "",
  roblox_auto_rejoin: true,
  roblox_rejoin_delay: 10,
  roblox_rejoin_same_server: true,
  roblox_mod_studio: { cursor: "ring-dot", cursor_colour: "#ffffff", cursor_scale: 100, cursor_arrow: null, cursor_far: null, shiftlock: "cross", shiftlock_file: null, death_sound: null },
  roblox_mod_presets: { "Clean aim": { cursor: "dot", cursor_colour: "#b8bbff", cursor_scale: 90, cursor_arrow: null, cursor_far: null, shiftlock: "dot", shiftlock_file: null, death_sound: null } },
  fast_flags: { FFlagDebugGraphicsPreferD3D11: "True", FIntDebugForceMSAASamples: "0", FFlagDebugDisplayFPS: "True" },
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
  steamgriddb_key: "",
  track_playtime: true,
  ui: (() => {
    const q = new URLSearchParams(location.search);
    const preset = q.get("look");
    const addons = q.get("addons");
    const base = preset ? { brand: { name: q.get("name") ?? "Arcadia", tagline: q.get("tag") ?? "Games and Roblox" }, look: { preset, ...(q.get("wall") ? { wallpaper: `builtin:${q.get("wall")}` } : {}) }, saved: [
      { name: "Gruvbox night", look: { preset: "gruvbox" } },
      { name: "Glass", look: { preset: "frost" } },
      { name: "Zen", look: { preset: "obsidian" } },
    ], applied: "Gruvbox night" } : null;
    // uix: extra UI state as JSON, for previews (e.g. {"stats":{"skin":"glass"}}).
    const extra = q.get("uix") ? (JSON.parse(q.get("uix")!) as Record<string, unknown>) : {};
    const ui = addons ? { ...(base ?? {}), addons: { installed: addons.split(",") } } : base;
    return ui || q.get("uix") ? { ...(ui ?? {}), ...extra } : null;
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
  if (cmd.startsWith("music_") || cmd.startsWith("streaming_") || cmd.startsWith("lyrics_")) return mockMusic(cmd, args, emit);
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
    case "library_meta": return structuredClone(libMeta);
    case "library_meta_set": libMeta.meta.games[String(args.id)] = args.meta as Record<string, unknown>; return;
    case "library_collections_set": libMeta.meta.collections = args.list as { name: string; icon: string }[]; return;
    case "library_art_import": return String(args.path);
    case "library_art_search": await sleep(400); return [
      { id: 101, name: String(args.term), year: 2023 },
      { id: 102, name: `${args.term}: Definitive Edition`, year: 2024 },
      { id: 103, name: `${args.term} Demo`, year: 2022 },
    ];
    case "library_art": await sleep(500); return Array.from({ length: args.kind === "logo" ? 6 : 12 }, (_, i) => { const u = mockArt(String(args.kind), i); return { url: u, thumb: u, width: 600, height: 900, style: ["alternate", "blurred", "white_logo", "material"][i % 4] }; });
    case "library_big_picture": return;
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
    // ── Roblox+ ──
    case "roblox_play": {
      emit("roblox-progress", { stage: "checking" });
      await sleep(600);
      emit("roblox-progress", { stage: "ready", version: "version-6d7e1c9a4b2f4e31" });
      emit("roblox-launched", "version-6d7e1c9a4b2f4e31");
      if (args.place) {
        await sleep(400);
        emit("roblox-activity", { type: "joining", place_id: args.place, job_id: args.job ?? "4f1c9a3e-2b7d-4e8f-9a10-1234567890ab" });
      }
      return "version-6d7e1c9a4b2f4e31";
    }
    case "roblox_rejoin_cancel": emit("roblox-rejoin", { state: "cancelled", placeId: 0, seconds: 0, message: null }); return;
    case "roblox_accounts": return structuredClone(rbxAccounts);
    case "roblox_account_login": {
      await sleep(1500);
      const a = { id: 3000000000 + rbxAccounts.list.length, name: "new_alt_" + rbxAccounts.list.length, displayName: "New alt", avatar: null, note: "", added: Date.now() / 1000, lastUsed: null };
      rbxAccounts.list.push(a);
      return a;
    }
    case "roblox_account_cancel_login": return;
    case "roblox_account_remove": rbxAccounts.list = rbxAccounts.list.filter((a) => a.id !== args.id); if (rbxAccounts.default === args.id) rbxAccounts.default = null; return;
    case "roblox_account_default": rbxAccounts.default = args.id as number | null; return;
    case "roblox_account_note": { const a = rbxAccounts.list.find((x) => x.id === args.id); if (a) a.note = String(args.note); return; }
    case "roblox_account_reorder": { const ids = args.ids as number[]; rbxAccounts.list.sort((a, b) => ids.indexOf(a.id) - ids.indexOf(b.id)); return; }
    case "roblox_account_check": await sleep(500); return args.id !== 2;
    case "roblox_servers": {
      await sleep(500);
      const page = args.cursor ? 2 : 1;
      const servers = Array.from({ length: page === 1 ? 24 : 12 }, (_, i) => {
        const max = 24;
        const playing = Math.max(1, Math.round(max - (i + (page - 1) * 24) * 0.7 - Math.random() * 3));
        return { id: `job-${page}-${i}-${Math.random().toString(16).slice(2, 10)}`, playing, maxPlayers: max, ping: 30 + Math.round(Math.random() * 160), fps: 55 + Math.random() * 5 };
      });
      return { servers, next: page === 1 ? "page2" : null };
    }
    case "roblox_server_region": {
      await sleep(250 + Math.random() * 500);
      const r = ["Ashburn, United States", "Frankfurt, Germany", "London, United Kingdom", "Singapore, Singapore", "Los Angeles, United States", "Tokyo, Japan", "Sydney, Australia", "Amsterdam, Netherlands", "Mumbai, India", "Dallas, United States"];
      return r[Math.abs([...String(args.job)].reduce((h, c) => h * 31 + c.charCodeAt(0), 7)) % r.length];
    }
    case "roblox_games": return (args.places as number[]).map((p) => { const g = rbxHistory.games.find((x) => x.placeId === p); return { placeId: p, universeId: p + 1, name: g?.name ?? `Place ${p}`, creator: g?.creator ?? "Someone", icon: null, playing: 12000 }; });
    case "roblox_place_from": { const m = String(args.text).match(/(?:games\/|placeid=)?(\d{3,})/i); return m ? Number(m[1]) : null; }
    case "roblox_history": return structuredClone(rbxHistory);
    case "roblox_history_add": rbxHistory.games.push({ placeId: args.place as number, name: "Natural Disaster Survival", creator: "Stickmasterluke", icon: null, seconds: 0, sessions: 0, last: Date.now() / 1000, favourite: false }); emit("roblox-history", null); return;
    case "roblox_history_favourite": { const g = rbxHistory.games.find((x) => x.placeId === args.place); if (g) g.favourite = !!args.on; return; }
    case "roblox_history_forget": rbxHistory.games = rbxHistory.games.filter((g) => g.placeId !== args.place); rbxHistory.visits = rbxHistory.visits.filter((g) => g.placeId !== args.place); return;
    case "roblox_history_clear": rbxHistory.visits = []; return;
    case "roblox_shortcut": return `C:\\Users\\you\\Desktop\\${args.name}.lnk`;
    case "roblox_cursor_preview": return mockCursor(String(args.style), String(args.colour), Number(args.scale));
    case "roblox_flag_allowlist": return ["DFIntCSGLevelOfDetailSwitchingDistance", "DFIntCSGLevelOfDetailSwitchingDistanceL12", "DFIntCSGLevelOfDetailSwitchingDistanceL23", "DFIntCSGLevelOfDetailSwitchingDistanceL34", "FFlagHandleAltEnterFullscreenManually", "DFFlagTextureQualityOverrideEnabled", "DFIntTextureQualityOverride", "FIntDebugForceMSAASamples", "DFFlagDisableDPIScale", "FFlagDebugGraphicsPreferD3D11", "FFlagDebugSkyGray", "DFFlagDebugPauseVoxelizer", "DFIntDebugFRMQualityLevelOverride", "FIntFRMMaxGrassDistance", "FIntFRMMinGrassDistance", "FFlagDebugGraphicsPreferVulkan", "FFlagDebugGraphicsPreferOpenGL", "FIntGrassMovementReducedMotionFactor"];
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
    // ── Minecraft+ ──
    case "mc_packs": {
      if (args.kind === "shaderpacks") return [
        { fileName: "ComplementaryReimagined_r5.4.zip", name: "ComplementaryReimagined_r5.4", enabled: true, folder: false, size: 1_420_000, description: "Complementary Reimagined, by EminGT", icon: null },
        { fileName: "BSL_v8.4.zip.disabled", name: "BSL_v8.4", enabled: false, folder: false, size: 980_000, description: "BSL Shaders", icon: null },
      ];
      return [
        { fileName: "Faithful 32x - 1.21.4.zip", name: "Faithful 32x - 1.21.4", enabled: true, folder: false, size: 6_200_000, description: "The original Faithful, at double resolution", icon: null },
        { fileName: "Fresh Animations.zip", name: "Fresh Animations", enabled: true, folder: false, size: 2_100_000, description: "Smooth, lively mob animations", icon: null },
        { fileName: "My Tweaks", name: "My Tweaks", enabled: true, folder: true, size: 44_000, description: "Personal UI tweaks", icon: null },
        { fileName: "Low Fire.zip.disabled", name: "Low Fire", enabled: false, folder: false, size: 12_000, description: "Shorter fire overlay", icon: null },
      ];
    }
    case "mc_pack_toggle": return String(args.on ? String(args.file).replace(/\.disabled$/, "") : `${args.file}.disabled`);
    case "mc_pack_remove": return;
    case "mc_packs_add": return (args.files as string[]).length;
    case "mc_worlds": {
      const now = Date.now();
      return [
        { folder: "Survival Island", name: "Survival Island", lastPlayed: now - 3_600_000, gameMode: 0, hardcore: false, cheats: false, version: "1.21.4", seed: -4172144997902289642, size: 184_000_000, icon: null, backups: [{ file: "2026-10-27_18-02-11.zip", created: now / 1000 - 172800, size: 92_000_000 }, { file: "2026-10-20_21-44-02.zip", created: now / 1000 - 777600, size: 80_000_000 }] },
        { folder: "Creative Build", name: "Castle Build", lastPlayed: now - 86_400_000 * 3, gameMode: 1, hardcore: false, cheats: true, version: "1.21.4", seed: 42, size: 61_000_000, icon: null, backups: [] },
        { folder: "Hardcore", name: "Hardcore #3", lastPlayed: now - 86_400_000 * 20, gameMode: 0, hardcore: true, cheats: false, version: "1.21.1", seed: 1234567, size: 22_000_000, icon: null, backups: [] },
      ];
    }
    case "mc_world_backup": await sleep(900); return { file: "2026-10-29_23-30-00.zip", created: Date.now() / 1000, size: 93_000_000 };
    case "mc_world_restore": case "mc_world_backup_delete": case "mc_world_delete": await sleep(400); return;
    case "mc_screenshots": return Array.from({ length: 9 }, (_, i) => ({ file: `2026-10-${29 - i}_19.4${i}.12.png`, path: mockShot(i), taken: Date.now() / 1000 - i * 90000, size: 2_400_000 }));
    case "mc_screenshot_delete": return;
    case "mc_servers": return [
      { name: "Hypixel", address: "mc.hypixel.net", icon: null },
      { name: "Friends SMP", address: "smp.example.net:25570", icon: null },
      { name: "Local test", address: "localhost", icon: null },
    ];
    case "mc_server_add": case "mc_server_remove": return;
    case "mc_server_ping": {
      await sleep(300 + Math.random() * 600);
      if (args.address === "localhost") throw "the server didn't answer";
      return args.address === "mc.hypixel.net"
        ? { online: 31248, max: 200000, version: "Requires MC 1.8 / 1.21", motd: "Hypixel Network [1.8-1.21]\nSKYBLOCK 0.23 | NEW PARTY GAMES", icon: null, pingMs: 34 }
        : { online: 6, max: 20, version: "Paper 1.21.4", motd: "Welcome back! Season 4 is live", icon: null, pingMs: 61 };
    }
    case "mc_updates": await sleep(900); return [
      { folder: "mods", fileName: "sodium-fabric-0.6.5+mc1.21.4.jar", projectId: "AANobbMI", title: "Sodium", icon: null, current: "0.6.5", latest: "0.6.9", latestId: "x1", latestFile: "sodium-fabric-0.6.9+mc1.21.4.jar" },
      { folder: "mods", fileName: "iris-fabric-1.8.5.jar", projectId: "YL57xq9U", title: "Iris Shaders", icon: null, current: "1.8.5", latest: "1.8.8", latestId: "x2", latestFile: "iris-fabric-1.8.8.jar" },
      { folder: "resourcepacks", fileName: "Fresh Animations.zip", projectId: "50dA9Sha", title: "Fresh Animations", icon: null, current: "1.9.2", latest: "1.9.4", latestId: "x3", latestFile: "FreshAnimations_v1.9.4.zip" },
    ];
    case "mc_update_apply": await sleep(700); return (args.update as { latestFile: string }).latestFile;
    case "mc_export": await sleep(1200); return { path: args.dest, linked: 5, packed: 41 };
    case "mc_java_runtimes": return [{ id: "jre-legacy", label: "Java 8" }, { id: "java-runtime-alpha", label: "Java 16" }, { id: "java-runtime-gamma", label: "Java 17" }, { id: "java-runtime-delta", label: "Java 21" }];
    case "mc_set_launch": return;
    case "mc_logs": return [
      { path: "crash-reports/crash-2026-10-29_21.14.03-client.txt", kind: "crash", modified: Date.now() / 1000 - 7200, size: 18_200 },
      { path: "logs/latest.log", kind: "log", modified: Date.now() / 1000 - 7000, size: 240_000 },
      { path: "logs/debug.log", kind: "log", modified: Date.now() / 1000 - 7000, size: 1_900_000 },
    ];
    case "mc_log_read": return mockLog(String(args.path));
    case "mc_explain": return {
      instance: args.id, path: "crash-reports/crash-2026-10-29_21.14.03-client.txt",
      diagnoses: [
        { kind: "missing-dependency", title: "Sodium Extra needs sodium", detail: "Sodium Extra won't load without sodium 0.6.0 or newer.", fix: "Install sodium 0.6.0 or newer from Add content, or remove Sodium Extra.", mods: ["Sodium Extra", "sodium"] },
        { kind: "suspect", title: "Probably caused by Iris", detail: "The crash report points at these mods.", fix: "Update them, or turn them off one at a time to find the culprit.", mods: ["Iris"] },
      ],
      lastError: "Caused by: java.lang.IllegalStateException: Sodium Extra requires Sodium 0.6.0",
    };
    case "mc_skins": return structuredClone(mcSkins);
    case "mc_skin_add_file": case "mc_skin_add_player": { const s = { id: `s${mcSkins.length + 1}`, name: String(args.name ?? "New skin"), variant: "classic" as const, added: Date.now() / 1000, data: mockSkin("#c0563b", "#3f6fd1") }; mcSkins.unshift(s); return s; }
    case "mc_skin_edit": { const s = mcSkins.find((x) => x.id === args.id); if (s) { s.name = String(args.name); s.variant = args.variant as "classic" | "slim"; } return; }
    case "mc_skin_remove": mcSkins = mcSkins.filter((x) => x.id !== args.id); return;
    case "mc_profile": await sleep(500); return structuredClone(mcProfile);
    case "mc_skin_wear": { await sleep(700); const s = mcSkins.find((x) => x.id === args.id); if (s) { mcProfile.skin = s.data; mcProfile.variant = s.variant; } return structuredClone(mcProfile); }
    case "mc_cape_set": { await sleep(500); mcProfile.capes.forEach((c) => (c.active = c.id === args.cape)); return structuredClone(mcProfile); }
    case "mc_instances": {
      const now = Math.floor(Date.now() / 1000);
      return [
        { id: "a1", name: "Fabulously Optimized", mcVersion: "1.21.4", loader: "fabric", loaderVersion: "0.16.10", versionId: null, memoryMb: 4096, javaArgs: "", created: now - 864000, lastPlayed: now - 7200, playtimeSecs: 154800, source: { platform: "modrinth", name: "Fabulously Optimized" }, manualDownloads: [], java: "", javaPath: null, autoJoin: "" },
        { id: "a2", name: "All the Mods 10", mcVersion: "1.21.1", loader: "neoforge", loaderVersion: "21.1.93", versionId: null, memoryMb: 8192, javaArgs: "", created: now - 400000, lastPlayed: now - 172800, playtimeSecs: 41200, source: { platform: "curseforge" }, manualDownloads: [{ name: "Example Mod", fileName: "examplemod-1.21.1.jar", pageUrl: "https://www.curseforge.com/minecraft/mc-mods/", folder: "mods" }], java: "", javaPath: null, autoJoin: "" },
        { id: "a3", name: "RLCraft", mcVersion: "1.12.2", loader: "forge", loaderVersion: "14.23.5.2860", versionId: null, memoryMb: 6144, javaArgs: "", created: now - 2000000, lastPlayed: now - 1500000, playtimeSecs: 9000, source: null, manualDownloads: [], java: "", javaPath: null, autoJoin: "" },
        { id: "a4", name: "Vanilla 1.21.4", mcVersion: "1.21.4", loader: "vanilla", loaderVersion: null, versionId: null, memoryMb: 3072, javaArgs: "", created: now - 50000, lastPlayed: null, playtimeSecs: 0, source: null, manualDownloads: [], java: "", javaPath: null, autoJoin: "" },
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
