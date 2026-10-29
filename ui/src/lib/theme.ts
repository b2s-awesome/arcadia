// Looks: a preset (palette, type, shape) plus the user's overrides.
// Everything is applied as CSS custom properties on <html>.

export type PresetId = "obsidian" | "gruvbox" | "frost" | "dusk";
export type FontChoice = "theme" | "grotesk" | "mono" | "rounded" | "display";
export type RadiusChoice = "theme" | "sharp" | "soft" | "round";

export interface Look {
  preset: PresetId;
  accent: string | null;
  font: FontChoice;
  radius: RadiusChoice;
  /** Opacity of the main window panel over the wallpaper, 0.3–1. */
  panelOpacity: number | null;
  /** "builtin:<id>", a file path, or null for a plain background. */
  wallpaper: string | null;
  wallpaperBlur: number;
  wallpaperDim: number;
  /** Window floats over the wallpaper with a gap, like a tiling WM. */
  floating: boolean;
}

export interface BarPrefs {
  show: boolean;
  style: "island" | "strip";
  clock24: boolean;
  seconds: boolean;
  date: boolean;
}

export interface SavedLook {
  name: string;
  look: Look;
}

export interface Brand {
  name: string;
  tagline: string;
}

export const DEFAULT_BRAND: Brand = { name: "Arcadia", tagline: "Games and Roblox" };

/** The vertical sidebar: which entries show, in what order. */
export interface RailPrefs {
  /** Entry ids top to bottom: pages, "marketplace" and "addon:<id>". */
  order: string[];
  hidden: string[];
  /** Icons only, with labels on hover. */
  compact: boolean;
}

/** The logo animation at startup. */
export interface SplashPrefs {
  length: "off" | "short" | "normal" | "long";
  /** Play the animation even when Windows animations are turned off. */
  animateAlways: boolean;
}

/** How long the splash stays up, from first paint, in ms. The logo
 *  animation itself takes about 1.3 s. */
export const SPLASH_MS: Record<Exclude<SplashPrefs["length"], "off">, number> = { short: 1500, normal: 2600, long: 4000 };

/** Translucent window addon: the desktop shows through, blurred by Windows. */
export interface SeeThroughPrefs {
  enabled: boolean;
  effect: "acrylic" | "mica" | "mica-dark" | "tabbed" | "blur";
  /** Acrylic/Blur tint colour and how strong it is (0–1). */
  tint: string;
  strength: number;
  /** Opacity of the app's panel over the blur, 0.15–1. */
  panel: number;
  /** Keep Arcadia's wallpaper, faintly, instead of the real desktop. */
  keepWallpaper: boolean;
}

export const DEFAULT_SEE_THROUGH: SeeThroughPrefs = {
  enabled: true, effect: "acrylic", tint: "#0e0e12", strength: 0.35, panel: 0.6, keepWallpaper: false,
};

/** The Music page, all of it changeable from its Customise panel. */
export type MusicSection = "hero" | "recent" | "added" | "playlists" | "most" | "favourites";
export interface MusicPrefs {
  layout: "tonal" | "tiled";
  /** Tonal: where the player bar sits. */
  bar: "bottom" | "top";
  /** Tint the page with the playing cover's colour. */
  artColours: boolean;
  /** Smallest album tile, px. */
  cover: number;
  compact: boolean;
  /** Home sections in order, and which are hidden. */
  sections: MusicSection[];
  hidden: MusicSection[];
  /** Tiled: show the queue pane. */
  queuePane: boolean;
  /** Music's own tabs, in order, and which are hidden. */
  tabs: MusicTab[];
  hiddenTabs: MusicTab[];
}

export type MusicTab = "home" | "online" | "albums" | "artists" | "songs" | "playlists" | "favourites" | "lyrics" | "stats" | "downloads";
export const MUSIC_TABS: { id: MusicTab; label: string; addon?: string }[] = [
  { id: "home", label: "Home" },
  { id: "online", label: "Online", addon: "streaming" },
  { id: "albums", label: "Albums" },
  { id: "artists", label: "Artists" },
  { id: "songs", label: "Songs" },
  { id: "playlists", label: "Playlists" },
  { id: "favourites", label: "Favourites" },
  { id: "lyrics", label: "Lyrics", addon: "lyrics-plus" },
  { id: "stats", label: "Stats", addon: "stats" },
  { id: "downloads", label: "Downloads", addon: "streaming" },
];

