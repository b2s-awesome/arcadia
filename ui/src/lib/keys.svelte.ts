// Keyboard shortcuts: a list of actions, your keys for each, and which also
// work while Arcadia is in the background (global hotkeys).
import { app, saveUi, type Page } from "./state.svelte";

export interface Action {
  id: string;
  label: string;
  group: "App" | "Pages" | "Music" | "Games";
  /** Default combo, "" for none. */
  key: string;
  /** Can be made global (works when Arcadia isn't focused). */
  canGlobal: boolean;
}

export const ACTIONS: Action[] = [
  { id: "launcher", label: "Search and launch", group: "App", key: "Ctrl+K", canGlobal: true },
  { id: "show", label: "Bring Arcadia to the front", group: "App", key: "", canGlobal: true },
  { id: "bigpicture", label: "Big Picture", group: "App", key: "Ctrl+Shift+B", canGlobal: true },
  { id: "settings", label: "Settings", group: "App", key: "Ctrl+Comma", canGlobal: false },
  { id: "page:home", label: "Home", group: "Pages", key: "Ctrl+0", canGlobal: false },
  { id: "page:library", label: "Library", group: "Pages", key: "Ctrl+1", canGlobal: false },
  { id: "page:music", label: "Music", group: "Pages", key: "Ctrl+2", canGlobal: false },
  { id: "page:minecraft", label: "Minecraft", group: "Pages", key: "Ctrl+3", canGlobal: false },
  { id: "page:roblox", label: "Roblox", group: "Pages", key: "Ctrl+4", canGlobal: false },
  { id: "page:appearance", label: "Appearance", group: "Pages", key: "", canGlobal: false },
  { id: "music:toggle", label: "Play or pause", group: "Music", key: "Ctrl+Alt+Space", canGlobal: true },
  { id: "music:next", label: "Next song", group: "Music", key: "Ctrl+Alt+ArrowRight", canGlobal: true },
  { id: "music:prev", label: "Previous song", group: "Music", key: "Ctrl+Alt+ArrowLeft", canGlobal: true },
  { id: "music:up", label: "Volume up", group: "Music", key: "Ctrl+Alt+ArrowUp", canGlobal: true },
  { id: "music:down", label: "Volume down", group: "Music", key: "Ctrl+Alt+ArrowDown", canGlobal: true },
  { id: "roblox:play", label: "Play Roblox", group: "Games", key: "", canGlobal: true },
  { id: "game:last", label: "Play your last game", group: "Games", key: "", canGlobal: true },
];

export const keyOf = (id: string): string => app.ui.keys.bindings[id] ?? ACTIONS.find((a) => a.id === id)?.key ?? "";
export const isGlobal = (id: string) => app.ui.keys.global.includes(id);

const NAMES: Record<string, string> = {
  Space: "Space", Comma: ",", Period: ".", Slash: "/", Backslash: "\\", Minus: "-", Equal: "=", BracketLeft: "[", BracketRight: "]",
  Semicolon: ";", Quote: "'", Backquote: "`", ArrowUp: "↑", ArrowDown: "↓", ArrowLeft: "←", ArrowRight: "→", Enter: "Enter", Escape: "Esc",
  Backspace: "Backspace", Delete: "Del", Tab: "Tab", Home: "Home", End: "End", PageUp: "PgUp", PageDown: "PgDn",
};

/** A combo from a key event, using physical keys so layouts don't matter. */
export function comboOf(e: KeyboardEvent): string | null {
  const code = e.code;
  if (!code || ["ControlLeft", "ControlRight", "ShiftLeft", "ShiftRight", "AltLeft", "AltRight", "MetaLeft", "MetaRight"].includes(code)) return null;
  let key = code;
  if (/^Key[A-Z]$/.test(code)) key = code.slice(3);
  else if (/^Digit\d$/.test(code)) key = code.slice(5);
  else if (/^Numpad\d$/.test(code)) key = `Num${code.slice(6)}`;
  const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"].filter(Boolean);
  return [...mods, key].join("+");
}

/** How a combo is shown: "Ctrl + Alt + →". */
export const pretty = (combo: string) => (combo ? combo.split("+").map((p) => NAMES[p] ?? p).join(" + ") : "");

/** Global hotkeys use the same names, except a few the OS spells out. */
const toAccelerator = (combo: string) => combo.replace(/\bSuper\b/, "Super").replace(/\bNum(\d)\b/, "Numpad$1");

/** Other actions using the same combo. */
export function clashes(id: string, combo: string): Action[] {
  if (!combo) return [];
  return ACTIONS.filter((a) => a.id !== id && keyOf(a.id) === combo);
}

export function setKey(id: string, combo: string) {
  const a = ACTIONS.find((x) => x.id === id);
  if (!a) return;
  if (combo === a.key) delete app.ui.keys.bindings[id];
  else app.ui.keys.bindings[id] = combo;
  saveUi();
}
export function setGlobal(id: string, on: boolean) {
  const g = app.ui.keys.global.filter((x) => x !== id);
  app.ui.keys.global = on ? [...g, id] : g;
  saveUi();
}
export function resetKeys() {
  app.ui.keys = { bindings: {}, global: [] };
  saveUi();
}

// ── running actions ──

type Runner = (id: string) => void;
let runner: Runner = () => {};
/** App.svelte supplies what each action does. */
export function onAction(r: Runner) {
  runner = r;
}

const typing = (t: EventTarget | null) => {
  const el = t as HTMLElement | null;
  return !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || el.isContentEditable);
};

/** In-app shortcuts. Returns true if the key was used. */
export function handleKey(e: KeyboardEvent): boolean {
  const combo = comboOf(e);
  if (!combo) return false;
  // Plain keys stay with text fields; combos with Ctrl/Alt work anywhere.
  if (typing(e.target) && !e.ctrlKey && !e.altKey) return false;
  if (recording.on) return false;
  const a = ACTIONS.find((x) => keyOf(x.id) === combo);
  if (!a) return false;
  e.preventDefault();
  runner(a.id);
  return true;
}

/** While the key editor is listening for a new combo. */
export const recording = $state({ on: false });

// ── global hotkeys ──

let registered: string[] = [];
export async function syncGlobal() {
  if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window) || app.mode !== "hub") return;
  const gs = await import("@tauri-apps/plugin-global-shortcut");
  if (registered.length) await gs.unregister(registered).catch(() => {});
  registered = [];
  for (const id of app.ui.keys.global) {
    const combo = keyOf(id);
    const a = ACTIONS.find((x) => x.id === id);
    if (!combo || !a?.canGlobal) continue;
    const acc = toAccelerator(combo);
    try {
      await gs.register(acc, (ev) => {
        if (ev.state === "Pressed") runner(id);
      });
      registered.push(acc);
    } catch (err) {
      console.warn(`global hotkey ${acc} unavailable`, err);
    }
  }
}

export const pageAction = (id: string): Page | null => (id.startsWith("page:") ? (id.slice(5) as Page) : null);
