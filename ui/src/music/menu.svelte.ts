// One context menu for the Music page (right-click or the ⋯ button).
import type { Track } from "../lib/types";
import { streaming } from "../lib/api";
import { app, toast } from "../lib/state.svelte";
import { addToPlaylist, addToQueue, go, albumKey, ms, newPlaylist, playNext, setFavourite } from "./store.svelte";

export interface MenuItem {
  label: string;
  icon?: string;
  run?: () => void;
  sub?: MenuItem[];
  danger?: boolean;
  sep?: boolean;
}

export const menu = $state({ open: false, x: 0, y: 0, items: [] as MenuItem[] });

export function openMenu(e: MouseEvent, items: MenuItem[]) {
  e.preventDefault();
  e.stopPropagation();
  const r = (e.currentTarget as HTMLElement | null)?.getBoundingClientRect();
  // From a button: under it. From a right-click: at the pointer.
  const fromButton = e.type === "click" && r;
  menu.x = fromButton ? r!.right : e.clientX;
  menu.y = fromButton ? r!.bottom + 4 : e.clientY;
  menu.items = items;
  menu.open = true;
}

/** Ask for a name (the Music page shows the prompt). */
export const prompt = $state({ open: false, title: "", value: "", resolve: null as ((v: string | null) => void) | null });
export function ask(title: string, value = ""): Promise<string | null> {
  return new Promise((resolve) => {
    prompt.title = title;
    prompt.value = value;
    prompt.resolve = resolve;
    prompt.open = true;
  });
}

export function playlistItems(tracks: Track[]): MenuItem[] {
  return [
    { label: "New playlist…", icon: "plus", run: async () => {
      const name = await ask("New playlist", tracks.length === 1 ? tracks[0].album : "");
      if (name === null) return;
      const id = await newPlaylist(name, tracks);
      go({ kind: "playlist", id });
    } },
    ...ms.playlists.map((p) => ({ label: p.name, run: () => addToPlaylist(p.id, tracks) })),
  ];
}

export async function downloadTracks(tracks: Track[]) {
  try {
    const n = await streaming.download(tracks.filter((t) => t.origin === "stream").map((t) => t.id));
    toast(n ? `Downloading ${n === 1 ? `“${tracks[0].title}”` : `${n} songs`}. Progress is under Downloads.` : "Already downloaded or downloading.");
  } catch (e) {
    toast(String(e), "error");
  }
}

/** The usual actions for one or more tracks. */
export function trackMenu(tracks: Track[], extra: MenuItem[] = []): MenuItem[] {
  const one = tracks.length === 1 ? tracks[0] : null;
  return [
    { label: "Play next", icon: "queue", run: () => playNext(tracks) },
    { label: "Add to queue", icon: "plus", run: () => addToQueue(tracks) },
    { label: "Add to playlist", icon: "music", sub: playlistItems(tracks) },
    ...(app.ui.addons.installed.includes("streaming") && tracks.some((t) => t.origin === "stream")
      ? [{ label: tracks.length === 1 ? "Download" : "Download these", icon: "download", run: () => downloadTracks(tracks) }]
      : []),
    ...(one ? [
      { label: one.favourite ? "Remove from favourites" : "Add to favourites", icon: "heart", run: () => setFavourite(one, !one.favourite) },
      { label: "", sep: true },
      { label: "Go to album", run: () => go({ kind: "album", key: albumKey(one) }) },
      { label: "Go to artist", run: () => go({ kind: "artist", name: one.albumArtist }) },
    ] : []),
    ...extra,
  ];
}
