<script lang="ts">
  import AlbumGrid from "../AlbumGrid.svelte";
  import { lib } from "../store.svelte";
  type Sort = "title" | "artist" | "year" | "added";
  let sort: Sort = $state("title");
  const albums = $derived.by(() => {
    const v = [...lib.albums];
    const c = (a: string, b: string) => a.localeCompare(b, undefined, { sensitivity: "base" });
    if (sort === "artist") v.sort((a, b) => c(a.artist, b.artist) || (a.year ?? 0) - (b.year ?? 0));
    if (sort === "year") v.sort((a, b) => (b.year ?? 0) - (a.year ?? 0));
    if (sort === "added") v.sort((a, b) => b.addedAt - a.addedAt);
    return v;
  });
</script>

<div class="bar">
  <h2>Albums</h2><span class="muted">{albums.length.toLocaleString()}</span><span class="grow"></span>
  <label class="sort"><span class="muted">Sort</span>
    <select bind:value={sort}><option value="title">Title</option><option value="artist">Artist</option><option value="year">Year</option><option value="added">Recently added</option></select>
  </label>
</div>
<AlbumGrid {albums} />

<style>
  .bar { display: flex; align-items: center; gap: 0.7rem; margin-bottom: 1rem; }
  .bar h2 { font-size: 1.5rem; }
  .sort { display: flex; align-items: center; gap: 0.4rem; font-size: 0.85rem; }
</style>
