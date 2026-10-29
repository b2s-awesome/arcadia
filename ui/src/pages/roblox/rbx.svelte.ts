// Roblox+ state shared by the Roblox page's sections: accounts, play
// history, and what auto-rejoin is doing.
import { api, on } from "../../lib/api";
import { toast } from "../../lib/state.svelte";
import type { RejoinEvent, RobloxAccounts, RobloxHistory, RobloxPlayed } from "../../lib/types";

export const rbx = $state({
  accounts: { list: [], default: null } as RobloxAccounts,
  history: { visits: [], games: [] } as RobloxHistory,
  /** Account the Play button uses: null = default, 0 = the Roblox app's own sign-in. */
  picked: null as number | null,
  signingIn: false,
  rejoin: null as RejoinEvent | null,
  /** Game open in the server browser. */
  serversFor: null as number | null,
  loaded: false,
});

export async function loadAccounts() {
  rbx.accounts = await api.robloxAccounts();
}
export async function loadHistory() {
  rbx.history = await api.robloxHistory();
}

let started = false;
/** Load everything once and keep it fresh from backend events. */
export function startRbx() {
  if (started) return;
  started = true;
  Promise.all([loadAccounts(), loadHistory()]).then(() => (rbx.loaded = true));
  on("roblox-history", () => loadHistory());
  let tick: ReturnType<typeof setInterval> | undefined;
  on<RejoinEvent>("roblox-rejoin", (e) => {
    clearInterval(tick);
    rbx.rejoin = e.state === "done" || e.state === "cancelled" || e.state === "limit" ? null : { ...e };
    // Count down on screen; the backend only says how long at the start.
    if (e.state === "waiting") {
      tick = setInterval(() => {
        if (rbx.rejoin?.state === "waiting" && rbx.rejoin.seconds > 0) rbx.rejoin.seconds--;
        else clearInterval(tick);
      }, 1000);
    }
    if (e.state === "failed") toast(`Couldn't rejoin: ${e.message}`, "error");
    if (e.state === "limit") toast("Auto-rejoin stopped after five rejoins in half an hour", "error");
    if (e.state === "cancelled") toast("Rejoin cancelled");
  });
}

/** The account Play uses right now. */
export const effectiveAccount = () => (rbx.picked === null ? rbx.accounts.default : rbx.picked || null);
export const accountById = (id: number | null) => rbx.accounts.list.find((a) => a.id === id) ?? null;

export async function play(place: number | null = null, job: string | null = null, account: number | null = rbx.picked) {
  try {
    await api.robloxPlay(account, place, job);
  } catch (e) {
    toast(String(e), "error");
  }
}

export async function signIn() {
  rbx.signingIn = true;
  try {
    const a = await api.robloxAccountLogin();
    await loadAccounts();
    toast(`Signed in as ${a.displayName} (@${a.name})`);
  } catch (e) {
    if (!String(e).includes("closed")) toast(String(e), "error");
  } finally {
    rbx.signingIn = false;
  }
}

export const gameName = (place: number) => rbx.history.games.find((g) => g.placeId === place)?.name || `Place ${place}`;
export const gameOf = (place: number): RobloxPlayed | null => rbx.history.games.find((g) => g.placeId === place) ?? null;

// ── formatting ──
export function hours(sec: number): string {
  if (sec < 60) return sec ? "<1m" : "0m";
  const h = Math.floor(sec / 3600);
  const m = Math.round((sec % 3600) / 60);
  return h ? (m ? `${h}h ${m}m` : `${h}h`) : `${m}m`;
}
export function ago(t: number | null): string {
  if (!t) return "never";
  const d = Date.now() / 1000 - t;
  if (d < 90) return "just now";
  if (d < 3600) return `${Math.round(d / 60)} min ago`;
  if (d < 86400) return `${Math.round(d / 3600)} h ago`;
  if (d < 86400 * 2) return "yesterday";
  if (d < 86400 * 30) return `${Math.round(d / 86400)} days ago`;
  return new Date(t * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
}

/** A steady colour for a game or person without a picture. */
export function hue(seed: number | string): number {
  const s = String(seed);
  let h = 7;
  for (const c of s) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return h % 360;
}
export const initials = (name: string) =>
  name
    .replace(/[^\p{L}\p{N} ]/gu, "")
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => w[0]!.toUpperCase())
    .join("") || "?";
