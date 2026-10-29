<script lang="ts">
  // Shuffle+ settings: the same modes as right-clicking the shuffle button.
  import { ms } from "../../music/store.svelte";
  import { SHUFFLE_MODES, setShuffleMode } from "./listening.svelte";
  const cur = $derived(ms.status?.shuffleMode ?? "random");
</script>

<p class="note">Right-click the shuffle button in Music to change this any time. Picking a mode turns shuffle on; the song playing keeps playing.</p>
<div class="modes" role="radiogroup" aria-label="Shuffle mode">
  {#each SHUFFLE_MODES as m}
    <button role="radio" aria-checked={cur === m.id} onclick={() => setShuffleMode(m.id)}><b>{m.label}</b><span>{m.detail}</span></button>
  {/each}
</div>

<style>
  .note { color: var(--muted); font-size: 0.86rem; padding: 0.3rem 0 0.6rem; }
  .modes { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 0.5rem; }
  .modes button { display: flex; flex-direction: column; align-items: flex-start; gap: 0.2rem; text-align: left; padding: 0.7rem 0.9rem; border-radius: var(--r-md); background: transparent; }
  .modes button span { font-size: 0.8rem; color: var(--muted); font-weight: 500; }
  .modes button[aria-checked="true"] { border-color: var(--peach); background: color-mix(in srgb, var(--peach) 12%, transparent); }
</style>
