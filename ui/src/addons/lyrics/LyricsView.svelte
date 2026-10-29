<script lang="ts">
  // Music → Lyrics. Karaoke (Tonal), Tiled (with timestamps and sources)
  // or Glass, from the addon's setting or Music's layout.
  import { onMount } from "svelte";
  import Icon from "../../components/Icon.svelte";
  import { lyricsApi } from "../../lib/api";
  import { app, saveUi, toast } from "../../lib/state.svelte";
  import { skinFor } from "../../lib/theme";
  import Art from "../../music/Art.svelte";
  import Seek from "../../music/Seek.svelte";
  import Transport from "../../music/Transport.svelte";
  import { artUrl, ms } from "../../music/store.svelte";
  import Lines from "./Lines.svelte";
  import TimingEditor from "./TimingEditor.svelte";
  import { ly, nudge, offsetFor, reloadLyrics, targetLang, useLyrics } from "./lyrics.svelte";
  import type { LyricsPrefs } from "../../lib/theme";

  const LANGS: [string, string][] = [["en", "English"], ["es", "Spanish"], ["fr", "French"], ["de", "German"], ["it", "Italian"], ["pt", "Portuguese"], ["nl", "Dutch"], ["pl", "Polish"], ["tr", "Turkish"], ["ru", "Russian"], ["uk", "Ukrainian"], ["ar", "Arabic"], ["hi", "Hindi"], ["ja", "Japanese"], ["ko", "Korean"], ["zh", "Chinese"], ["id", "Indonesian"], ["vi", "Vietnamese"], ["th", "Thai"], ["tl", "Filipino"]];
  function setOpt<K extends keyof LyricsPrefs>(k: K, v: LyricsPrefs[K]) {
    app.ui.lyrics[k] = v;
    saveUi();
  }

  onMount(() => useLyrics());
  const p = $derived(app.ui.lyrics);
  const skin = $derived(skinFor(p.skin, app.ui.music.layout));
  const t = $derived(ms.status?.track ?? null);
  const l = $derived(ly.id === t?.id ? ly.lyrics : null);
  const offset = $derived(offsetFor(t?.id ?? null));
  let editing = $state(false);
  $effect(() => {
    void t?.id;
    editing = false;
  });

  const SOURCE: Record<string, string> = { file: ".lrc file beside the song", tags: "the song's tags", lrclib: "LRCLIB", yours: "your timing" };
  const sourceLine = $derived(l ? `${l.synced ? (l.words ? "Synced · word by word" : "Synced") : "Not synced"} · from ${SOURCE[l.source] ?? l.source}` : "");

  async function popOut() {
    try {
      await lyricsApi.window("popup", true);
    } catch (e) {
      toast(String(e), "error");
    }
  }
  async function toggleBar() {
    try {
      await lyricsApi.window("bar", !deskBar.open, p.bar.position === "top");
      deskBar.open = !deskBar.open;
    } catch (e) {
      toast(String(e), "error");
    }
  }
  function setSkin(s: typeof p.skin) {
    app.ui.lyrics.skin = s;
    saveUi();
  }
</script>

<script module lang="ts">
  /** Whether this window opened the desktop bar (it can't be clicked). */
  export const deskBar = $state({ open: false });
</script>

