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

export interface UiState {
  brand: Brand;
  look: Look;
  bar: BarPrefs;
  saved: SavedLook[];
  applied: string | null;
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
  return { brand, look, bar: { ...d.bar, ...(u.bar ?? {}) }, saved: Array.isArray(u.saved) ? u.saved : [], applied: u.applied ?? null };
}

function rgba(color: string, alpha: number): string {
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
