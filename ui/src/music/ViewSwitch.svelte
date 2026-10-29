<script lang="ts">
  import Home from "./views/Home.svelte";
  import Albums from "./views/Albums.svelte";
  import Album from "./views/Album.svelte";
  import Artists from "./views/Artists.svelte";
  import Artist from "./views/Artist.svelte";
  import Songs from "./views/Songs.svelte";
  import Playlists from "./views/Playlists.svelte";
  import Playlist from "./views/Playlist.svelte";
  import Search from "./views/Search.svelte";
  import Online from "./views/Online.svelte";
  import Downloads from "./views/Downloads.svelte";
  import { streamingOn } from "./streaming.svelte";
  import { ms } from "./store.svelte";
  const v = $derived(ms.view);
</script>

{#if v.kind === "home"}<Home />
{:else if v.kind === "albums"}<Albums />
{:else if v.kind === "album"}<Album key={v.key} />
{:else if v.kind === "artists"}<Artists />
{:else if v.kind === "artist"}<Artist name={v.name} />
{:else if v.kind === "songs"}<Songs />
{:else if v.kind === "favourites"}<Songs favourites />
{:else if v.kind === "playlists"}<div class="ph"><h2>Playlists</h2></div><Playlists />
{:else if v.kind === "playlist"}<Playlist id={v.id} />
{:else if v.kind === "search"}<Search q={v.q} />
{:else if v.kind === "online"}{#if streamingOn()}<Online />{:else}<p class="muted">Add Streaming & downloads from the Marketplace to search online.</p>{/if}
{:else if v.kind === "downloads"}{#if streamingOn()}<Downloads />{:else}<p class="muted">Add Streaming & downloads from the Marketplace to download music.</p>{/if}{/if}

<style>
  .ph { margin-bottom: 1rem; }
  .ph h2 { font-size: 1.5rem; }
</style>
