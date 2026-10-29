<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import { cycleRepeat, ms, next, prev, setShuffle, toggle } from "./store.svelte";
  let { big = false }: { big?: boolean } = $props();
  const s = $derived(ms.status);
</script>

<div class="tp" class:big>
  <button class="ic" class:on={s?.shuffle} aria-pressed={!!s?.shuffle} aria-label="Shuffle" onclick={() => setShuffle(!s?.shuffle)}><Icon name="shuffle" size={big ? 20 : 17} /></button>
  <button class="ic" aria-label="Previous" onclick={prev} disabled={!s?.track}><Icon name="skip-prev" size={big ? 24 : 19} filled /></button>
  <button class="pp" class:loading={s?.loading} aria-label={s?.loading ? "Fetching the song" : s?.playing ? "Pause" : "Play"} onclick={toggle} disabled={!s?.track}>
    {#if s?.loading}<span class="spin" aria-hidden="true"></span>{:else}<Icon name={s?.playing ? "pause" : "play"} size={big ? 26 : 18} filled />{/if}
  </button>
  <button class="ic" aria-label="Next" onclick={next} disabled={!s?.track}><Icon name="skip-next" size={big ? 24 : 19} filled /></button>
  <button class="ic" class:on={s?.repeat !== "off"} aria-label="Repeat: {s?.repeat ?? 'off'}" onclick={cycleRepeat}><Icon name={s?.repeat === "one" ? "repeat-one" : "repeat"} size={big ? 20 : 17} /></button>
</div>

<style>
  .tp { display: flex; align-items: center; justify-content: center; gap: 0.9rem; }
  .tp.big { gap: 1.6rem; }
  .ic { border: 0; background: transparent; padding: 0.3rem; color: var(--muted); display: grid; place-items: center; position: relative; }
  .ic:hover:not(:disabled) { color: var(--text); background: transparent; }
  .ic.on { color: var(--m-accent, var(--peach)); }
  .ic.on::after { content: ""; position: absolute; bottom: -3px; left: 50%; width: 4px; height: 4px; margin-left: -2px; border-radius: 50%; background: currentColor; }
  .pp { width: 42px; height: 42px; border-radius: 50%; padding: 0; display: grid; place-items: center; border: 0; background: var(--text); color: var(--ink); }
  .pp:hover:not(:disabled) { background: var(--text); transform: scale(1.05); }
  .spin { width: 40%; height: 40%; border-radius: 50%; border: 2.5px solid currentColor; border-right-color: transparent; animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  .big .pp { width: 66px; height: 66px; border-radius: 22px; background: var(--m-accent, var(--peach)); color: var(--m-ink, var(--peach-ink)); }
  .big .pp:hover:not(:disabled) { background: var(--m-accent, var(--peach)); }
</style>