/** Which drawing the visual addons use. "auto" follows Music's layout:
 *  Tonal gets the first, Tiled the second. */
export type Skin = "auto" | "tonal" | "tiled" | "glass";

/** Full App Display addon. */
export interface FadPrefs {
  skin: Skin;
  /** Tonal: cover beside the title, or everything centred. */
  layout: "split" | "centred";
  /** Background blur of the cover, px. */
  blur: number;
  clock: boolean;
  upNext: boolean;
  /** The current lyric line under the title (needs Lyrics Plus). */
  lyric: boolean;
  /** Tiled: the spectrum visualiser. */
  visualiser: boolean;
  bands: number;
  vizStyle: "bars" | "blocks" | "mirror";
  /** Make the window full screen too, not just fill it. */
  fullscreen: boolean;
}
export const DEFAULT_FAD: FadPrefs = { skin: "auto", layout: "split", blur: 70, clock: true, upNext: true, lyric: true, visualiser: true, bands: 48, vizStyle: "bars", fullscreen: true };

/** Lyrics Plus addon. */
export interface LyricsPrefs {
  skin: Skin;
  align: "left" | "center";
  /** Current line size, px. */
  size: number;
  /** Highlight word by word when the lyrics have word timings. */
  words: boolean;
  /** Look songs up on LRCLIB when there's nothing on this PC. */
  online: boolean;
  /** Save lyrics found online as .lrc files beside your songs. */
  sidecar: boolean;
  /** Seconds to shift lyrics by, per song id. */
  offsets: Record<string, number>;
  bar: { position: "bottom" | "top"; size: number; tint: number };
  popup: { tint: number };
  /** Show romanisation under non-Latin lyrics. */
  romanise: boolean;
  /** Show a translation under each line. */
  translate: boolean;
  /** Language to translate into (ISO code). */
  target: string;
  /** Optional contact email for the free translation service (raises its daily limit). */
  email: string;
}
export const DEFAULT_LYRICS: LyricsPrefs = { skin: "auto", align: "left", size: 52, words: true, online: true, sidecar: false, offsets: {}, bar: { position: "bottom", size: 26, tint: 0.35 }, popup: { tint: 0.16 }, romanise: false, translate: false, target: "", email: "" };

/** Stats addon. */
export interface StatsPrefs {
  skin: Skin;
  range: "4w" | "6m" | "1y" | "all";
  /** Daily listening goal in minutes (the Glass ring). */
  goal: number;
}
export const DEFAULT_STATS: StatsPrefs = { skin: "auto", range: "4w", goal: 90 };

/** Themes & CSS snippets addon. */
export interface ThemesPrefs {
  /** A theme pack id, or null to use the Look as it is. */
  theme: string | null;
  /** Built-in snippets switched on. */
  snippets: string[];
  /** Your own CSS. */
  css: string;
  /** Where your CSS applies. */
  scope: "app" | "music";
  /** Snippets you saved. */
  mine: { id: string; name: string; css: string; on: boolean }[];
}
export const DEFAULT_THEMES: ThemesPrefs = { theme: null, snippets: [], css: "", scope: "app", mine: [] };

/** Liquid Glass addon. */
export interface GlassPrefs {
  enabled: boolean;
  /** Everything, or just Music. */
  scope: "app" | "music";
  /** Blur behind the glass, px. */
  frost: number;
  /** How solid the glass is, 0–0.7. */
  tint: number;
  /** Frosted white glass, smoked dark glass, or whichever suits the Look
   *  (frosted on light looks, smoked on dark ones, so text stays readable). */
  shade: "auto" | "frost" | "smoke";
  /** Brightness of the lit edges, 0–1. */
  edge: number;
  sheen: boolean;
}
export const DEFAULT_GLASS: GlassPrefs = { enabled: true, scope: "app", frost: 24, tint: 0.4, shade: "auto", edge: 0.55, sheen: true };

/** A bundle you made from your addons (and optionally their settings). */
export interface UserBundle {
  id: string;
  name: string;
  addons: string[];
  /** Settings carried with it, by UiState key. */
  settings: Partial<Record<BundleKey, unknown>> | null;
}
export type BundleKey = "fad" | "lyrics" | "stats" | "themes" | "seeThrough" | "music" | "glass";

