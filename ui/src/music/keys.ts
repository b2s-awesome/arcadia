// Keyboard control for the Music page. Ignored while typing.
import { ms, next, prev, seek, setVolume, toggle } from "./store.svelte";
import { app } from "../lib/state.svelte";
import { LOOP, addBookmark, clearLoop, setA, setB } from "../addons/listening/listening.svelte";
import { openFad } from "../addons/fad/state.svelte";

export function musicKeys(e: KeyboardEvent, extra: (e: KeyboardEvent) => boolean = () => false): void {
  const t = e.target as HTMLElement | null;
  if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable)) return;
  if (e.ctrlKey || e.metaKey || e.altKey) return;
  if (extra(e)) {
    e.preventDefault();
    return;
  }
  const s = ms.status;
  switch (e.key) {
    case " ":
      if (t?.tagName === "BUTTON") return;
      toggle();
      break;
    case "ArrowRight":
      if (e.shiftKey) next();
      else seek(Math.min((s?.duration ?? 0) - 0.5, ms.pos + 5));
      break;
    case "ArrowLeft":
      if (e.shiftKey) prev();
      else seek(Math.max(0, ms.pos - 5));
      break;
    case "ArrowUp":
      setVolume(Math.min(1, (s?.volume ?? 0.8) + 0.05));
      break;
    case "ArrowDown":
      setVolume(Math.max(0, (s?.volume ?? 0.8) - 0.05));
      break;
    case "[":
    case "]":
    case "\\":
    case "m":
      if (!app.ui.addons.installed.includes(LOOP) || !s?.track) return;
      if (e.key === "[") setA();
      else if (e.key === "]") setB();
      else if (e.key === "m") addBookmark();
      else clearLoop();
      break;
    case "f":
      if (!app.ui.addons.installed.includes("full-app-display") || !s?.track) return;
      openFad();
      break;
    default:
      return;
  }
  e.preventDefault();
}
