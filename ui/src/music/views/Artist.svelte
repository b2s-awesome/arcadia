<script lang="ts">
  import TrackList from "../TrackList.svelte";
  import AlbumGrid from "../AlbumGrid.svelte";
  import Collection from "./Collection.svelte";
  import { lib, play } from "../store.svelte";

  let { name }: { name: string } = $props();
  const artist = $derived(lib.artists.find((a) => a.name === name) ?? null);
  // Songs where they feature on someone else's album.
  const appears = $derived(lib.tracks.filter((t) => t.albumArtist !== name && t.artist.toLowerCase().includes(name.toLowerCase())));
  const top = $derived(artist ? [...artist.tracks].sort((a, b) => b.playCount - a.playCount).slice(0, 5) : []);
  const all = $derived(artist ? [...artist.tracks, ...appears] : appears);
  const albums = $derived(artist ? [...artist.albums].sort((a, b) => (b.year ?? 0) - (a.year ?? 0)) : []);
</script>

<Collection kind="Artist" title={name} art={artist?.art ?? null} color={artist?.albums[0]?.artColor ?? null} tracks={all} round />
{#if top.length && top[0].playCount > 0}
  <h3>Most played</h3>
  <TrackList tracks={top} numbered onplay={(i) => play(top, i)} />
{/if}
{#if albums.length}
  <h3>Albums</h3>
  <AlbumGrid {albums} sub="year" />
{/if}
{#if appears.length}
  <h3>Appears on</h3>
  <TrackList tracks={appears} />
{/if}

<style>
  h3 { font-size: 1.1rem; margin: 1.6rem 0 0.8rem; }
</style>
