<script lang="ts">
  import { ms, seek, time } from "./store.svelte";
  let { times = true, thin = false }: { times?: boolean; thin?: boolean } = $props();
  let dragging = $state(false);
  let drag = $state(0);
  const dur = $derived(ms.status?.duration ?? 0);
  const value = $derived(dragging ? drag : ms.pos);
  const pct = $derived(dur > 0 ? Math.min(100, (value / dur) * 100) : 0);
</script>

<div class="seek" class:thin>
  {#if times}<span class="t">{time(value)}</span>{/if}
  <input
    type="range" min="0" max={dur || 1} step="0.1" value={value} disabled={!dur}
    style:--pct="{pct}%"
    aria-label="Position" aria-valuetext="{time(value)} of {time(dur)}"
    oninput={(e) => { dragging = true; drag = Number((e.currentTarget as HTMLInputElement).value); }}
    onchange={(e) => { dragging = false; seek(Number((e.currentTarget as HTMLInputElement).value)); }}
  />
  {#if times}<span class="t">{time(dur)}</span>{/if}
</div>

<style>
  .seek { display: flex; align-items: center; gap: 0.6rem; width: 100%; }
  .t { font-size: 0.74rem; color: var(--muted); font-variant-numeric: tabular-nums; min-width: 2.6rem; text-align: center; }
  input { flex: 1; appearance: none; -webkit-appearance: none; height: 16px; background: transparent; cursor: pointer; padding: 0; border: 0; }
  input::-webkit-slider-runnable-track {
    height: 5px; border-radius: 9px;
    background: linear-gradient(90deg, var(--m-accent, var(--peach)) var(--pct), color-mix(in srgb, var(--text) 16%, transparent) var(--pct));
  }
  .thin input::-webkit-slider-runnable-track { height: 3px; }
  input::-webkit-slider-thumb {
    -webkit-appearance: none; width: 13px; height: 13px; border-radius: 50%; margin-top: -4px;
    background: var(--text); opacity: 0; transition: opacity 0.12s;
  }
  .thin input::-webkit-slider-thumb { margin-top: -5px; }
  .seek:hover input::-webkit-slider-thumb, input:focus-visible::-webkit-slider-thumb { opacity: 1; }
  input:disabled { cursor: default; }
</style>
