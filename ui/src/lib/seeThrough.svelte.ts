// Translucent window addon: whether this window is see-through right now,
// and applying the Windows backdrop to it.
import { windowFx, type EffectsStatus } from "./api";
import { rgba, resolve, type UiState } from "./theme";

export const fx = $state({ status: null as EffectsStatus | null });

export async function loadFxStatus() {
  try {
    fx.status = await windowFx.status();
  } catch {
    fx.status = null;
  }
}

/** See-through is live: installed, switched on, and this run's windows were
 *  created transparent (that part needs a restart to change). */
export function seeThroughLive(ui: UiState): boolean {
  return !!fx.status?.active && ui.addons.installed.includes("see-through") && ui.seeThrough.enabled;
}

function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

let timer: ReturnType<typeof setTimeout> | undefined;
let lastSent = "";
/** Apply the look's CSS side and (debounced) the window backdrop. */
export function applySeeThrough(ui: UiState) {
  const live = seeThroughLive(ui);
  const p = ui.seeThrough;
  const root = document.documentElement;
  root.dataset.seeThrough = String(live);
  root.dataset.keepWall = String(live && p.keepWallpaper);
  if (live) root.style.setProperty("--panel", rgba(resolve(ui.look).preset.colors.panel, p.panel));

  // Only talk to the window when this run is transparent at all.
  if (!fx.status?.active) return;
  const tinted = p.effect === "acrylic" || p.effect === "blur";
  const color = tinted ? [...hexToRgb(p.tint), Math.round(p.strength * 255)] as [number, number, number, number] : null;
  const key = live ? `${p.effect}:${color?.join(",") ?? ""}` : "off";
  if (key === lastSent) return;
  clearTimeout(timer);
  timer = setTimeout(() => {
    lastSent = key;
    windowFx.set(live ? p.effect : null, live ? color : null).catch(() => {});
  }, 120);
}

/** The addon was removed: drop the backdrop now; transparency goes on restart. */
export async function seeThroughRemoved() {
  await windowFx.set(null, null).catch(() => {});
  lastSent = "off";
  await loadFxStatus();
}
