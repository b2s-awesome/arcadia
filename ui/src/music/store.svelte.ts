// Music page state, shared by both layouts, the top-bar chip and the
// launcher. One copy per window.
import { fileUrl, music, on } from "../lib/api";
import { toast } from "../lib/state.svelte";
import type { MusicPlaylist, MusicScanProgress, MusicStatus, RepeatMode, Track } from "../lib/types";

export interface Album {
  key: string;
  title: string;
  artist: string;
  year: number | null;
  tracks: Track[];
  art: string | null;
  artColor: string | null;
  duration: number;
  addedAt: number;
  lastPlayed: number;
  plays: number;
}
export interface Artist {
  name: string;
  albums: Album[];
  tracks: Track[];
  art: string | null;
}

/** Where the Music page is. */
export type MusicView =
  | { kind: "home" }
  | { kind: "albums" }
  | { kind: "artists" }
  | { kind: "songs" }
  | { kind: "playlists" }
  | { kind: "favourites" }
  | { kind: "album"; key: string }
  | { kind: "artist"; name: string }
  | { kind: "playlist"; id: number }
  | { kind: "search"; q: string }
  | { kind: "online" }
  | { kind: "downloads" }
  | { kind: "lyrics" }
  | { kind: "stats" }
  | { kind: "duplicates" };

export const ms = $state({
  loaded: false,
  tracks: [] as Track[],
  playlists: [] as MusicPlaylist[],
  status: null as MusicStatus | null,
  queue: { index: null as number | null, tracks: [] as Track[] },
  scan: null as MusicScanProgress | null,
  view: { kind: "home" } as MusicView,
  history: [] as MusicView[],
  nowPlaying: false,
  customise: false,
  /** Position, smoothed between backend updates. */
  pos: 0,
});

export const artUrl = (p: string | null) => (p ? fileUrl(p) : null);
export const albumKey = (t: Track) => `${t.albumArtist}\u0000${t.album}`;

/** Albums and artists, rebuilt when the library changes. */
export const lib = $state({ albums: [] as Album[], artists: [] as Artist[], byId: new Map<number, Track>(), albumByKey: new Map<string, Album>(), tracks: [] as Track[], streamed: [] as Track[] });

function rebuild() {
  const byId = new Map<number, Track>();
  const albums = new Map<string, Album>();
  // Streamed songs aren't part of the library until they're downloaded.
  const own: Track[] = [];
  const streamed: Track[] = [];
  for (const t of ms.tracks) {
    byId.set(t.id, t);
    if (t.origin === "stream") {
      streamed.push(t);
      continue;
    }
    own.push(t);
    const k = albumKey(t);
    let a = albums.get(k);
    if (!a) {
      a = { key: k, title: t.album, artist: t.albumArtist, year: t.year, tracks: [], art: t.art, artColor: t.artColor, duration: 0, addedAt: 0, lastPlayed: 0, plays: 0 };
      albums.set(k, a);
    }
    a.tracks.push(t);
    a.duration += t.duration;
    a.addedAt = Math.max(a.addedAt, t.addedAt);
    a.lastPlayed = Math.max(a.lastPlayed, t.lastPlayed ?? 0);
    a.plays += t.playCount;
    if (!a.art && t.art) {
      a.art = t.art;
      a.artColor = t.artColor;
    }
    if (!a.year && t.year) a.year = t.year;
  }
  for (const a of albums.values()) {
    a.tracks.sort((x, y) => (x.discNo ?? 1) - (y.discNo ?? 1) || (x.trackNo ?? 999) - (y.trackNo ?? 999) || x.title.localeCompare(y.title));
  }
  const albumList = [...albums.values()].sort((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: "base" }));
  const artists = new Map<string, Artist>();
  for (const a of albumList) {
    let ar = artists.get(a.artist);
    if (!ar) artists.set(a.artist, (ar = { name: a.artist, albums: [], tracks: [], art: a.art }));
    ar.albums.push(a);
    ar.tracks.push(...a.tracks);
  }
  lib.byId = byId;
  lib.tracks = own;
  lib.streamed = streamed.sort((a, b) => (b.lastPlayed ?? b.addedAt) - (a.lastPlayed ?? a.addedAt));
  lib.albumByKey = albums;
  lib.albums = albumList;
  lib.artists = [...artists.values()].sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }));
}

