<script lang="ts">
  import TrackList from "../TrackList.svelte";
  import Collection from "./Collection.svelte";
  import AlbumGrid from "../AlbumGrid.svelte";
  import { go, lib, quality } from "../store.svelte";

  let { key }: { key: string } = $props();
  const album = $derived(lib.albumByKey.get(key) ?? null);
  const more = $derived(album ? (lib.artists.find((a) => a.name === album.artist)?.albums ?? []).filter((a) => a.key !== album.key) : []);
  const discs = $derived(album ? new Set(album.tracks.map((t) => t.discNo ?? 1)).size : 0);
  const formats = $derived(album ? [...new Set(album.tracks.map(quality))].join(", ") : "");
</script>

{#if album}
  <Collection kind="Album" title={album.title} sub={[album.artist, album.year].filter(Boolean).join(" · ")} art={album.art} color={album.artColor} tracks={album.tracks}
    more={[{ label: "Go to artist", run: () => go({ kind: "artist", name: album.artist }) }]} />
  {#if discs > 1}
    {#each [...new Set(album.tracks.map((t) => t.discNo ?? 1))] as d}
      <h3 class="disc">Disc {d}</h3>
      <TrackList tracks={album.tracks.filter((t) => (t.discNo ?? 1) === d)} album />
    {/each}
  {:else}
    <TrackList tracks={album.tracks} album />
  {/if}
  <p class="fmt">{formats}</p>
  {#if more.length}
    <h3>More by {album.artist}</h3>
    <AlbumGrid albums={more} sub="year" />
  {/if}
{:else}
  <p class="muted">That album isn't in your library any more.</p>
{/if}

<style>
  .disc { margin: 1rem 0 0.4rem 0.6rem; font-size: 0.8rem; color: var(--muted); letter-spacing: 0.08em; text-transform: uppercase; }
  .fmt { font-size: 0.78rem; color: var(--faint); margin: 1rem 0.6rem 2rem; }
  h3 { font-size: 1.1rem; margin-bottom: 0.8rem; }
</style>
