// The game library: what the launchers report, plus your own info about
// each game (favourites, collections, art, playtime). Shared by the Library
// page and Big Picture.
import { api, fileUrl, on } from "./api";
import { toast } from "./state.svelte";
import type { Game, GameCollection, GameMeta, Source } from "./types";

export const EMPTY_META: GameMeta = {
  favourite: false, hidden: false, collections: [], notes: "", launchArgs: "", before: "", after: "",
  cover: null, hero: null, logo: null, playtime: 0, sessions: 0, lastPlayed: null,
  beat: { main: null, extra: null, complete: null }, rename: null,
};

export const lib = $state({
  games: [] as Game[],
  launchers: [] as Source[],
  meta: {} as Record<string, GameMeta>,
  collections: [] as GameCollection[],
  steam: {} as Record<string, [number, number]>,
  arcadia: {} as Record<string, [number, number]>,
  loading: true,
  loaded: false,
  bigPicture: false,
});

export async function loadMeta() {
  const m = await api.libraryMeta();
  lib.meta = Object.fromEntries(Object.entries(m.meta.games).map(([k, v]) => [k, { ...EMPTY_META, ...v, beat: { ...EMPTY_META.beat, ...(v.beat ?? {}) } }]));
  lib.collections = m.meta.collections;
  lib.steam = m.steam;
  lib.arcadia = m.arcadia;
}

export async function refreshLibrary() {
  lib.loading = true;
  try {
    const [r] = await Promise.all([api.scanLibrary(), loadMeta()]);
    lib.games = r.games;
    lib.launchers = r.launchers;
    r.warnings.forEach((w) => toast(w, "error"));
    lib.loaded = true;
  } catch (e) {
    toast(String(e), "error");
  } finally {
    lib.loading = false;
  }
}

let listening = false;
export function startLibrary() {
  if (!lib.loaded && lib.loading) refreshLibrary();
  if (listening) return;
  listening = true;
  on<{ id: string; name: string; seconds: number }>("library-played", (p) => {
    loadMeta();
    if (p.seconds >= 60) toast(`Played ${p.name} for ${playtime(p.seconds)}`);
  });
}

export const metaOf = (id: string): GameMeta => lib.meta[id] ?? EMPTY_META;

export async function saveMeta(id: string, patch: Partial<GameMeta>) {
  const next = { ...metaOf(id), ...patch };
  lib.meta[id] = next;
  try {
    await api.libraryMetaSet(id, $state.snapshot(next) as GameMeta);
  } catch (e) {
    toast(String(e), "error");
  }
}

export async function saveCollections(list: GameCollection[]) {
  lib.collections = list;
  await api.libraryCollectionsSet($state.snapshot(list) as GameCollection[]);
  await loadMeta();
}

/** Display name: your rename, else the launcher's. */
export const nameOf = (g: Game) => metaOf(g.id).rename || g.name;

/** Total seconds played: Arcadia's tracking, Steam's own record, or the
 *  Roblox/Minecraft trackers, whichever knows more. */
export function secondsOf(g: Game): number {
  const m = metaOf(g.id).playtime;
  const s = (lib.steam[g.id]?.[0] ?? 0) * 60;
  const a = lib.arcadia[g.id]?.[0] ?? 0;
  return Math.max(m, s, a);
}
export function lastOf(g: Game): number {
  return Math.max(metaOf(g.id).lastPlayed ?? 0, lib.steam[g.id]?.[1] ?? 0, lib.arcadia[g.id]?.[1] ?? 0);
}

/** Steam's cached art is a local file; everything else is a URL. */
export const art = (u: string | null | undefined) => (!u ? null : /^(https?:|data:)/.test(u) ? u : fileUrl(u));
export const coverOf = (g: Game) => art(metaOf(g.id).cover ?? g.cover);
export const heroOf = (g: Game) => art(metaOf(g.id).hero ?? g.hero);
export const logoOf = (g: Game) => art(metaOf(g.id).logo);

export function playtime(sec: number): string {
  if (sec < 60) return sec ? "under a minute" : "not played";
  const h = Math.floor(sec / 3600);
  const m = Math.round((sec % 3600) / 60);
  if (h >= 100) return `${h} h`;
  return h ? (m ? `${h} h ${m} min` : `${h} h`) : `${m} min`;
}
export function when(t: number): string {
  if (!t) return "never";
  const d = Date.now() / 1000 - t;
  if (d < 3600) return "just now";
  if (d < 86400) return `${Math.round(d / 3600)} h ago`;
  if (d < 86400 * 2) return "yesterday";
  if (d < 86400 * 30) return `${Math.round(d / 86400)} days ago`;
  return new Date(t * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
}

export async function launch(g: Game) {
  try {
    await api.launchGame(g.id);
    if (g.source !== "roblox" && g.source !== "minecraft") toast(`Opening ${nameOf(g)}`);
  } catch (e) {
    toast(String(e), "error");
  }
}