export const MUSIC_SECTIONS: { id: MusicSection; label: string }[] = [
  { id: "hero", label: "Now playing / shuffle all" },
  { id: "recent", label: "Jump back in" },
  { id: "added", label: "Recently added" },
  { id: "playlists", label: "Playlists" },
  { id: "most", label: "Most played" },
  { id: "favourites", label: "Favourites" },
];

export const DEFAULT_MUSIC: MusicPrefs = {
  layout: "tonal", bar: "bottom", artColours: true, cover: 168, compact: false,
  sections: MUSIC_SECTIONS.map((s) => s.id), hidden: [], queuePane: true,
  tabs: MUSIC_TABS.map((t) => t.id), hiddenTabs: [],
};

export function normalizeMusic(v: Partial<MusicPrefs> | undefined): MusicPrefs {
  const d = DEFAULT_MUSIC;
  if (!v || typeof v !== "object") return { ...d, sections: [...d.sections], tabs: [...d.tabs], hiddenTabs: [] };
  const tabIds = MUSIC_TABS.map((t) => t.id);
  const tabOrder = Array.isArray(v.tabs) ? v.tabs.filter((x): x is MusicTab => tabIds.includes(x as MusicTab)) : [];
  const ids = MUSIC_SECTIONS.map((s) => s.id);
  const order = Array.isArray(v.sections) ? v.sections.filter((x): x is MusicSection => ids.includes(x as MusicSection)) : [];
  return {
    layout: v.layout === "tiled" ? "tiled" : "tonal",
    bar: v.bar === "top" ? "top" : "bottom",
    artColours: v.artColours !== false,
    cover: typeof v.cover === "number" ? Math.min(260, Math.max(110, v.cover)) : d.cover,
    compact: v.compact === true,
    sections: [...order, ...ids.filter((x) => !order.includes(x))],
    hidden: Array.isArray(v.hidden) ? v.hidden.filter((x): x is MusicSection => ids.includes(x as MusicSection)) : [],
    queuePane: v.queuePane !== false,
    // New tabs (from updates or addons) slot in at their default place.
    tabs: tabIds.reduce<MusicTab[]>((acc, id) => (acc.includes(id) ? acc : (() => { const i = tabIds.indexOf(id); const after = tabIds.slice(0, i).reverse().find((p) => acc.includes(p)); const at = after ? acc.indexOf(after) + 1 : 0; acc.splice(at, 0, id); return acc; })()), [...tabOrder]),
    hiddenTabs: Array.isArray(v.hiddenTabs) ? v.hiddenTabs.filter((x): x is MusicTab => tabIds.includes(x as MusicTab) && x !== "home") : [],
  };
}

export interface AddonPrefs {
  installed: string[];
}

export interface UiState {
  brand: Brand;
  look: Look;
  bar: BarPrefs;
  saved: SavedLook[];
  applied: string | null;
  rail: RailPrefs;
  addons: AddonPrefs;
  splash: SplashPrefs;
  seeThrough: SeeThroughPrefs;
  music: MusicPrefs;
  fad: FadPrefs;
  lyrics: LyricsPrefs;
  stats: StatsPrefs;
  themes: ThemesPrefs;
  glass: GlassPrefs;
  bundles: UserBundle[];
}

interface Preset {
  id: PresetId;
  name: string;
  blurb: string;
  scheme: "dark" | "light";
  /** Solid colors; --panel is derived from `panel` + opacity. */
  colors: {
    desk: string; panel: string; raised: string; sunk: string; rail: string;
    line: string; lineSoft: string; text: string; muted: string; faint: string;
    accent: string; accentInk: string; ok: string; bad: string; info: string;
  };
  font: Exclude<FontChoice, "theme">;
  radius: Exclude<RadiusChoice, "theme">;
  panelOpacity: number;
  glassBlur: number;
  /** Sections drawn as bordered groups (vs. hairline-separated). */
  grouped: boolean;
  /** Small spaced capitals for section headers. */
  caps: boolean;
  defaults: Pick<Look, "wallpaper" | "wallpaperBlur" | "wallpaperDim" | "floating">;
}

