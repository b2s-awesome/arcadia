<script lang="ts">
  import AlbumGrid from "../AlbumGrid.svelte";
  import TrackList from "../TrackList.svelte";
  import Art from "../Art.svelte";
  import { go, play, search } from "../store.svelte";
  let { q }: { q: string } = $props();
  const r = $derived(search(q));
</script>

{#if !r.tracks.length && !r.albums.length && !r.artists.length}
  <p class="muted">Nothing in your library matches “{q}”.</p>
{/if}
{#if r.artists.length}
  <h3>Artists</h3>
  <div class="artists">
    {#each r.artists as a (a.name)}
      <button class="artist" onclick={() => go({ kind: "artist", name: a.name })}><Art src={a.art} name={a.name} size={44} round /><span>{a.name}</span></button>
    {/each}
  </div>
{/if}
{#if r.tracks.length}
  <h3>Songs</h3>
  <TrackList tracks={r.tracks.slice(0, 50)} onplay={(i) => play(r.tracks, i)} />
{/if}
{#if r.albums.length}
  <h3>Albums</h3>
  <AlbumGrid albums={r.albums} />
{/if}

<style>
  h3 { font-size: 1.1rem; margin: 1.2rem 0 0.7rem; }
  h3:first-child { margin-top: 0; }
  .artists { display: flex; flex-wrap: wrap; gap: 0.5rem; }
  .artist { display: flex; align-items: center; gap: 0.6rem; border-radius: 999px; padding: 0.25rem 1rem 0.25rem 0.25rem; }
</style>