{#snippet tools(compact: boolean)}
  <div class="tools" class:compact>
    {#if l?.synced}
      <span class="off" role="group" aria-label="Lyrics timing">
        <button onclick={() => nudge(-0.1)} aria-label="Lyrics earlier by a tenth of a second">−</button>
        <span title="Shift for this song">{offset === 0 ? "In time" : `${offset > 0 ? "+" : ""}${offset.toFixed(1)} s`}</span>
        <button onclick={() => nudge(0.1)} aria-label="Lyrics later by a tenth of a second">+</button>
      </span>
    {/if}
    {#if l && !l.instrumental}
      <button class:on={p.romanise} aria-pressed={p.romanise} onclick={() => setOpt("romanise", !p.romanise)} title="Show non-Latin lyrics in Latin letters">Romanise</button>
      <span class="tr" class:on={p.translate}>
        <button class:on={p.translate} aria-pressed={p.translate} onclick={() => setOpt("translate", !p.translate)}>{ly.translating ? "Translating…" : "Translate"}</button>
        {#if p.translate}
          <select aria-label="Translate into" value={targetLang()} onchange={(e) => setOpt("target", (e.currentTarget as HTMLSelectElement).value)}>
            {#each LANGS as [code, name]}<option value={code}>{name}</option>{/each}
          </select>
        {/if}
      </span>
    {/if}
    <button onclick={() => (editing = true)} disabled={!t}><Icon name="brush" size={14} /> {l ? "Fix timing" : "Write lyrics"}</button>
    <button onclick={reloadLyrics} disabled={!t} aria-label="Look again"><Icon name="refresh" size={14} />{#if !compact} Look again{/if}</button>
    <span class="grow"></span>
    <button onclick={popOut} disabled={!t}><Icon name="window" size={14} /> Pop out</button>
    <button class:on={deskBar.open} aria-pressed={deskBar.open} onclick={toggleBar}><Icon name="lyrics" size={14} /> Desktop bar</button>
  </div>
{/snippet}

{#snippet body()}
  {#if !t}
    <p class="empty">Play something and its lyrics show here.</p>
  {:else if editing}
    <TimingEditor id={t.id} start={l} onclose={() => (editing = false)} />
  {:else if ly.loading}
    <p class="empty">Looking for lyrics…</p>
  {:else if ly.error}
    <p class="empty">{ly.error}</p>
  {:else if !l}
    <div class="empty">
      <p>No lyrics found for “{t.title}”.</p>
      <p class="muted small">Put a .lrc file with the same name beside the song, or <button class="link" onclick={() => (editing = true)}>write them and tap along</button>.{#if !p.online} Looking online is off in Lyrics Plus settings.{/if}</p>
    </div>
  {:else if l.instrumental}
    <p class="empty big">♪ Instrumental</p>
  {:else}
    <Lines lyrics={l} under={ly.under} stamps={skin === "tiled"} size={skin === "tiled" ? Math.min(p.size, 26) : skin === "glass" ? Math.min(p.size, 44) : null} />
  {/if}
{/snippet}

<div class="lyr {skin}" style:--c={t?.artColor ?? "var(--peach)"}>
  {#if skin === "tonal"}
    <aside class="k-side">
      {#if t}
        <div class="k-cover"><Art src={t.art} name={t.album} color={t.artColor} radius="18px" /></div>
        <div><b>{t.title}</b><span class="muted">{t.artist} · {t.album}</span></div>
        <Seek />
        <Transport />
      {/if}
    </aside>
    <section class="k-main">
      {@render tools(false)}
      <div class="k-lines">{@render body()}</div>
      {#if ly.translateError}<span class="src err">{ly.translateError}</span>{:else if l}<span class="src muted">{sourceLine}{p.translate && ly.under ? " · translated by MyMemory" : ""} · click a line to play from there</span>{/if}
    </section>
  {:else if skin === "tiled"}
    <section class="g-pane g-focus">
      <div class="g-ph"><b>lyrics</b><span>{sourceLine || "—"}</span><span class="grow"></span><span>offset {offset >= 0 ? "+" : ""}{offset.toFixed(1)}s</span></div>
      <div class="g-body">{@render body()}</div>
    </section>
    <aside class="g-pane g-side">
      <div class="g-ph"><b>sources</b><span>first match wins</span></div>
      <ol class="g-src">
        {#each [["file", ".lrc beside the song", "yours to edit"], ["tags", "Embedded tags", "lyrics in the file"], ["yours", "Your timing", "from Fix timing"], ["lrclib", "LRCLIB", p.online ? "free, open synced lyrics" : "off in settings"]] as [id, name, note]}
          <li class:hit={l?.source === id}><span>{l?.source === id ? "✓" : "·"}</span><span><span class="nm">{name}</span><span class="nt">{note}</span></span></li>
        {/each}
      </ol>
      <div class="g-ph g-top"><b>tools</b></div>
      {@render tools(true)}
    </aside>
  {:else}
    <div class="wall" style:background-image={t?.art ? `url("${artUrl(t.art)}")` : undefined} aria-hidden="true"></div>
    <section class="lg c-panel">
      <span class="sheen"></span>
      {#if t}<div class="c-head"><Art src={t.art} name={t.album} color={t.artColor} size={44} radius="12px" /><div><b>{t.title}</b><span>{t.artist}</span></div></div>{/if}
      <div class="c-lines">{@render body()}</div>
      {@render tools(true)}
    </section>
  {/if}
</div>

<div class="skins" role="radiogroup" aria-label="Lyrics style">
  {#each [["auto", "Follow Music"], ["tonal", "Karaoke"], ["tiled", "Tiled"], ["glass", "Glass"]] as [id, label]}
    <button role="radio" aria-checked={p.skin === id} onclick={() => setSkin(id as typeof p.skin)}>{label}</button>
  {/each}
</div>

<style>
  .lyr { height: calc(100% - 2.6rem); min-height: 380px; position: relative; }
  .empty { display: grid; place-items: center; align-content: center; gap: 0.4rem; height: 100%; text-align: center; color: var(--muted); }
  .empty.big { font-size: 2rem; font-weight: 800; }
  .small { font-size: 0.86rem; }
  .empty button.link { border: 0; background: none; padding: 0; font: inherit; color: var(--lilac); text-decoration: underline; text-underline-offset: 0.15em; }
  .glass .empty button.link { color: #fff; font-weight: 700; }
  .glass .empty .muted { color: rgba(255, 255, 255, 0.85); }
  .grow { flex: 1; }
  .tools { display: flex; flex-wrap: wrap; align-items: center; gap: 0.45rem; }
  .tools button { display: inline-flex; align-items: center; gap: 0.35rem; border-radius: 999px; padding: 0.4rem 0.8rem; font-size: 0.82rem; background: transparent; }
  .tools button.on { background: var(--m-accent, var(--peach)); color: var(--m-ink, var(--peach-ink)); border-color: transparent; }
  .tr { display: inline-flex; align-items: center; gap: 0.2rem; border-radius: 999px; }
  .tr.on { background: color-mix(in srgb, var(--m-accent, var(--peach)) 18%, transparent); padding-right: 0.2rem; }
  .tr select { font-size: 0.8rem; padding: 0.2rem 0.3rem; border-radius: 999px; background: transparent; border: 0; }
  .err { color: var(--rose); font-size: 0.78rem; }
  .off { display: inline-flex; align-items: center; gap: 0.2rem; border: 1px solid var(--line); border-radius: 999px; padding: 0 0.2rem; font-size: 0.82rem; }
  .off button { border: 0; padding: 0.3rem 0.55rem; }
  .off span { min-width: 3.6rem; text-align: center; font-variant-numeric: tabular-nums; }
  .skins { display: flex; gap: 0.3rem; justify-content: flex-end; margin-top: 0.8rem; }
  .skins button { font-size: 0.76rem; padding: 0.25rem 0.6rem; border-radius: 999px; background: transparent; color: var(--muted); }
  .skins button[aria-checked="true"] { color: var(--text); border-color: var(--text); }

  /* Karaoke */
  .tonal { display: grid; grid-template-columns: minmax(200px, 280px) minmax(0, 1fr); gap: 3rem; }
  .k-side { display: flex; flex-direction: column; gap: 0.9rem; justify-content: flex-end; }
  .k-side div:has(> b) { display: flex; flex-direction: column; }
  .k-side b { font-size: 1.3rem; }
  .k-cover { box-shadow: 0 24px 60px rgba(0, 0, 0, 0.4); border-radius: 18px; }
  .k-main { display: flex; flex-direction: column; gap: 0.8rem; min-height: 0; }
  .k-lines { flex: 1; min-height: 0; }
  .k-lines :global(p.now) { color: var(--c); }
  .src { font-size: 0.78rem; }

  /* Tiled */
  .tiled { display: grid; grid-template-columns: minmax(0, 1fr) 280px; gap: 10px; font-family: "JetBrains Mono Variable", Consolas, monospace; font-size: 0.84rem; }
  .g-pane { border: 1px solid var(--line); border-radius: 4px; background: var(--sunk); display: flex; flex-direction: column; min-height: 0; }
  .g-focus { border: 2px solid var(--m-accent, var(--peach)); }
  .g-ph { display: flex; gap: 0.8rem; padding: 0.45rem 0.7rem; border-bottom: 1px solid var(--line-soft); color: var(--muted); }
  .g-ph b { color: var(--m-accent, var(--peach)); }
  .g-top { border-top: 1px solid var(--line-soft); }
  .g-body { flex: 1; min-height: 0; padding: 0 1rem; font-family: var(--body); }
  .g-body :global(p.now) { background: var(--m-accent, var(--peach)); color: var(--m-ink, var(--peach-ink)); border-radius: 2px; padding: 0.15rem 0.4rem; }
  .g-src { list-style: none; margin: 0; padding: 0.4rem 0; }
  .g-src li { display: grid; grid-template-columns: 1.4rem 1fr; padding: 0.4rem 0.7rem; color: var(--muted); }
  .g-src li.hit { color: var(--text); }
  .g-src li.hit > span:first-child { color: var(--m-accent, var(--peach)); }
  .nm { display: block; }
  .nt { font-size: 0.76rem; color: var(--faint); }
  .g-side .tools { padding: 0.6rem 0.7rem; }
  .g-side .tools button { border-radius: 2px; }

  /* Glass */
  .glass { border-radius: var(--r-lg); overflow: hidden; color: #fff; font-family: "Figtree Variable", "Segoe UI", sans-serif; }
  .wall { position: absolute; inset: 0; background: var(--c) center / cover; filter: saturate(1.2); }
  .lg { position: relative; backdrop-filter: blur(22px) saturate(1.9) brightness(1.06); background: rgba(255, 255, 255, 0.16);
    box-shadow: inset 0 1.5px 0.5px rgba(255, 255, 255, 0.75), inset 0 -1px 0.5px rgba(255, 255, 255, 0.22), inset 0 0 0 1px rgba(255, 255, 255, 0.18), 0 0 0 1px rgba(0, 0, 0, 0.22), 0 14px 34px rgba(0, 0, 0, 0.28); }
  .sheen { position: absolute; inset: 0; border-radius: inherit; pointer-events: none; background: linear-gradient(135deg, rgba(255, 255, 255, 0.42), rgba(255, 255, 255, 0) 38%, rgba(255, 255, 255, 0) 72%, rgba(255, 255, 255, 0.14)); mix-blend-mode: screen; }
  .c-panel { background: rgba(24, 14, 34, 0.24); text-shadow: 0 2px 14px rgba(0, 0, 0, 0.28); }
  .c-panel :global(.lines p) { opacity: 0.55; }
  .c-panel :global(.lines p.past) { opacity: 0.4; }
  .c-panel :global(.lines p.now) { opacity: 1; }
  .c-panel { position: absolute; top: 4%; bottom: 4%; right: 4%; width: min(620px, 70%); border-radius: 36px; padding: 1.4rem 1.6rem; display: flex; flex-direction: column; gap: 0.9rem; }
  .c-head { display: flex; gap: 0.8rem; align-items: center; }
  .c-head div { display: flex; flex-direction: column; }
  .c-head span { opacity: 0.85; font-size: 0.86rem; font-weight: 600; }
  .c-lines { flex: 1; min-height: 0; }
  .glass .tools button { color: #fff; border-color: rgba(255, 255, 255, 0.3); }
  .glass .off { border-color: rgba(255, 255, 255, 0.3); }
  .glass .empty { color: rgba(255, 255, 255, 0.85); }
  @media (max-width: 900px) { .tonal { grid-template-columns: 1fr; } .k-side { display: none; } .tiled { grid-template-columns: 1fr; } .g-side { display: none; } .c-panel { left: 4%; width: auto; } }
</style>