export const PRESETS: Record<PresetId, Preset> = {
  obsidian: {
    id: "obsidian",
    name: "Obsidian",
    blurb: "Black floating panel, grouped settings, white controls.",
    scheme: "dark",
    colors: {
      desk: "#e7e8ec", panel: "#0b0b0d", raised: "#141417", sunk: "#101012", rail: "transparent",
      line: "#26262c", lineSoft: "#1b1b1f", text: "#f3f3f5", muted: "#9c9ca6", faint: "#62626b",
      accent: "#e9e9f2", accentInk: "#0b0b0d", ok: "#dfe0f0", bad: "#ff7b86", info: "#b8bbff",
    },
    font: "grotesk",
    radius: "soft",
    panelOpacity: 1,
    glassBlur: 0,
    grouped: true,
    caps: true,
    defaults: { wallpaper: "builtin:paper", wallpaperBlur: 0, wallpaperDim: 0, floating: true },
  },
  gruvbox: {
    id: "gruvbox",
    name: "Gruvbox",
    blurb: "Warm browns and orange, monospace everything.",
    scheme: "dark",
    colors: {
      desk: "#1d2021", panel: "#282828", raised: "#32302f", sunk: "#1d2021", rail: "transparent",
      line: "#504945", lineSoft: "#3c3836", text: "#ebdbb2", muted: "#a89984", faint: "#7c6f64",
      accent: "#fe8019", accentInk: "#1d2021", ok: "#b8bb26", bad: "#fb4934", info: "#83a598",
    },
    font: "mono",
    radius: "sharp",
    panelOpacity: 0.9,
    glassBlur: 14,
    grouped: true,
    caps: true,
    defaults: { wallpaper: "builtin:ember", wallpaperBlur: 0, wallpaperDim: 0.15, floating: true },
  },
  frost: {
    id: "frost",
    name: "Frost",
    blurb: "Frosted glass over your wallpaper.",
    scheme: "light",
    colors: {
      desk: "#cfe0f2", panel: "#eef2f8", raised: "#ffffff", sunk: "#ffffff", rail: "transparent",
      line: "rgba(30, 42, 70, 0.14)", lineSoft: "rgba(30, 42, 70, 0.08)", text: "#18202f", muted: "#55607a", faint: "#8a93a8",
      accent: "#3d6bff", accentInk: "#ffffff", ok: "#14a37f", bad: "#e5484d", info: "#6e56cf",
    },
    font: "rounded",
    radius: "round",
    panelOpacity: 0.62,
    glassBlur: 30,
    grouped: true,
    caps: false,
    defaults: { wallpaper: "builtin:snowfield", wallpaperBlur: 0, wallpaperDim: 0, floating: true },
  },
  dusk: {
    id: "dusk",
    name: "Dusk",
    blurb: "Plum and peach, the original Arcadia look.",
    scheme: "dark",
    colors: {
      desk: "#1b1428", panel: "#1b1428", raised: "#251c36", sunk: "#160f22", rail: "#160f22",
      line: "#3a2f52", lineSoft: "#2d2442", text: "#f1eaff", muted: "#a99cc4", faint: "#776a92",
      accent: "#ffb38a", accentInk: "#2a1408", ok: "#8fe3c8", bad: "#ff8fa8", info: "#b9a6ff",
    },
    font: "display",
    radius: "soft",
    panelOpacity: 1,
    glassBlur: 0,
    grouped: false,
    caps: false,
    defaults: { wallpaper: null, wallpaperBlur: 0, wallpaperDim: 0, floating: false },
  },
};

export const BUILTIN_WALLPAPERS: { id: string; name: string }[] = [
  { id: "paper", name: "Paper" },
  { id: "snowfield", name: "Snowfield" },
  { id: "aurora", name: "Aurora" },
  { id: "ember", name: "Ember" },
  { id: "tide", name: "Tide" },
];

export const ACCENTS = ["#e9e9f2", "#fe8019", "#ffb38a", "#ff8fa8", "#c9a7ff", "#7aa2ff", "#3d6bff", "#8fe3c8", "#b8bb26", "#fabd2f"];

