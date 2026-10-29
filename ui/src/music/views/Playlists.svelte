<script lang="ts">
  import Icon from "../../components/Icon.svelte";
  import { app } from "../../lib/state.svelte";
  import Art from "../Art.svelte";
  import { ask } from "../menu.svelte";
  import { go, longTime, ms, newPlaylist } from "../store.svelte";

  let { row = false }: { row?: boolean } = $props();
  const size = $derived(app.ui.music.cover);

  async function create() {
    const name = await ask("New playlist");
    if (name === null) return;
    const id = await newPlaylist(name);
    go({ kind: "playlist", id });
  }
</script>

<div class="grid" class:row style:--tile="{size}px">
  {#if !row}
    <button class="tile new" onclick={create}>
      <span class="newart"><Icon name="plus" size={30} /></span>
      <span class="t">New playlist</span>
    </button>
  {/if}
  {#each ms.playlists as p (p.id)}
    <button class="tile" onclick={() => go({ kind: "playlist", id: p.id })}>
      <span class="mosaic" class:four={p.arts.length >= 4}>
        {#if p.arts.length >= 4}
          {#each p.arts.slice(0, 4) as a}<Art src={a} radius="0" />{/each}
        {:else}
          <Art src={p.arts[0] ?? null} name={p.name} radius="0" />
        {/if}
      </span>
      <span class="t">{p.name}</span>
      <span class="s">{p.count} songs · {longTime(p.duration)}</span>
    </button>
  {/each}
</div>

<style>
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(var(--tile), 1fr)); gap: 1.2rem 1rem; }
  .grid.row { grid-auto-flow: column; grid-template-columns: none; grid-auto-columns: var(--tile); overflow-x: auto; padding-bottom: 0.4rem; }
  .tile { border: 0; background: transparent; padding: 0; text-align: left; display: flex; flex-direction: column; gap: 0.5rem; min-width: 0; }
  .tile:hover { background: transparent; }
  .mosaic, .newart { aspect-ratio: 1; border-radius: var(--r-lg); overflow: hidden; display: grid; transition: transform 0.15s; }
  .mosaic.four { grid-template-columns: 1fr 1fr; }
  .tile:hover .mosaic, .tile:hover .newart { transform: translateY(-2px); }
  .newart { place-items: center; border: 2px dashed var(--line); color: var(--muted); }
  .t { font-weight: 700; font-size: 0.9rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .s { font-size: 0.8rem; color: var(--muted); margin-top: -0.35rem; }
</style>
