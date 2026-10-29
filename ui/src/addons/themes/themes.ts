// Themes & CSS snippets: theme packs recolour the app on top of the Look;
// snippets and your own CSS go in one <style> at the end of <head>.
import { rgba, resolve, type UiState } from "../../lib/theme";

export const THEMES_ID = "themes";

export interface ThemePack {
  id: string;
  name: string;
  scheme: "dark" | "light";
  /** desk, panel, raised, sunk, line, text, muted, faint, accent, alt (info), ok, bad */
  c: { desk: string; panel: string; raised: string; sunk: string; line: string; lineSoft: string; text: string; muted: string; faint: string; accent: string; ink: string; alt: string; ok: string; bad: string };
}

export const PACKS: ThemePack[] = [
  { id: "peach-rice", name: "Peach Rice", scheme: "dark", c: { desk: "#11111b", panel: "#1e1e2e", raised: "#313244", sunk: "#181825", line: "#45475a", lineSoft: "#313244", text: "#cdd6f4", muted: "#a6adc8", faint: "#7f849c", accent: "#fab387", ink: "#1e1e2e", alt: "#cba6f7", ok: "#a6e3a1", bad: "#f38ba8" } },
  { id: "gruvbox-hard", name: "Gruvbox Hard", scheme: "dark", c: { desk: "#141617", panel: "#1d2021", raised: "#32302f", sunk: "#141617", line: "#504945", lineSoft: "#3c3836", text: "#ebdbb2", muted: "#a89984", faint: "#7c6f64", accent: "#fe8019", ink: "#1d2021", alt: "#83a598", ok: "#b8bb26", bad: "#fb4934" } },
  { id: "nord-frost", name: "Nord Frost", scheme: "dark", c: { desk: "#242933", panel: "#2e3440", raised: "#3b4252", sunk: "#272c36", line: "#4c566a", lineSoft: "#3b4252", text: "#eceff4", muted: "#c0c8d6", faint: "#8891a3", accent: "#88c0d0", ink: "#2e3440", alt: "#b48ead", ok: "#a3be8c", bad: "#bf616a" } },
  { id: "rose-night", name: "Rosé Night", scheme: "dark", c: { desk: "#121019", panel: "#191724", raised: "#26233a", sunk: "#1f1d2e", line: "#403d52", lineSoft: "#2a2740", text: "#e0def4", muted: "#b5b2cc", faint: "#7e7a96", accent: "#ebbcba", ink: "#191724", alt: "#9ccfd8", ok: "#31748f", bad: "#eb6f92" } },
  { id: "paper", name: "Paper", scheme: "light", c: { desk: "#e8e3d8", panel: "#f4f1ea", raised: "#fffdf8", sunk: "#ebe6db", line: "#d2cabb", lineSoft: "#e2dccf", text: "#2b2a27", muted: "#5f5a51", faint: "#8a8478", accent: "#b44e28", ink: "#ffffff", alt: "#3a6ea5", ok: "#3f7d4a", bad: "#b3261e" } },
  { id: "oled", name: "OLED", scheme: "dark", c: { desk: "#000000", panel: "#000000", raised: "#111111", sunk: "#070707", line: "#262626", lineSoft: "#161616", text: "#f2f2f2", muted: "#a3a3a3", faint: "#6e6e6e", accent: "#ffffff", ink: "#000000", alt: "#9aa4ff", ok: "#8fe3c8", bad: "#ff7b86" } },
];

export interface Snippet {
  id: string;
  name: string;
  note: string;
  css: string;
}

