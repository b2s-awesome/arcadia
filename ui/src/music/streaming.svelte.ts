// State for the Streaming & downloads addon (Online and Downloads in Music).
import { on, streaming } from "../lib/api";
import { MUSIC_TABS } from "../lib/theme";
import { app, toast } from "../lib/state.svelte";
import type { DownloadJob, InstallProgress, OnlineItem, StreamingStatus, SyncedSource } from "../lib/types";
import { addToPlaylist, newPlaylist, ms, reloadLibrary, reloadPlaylists } from "./store.svelte";
import { music } from "../lib/api";

export const streamingOn = () => app.ui.addons.installed.includes("streaming");

/** Music's tabs as the user arranged them, minus ones whose addon is missing. */
export function visibleTabs() {
  const p = app.ui.music;
  return p.tabs
    .map((id) => MUSIC_TABS.find((t) => t.id === id)!)
    .filter((t) => t && (!t.addon || app.ui.addons.installed.includes(t.addon)) && !p.hiddenTabs.includes(t.id));
}

export const sx = $state({
  status: null as StreamingStatus | null,
  jobs: [] as DownloadJob[],
  sources: [] as SyncedSource[],
  installing: null as InstallProgress | null,
});

let started = false;
export async function initStreaming() {
  if (started || !streamingOn()) return;
  started = true;
  on<DownloadJob[]>("streaming-jobs", (j) => (sx.jobs = j));
  on<InstallProgress>("streaming-install", (p) => (sx.installing = p.stage === "done" ? null : p));
  await refresh();
}

export async function refresh() {
  try {
    const [status, jobs, sources] = await Promise.all([streaming.status(), streaming.jobs(), streaming.sources()]);
    sx.status = status;
    sx.jobs = jobs;
    sx.sources = sources;
  } catch (e) {
    toast(String(e), "error");
  }
}

export async function install(tool: "yt-dlp" | "ffmpeg") {
  sx.installing = { tool, done: 0, total: 0, stage: "starting" };
  try {
    await streaming.install(tool);
    toast(`${tool === "yt-dlp" ? "yt-dlp" : "ffmpeg"} is ready`);
  } catch (e) {
    toast(String(e), "error");
  }
  sx.installing = null;
  await refresh();
}

/** Online results → library entries, then the usual player actions. */
const ids = (items: OnlineItem[]) => streaming.tracks($state.snapshot(items) as OnlineItem[]);
const run = async (f: () => Promise<unknown>) => {
  try {
    await f();
  } catch (e) {
    toast(String(e), "error");
  }
};

export const playOnline = (items: OnlineItem[], start = 0) => run(async () => {
  const v = await ids(items);
  await music.play(v, start);
  reloadLibrary();
});
export const queueOnline = (items: OnlineItem[], next: boolean) => run(async () => {
  const v = await ids(items);
  await music.queueAdd(v, next);
  toast(next ? `${items.length === 1 ? `“${items[0].title}”` : `${items.length} songs`} play next` : `Added ${items.length === 1 ? `“${items[0].title}”` : `${items.length} songs`} to the queue`);
  reloadLibrary();
});
export const downloadOnline = (items: OnlineItem[]) => run(async () => {
  const v = await ids(items);
  const n = await streaming.download(v);
  toast(n ? `Downloading ${n === 1 ? `“${items[0].title}”` : `${n} songs`}` : "Already downloaded or downloading");
  reloadLibrary();
});
export const playlistOnline = (pid: number, items: OnlineItem[]) => run(async () => {
  const v = await ids(items);
  await reloadLibrary();
  await addToPlaylist(pid, v.map((id) => ms.tracks.find((t) => t.id === id)!).filter(Boolean));
});
export const newPlaylistOnline = (name: string, items: OnlineItem[]) => run(async () => {
  const v = await ids(items);
  await reloadLibrary();
  await newPlaylist(name, v.map((id) => ms.tracks.find((t) => t.id === id)!).filter(Boolean));
  toast(`Made “${name}”`);
});
export async function importLink(url: string, name: string | null, download: boolean): Promise<number | null> {
  try {
    const id = await streaming.import(url, name, download);
    await Promise.all([reloadLibrary(), reloadPlaylists(), refresh()]);
    return id;
  } catch (e) {
    toast(String(e), "error");
    return null;
  }
}

export function bytes(n: number): string {
  if (n < 1024 * 1024) return `${Math.round(n / 1024)} KB`;
  if (n < 1024 ** 3) return `${(n / 1024 ** 2).toFixed(n < 100 * 1024 ** 2 ? 1 : 0)} MB`;
  return `${(n / 1024 ** 3).toFixed(2)} GB`;
}
