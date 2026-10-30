<script lang="ts">
  import TrackList from "../TrackList.svelte";
  import Icon from "../../components/Icon.svelte";
  import { go, lib, ms, play, shuffleAll } from "../store.svelte";
  import type { Track } from "../../lib/types";
  import { app } from "../../lib/state.svelte";
  import SortPlay from "../../addons/listening/SortPlay.svelte";

  let { favourites = false }: { favourites?: boolean } = $props();
  type Sort = "title" | "artist" | "album" | "added" | "plays";
  let sort: Sort = $state("artist");
  const base = $derived(favourites ? ms.tracks.filter((t) => t.favourite) : lib.tracks);
  const cmp = (a: string, b: string) => a.localeCompare(b, undefined, { sensitivity: "base" });
  const sorted = $derived.by(() => {
    const v = [...base];
    const by: Record<Sort, (a: Track, b: Track) => number> = {
      title: (a, b) => cmp(a.title, b.title),
      artist: (a, b) => cmp(a.albumArtist, b.albumArtist) || cmp(a.album, b.album) || (a.trackNo ?? 0) - (b.trackNo ?? 0),
      album: (a, b) => cmp(a.album, b.album) || (a.trackNo ?? 0) - (b.trackNo ?? 0),
      added: (a, b) => b.addedAt - a.addedAt,
      plays: (a, b) => b.playCount - a.playCount,
    };
    return v.sort(by[sort]);
  });
</script>

<div class="bar">
  <h2>{favourites ? "Favourites" : "Songs"}</h2>
  <span class="muted">{base.length.toLocaleString()}</span>
  <span class="grow"></span>
  <button class="ghosty" onclick={() => shuffleAll(sorted)} disabled={!sorted.length}><Icon name="shuffle" size={15} /> Shuffle</button>
  {#if !favourites}<button class="ghosty" onclick={() => go({ kind: "duplicates" })} disabled={base.length < 2}><Icon name="copy" size={15} /> Duplicates</button>{/if}
  {#if app.ui.addons.installed.includes("sort-play")}<SortPlay tracks={sorted} />{/if}
  <label class="sort"><span class="muted">Sort</span>
    <select bind:value={sort}>
      <option value="artist">Artist</option><option value="title">Title</option><option value="album">Album</option>
      <option value="added">Recently added</option><option value="plays">Most played</option>
    </select>
  </label>
</div>
{#if !sorted.length}
  <p class="muted pad">{favourites ? "Tap the heart on any song to keep it here." : "No songs yet."}</p>
{/if}
<TrackList tracks={sorted} onplay={(i) => play(sorted, i)} />

<style>
  .bar { display: flex; align-items: center; gap: 0.7rem; margin-bottom: 1rem; flex-wrap: wrap; }
  .bar h2 { font-size: 1.5rem; }
  .ghosty { display: inline-flex; align-items: center; gap: 0.4rem; border-radius: 999px; padding: 0.4rem 0.9rem; background: transparent; }
  .sort { display: flex; align-items: center; gap: 0.4rem; font-size: 0.85rem; }
  .pad { padding: 1rem 0.6rem; }
</style>
