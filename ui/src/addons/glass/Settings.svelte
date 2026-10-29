<script lang="ts">
  // Liquid Glass settings (Appearance → Addons).
  import Toggle from "../../components/Toggle.svelte";
  import { app, saveUi } from "../../lib/state.svelte";
  import type { GlassPrefs } from "../../lib/theme";

  const g = $derived(app.ui.glass);
  const behind = $derived(app.ui.look.wallpaper !== null || (app.ui.addons.installed.includes("see-through") && app.ui.seeThrough.enabled));
  function set<K extends keyof GlassPrefs>(k: K, v: GlassPrefs[K]) {
    app.ui.glass[k] = v;
    saveUi();
  }
</script>

{#if !behind}
  <p class="note">Glass needs something behind it. Pick a wallpaper under Look, or add Translucent window to see your desktop through it.</p>
{/if}
<div class="setting"><div><div class="label">Liquid Glass</div></div><Toggle label="Liquid Glass" checked={g.enabled} onchange={(v) => set("enabled", v)} /></div>
<div class="setting">
  <div><div class="label">Where</div></div>
  <div class="seg" role="radiogroup" aria-label="Where">
    <button role="radio" aria-checked={g.scope === "app"} onclick={() => set("scope", "app")}>Whole app</button>
    <button role="radio" aria-checked={g.scope === "music"} onclick={() => set("scope", "music")}>Music only</button>
  </div>
</div>
<div class="setting">
  <div><div class="label">Glass</div><div class="desc">Frosted is bright and milky; smoked is dark and moody. Match keeps text readable with your Look.</div></div>
  <div class="seg" role="radiogroup" aria-label="Glass">
    <button role="radio" aria-checked={g.shade === "auto"} onclick={() => set("shade", "auto")}>Match the Look</button>
    <button role="radio" aria-checked={g.shade === "frost"} onclick={() => set("shade", "frost")}>Frosted</button>
    <button role="radio" aria-checked={g.shade === "smoke"} onclick={() => set("shade", "smoke")}>Smoked</button>
  </div>
</div>
<div class="setting">
  <div><div class="label">Blur</div><div class="desc">Less blur is lighter on older graphics.</div></div>
  <div class="row"><input type="range" min="0" max="60" step="1" value={g.frost} oninput={(e) => set("frost", +(e.currentTarget as HTMLInputElement).value)} aria-label="Blur" /><span class="num">{g.frost}px</span></div>
</div>
<div class="setting">
  <div><div class="label">How solid</div></div>
  <div class="row"><input type="range" min="0" max="0.7" step="0.01" value={g.tint} oninput={(e) => set("tint", +(e.currentTarget as HTMLInputElement).value)} aria-label="How solid" /><span class="num">{Math.round(g.tint * 100)}%</span></div>
</div>
<div class="setting">
  <div><div class="label">Edge light</div><div class="desc">The bright rim along the top of each pane.</div></div>
  <div class="row"><input type="range" min="0" max="1" step="0.05" value={g.edge} oninput={(e) => set("edge", +(e.currentTarget as HTMLInputElement).value)} aria-label="Edge light" /><span class="num">{Math.round(g.edge * 100)}%</span></div>
</div>
<div class="setting"><div><div class="label">Sheen</div><div class="desc">A soft diagonal shine across the glass.</div></div><Toggle label="Sheen" checked={g.sheen} onchange={(v) => set("sheen", v)} /></div>

<style>
  .note { color: var(--muted); font-size: 0.86rem; padding: 0.3rem 0; }
  .row { gap: 0.6rem; }
  .num { width: 3rem; text-align: right; font-variant-numeric: tabular-nums; color: var(--muted); font-size: 0.86rem; }
</style>
