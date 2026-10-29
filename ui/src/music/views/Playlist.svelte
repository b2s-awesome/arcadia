<script lang="ts">
  import { music } from "../../lib/api";
  import { toast } from "../../lib/state.svelte";
  import type { Track } from "../../lib/types";
  import TrackList from "../TrackList.svelte";
  import Collection from "./Collection.svelte";
  import { ask } from "../menu.svelte";
  import { back, ms, play, playlistTracks, reloadPlaylists } from "../store.svelte";

  let { id }: { id: number } = $props();
  const pl = $derived(ms.playlists.find((p) => p.id === id) ?? null);
  let tracks: Track[] = $state([]);
  async function load() {
    tracks = await playlistTracks(id);
  }
  $effect(() => {
    void id;
    void ms.tracks.length;
    load();
  });

  async function save(next: Track[]) {
    tracks = next;
    await music.playlistSet(id, next.map((t) => t.id));
    reloadPlaylists();
  }
  const reorder = (from: number, to: number) => {
    const v = [...tracks];
    const [t] = v.splice(from, 1);
    v.splice(to, 0, t);
    save(v);
  };
  const removeAt = (i: number) => save(tracks.filter((_, j) => j !== i));
  async function rename() {
    const name = await ask("Rename playlist", pl?.name ?? "");
    if (name === null || !name) return;
    await music.playlistRename(id, name);
    reloadPlaylists();
  }
  async function del() {
    await music.playlistDelete(id);
    await reloadPlaylists();
    toast(`Deleted “${pl?.name}”. The songs are still in your library.`);
    back();
  }
</script>

{#if pl}
  <Collection kind="Playlist" title={pl.name} art={tracks[0]?.art ?? null} color={tracks[0]?.artColor ?? null} {tracks}
    more={[{ label: "", sep: true }, { label: "Rename", run: rename }, { label: "Delete playlist", danger: true, run: del }]} />
  {#if !tracks.length}
    <p class="muted">Empty for now. Right-click any song and choose <b>Add to playlist</b>.</p>
  {/if}
  <TrackList {tracks} numbered onplay={(i) => play(tracks, i)} {reorder} extra={(_, i) => [{ label: "Remove from playlist", danger: true, run: () => removeAt(i) }]} />
{:else}
  <p class="muted">That playlist was deleted.</p>
{/if}