export async function reloadLibrary() {
  ms.tracks = await music.library();
  rebuild();
}
export async function reloadPlaylists() {
  ms.playlists = await music.playlists();
}
export async function reloadQueue() {
  ms.queue = await music.queue();
}

let playerStarted = false;
let libraryStarted = false;
let anchor = { pos: 0, at: 0 };

/** Status and events only: cheap, so the top-bar chip works from startup. */
export async function initPlayer() {
  if (playerStarted) return;
  playerStarted = true;
  on<MusicStatus>("music-status", (s) => {
    setStatus(s);
    if (libraryStarted) reloadQueue();
  });
  on<number>("music-position", (p) => {
    if (ms.status) ms.status.pos = p;
    anchor = { pos: p, at: performance.now() };
    ms.pos = p;
    kick();
  });
  on<string>("music-error", (m) => toast(m, "error"));
  on<MusicScanProgress>("music-scan", (p) => (ms.scan = p.total && p.done >= p.total ? null : p));
  on<null>("music-library-changed", () => libraryStarted && reloadLibrary());
  on<null>("music-queue-changed", () => libraryStarted && reloadQueue());
  try {
    setStatus(await music.status());
  } catch {
    // No player (no sound card): the Music page explains.
  }
}

/** Everything the Music page needs; loaded the first time it opens. */
export async function initMusic() {
  await initPlayer();
  if (libraryStarted) return;
  libraryStarted = true;
  try {
    await Promise.all([reloadLibrary(), reloadPlaylists(), reloadQueue()]);
  } catch (e) {
    toast(String(e), "error");
  }
  ms.loaded = true;
}

// Smooth the progress bar between the backend's 4-a-second updates. Runs
// only while something plays, at most ~20 updates a second.
let raf = 0;
function smooth() {
  const s = ms.status;
  if (!s?.playing) {
    raf = 0;
    return;
  }
  const p = Math.min(s.duration || Infinity, anchor.pos + (performance.now() - anchor.at) / 1000);
  if (Math.abs(p - ms.pos) >= 0.05) ms.pos = p;
  raf = requestAnimationFrame(smooth);
}
function kick() {
  if (!raf && ms.status?.playing && typeof requestAnimationFrame !== "undefined") raf = requestAnimationFrame(smooth);
}

function setStatus(s: MusicStatus) {
  ms.status = s;
  anchor = { pos: s.pos, at: performance.now() };
  ms.pos = s.pos;
  kick();
  // Keep the library's copy of the playing track fresh (play counts).
  if (s.track) {
    const i = ms.tracks.findIndex((t) => t.id === s.track!.id);
    if (i >= 0) ms.tracks[i] = s.track;
  }
}

// ── navigation ─────────────────────────────────────────────

export function go(v: MusicView) {
  ms.history.push($state.snapshot(ms.view) as MusicView);
  if (ms.history.length > 50) ms.history.shift();
  ms.view = v;
  ms.nowPlaying = false;
}
export function back() {
  const v = ms.history.pop();
  if (v) ms.view = v;
}

// ── actions ────────────────────────────────────────────────

const run = (p: Promise<unknown>) => p.catch((e) => toast(String(e), "error"));

