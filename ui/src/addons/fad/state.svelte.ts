// Full App Display: open/close, and the window's full-screen state with it.
import { app } from "../../lib/state.svelte";

export const fad = $state({ open: false, fullscreen: false });

const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function setFullscreen(on: boolean) {
  fad.fullscreen = on;
  if (inTauri) {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().setFullscreen(on).catch(() => {});
  } else if (on) {
    await document.documentElement.requestFullscreen?.().catch(() => {});
  } else if (document.fullscreenElement) {
    await document.exitFullscreen().catch(() => {});
  }
}

export function openFad() {
  if (!app.ui.addons.installed.includes("full-app-display")) return;
  fad.open = true;
  if (app.ui.fad.fullscreen) setFullscreen(true);
}

export function closeFad() {
  fad.open = false;
  if (fad.fullscreen) setFullscreen(false);
}

export const toggleFullscreen = () => setFullscreen(!fad.fullscreen);
