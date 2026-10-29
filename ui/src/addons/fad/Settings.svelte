<script lang="ts">
  // Full App Display settings (Appearance → Addons).
  import Toggle from "../../components/Toggle.svelte";
  import { app, saveUi } from "../../lib/state.svelte";
  import type { FadPrefs } from "../../lib/theme";
  import { openFad } from "./fad.svelte";

  const p = $derived(app.ui.fad);
  function set<K extends keyof FadPrefs>(k: K, v: FadPrefs[K]) {
    app.ui.fad[k] = v;
    saveUi();
  }
  const skins: { id: FadPrefs["skin"]; label: string }[] = [
    { id: "auto", label: "Follow Music" },
    { id: "tonal", label: "Ambient" },
    { id: "tiled", label: "Tiled" },
    { id: "glass", label: "Liquid Glass" },
  ];
</script>

<div class="setting col">
  <div><div class="label">Look</div><div class="desc">“Follow Music” uses Ambient with the Tonal layout and Tiled with the Tiled one.</div></div>
  <div class="seg" role="radiogroup" aria-label="Full App Display look">
    {#each skins as s}<button role="radio" aria-checked={p.skin === s.id} onclick={() => set("skin", s.id)}>{s.label}</button>{/each}
  </div>
</div>
<div class="setting">
  <div><div class="label">Ambient layout</div><div class="desc">Cover beside the title, or everything centred.</div></div>
  <div class="seg" role="radiogroup" aria-label="Ambient layout">
    <button role="radio" aria-checked={p.layout === "split"} onclick={() => set("layout", "split")}>Side by side</button>
    <button role="radio" aria-checked={p.layout === "centred"} onclick={() => set("layout", "centred")}>Centred</button>
  </div>
</div>
<div class="setting">
  <div><div class="label">Cover blur</div><div class="desc">How soft the cover behind Ambient is.</div></div>
  <div class="row"><input type="range" min="0" max="160" step="2" value={p.blur} oninput={(e) => set("blur", +(e.currentTarget as HTMLInputElement).value)} aria-label="Cover blur" /><span class="num">{p.blur}px</span></div>
</div>
<div class="setting">
  <div><div class="label">Visualiser</div><div class="desc">Tiled: a live spectrum from the audio engine. Only runs while it's on screen.</div></div>
  <Toggle label="Visualiser" checked={p.visualiser} onchange={(v) => set("visualiser", v)} />
</div>
{#if p.visualiser}
  <div class="setting">
    <div><div class="label">Visualiser style</div></div>
    <div class="row">
      <div class="seg" role="radiogroup" aria-label="Visualiser style">
        {#each [["bars", "Bars"], ["blocks", "Blocks"], ["mirror", "Mirror"]] as [id, label]}<button role="radio" aria-checked={p.vizStyle === id} onclick={() => set("vizStyle", id as FadPrefs["vizStyle"])}>{label}</button>{/each}
      </div>
      <input type="range" min="12" max="128" step="4" value={p.bands} oninput={(e) => set("bands", +(e.currentTarget as HTMLInputElement).value)} aria-label="Bands" /><span class="num">{p.bands}</span>
    </div>
  </div>
{/if}
<div class="setting"><div><div class="label">Clock</div></div><Toggle label="Clock" checked={p.clock} onchange={(v) => set("clock", v)} /></div>
<div class="setting"><div><div class="label">Up next</div></div><Toggle label="Up next" checked={p.upNext} onchange={(v) => set("upNext", v)} /></div>
<div class="setting"><div><div class="label">Lyric line</div><div class="desc">The line being sung, under the title. Needs Lyrics Plus.</div></div><Toggle label="Lyric line" checked={p.lyric} onchange={(v) => set("lyric", v)} /></div>
<div class="setting"><div><div class="label">Go full screen</div><div class="desc">Make the window full screen when it opens. Double-click toggles it.</div></div><Toggle label="Go full screen" checked={p.fullscreen} onchange={(v) => set("fullscreen", v)} /></div>
<div class="setting"><div><div class="label">Try it</div><div class="desc">Or press F in Music.</div></div><button onclick={openFad} disabled={!app.ui.addons.installed.includes("full-app-display")}>Open Full App Display</button></div>

<style>
  .setting.col { flex-direction: column; align-items: flex-start; gap: 0.6rem; }
  .row { gap: 0.6rem; }
  .num { width: 3rem; text-align: right; font-variant-numeric: tabular-nums; color: var(--muted); font-size: 0.86rem; }
</style>
