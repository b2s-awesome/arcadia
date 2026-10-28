import { api } from "./api";
import type { Settings } from "./types";
import { normalizeUi, type UiState } from "./theme";

type Toast = { id: number; text: string; tone: "ok" | "error" };
export type Page = "library" | "minecraft" | "roblox" | "proxy" | "inspector" | "appearance" | "settings";

export const app = $state({
  settings: null as Settings | null,
  ui: normalizeUi(null) as UiState,
  page: ((typeof location !== "undefined" && new URLSearchParams(location.search).get("page")) || "library") as Page,
  launcher: false,
  /** "minecraft" when this webview is the standalone Minecraft window. */
  mode: ((typeof location !== "undefined" && new URLSearchParams(location.search).get("mode")) === "minecraft" ? "minecraft" : "hub") as "hub" | "minecraft",
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
/** Debounced save, so sliders and text fields don't hammer the disk. */
export function saveSettings() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(async () => {
    if (!app.settings) return;
    try {
      await api.saveSettings($state.snapshot(app.settings));
    } catch (e) {
      toast(String(e), "error");
    }
  }, 300);
}

export function bytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 ** 2) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 ** 3) return `${(n / 1024 ** 2).toFixed(1)} MB`;
  return `${(n / 1024 ** 3).toFixed(2)} GB`;
}
