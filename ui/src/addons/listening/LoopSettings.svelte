<script lang="ts">
  // Loopy Loop + Bookmarks: how to use it, and every bookmark.
  import { onMount } from "svelte";
  import { ms, time } from "../../music/store.svelte";
  import { ask } from "../../music/menu.svelte";
  import { clearLoop, initListening, lx, playBookmark, removeBookmark, renameBookmark } from "./listening.svelte";

  onMount(() => {
    initListening();
  });
  const ab = $derived(ms.status?.loopAb ?? null);
  async function rename(id: string, label: string) {
    const v = await ask("Rename bookmark", label);
    if (v) renameBookmark(id, v);
  }
</script>

<div class="keys">
  <span><kbd>[</kbd> set A</span><span><kbd>]</kbd> set B</span><span><kbd>\</kbd> stop looping</span><span><kbd>M</kbd> bookmark now</span><span>or right-click the seek bar</span>
</div>
{#if ab}
  <div class="setting"><div><div class="label">Looping</div><div class="desc">{ms.status?.track?.title} · {time(ab[0])} to {time(ab[1])}</div></div><button onclick={clearLoop}>Stop</button></div>
{/if}
<div class="setting col">
  <div class="label">Bookmarks</div>
  {#if !lx.bookmarks.length}
    <p class="note">None yet. Press M while a song plays to remember the moment.</p>
  {:else}
    <ul>
      {#each lx.bookmarks as b (b.id)}
        <li>
          <button class="play" onclick={() => playBookmark(b)}><b>{b.label}</b><span class="muted">{b.title} · {b.artist} · {time(b.at)}</span></button>
          <button onclick={() => rename(b.id, b.label)}>Rename</button>
          <button onclick={() => removeBookmark(b.id)}>Remove</button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .keys { display: flex; flex-wrap: wrap; gap: 0.4rem 1rem; color: var(--muted); font-size: 0.86rem; padding: 0.3rem 0 0.6rem; }
  kbd { font: inherit; font-size: 0.78rem; border: 1px solid var(--line); border-radius: 5px; padding: 0 0.4rem; color: var(--text); }
  .note { color: var(--muted); font-size: 0.86rem; }
  .setting.col { flex-direction: column; align-items: stretch; gap: 0.5rem; }
  ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.3rem; max-height: 300px; overflow-y: auto; }
  li { display: flex; align-items: center; gap: 0.4rem; padding: 0.3rem; border-radius: var(--r-sm); background: var(--sunk); }
  li button { padding: 0.25rem 0.7rem; font-size: 0.82rem; }
  .play { flex: 1; display: flex; flex-direction: column; align-items: flex-start; text-align: left; border: 0; background: transparent; min-width: 0; }
  .play span { font-size: 0.78rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 100%; }
</style>
