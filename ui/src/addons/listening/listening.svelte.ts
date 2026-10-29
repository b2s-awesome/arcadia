// The listening addons: Trash Bin + Auto Skip, Shuffle+, Loopy Loop +
// Bookmarks and Sort & Play. Small, so they share one module; each part
// only shows when its addon is installed.
import { addons, music } from "../../lib/api";
import { app, toast } from "../../lib/state.svelte";
import type { ShuffleMode, Track } from "../../lib/types";
import { lib, ms, play, seek } from "../../music/store.svelte";

export const TRASH = "trash-bin";
export const SHUFFLE = "shuffle-plus";
export const LOOP = "loopy-loop";
export const SORT = "sort-play";

export const isOn = (id: string) => app.ui.addons.installed.includes(id);

const run = (p: Promise<unknown>) => p.catch((e) => toast(String(e), "error"));

// ── Trash Bin + Auto Skip ───────────────────────────────────

export interface SkipRules {
  trashSongs: number[];
  trashArtists: string[];
  trashed: boolean;
  explicit: boolean;
  short: boolean;
  live: boolean;
  remix: boolean;
}
const NO_RULES: SkipRules = { trashSongs: [], trashArtists: [], trashed: true, explicit: false, short: false, live: false, remix: false };

export const lx = $state({
  rules: { ...NO_RULES } as SkipRules,
  bookmarks: [] as Bookmark[],
  /** A set but not yet B. */
  pendingA: null as number | null,
});

const loaded = new Set<string>();
/** Read the trash and bookmarks, once each addon is installed. */
export async function initListening(): Promise<void> {
  if (isOn(TRASH) && !loaded.has(TRASH)) {
    loaded.add(TRASH);
    const r = await addons.get<Partial<SkipRules>>(TRASH, "rules").catch(() => null);
    lx.rules = { ...NO_RULES, ...(r ?? {}) };
  }
  if (isOn(LOOP) && !loaded.has(LOOP)) {
    loaded.add(LOOP);
    lx.bookmarks = (await addons.get<Bookmark[]>(LOOP, "bookmarks").catch(() => null)) ?? [];
  }
}

export async function saveRules() {
  await run(addons.set(TRASH, "rules", $state.snapshot(lx.rules)).then(() => music.reloadRules()));
}

export const isTrashed = (t: Track) => lx.rules.trashSongs.includes(t.id) || lx.rules.trashArtists.some((a) => a.toLowerCase() === t.artist.toLowerCase());

export async function trashSongs(ts: Track[]) {
  await initListening();
  const add = ts.map((t) => t.id).filter((id) => !lx.rules.trashSongs.includes(id));
  if (!add.length) return;
  lx.rules.trashSongs = [...lx.rules.trashSongs, ...add];
  await saveRules();
  toast(ts.length === 1 ? `“${ts[0].title}” is in the trash: it won't come up by itself` : `${add.length} songs are in the trash`);
  // Playing one now? Move on.
  const cur = ms.status?.track?.id;
  if (cur !== undefined && add.includes(cur) && ms.status?.playing) run(music.next());
}
export async function trashArtist(name: string) {
  await initListening();
  if (lx.rules.trashArtists.some((a) => a.toLowerCase() === name.toLowerCase())) return;
  lx.rules.trashArtists = [...lx.rules.trashArtists, name];
  await saveRules();
  toast(`${name} is in the trash`);
  if (ms.status?.track && ms.status.track.artist.toLowerCase() === name.toLowerCase() && ms.status.playing) run(music.next());
}
export async function untrashSong(id: number) {
  lx.rules.trashSongs = lx.rules.trashSongs.filter((x) => x !== id);
  await saveRules();
}
export async function untrashArtist(name: string) {
  lx.rules.trashArtists = lx.rules.trashArtists.filter((x) => x !== name);
  await saveRules();
}

// ── Shuffle+ ────────────────────────────────────────────────

export const SHUFFLE_MODES: { id: ShuffleMode; label: string; detail: string }[] = [
  { id: "random", label: "True random", detail: "Every order equally likely." },
  { id: "spreadArtists", label: "Spread artists", detail: "The same artist twice in a row as rarely as possible." },
  { id: "fewerRepeats", label: "Fewer repeats", detail: "Songs you've heard lately drift to the end." },
  { id: "lessPlayed", label: "Favour less played", detail: "Songs you play least come up sooner." },
];

export function setShuffleMode(mode: ShuffleMode) {
  if (ms.status) ms.status.shuffleMode = mode;
  run(music.shuffleMode(mode).then(() => (mode !== "random" && !ms.status?.shuffle ? music.shuffle(true) : undefined)));
}

// ── Loopy Loop + Bookmarks ──────────────────────────────────

export interface Bookmark {
  id: string;
  trackId: number;
  at: number;
  label: string;
  title: string;
  artist: string;
}

