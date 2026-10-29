<script lang="ts">
  import { app } from "../../lib/state.svelte";
  import Art from "../Art.svelte";
  import { go, lib } from "../store.svelte";
  const size = $derived(Math.round(app.ui.music.cover * 0.85));
</script>

<div class="grid" style:--tile="{size}px">
  {#each lib.artists as a (a.name)}
    <button class="tile" onclick={() => go({ kind: "artist", name: a.name })}>
      <Art src={a.art} name={a.name} color={a.albums[0]?.artColor} round />
      <span class="t">{a.name}</span>
      <span class="s">{a.albums.length} {a.albums.length === 1 ? "album" : "albums"}</span>
    </button>
  {/each}
</div>

<style>
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(var(--tile), 1fr)); gap: 1.4rem 1rem; }
  .tile { border: 0; background: transparent; padding: 0; display: flex; flex-direction: column; align-items: center; gap: 0.4rem; text-align: center; min-width: 0; }
  .tile:hover { background: transparent; }
  .tile :global(.art) { transition: transform 0.15s; }
  .tile:hover :global(.art) { transform: translateY(-2px); }
  .t { font-weight: 700; font-size: 0.9rem; max-width: 100%; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .s { font-size: 0.8rem; color: var(--muted); margin-top: -0.3rem; }
</style>