export const SNIPPETS: Snippet[] = [
  { id: "round-covers", name: "Rounder covers", note: "More radius on every cover in Music", css: ".music .art:not(.round) { border-radius: 18px !important; }" },
  { id: "square", name: "Square corners", note: "Sharp corners everywhere", css: ":root { --r-sm: 2px !important; --r-md: 3px !important; --r-lg: 4px !important; --r-win: 6px !important; }" },
  { id: "no-album-col", name: "Hide the album column", note: "In song lists", css: ".music .row .album-col { visibility: hidden; }" },
  { id: "no-durations", name: "Hide song lengths", note: "In song lists", css: ".music .row .dur { visibility: hidden; }" },
  { id: "no-hearts", name: "Hide hearts in lists", note: "Favourite from the menu instead", css: ".music .row .fav:not(.on) { visibility: hidden; }" },
  { id: "borderless-tabs", name: "Borderless tabs", note: "Music tabs without outlines", css: ".music .tabs button { border-color: transparent !important; }" },
  { id: "big-lyrics", name: "Big lyrics", note: "The line being sung at 72 px", css: ".lines p.now { font-size: 72px !important; }" },
  { id: "thin-scroll", name: "Thin scrollbars", note: "Everywhere", css: "::-webkit-scrollbar { width: 6px !important; height: 6px !important; } ::-webkit-scrollbar-thumb { border-width: 1px !important; }" },
  { id: "still", name: "Fewer animations", note: "Turn off most motion in the app", css: "*, *::before, *::after { animation-duration: 0.01ms !important; transition-duration: 0.01ms !important; }" },
];

let safe = false;
/** Hold Shift while Arcadia starts: your CSS stays off for this run. */
export function watchSafeMode(onSafe: () => void) {
  if (typeof window === "undefined") return;
  const k = (e: KeyboardEvent) => {
    if (e.key === "Shift" && !safe) {
      safe = true;
      document.getElementById("arcadia-css")?.remove();
      onSafe();
    }
  };
  window.addEventListener("keydown", k);
  setTimeout(() => window.removeEventListener("keydown", k), 2500);
}
export const inSafeMode = () => safe;

/** Recolour from a theme pack (after applyLook), and add snippets and CSS. */
export function applyThemes(ui: UiState) {
  const on = ui.addons.installed.includes(THEMES_ID);
  const t = ui.themes;
  const pack = on && t.theme ? PACKS.find((p) => p.id === t.theme) : undefined;
  const root = document.documentElement;
  if (pack) {
    const c = pack.c;
    const opacity = ui.look.panelOpacity ?? resolve(ui.look).preset.panelOpacity;
    const light = pack.scheme === "light";
    const vars: Record<string, string> = {
      "--desk": c.desk, "--ink": c.panel, "--panel": rgba(c.panel, opacity),
      "--raised": opacity < 1 && light ? rgba(c.raised, 0.55) : c.raised,
      "--sunk": opacity < 1 && light ? rgba(c.sunk, 0.6) : c.sunk,
      "--line": c.line, "--line-soft": c.lineSoft, "--text": c.text, "--muted": c.muted, "--faint": c.faint,
      "--peach": ui.look.accent ?? c.accent, "--peach-ink": ui.look.accent ? resolve(ui.look).vars["--peach-ink"] : c.ink,
      "--mint": c.ok, "--rose": c.bad, "--lilac": c.alt,
      "--group": light ? "rgba(255,255,255,0.42)" : rgba(c.raised, opacity < 1 ? 0.5 : 0.45),
    };
    for (const [k, v] of Object.entries(vars)) root.style.setProperty(k, v);
    root.style.colorScheme = pack.scheme;
    root.dataset.scheme = pack.scheme;
  }
  root.dataset.themePack = pack?.id ?? "";

  let el = document.getElementById("arcadia-css") as HTMLStyleElement | null;
  const parts: string[] = [];
  if (on && !safe) {
    for (const s of SNIPPETS) if (t.snippets.includes(s.id)) parts.push(`/* ${s.name} */\n${s.css}`);
    for (const m of t.mine) if (m.on) parts.push(`/* ${m.name.replace(/\*\//g, "")} */\n${m.css}`);
    if (t.css.trim()) parts.push(t.scope === "music" ? `@scope (.music) {\n${t.css}\n}` : t.css);
  }
  const css = parts.join("\n\n");
  if (!css) {
    el?.remove();
    return;
  }
  if (!el) {
    el = document.createElement("style");
    el.id = "arcadia-css";
    document.head.appendChild(el);
  }
  if (el.textContent !== css) el.textContent = css;
}