const FONTS: Record<Exclude<FontChoice, "theme">, { body: string; display: string; mono: boolean }> = {
  grotesk: { body: '"Geist Variable", "Segoe UI", sans-serif', display: '"Geist Variable", "Segoe UI", sans-serif', mono: false },
  mono: { body: '"JetBrains Mono Variable", Consolas, monospace', display: '"JetBrains Mono Variable", Consolas, monospace', mono: true },
  rounded: { body: '"Figtree Variable", "Segoe UI", sans-serif', display: '"Figtree Variable", "Segoe UI", sans-serif', mono: false },
  display: { body: '"Figtree Variable", "Segoe UI", sans-serif', display: '"Bricolage Grotesque Variable", "Segoe UI", sans-serif', mono: false },
};

const RADII: Record<Exclude<RadiusChoice, "theme">, { sm: number; md: number; lg: number; win: number }> = {
  sharp: { sm: 3, md: 4, lg: 6, win: 8 },
  soft: { sm: 6, md: 10, lg: 14, win: 26 },
  round: { sm: 10, md: 14, lg: 20, win: 30 },
};

export function defaultLook(preset: PresetId = "obsidian"): Look {
  return { preset, accent: null, font: "theme", radius: "theme", panelOpacity: null, ...PRESETS[preset].defaults };
}

export function defaultUi(): UiState {
  return {
    brand: { ...DEFAULT_BRAND },
    look: defaultLook("obsidian"),
    bar: { show: true, style: "island", clock24: false, seconds: false, date: true },
    saved: [],
    applied: null,
    rail: { order: [], hidden: [], compact: false },
    addons: { installed: [] },
    splash: { length: "normal", animateAlways: false },
    seeThrough: { ...DEFAULT_SEE_THROUGH },
    music: normalizeMusic(undefined),
    fad: normalizeFad(undefined),
    lyrics: normalizeLyrics(undefined),
    stats: normalizeStats(undefined),
    themes: normalizeThemes(undefined),
    glass: normalizeGlass(undefined),
    bundles: [],
  };
}

/** Fill in anything missing from a stored (possibly older) UI state. */
export function normalizeUi(v: unknown): UiState {
  const d = defaultUi();
  if (!v || typeof v !== "object") return d;
  const u = v as Partial<UiState>;
  const preset = u.look?.preset && u.look.preset in PRESETS ? u.look.preset : "obsidian";
  const look = { ...defaultLook(preset), ...(u.look ?? {}), preset };
  const brand = { ...DEFAULT_BRAND, ...(u.brand ?? {}) };
  const strs = (a: unknown): string[] => (Array.isArray(a) ? a.filter((x): x is string => typeof x === "string") : []);
  const rail: RailPrefs = {
    order: strs(u.rail?.order),
    hidden: strs(u.rail?.hidden),
    compact: u.rail?.compact === true,
  };
  return {
    brand,
    look,
    bar: { ...d.bar, ...(u.bar ?? {}) },
    saved: Array.isArray(u.saved) ? u.saved : [],
    applied: u.applied ?? null,
    rail,
    addons: { installed: strs(u.addons?.installed) },
    splash: {
      length: (["off", "short", "normal", "long"] as const).includes(u.splash?.length as SplashPrefs["length"]) ? u.splash!.length : d.splash.length,
      animateAlways: u.splash?.animateAlways === true,
    },
    seeThrough: normalizeSeeThrough(u.seeThrough),
    music: normalizeMusic(u.music),
    fad: normalizeFad(u.fad),
    lyrics: normalizeLyrics(u.lyrics),
    stats: normalizeStats(u.stats),
    themes: normalizeThemes(u.themes),
    glass: normalizeGlass(u.glass),
    bundles: normalizeBundles(u.bundles),
  };
}

const num = (x: unknown, lo: number, hi: number, def: number) => (typeof x === "number" && isFinite(x) ? Math.min(hi, Math.max(lo, x)) : def);
const pick = <T extends string>(x: unknown, opts: readonly T[], def: T): T => (opts.includes(x as T) ? (x as T) : def);
const SKINS = ["auto", "tonal", "tiled", "glass"] as const;

export function normalizeFad(v: Partial<FadPrefs> | undefined): FadPrefs {
  const d = DEFAULT_FAD;
  if (!v || typeof v !== "object") return { ...d };
  return {
    skin: pick(v.skin, SKINS, d.skin),
    layout: pick(v.layout, ["split", "centred"] as const, d.layout),
    blur: num(v.blur, 0, 160, d.blur),
    clock: v.clock !== false,
    upNext: v.upNext !== false,
    lyric: v.lyric !== false,
    visualiser: v.visualiser !== false,
    bands: Math.round(num(v.bands, 12, 128, d.bands)),
    vizStyle: pick(v.vizStyle, ["bars", "blocks", "mirror"] as const, d.vizStyle),
    fullscreen: v.fullscreen !== false,
  };
}