export function setA(at = ms.pos) {
  const b = ms.status?.loopAb?.[1];
  if (b !== undefined && at < b - 0.1) {
    run(music.loop(at, b));
  } else {
    lx.pendingA = at;
    toast("A set. Now set B where the loop should end.");
  }
}
export function setB(at = ms.pos) {
  const a = lx.pendingA ?? ms.status?.loopAb?.[0] ?? null;
  if (a === null) {
    toast("Set A first (the [ key, or right-click the seek bar).", "error");
    return;
  }
  if (at <= a + 0.1) {
    toast("B has to come after A.", "error");
    return;
  }
  lx.pendingA = null;
  run(music.loop(a, at));
}
export function clearLoop() {
  lx.pendingA = null;
  run(music.loop(null, null));
}

async function saveBookmarks() {
  await run(addons.set(LOOP, "bookmarks", $state.snapshot(lx.bookmarks)));
}
export async function addBookmark(label?: string) {
  const t = ms.status?.track;
  if (!t) return;
  await initListening();
  const at = Math.round(ms.pos * 10) / 10;
  lx.bookmarks = [{ id: crypto.randomUUID(), trackId: t.id, at, label: label?.trim() || `At ${fmt(at)}`, title: t.title, artist: t.artist }, ...lx.bookmarks].slice(0, 500);
  await saveBookmarks();
  toast(`Bookmarked ${t.title} at ${fmt(at)}`);
}
export async function removeBookmark(id: string) {
  lx.bookmarks = lx.bookmarks.filter((b) => b.id !== id);
  await saveBookmarks();
}
export async function renameBookmark(id: string, label: string) {
  const b = lx.bookmarks.find((x) => x.id === id);
  if (!b) return;
  b.label = label.trim() || b.label;
  await saveBookmarks();
}
export async function playBookmark(b: Bookmark) {
  if (ms.status?.track?.id === b.trackId) {
    seek(b.at);
    if (!ms.status.playing) run(music.pause(false));
    return;
  }
  const t = lib.byId.get(b.trackId);
  if (!t) {
    toast("That song isn't in your library any more.", "error");
    return;
  }
  await run(music.play([t.id], 0));
  // Give the engine a moment to open the file, then jump.
  setTimeout(() => seek(b.at), 250);
}
export const bookmarksFor = (id: number | undefined) => (id === undefined ? [] : lx.bookmarks.filter((b) => b.trackId === id));

function fmt(s: number) {
  const m = Math.floor(s / 60);
  return `${m}:${String(Math.floor(s % 60)).padStart(2, "0")}`;
}

// ── Sort & Play ─────────────────────────────────────────────

export type SortHow = "most" | "least" | "added" | "recent" | "longest" | "shortest" | "title" | "artist" | "albumShuffle" | "favouritesFirst";
export const SORTS: { id: SortHow; label: string }[] = [
  { id: "most", label: "Most played" },
  { id: "least", label: "Least played" },
  { id: "recent", label: "Recently played" },
  { id: "added", label: "Recently added" },
  { id: "longest", label: "Longest first" },
  { id: "shortest", label: "Shortest first" },
  { id: "title", label: "Title A–Z" },
  { id: "artist", label: "Artist A–Z" },
  { id: "favouritesFirst", label: "Favourites first" },
  { id: "albumShuffle", label: "Album shuffle" },
];

export function sorted(tracks: Track[], how: SortHow): Track[] {
  const v = [...tracks];
  const by = (f: (t: Track) => number) => v.sort((a, b) => f(b) - f(a));
  const az = (f: (t: Track) => string) => v.sort((a, b) => f(a).localeCompare(f(b), undefined, { sensitivity: "base" }));
  switch (how) {
    case "most": return by((t) => t.playCount);
    case "least": return by((t) => -t.playCount);
    case "recent": return by((t) => t.lastPlayed ?? 0);
    case "added": return by((t) => t.addedAt);
    case "longest": return by((t) => t.duration);
    case "shortest": return by((t) => -t.duration);
    case "title": return az((t) => t.title);
    case "artist": return az((t) => `${t.artist}\u0000${t.album}\u0000${String(t.discNo ?? 1).padStart(3, "0")}${String(t.trackNo ?? 999).padStart(4, "0")}`);
    case "favouritesFirst": return by((t) => (t.favourite ? 1 : 0));
    case "albumShuffle": {
      // Albums in random order, songs in album order.
      const groups = new Map<string, Track[]>();
      for (const t of v) {
        const k = `${t.albumArtist}\u0000${t.album}`;
        groups.set(k, [...(groups.get(k) ?? []), t]);
      }
      const albums = [...groups.values()];
      for (let i = albums.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1));
        [albums[i], albums[j]] = [albums[j], albums[i]];
      }
      return albums.flatMap((a) => a.sort((x, y) => (x.discNo ?? 1) - (y.discNo ?? 1) || (x.trackNo ?? 999) - (y.trackNo ?? 999)));
    }
  }
}

/** Sort, then play from the top in that order (shuffle off). */
export async function sortAndPlay(tracks: Track[], how: SortHow) {
  if (!tracks.length) return;
  if (ms.status?.shuffle) await run(music.shuffle(false));
  play(sorted(tracks, how), 0);
  toast(`Playing ${tracks.length} songs, ${SORTS.find((s) => s.id === how)!.label.toLowerCase()}`);
}

export const queueTool = (tool: "dedupe" | "clearPlayed" | "shuffleRest") => run(music.queueTool(tool));
