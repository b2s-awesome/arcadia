// Shared Minecraft launcher state, used by both the in-hub page and the
// standalone window.
import { mc, on } from "./api";
import { toast } from "./state.svelte";
import type { McAccount, McCrashInfo, McInstance, McProgress } from "./types";

export type McTab = "instances" | "modpacks" | "skins" | "import" | "security" | "accounts";

const MAX_LOG = 3000;

export const mcs = $state({
  tab: ((typeof location !== "undefined" && new URLSearchParams(location.search).get("mctab")) || "instances") as McTab,
  instances: [] as McInstance[],
  selected: null as string | null,
  running: [] as string[],
  accounts: [] as McAccount[],
  progress: null as McProgress | null,
  logs: {} as Record<string, string[]>,
  consoleFor: null as string | null,
  /** The last crash, explained. */
  crash: null as McCrashInfo | null,
  /** Ask the instance view to open a section (e.g. "logs"). */
  openSection: null as string | null,
  loaded: false,
});

export async function refreshInstances() {
  mcs.instances = await mc.instances();
  mcs.instances.sort((a, b) => (b.lastPlayed ?? b.created) - (a.lastPlayed ?? a.created));
  if (!mcs.selected || !mcs.instances.some((i) => i.id === mcs.selected)) mcs.selected = mcs.instances[0]?.id ?? null;
}

export async function refreshAccounts() {
  mcs.accounts = await mc.accounts();
}

let started = false;
/** Load everything once and subscribe to backend events. */
export async function initMinecraft() {
  if (started) return;
  started = true;
  await Promise.all([refreshInstances(), refreshAccounts(), mc.running().then((r) => (mcs.running = r))]);
  mcs.loaded = true;
  on<McProgress>("mc-progress", (p) => {
    mcs.progress = p.total > 0 && p.done >= p.total ? null : p;
  });
  on<string>("mc-started", (id) => {
    if (!mcs.running.includes(id)) mcs.running.push(id);
    mcs.progress = null;
    mcs.logs[id] = [];
  });
  on<{ instance: string; line: string }>("mc-log", ({ instance, line }) => {
    const l = (mcs.logs[instance] ??= []);
    l.push(line);
    if (l.length > MAX_LOG) l.splice(0, l.length - MAX_LOG);
  });
  on<McCrashInfo>("mc-crash", (c) => (mcs.crash = c));
  on<{ instance: string; code: number | null; seconds: number }>("mc-exited", ({ instance, code }) => {
    mcs.running = mcs.running.filter((r) => r !== instance);
    const name = mcs.instances.find((i) => i.id === instance)?.name ?? "Minecraft";
    if (code && code !== 0 && mcs.crash?.instance === instance) {
      // The crash dialog explains it.
    } else if (code && code !== 0) {
      toast(`${name} closed with an error (code ${code}). Open its console to see why.`, "error");
      mcs.consoleFor = instance;
    }
    refreshInstances();
  });
}

export const activeAccount = () => mcs.accounts.find((a) => a.active) ?? null;

export async function play(id: string) {
  try {
    mcs.progress = { task: "launch", message: "Getting ready", done: 0, total: 0 };
    await mc.play(id);
  } catch (e) {
    mcs.progress = null;
    toast(String(e), "error");
  }
}

export function playtime(secs: number): string {
  if (secs < 60) return secs ? "under a minute" : "never played";
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return h ? `${h} h ${m} min` : `${m} min`;
}

/** `secs` is a Unix timestamp in seconds, as stored by the backend. */
export function ago(secs: number | null): string {
  if (!secs) return "never";
  const d = Date.now() / 1000 - secs;
  if (d < 90) return "just now";
  if (d < 3600) return `${Math.round(d / 60)} min ago`;
  if (d < 86400) return `${Math.round(d / 3600)} h ago`;
  return `${Math.round(d / 86400)} days ago`;
}

/** Move the launcher into its own window (or back), reporting failures. */
export async function popOut() {
  try {
    await mc.popOut();
  } catch (e) {
    toast(String(e), "error");
  }
}
export async function dock() {
  try {
    await mc.dock();
  } catch (e) {
    toast(String(e), "error");
  }
}