export const play = (tracks: Track[], start = 0) => tracks.length && run(music.play(tracks.map((t) => t.id), start));
export const shuffleAll = (tracks: Track[]) => {
  if (!tracks.length) return;
  run(music.shuffle(true).then(() => music.play(tracks.map((t) => t.id), Math.floor(Math.random() * tracks.length))));
};
export const toggle = () => run(music.pause(null));
export const next = () => run(music.next());
export const prev = () => run(music.prev());
export const seek = (s: number) => {
  ms.pos = s;
  anchor = { pos: s, at: performance.now() };
  run(music.seek(s));
};
let volTimer: ReturnType<typeof setTimeout> | undefined;
export const setVolume = (v: number) => {
  if (ms.status) ms.status.volume = v;
  clearTimeout(volTimer);
  volTimer = setTimeout(() => run(music.volume(v)), 30);
};
export const setShuffle = (onOff: boolean) => run(music.shuffle(onOff));
export const cycleRepeat = () => {
  const order: RepeatMode[] = ["off", "all", "one"];
  const cur = ms.status?.repeat ?? "off";
  run(music.repeat(order[(order.indexOf(cur) + 1) % 3]));
};
export const playNext = (tracks: Track[]) => run(music.queueAdd(tracks.map((t) => t.id), true).then(() => toast(tracks.length === 1 ? `“${tracks[0].title}” plays next` : `${tracks.length} songs play next`)));
export const addToQueue = (tracks: Track[]) => run(music.queueAdd(tracks.map((t) => t.id), false).then(() => toast(tracks.length === 1 ? `Added “${tracks[0].title}” to the queue` : `Added ${tracks.length} songs to the queue`)));

export async function setFavourite(t: Track, onOff: boolean) {
  const i = ms.tracks.findIndex((x) => x.id === t.id);
  if (i >= 0) ms.tracks[i].favourite = onOff;
  if (ms.status?.track?.id === t.id) ms.status.track.favourite = onOff;
  rebuild();
  await run(music.favourite(t.id, onOff));
}

export async function addToPlaylist(id: number, tracks: Track[]) {
  await run(music.playlistAdd(id, tracks.map((t) => t.id)));
  await reloadPlaylists();
  const name = ms.playlists.find((p) => p.id === id)?.name ?? "the playlist";
  toast(`Added ${tracks.length === 1 ? `“${tracks[0].title}”` : `${tracks.length} songs`} to ${name}`);
}
export async function newPlaylist(name: string, tracks: Track[] = []) {
  const id = await music.playlistCreate(name, tracks.map((t) => t.id));
  await reloadPlaylists();
  return id;
}

export async function playlistTracks(id: number): Promise<Track[]> {
  const ids = await music.playlistItems(id);
  return ids.map((i) => lib.byId.get(i)).filter((t): t is Track => !!t);
}

// ── formatting ─────────────────────────────────────────────

export function time(s: number): string {
  if (!isFinite(s) || s < 0) s = 0;
  const m = Math.floor(s / 60);
  const ss = Math.floor(s % 60);
  if (m >= 60) return `${Math.floor(m / 60)}:${String(m % 60).padStart(2, "0")}:${String(ss).padStart(2, "0")}`;
  return `${m}:${String(ss).padStart(2, "0")}`;
}
export function longTime(s: number): string {
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min`;
  return `${Math.floor(m / 60)} h ${m % 60} min`;
}
export function quality(t: Track): string {
  const parts = [t.format];
  if (t.bitDepth && t.sampleRate) parts.push(`${t.bitDepth}/${+(t.sampleRate / 1000).toFixed(1)}`);
  else if (t.bitrate) parts.push(`${t.bitrate} kbps`);
  return parts.join(" ");
}

/** Search across songs, albums and artists. */
export function search(q: string) {
  const n = q.trim().toLowerCase();
  if (!n) return { tracks: [], albums: [], artists: [] };
  const words = n.split(/\s+/);
  const hit = (s: string) => {
    const l = s.toLowerCase();
    return words.every((w) => l.includes(w));
  };
  return {
    tracks: lib.tracks.filter((t) => hit(`${t.title} ${t.artist} ${t.album}`)).slice(0, 200),
    albums: lib.albums.filter((a) => hit(`${a.title} ${a.artist}`)).slice(0, 24),
    artists: lib.artists.filter((a) => hit(a.name)).slice(0, 12),
  };
}