export function normalizeLyrics(v: Partial<LyricsPrefs> | undefined): LyricsPrefs {
  const d = DEFAULT_LYRICS;
  if (!v || typeof v !== "object") return { ...d, offsets: {}, bar: { ...d.bar }, popup: { ...d.popup } };
  const offsets: Record<string, number> = {};
  if (v.offsets && typeof v.offsets === "object") for (const [k, x] of Object.entries(v.offsets)) if (typeof x === "number" && isFinite(x) && x !== 0) offsets[k] = Math.max(-30, Math.min(30, x));
  return {
    skin: pick(v.skin, SKINS, d.skin),
    align: pick(v.align, ["left", "center"] as const, d.align),
    size: num(v.size, 28, 96, d.size),
    words: v.words !== false,
    online: v.online !== false,
    sidecar: v.sidecar === true,
    offsets,
    bar: { position: pick(v.bar?.position, ["bottom", "top"] as const, d.bar.position), size: num(v.bar?.size, 16, 56, d.bar.size), tint: num(v.bar?.tint, 0, 0.9, d.bar.tint) },
    popup: { tint: num(v.popup?.tint, 0, 0.6, d.popup.tint) },
    romanise: v.romanise === true,
    translate: v.translate === true,
    target: typeof v.target === "string" && /^[a-z]{2,3}(-[A-Za-z]{2,4})?$/.test(v.target) ? v.target : "",
    email: typeof v.email === "string" ? v.email.slice(0, 120) : "",
  };
}

export function normalizeStats(v: Partial<StatsPrefs> | undefined): StatsPrefs {
  const d = DEFAULT_STATS;
  if (!v || typeof v !== "object") return { ...d };
  return { skin: pick(v.skin, SKINS, d.skin), range: pick(v.range, ["4w", "6m", "1y", "all"] as const, d.range), goal: Math.round(num(v.goal, 5, 600, d.goal)) };
}

export function normalizeThemes(v: Partial<ThemesPrefs> | undefined): ThemesPrefs {
  const d = DEFAULT_THEMES;
  if (!v || typeof v !== "object") return { ...d, snippets: [], mine: [] };
  const strs = (a: unknown): string[] => (Array.isArray(a) ? a.filter((x): x is string => typeof x === "string") : []);
  return {
    theme: typeof v.theme === "string" ? v.theme : null,
    snippets: strs(v.snippets),
    css: typeof v.css === "string" ? v.css.slice(0, 100_000) : "",
    scope: pick(v.scope, ["app", "music"] as const, d.scope),
    mine: Array.isArray(v.mine)
      ? v.mine.filter((m) => m && typeof m.id === "string" && typeof m.css === "string").map((m) => ({ id: m.id, name: String(m.name ?? "Snippet").slice(0, 60), css: m.css.slice(0, 50_000), on: m.on !== false }))
      : [],
  };
}

export function normalizeGlass(v: Partial<GlassPrefs> | undefined): GlassPrefs {
  const d = DEFAULT_GLASS;
  if (!v || typeof v !== "object") return { ...d };
  return {
    enabled: v.enabled !== false,
    scope: pick(v.scope, ["app", "music"] as const, d.scope),
    frost: num(v.frost, 0, 60, d.frost),
    tint: num(v.tint, 0, 0.7, d.tint),
    shade: pick(v.shade, ["auto", "frost", "smoke"] as const, d.shade),
    edge: num(v.edge, 0, 1, d.edge),
    sheen: v.sheen !== false,
  };
}

function normalizeBundles(v: unknown): UserBundle[] {
  if (!Array.isArray(v)) return [];
  return v
    .filter((b) => b && typeof b.id === "string" && Array.isArray(b.addons))
    .map((b) => ({ id: b.id, name: String(b.name ?? "My bundle").slice(0, 60), addons: b.addons.filter((x: unknown): x is string => typeof x === "string"), settings: b.settings && typeof b.settings === "object" ? b.settings : null }));
}

