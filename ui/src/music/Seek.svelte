<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { LOOP, addBookmark, bookmarksFor, clearLoop, lx, playBookmark, setA, setB } from "../addons/listening/listening.svelte";
  import { openMenu } from "./menu.svelte";
  import { ms, seek, time } from "./store.svelte";
  let { times = true, thin = false }: { times?: boolean; thin?: boolean } = $props();
  let dragging = $state(false);
  let drag = $state(0);
  const dur = $derived(ms.status?.duration ?? 0);
  const value = $derived(dragging ? drag : ms.pos);
  const pct = $derived(dur > 0 ? Math.min(100, (value / dur) * 100) : 0);

  // Loopy Loop: the loop and bookmarks drawn on the bar.
  const loopOn = $derived(app.ui.addons.installed.includes(LOOP));
  const ab = $derived(loopOn ? (ms.status?.loopAb ?? null) : null);
  const at = (s: number) => (dur > 0 ? Math.max(0, Math.min(100, (s / dur) * 100)) : 0);
  const marks = $derived(loopOn ? bookmarksFor(ms.status?.track?.id) : []);
  const pendingA = $derived(loopOn ? lx.pendingA : null);

  let track: HTMLDivElement | undefined = $state();
  function menu(e: MouseEvent) {
    if (!loopOn || !dur || !track) return;
    const r = track.getBoundingClientRect();
    const s = Math.max(0, Math.min(dur, ((e.clientX - r.left) / r.width) * dur));
    openMenu(e, [
      { label: `Set A at ${time(s)}`, icon: "loop", run: () => setA(s) },
      { label: `Set B at ${time(s)}`, icon: "loop", run: () => setB(s) },
      ...(ab || pendingA !== null ? [{ label: "Stop looping", run: clearLoop }] : []),
      { label: "", sep: true },
      { label: `Bookmark ${time(ms.pos)} (now)`, icon: "bookmark", run: () => addBookmark() },
    ]);
  }
</script>

<div class="seek" class:thin>
  {#if times}<span class="t">{time(value)}</span>{/if}
  <div class="track" bind:this={track} oncontextmenu={menu} role="presentation">
    {#if ab}<span class="loop" style:left="{at(ab[0])}%" style:width="{at(ab[1]) - at(ab[0])}%" title="Looping {time(ab[0])}–{time(ab[1])}"></span>{/if}
    {#if pendingA !== null}<span class="pin" style:left="{at(pendingA)}%" title="A: {time(pendingA)}"></span>{/if}
    <input
      type="range" min="0" max={dur || 1} step="0.1" value={value} disabled={!dur}
      style:--pct="{pct}%"
      aria-label="Position" aria-valuetext="{time(value)} of {time(dur)}"
      oninput={(e) => { dragging = true; drag = Number((e.currentTarget as HTMLInputElement).value); }}
      onchange={(e) => { dragging = false; seek(Number((e.currentTarget as HTMLInputElement).value)); }}
    />
    {#each marks as b (b.id)}
      <button class="mark" style:left="{at(b.at)}%" aria-label="Bookmark: {b.label}, {time(b.at)}" title="{b.label} · {time(b.at)}" onclick={() => playBookmark(b)}></button>
    {/each}
  </div>
  {#if times}<span class="t">{time(dur)}</span>{/if}
</div>

<style>
  .seek { display: flex; align-items: center; gap: 0.6rem; width: 100%; }
  .t { font-size: 0.74rem; color: var(--muted); font-variant-numeric: tabular-nums; min-width: 2.6rem; text-align: center; }
  .track { flex: 1; position: relative; display: flex; align-items: center; min-width: 0; }
  input { flex: 1; appearance: none; -webkit-appearance: none; height: 16px; background: transparent; cursor: pointer; padding: 0; border: 0; position: relative; z-index: 1; }
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
  /* The loop sits behind the bar as a soft band with A and B edges. */
  .loop { position: absolute; top: 50%; height: 13px; transform: translateY(-50%); border-radius: 4px; pointer-events: none;
    background: color-mix(in srgb, var(--m-accent, var(--peach)) 28%, transparent);
    box-shadow: inset 2px 0 0 var(--m-accent, var(--peach)), inset -2px 0 0 var(--m-accent, var(--peach)); }
  .pin { position: absolute; top: 50%; width: 2px; height: 15px; transform: translate(-1px, -50%); background: var(--m-accent, var(--peach)); pointer-events: none; }
  .mark { position: absolute; top: 50%; z-index: 2; width: 9px; height: 9px; padding: 0; border: 2px solid var(--ink, #000); border-radius: 2px;
    transform: translate(-50%, -50%) rotate(45deg); background: var(--text); cursor: pointer; }
  .mark:hover { background: var(--m-accent, var(--peach)); }
</style>
