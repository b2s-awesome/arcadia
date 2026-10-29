import { api } from "./api";
import type { Settings } from "./types";
import { normalizeUi, type UiState } from "./theme";

type Toast = { id: number; text: string; tone: "ok" | "error" };
export type CorePage = "library" | "minecraft" | "roblox" | "proxy" | "inspector" | "appearance" | "settings" | "marketplace";
/** A built-in page, or an installed addon's page ("addon:<id>"). */
export type Page = CorePage | `addon:${string}`;

/** Which window this webview is. The Minecraft window gets both a query
 *  string and an injected global, in case the query is dropped. */
function windowMode(): "hub" | "minecraft" {
  if (typeof window === "undefined") return "hub";
  const injected = (window as unknown as { __ARCADIA_MODE__?: string }).__ARCADIA_MODE__;
  const query = new URLSearchParams(location.search).get("mode");
  return injected === "minecraft" || query === "minecraft" ? "minecraft" : "hub";
}

export const app = $state({
  settings: null as Settings | null,
  ui: normalizeUi(null) as UiState,
  page: ((typeof location !== "undefined" && new URLSearchParams(location.search).get("page")) || "library") as Page,
  launcher: false,
  /** "minecraft" when this webview is the standalone Minecraft window. */
  mode: windowMode(),
  /** Live status shared by the top bar and pages. */
  playing: null as { placeId: number; ip?: string } | null,
  toasts: [] as Toast[],
});

let tid = 1;
export function toast(text: string, tone: Toast["tone"] = "ok") {
  const id = tid++;
  app.toasts.push({ id, text, tone });
  setTimeout(() => (app.toasts = app.toasts.filter((t) => t.id !== id)), tone === "error" ? 7000 : 3200);
}

export async function loadSettings() {
  app.settings = await api.getSettings();
  app.ui = normalizeUi(app.settings.ui);
}

/** Persist the appearance state into settings.ui. */
export function saveUi() {
  if (!app.settings) return;
  app.settings.ui = $state.snapshot(app.ui);
  saveSettings();
}

let saveTimer: ReturnType<typeof setTimeout> | undefined;
let pending = false;
/** Debounced save, so sliders and text fields don't hammer the disk. */
export function saveSettings() {
  clearTimeout(saveTimer);
  pending = true;
  saveTimer = setTimeout(saveSettingsNow, 300);
}

/** Save right away, e.g. before asking the backend about something that
 *  depends on the new settings (an addon that was just installed). */
export async function saveSettingsNow() {
  clearTimeout(saveTimer);
  pending = false;
  if (!app.settings) return;
  try {
    await api.saveSettings($state.snapshot(app.settings));
  } catch (e) {
    toast(String(e), "error");
  }
}

/** Another window saved settings: take its version, unless an edit of our
 *  own is about to be saved (that save will reach the other window). */
export async function reloadSettings() {
  if (pending) return;
  const fresh = await api.getSettings();
  if (pending) return;
  // Update in place: pages hold references to these objects.
  if (app.settings) Object.assign(app.settings, fresh);
  else app.settings = fresh;
  Object.assign(app.ui, normalizeUi(fresh.ui));
}

export function bytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 ** 2) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 ** 3) return `${(n / 1024 ** 2).toFixed(1)} MB`;
  return `${(n / 1024 ** 3).toFixed(2)} GB`;
}
