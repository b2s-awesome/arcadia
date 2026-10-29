<script lang="ts">
  // Settings for the Translucent window addon, shown in Appearance.
  import { windowFx } from "../../lib/api";
  import { app, saveSettingsNow, saveUi, toast } from "../../lib/state.svelte";
  import { fx, loadFxStatus } from "../../lib/seeThrough.svelte";
  import type { SeeThroughPrefs } from "../../lib/theme";
  import Toggle from "../../components/Toggle.svelte";

  const p = $derived(app.ui.seeThrough);
  const build = $derived(fx.status?.build ?? null);
  const onWindows = $derived(fx.status?.os === "windows");

  type Fx = SeeThroughPrefs["effect"];
  /** Minimum Windows build for each backdrop. */
  const effects: { id: Fx; label: string; min: number; note: string }[] = [
    { id: "acrylic", label: "Acrylic", min: 17763, note: "Real blur of the windows and desktop behind Arcadia. Can stutter while dragging or resizing on some Windows builds." },
    { id: "mica", label: "Mica", min: 22000, note: "A soft, wallpaper-tinted backdrop. Very light on performance. Windows 11." },
    { id: "mica-dark", label: "Mica dark", min: 22000, note: "Mica, always in its dark variant. Windows 11." },
    { id: "tabbed", label: "Mica Alt", min: 22621, note: "A stronger wallpaper tint, as used behind tabs. Windows 11 22H2 or newer." },
    { id: "blur", label: "Blur", min: 0, note: "Plain blur with a tint. Older style; can also stutter while dragging." },
  ];
  const supported = (min: number) => !onWindows || build === null || build >= min;
  const current = $derived(effects.find((e) => e.id === p.effect) ?? effects[0]);
  const tinted = $derived(p.effect === "acrylic" || p.effect === "blur");

  // Restart needed when what's saved differs from how this run started.
  const needsRestart = $derived(!!fx.status && fx.status.active !== (p.enabled));

  function set<K extends keyof SeeThroughPrefs>(k: K, v: SeeThroughPrefs[K]) {
    app.ui.seeThrough[k] = v;
    saveUi();
  }
  async function setEnabled(v: boolean) {
    app.ui.seeThrough.enabled = v;
    if (app.settings) app.settings.ui = $state.snapshot(app.ui);
    await saveSettingsNow();
    await loadFxStatus();
  }
  async function restart() {
    if (app.settings) app.settings.ui = $state.snapshot(app.ui);
    await saveSettingsNow();
    try {
      await windowFx.restart();
    } catch (e) {
      toast(String(e), "error");
    }
  }
</script>

{#if fx.status && !onWindows}
  <p class="note">The blur comes from Windows 10 and 11. On {fx.status.os} the background turns see-through only if your desktop has a compositor, and without blur.</p>
{/if}

<div class="setting">
  <div><div class="label">See-through window</div><div class="desc">Arcadia's background shows your desktop, blurred. Turning this on or off takes a restart.</div></div>
  <Toggle label="See-through window" checked={p.enabled} onchange={setEnabled} />
</div>

{#if needsRestart}
  <div class="restart">
    <span>{p.enabled ? "Restart Arcadia to see through it." : "Restart Arcadia to make the window solid again."}</span>
    <button class="go" onclick={restart}>Restart now</button>
  </div>
{/if}

<div class="setting col">
  <div><div class="label">Backdrop</div><div class="desc">{current.note}</div></div>
  <div class="seg" role="radiogroup" aria-label="Backdrop">
    {#each effects as e}
      {@const ok = supported(e.min)}
      <button role="radio" aria-checked={p.effect === e.id} disabled={!ok} title={ok ? e.label : "Needs a newer version of Windows"} onclick={() => set("effect", e.id)}>{e.label}</button>
    {/each}
  </div>
</div>

{#if tinted}
  <div class="setting">
    <div><div class="label">Tint</div><div class="desc">Colour laid over the blur, and how strong it is.</div></div>
    <div class="row">
      <input type="color" value={p.tint} oninput={(e) => set("tint", (e.currentTarget as HTMLInputElement).value)} aria-label="Tint colour" />
      <input type="range" min="0" max="1" step="0.01" value={p.strength} oninput={(e) => set("strength", Number((e.currentTarget as HTMLInputElement).value))} aria-label="Tint strength" />
      <span class="num">{Math.round(p.strength * 100)}%</span>
    </div>
  </div>
{/if}

<div class="setting">
  <div><div class="label">Panel</div><div class="desc">How solid the app's panel is over the blur. Lower shows more of the desktop.</div></div>
  <div class="row">
    <input type="range" min="0.15" max="1" step="0.01" value={p.panel} oninput={(e) => set("panel", Number((e.currentTarget as HTMLInputElement).value))} aria-label="Panel opacity" />
    <span class="num">{Math.round(p.panel * 100)}%</span>
  </div>
</div>

<div class="setting">
  <div><div class="label">Keep the wallpaper</div><div class="desc">Show Arcadia's wallpaper faintly over the desktop instead of the desktop alone.</div></div>
  <Toggle label="Keep the wallpaper" checked={p.keepWallpaper} onchange={(v) => set("keepWallpaper", v)} />
</div>

<style>
  .note { color: var(--muted); font-size: 0.86rem; padding: 0.4rem 0; }
  .restart {
    display: flex; align-items: center; justify-content: space-between; gap: 1rem;
    padding: 0.6rem 0.8rem; margin: 0.2rem 0; border-radius: var(--r-md);
    background: color-mix(in srgb, var(--lilac) 12%, var(--sunk));
    border: 1px solid color-mix(in srgb, var(--lilac) 30%, transparent); font-size: 0.9rem;
  }
  .go { background: var(--peach); border-color: var(--peach); color: var(--peach-ink); padding: 0.35rem 0.9rem; font-size: 0.86rem; }
  .setting.col { flex-direction: column; align-items: flex-start; gap: 0.6rem; }
  .row { gap: 0.6rem; }
  .num { width: 3rem; text-align: right; font-variant-numeric: tabular-nums; color: var(--muted); font-size: 0.86rem; }
  input[type="color"] { width: 2.2rem; height: 2rem; padding: 2px; cursor: pointer; }
</style>
