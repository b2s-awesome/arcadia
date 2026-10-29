<script lang="ts">
  // Stats settings (Appearance → Addons).
  import { app, saveUi } from "../../lib/state.svelte";
  import type { StatsPrefs } from "../../lib/theme";
  import { go } from "../../music/store.svelte";

  const p = $derived(app.ui.stats);
  function set<K extends keyof StatsPrefs>(k: K, v: StatsPrefs[K]) {
    app.ui.stats[k] = v;
    saveUi();
  }
  function open() {
    app.page = "music";
    go({ kind: "stats" });
  }
</script>

<div class="setting col">
  <div><div class="label">Look</div><div class="desc">“Follow Music” uses recap cards with the Tonal layout and terminal tables with the Tiled one.</div></div>
  <div class="seg" role="radiogroup" aria-label="Stats look">
    {#each [["auto", "Follow Music"], ["tonal", "Recap cards"], ["tiled", "Terminal"], ["glass", "Glass widgets"]] as [id, label]}
      <button role="radio" aria-checked={p.skin === id} onclick={() => set("skin", id as StatsPrefs["skin"])}>{label}</button>
    {/each}
  </div>
</div>
<div class="setting">
  <div><div class="label">Daily goal</div><div class="desc">Minutes of listening a day, for the ring on Glass widgets.</div></div>
  <div class="row"><input type="range" min="10" max="300" step="5" value={p.goal} oninput={(e) => set("goal", +(e.currentTarget as HTMLInputElement).value)} aria-label="Daily goal" /><span class="num">{p.goal} min</span></div>
</div>
<div class="setting"><div><div class="label">Where it comes from</div><div class="desc">Arcadia logs each song you listen to for more than a few seconds: when, for how long, and whether you skipped it. The log stays in your music library on this PC.</div></div><button onclick={open}>Open Stats</button></div>

<style>
  .setting.col { flex-direction: column; align-items: flex-start; gap: 0.6rem; }
  .row { gap: 0.6rem; }
  .num { width: 4rem; text-align: right; font-variant-numeric: tabular-nums; color: var(--muted); font-size: 0.86rem; }
</style>
