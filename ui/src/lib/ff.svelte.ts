// Shared FrameForge state for the Performance pages.
//
// Polling is reference-counted and tied to visible components: when no
// Performance page is on screen nothing is polled at all.
import { ff } from "./api";
import { toast } from "./state.svelte";
import type { FfConfig, MonitorSnapshot, Overview, Sample } from "./fftypes";

export type PerfTab = "dashboard" | "optimizer" | "monitor" | "profiles" | "gpu" | "nvidia" | "benchmarks";

const HISTORY = 90;

export const ffs = $state({
  tab: ((typeof location !== "undefined" && new URLSearchParams(location.search).get("perftab")) || "dashboard") as PerfTab,
  overview: null as Overview | null,
  loadingOverview: false,
  config: null as FfConfig | null,
  sample: null as Sample | null,
  history: [] as Sample[],
  snapshot: null as MonitorSnapshot | null,
});

export async function loadOverview(rescan = false) {
  ffs.loadingOverview = true;
  try {
    ffs.overview = await ff.overview(rescan);
  } catch (e) {
    toast(String(e), "error");
  } finally {
    ffs.loadingOverview = false;
  }
}

export async function loadConfig() {
  try {
    ffs.config = await ff.configGet();
  } catch (e) {
    toast(String(e), "error");
  }
}

export async function saveConfig(next: FfConfig) {
  try {
    ffs.config = await ff.configSet(next);
    return true;
  } catch (e) {
    toast(String(e), "error");
    return false;
  }
}

function poller(intervalMs: number, tick: () => Promise<void>) {
  let users = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let busy = false;
  const run = async () => {
    if (users === 0) return;
    // Skip work while the window is hidden; the next visible tick catches up.
    if (!busy && (typeof document === "undefined" || !document.hidden)) {
      busy = true;
      try { await tick(); } catch { /* shown by the page that asked */ } finally { busy = false; }
    }
    if (users > 0) timer = setTimeout(run, intervalMs);
  };
  /** Start polling; returns the function that stops this user's share. */
  return () => {
    users += 1;
    if (users === 1) run();
    return () => {
      users = Math.max(0, users - 1);
      if (users === 0) clearTimeout(timer);
    };
  };
}

/** System and GPU readings every 2 s while a page that shows them is open. */
export const useTelemetry = poller(2000, async () => {
  const s = await ff.telemetry();
  ffs.sample = s;
  ffs.history.push(s);
  if (ffs.history.length > HISTORY) ffs.history.splice(0, ffs.history.length - HISTORY);
});

/** Monitor snapshot twice a second while the Live Monitor is open. */
export const useMonitor = poller(500, async () => {
  ffs.snapshot = await ff.monitorSnapshot();
});

// ── formatting ──────────────────────────────────────────────────────────

export const DASH = "—";

export function num(v: number | null | undefined, digits = 0): string {
  return v === null || v === undefined || !Number.isFinite(v) ? DASH : v.toFixed(digits);
}

export function gb(bytes: number | null | undefined, digits = 1): string {
  return bytes === null || bytes === undefined ? DASH : (bytes / 1024 ** 3).toFixed(digits);
}

export function size(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined) return DASH;
  if (bytes >= 1024 ** 4) return `${(bytes / 1024 ** 4).toFixed(2)} TB`;
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(bytes >= 10 * 1024 ** 3 ? 0 : 1)} GB`;
  return `${Math.round(bytes / 1024 ** 2)} MB`;
}

export function duration(ms: number): string {
  const s = Math.floor(ms / 1000);
  const m = Math.floor(s / 60);
  return m > 0 ? `${m} min ${String(s % 60).padStart(2, "0")} s` : `${s} s`;
}

export function yesNo(v: boolean | null | undefined, yes = "On", no = "Off"): string {
  return v === null || v === undefined ? DASH : v ? yes : no;
}
