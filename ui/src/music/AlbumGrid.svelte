<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import { app } from "../lib/state.svelte";
  import Art from "./Art.svelte";
  import { openMenu, trackMenu } from "./contextMenu.svelte";
  import { go, play, type Album } from "./store.svelte";

  /** `row`: one scrolling line (Home); otherwise a wrapping grid. */
  let { albums, row = false, sub = "artist" }: { albums: Album[]; row?: boolean; sub?: "artist" | "year" } = $props();
  const size = $derived(app.ui.music.cover);
</script>

<div class="grid" class:row style:--tile="{size}px">
  {#each albums as a (a.key)}
    <div class="tile" role="group" aria-label={a.title} oncontextmenu={(e) => openMenu(e, trackMenu(a.tracks))}>
      <button class="cover" onclick={() => go({ kind: "album", key: a.key })} aria-label="Open {a.title}">
        <Art src={a.art} name={a.title} color={a.artColor} radius="var(--r-lg)" />
      </button>
      <button class="play" aria-label="Play {a.title}" onclick={() => play(a.tracks, 0)}><Icon name="play" size={18} filled /></button>
      <button class="meta" onclick={() => go({ kind: "album", key: a.key })}>
        <span class="t">{a.title}</span>
        <span class="s">{sub === "year" ? (a.year ?? "") : a.artist}</span>
      </button>
    </div>
  {/each}
</div>

<style>
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(var(--tile), 1fr)); gap: 1.2rem 1rem; }
  .grid.row { grid-auto-flow: column; grid-template-columns: none; grid-auto-columns: var(--tile); overflow-x: auto; padding-bottom: 0.4rem; scrollbar-width: thin; }
  .tile { position: relative; display: flex; flex-direction: column; gap: 0.5rem; min-width: 0; }
  .cover { border: 0; padding: 0; background: transparent; border-radius: var(--r-lg); display: block; transition: transform 0.15s; }
  .cover:hover { background: transparent; transform: translateY(-2px); }
  .play {
    position: absolute; right: 0.55rem; top: calc(var(--tile) - 3.2rem);
    width: 2.6rem; height: 2.6rem; border-radius: 50%; padding: 0; display: grid; place-items: center;
    background: var(--m-accent, var(--peach)); border: 0; color: var(--m-ink, var(--peach-ink));
    box-shadow: 0 8px 20px rgba(0, 0, 0, 0.3); opacity: 0; transform: translateY(6px); transition: opacity 0.15s, transform 0.15s;
  }
  .grid:not(.row) .play { top: auto; bottom: 3.3rem; }
  .tile:hover .play, .play:focus-visible { opacity: 1; transform: none; }
  .play:hover { background: var(--m-accent, var(--peach)); filter: brightness(1.08); }
  .meta { border: 0; background: transparent; padding: 0; text-align: left; display: flex; flex-direction: column; min-width: 0; }
  .meta:hover { background: transparent; }
  .t { font-weight: 700; font-size: 0.9rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .s { font-weight: 500; font-size: 0.8rem; color: var(--muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
</style>
