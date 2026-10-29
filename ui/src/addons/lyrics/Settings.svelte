<script lang="ts">
  // Lyrics Plus settings (Appearance → Addons).
  import Toggle from "../../components/Toggle.svelte";
  import { lyricsApi } from "../../lib/api";
  import { app, saveUi, toast } from "../../lib/state.svelte";
  import type { LyricsPrefs } from "../../lib/theme";
  import { deskBar } from "./LyricsView.svelte";

  const p = $derived(app.ui.lyrics);
  const on = $derived(app.ui.addons.installed.includes("lyrics-plus"));
  function set<K extends keyof LyricsPrefs>(k: K, v: LyricsPrefs[K]) {
    app.ui.lyrics[k] = v;
    saveUi();
  }
  async function win(kind: "popup" | "bar", open: boolean) {
    try {
      await lyricsApi.window(kind, open, p.bar.position === "top");
      if (kind === "bar") deskBar.open = open;
    } catch (e) {
      toast(String(e), "error");
    }
  }
  function barSet<K extends keyof LyricsPrefs["bar"]>(k: K, v: LyricsPrefs["bar"][K]) {
    app.ui.lyrics.bar[k] = v;
    saveUi();
    if (k === "position" && deskBar.open) win("bar", true);
  }
  const offsets = $derived(Object.keys(p.offsets).length);
</script>

<div class="setting col">
  <div><div class="label">Page style</div><div class="desc">“Follow Music” uses Karaoke with the Tonal layout and Tiled with the Tiled one.</div></div>
  <div class="seg" role="radiogroup" aria-label="Lyrics style">
    {#each [["auto", "Follow Music"], ["tonal", "Karaoke"], ["tiled", "Tiled"], ["glass", "Glass"]] as [id, label]}
      <button role="radio" aria-checked={p.skin === id} onclick={() => set("skin", id as LyricsPrefs["skin"])}>{label}</button>
    {/each}
  </div>
</div>
<div class="setting">
  <div><div class="label">Line up</div></div>
  <div class="seg" role="radiogroup" aria-label="Line up">
    <button role="radio" aria-checked={p.align === "left"} onclick={() => set("align", "left")}>Left</button>
    <button role="radio" aria-checked={p.align === "center"} onclick={() => set("align", "center")}>Centre</button>
  </div>
</div>
<div class="setting">
  <div><div class="label">Size of the line being sung</div></div>
  <div class="row"><input type="range" min="28" max="96" step="2" value={p.size} oninput={(e) => set("size", +(e.currentTarget as HTMLInputElement).value)} aria-label="Line size" /><span class="num">{p.size}px</span></div>
</div>
<div class="setting"><div><div class="label">Word by word</div><div class="desc">Fill each word as it's sung, when the lyrics have word timings.</div></div><Toggle label="Word by word" checked={p.words} onchange={(v) => set("words", v)} /></div>
<div class="setting"><div><div class="label">Look online (LRCLIB)</div><div class="desc">When there's no .lrc file or lyrics in the song's tags, ask lrclib.net, a free and open lyrics database. Sends the title, artist, album and length.</div></div><Toggle label="Look online" checked={p.online} onchange={(v) => set("online", v)} /></div>
<div class="setting"><div><div class="label">Save .lrc beside songs</div><div class="desc">Write lyrics found online (and your timings) as .lrc files next to your own songs, so other players see them too. Never replaces a file that's there.</div></div><Toggle label="Save .lrc files" checked={p.sidecar} onchange={(v) => set("sidecar", v)} /></div>
<div class="setting"><div><div class="label">Romanise</div><div class="desc">Korean, Japanese kana, Chinese (pinyin), Cyrillic and Greek in Latin letters under each line. Done on your PC.</div></div><Toggle label="Romanise" checked={p.romanise} onchange={(v) => set("romanise", v)} /></div>
<div class="setting"><div><div class="label">Translate</div><div class="desc">A translation under each line, from MyMemory (free, no account). Sends the lyric lines; translations are kept so each song is only sent once.</div></div><Toggle label="Translate" checked={p.translate} onchange={(v) => set("translate", v)} /></div>
<div class="setting"><div><div class="label">Email for translations</div><div class="desc">Optional. MyMemory allows ten times more translation a day when it has a contact email. It's only sent to MyMemory.</div></div><input type="email" value={p.email} placeholder="you@example.com" onchange={(e) => set("email", (e.currentTarget as HTMLInputElement).value.trim())} aria-label="Email for translations" /></div>
{#if offsets}
  <div class="setting"><div><div class="label">Timing shifts</div><div class="desc">{offsets} song{offsets === 1 ? "" : "s"} with lyrics shifted earlier or later.</div></div><button onclick={() => set("offsets", {})}>Reset all</button></div>
{/if}

<h4>Pop-up lyrics</h4>
<div class="setting">
  <div><div class="label">Pop-up window</div><div class="desc">A small glass window that stays on top. Drag it anywhere.</div></div>
  <div class="row"><button onclick={() => win("popup", true)} disabled={!on}>Open</button><button onclick={() => win("popup", false)} disabled={!on}>Close</button></div>
</div>
<div class="setting">
  <div><div class="label">Pop-up tint</div></div>
  <div class="row"><input type="range" min="0" max="0.6" step="0.02" value={p.popup.tint} oninput={(e) => { app.ui.lyrics.popup.tint = +(e.currentTarget as HTMLInputElement).value; saveUi(); }} aria-label="Pop-up tint" /><span class="num">{Math.round(p.popup.tint * 100)}%</span></div>
</div>
<div class="setting">
  <div><div class="label">Desktop lyrics bar</div><div class="desc">One line floating over everything, games included. Clicks go straight through it.</div></div>
  <Toggle label="Desktop lyrics bar" checked={deskBar.open} onchange={(v) => win("bar", v)} />
</div>
<div class="setting">
  <div><div class="label">Bar position and size</div></div>
  <div class="row">
    <div class="seg" role="radiogroup" aria-label="Bar position">
      <button role="radio" aria-checked={p.bar.position === "bottom"} onclick={() => barSet("position", "bottom")}>Bottom</button>
      <button role="radio" aria-checked={p.bar.position === "top"} onclick={() => barSet("position", "top")}>Top</button>
    </div>
    <input type="range" min="16" max="56" step="1" value={p.bar.size} oninput={(e) => barSet("size", +(e.currentTarget as HTMLInputElement).value)} aria-label="Bar text size" /><span class="num">{p.bar.size}px</span>
  </div>
</div>
<div class="setting">
  <div><div class="label">Bar backing</div><div class="desc">How dark the pill behind the words is. 0% is words only.</div></div>
  <div class="row"><input type="range" min="0" max="0.9" step="0.05" value={p.bar.tint} oninput={(e) => barSet("tint", +(e.currentTarget as HTMLInputElement).value)} aria-label="Bar backing" /><span class="num">{Math.round(p.bar.tint * 100)}%</span></div>
</div>

<style>
  .setting.col { flex-direction: column; align-items: flex-start; gap: 0.6rem; }
  .row { gap: 0.6rem; }
  .num { width: 3rem; text-align: right; font-variant-numeric: tabular-nums; color: var(--muted); font-size: 0.86rem; }
  h4 { margin: 1rem 0 0.2rem; font-size: 0.9rem; }
</style>