/** The drawing a visual addon uses, given its choice and Music's layout. */
export function skinFor(choice: Skin, layout: MusicPrefs["layout"]): Exclude<Skin, "auto"> {
  return choice === "auto" ? (layout === "tiled" ? "tiled" : "tonal") : choice;
}

export function normalizeSeeThrough(v: Partial<SeeThroughPrefs> | undefined): SeeThroughPrefs {
  const d = DEFAULT_SEE_THROUGH;
  if (!v || typeof v !== "object") return { ...d };
  return {
    enabled: v.enabled !== false,
    effect: (["acrylic", "mica", "mica-dark", "tabbed", "blur"] as const).includes(v.effect as SeeThroughPrefs["effect"]) ? v.effect! : d.effect,
    tint: typeof v.tint === "string" && /^#[0-9a-f]{6}$/i.test(v.tint) ? v.tint : d.tint,
    strength: num(v.strength, 0, 1, d.strength),
    panel: num(v.panel, 0.15, 1, d.panel),
    keepWallpaper: v.keepWallpaper === true,
  };
}

export function rgba(color: string, alpha: number): string {
  const m = /^#([0-9a-f]{6})$/i.exec(color);
  if (!m) return color;
  const n = parseInt(m[1], 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha})`;
}

/** Pick black or white text for a custom accent. */
function inkFor(hex: string): string {
  const m = /^#([0-9a-f]{6})$/i.exec(hex);
  if (!m) return "#111";
  const n = parseInt(m[1], 16);
  const lum = 0.2126 * ((n >> 16) & 255) + 0.7152 * ((n >> 8) & 255) + 0.0722 * (n & 255);
  return lum > 150 ? "#111114" : "#ffffff";
}

export function wallpaperUrl(w: string | null, toFileUrl: (p: string) => string): string | null {
  if (!w) return null;
  if (w.startsWith("builtin:")) return `/wallpapers/${w.slice(8)}.svg`;
  return toFileUrl(w);
}

/** Resolved values for a look, used both for the app and for previews. */
export function resolve(look: Look) {
  const p = PRESETS[look.preset];
  const font = FONTS[look.font === "theme" ? p.font : look.font];
  const radius = RADII[look.radius === "theme" ? p.radius : look.radius];
  const accent = look.accent ?? p.colors.accent;
  const accentInk = look.accent ? inkFor(look.accent) : p.colors.accentInk;
  const opacity = look.panelOpacity ?? p.panelOpacity;
  const c = p.colors;
  const vars: Record<string, string> = {
    "--desk": c.desk,
    "--ink": c.panel,
    "--panel": rgba(c.panel, opacity),
    "--raised": opacity < 1 && p.scheme === "light" ? rgba(c.raised, 0.55) : c.raised,
    "--sunk": opacity < 1 && p.scheme === "light" ? rgba(c.sunk, 0.6) : c.sunk,
    "--rail": c.rail,
    "--line": c.line,
    "--line-soft": c.lineSoft,
    "--text": c.text,
    "--muted": c.muted,
    "--faint": c.faint,
    "--peach": accent,
    "--peach-ink": accentInk,
    "--mint": c.ok,
    "--rose": c.bad,
    "--lilac": c.info,
    "--group": p.scheme === "light" ? "rgba(255,255,255,0.42)" : rgba(c.raised, opacity < 1 ? 0.5 : 0.45),
    "--glass-blur": `${opacity < 1 ? p.glassBlur || 18 : 0}px`,
    "--body": font.body,
    "--display": font.display,
    "--r-sm": `${radius.sm}px`,
    "--r-md": `${radius.md}px`,
    "--r-lg": `${radius.lg}px`,
    "--r-win": `${radius.win}px`,
    "--wall-blur": `${look.wallpaperBlur}px`,
    "--wall-dim": String(look.wallpaperDim),
  };
  return { preset: p, vars, mono: font.mono, scheme: p.scheme };
}

export function applyLook(look: Look) {
  const r = resolve(look);
  const root = document.documentElement;
  for (const [k, v] of Object.entries(r.vars)) root.style.setProperty(k, v);
  root.style.colorScheme = r.scheme;
  root.dataset.preset = look.preset;
  root.dataset.grouped = String(r.preset.grouped);
  root.dataset.caps = String(r.preset.caps);
  root.dataset.mono = String(r.mono);
  root.dataset.floating = String(look.floating);
  root.dataset.scheme = r.scheme;
}
