<script lang="ts">
  // Themes & CSS snippets: theme packs, one-click snippets, your own CSS.
  // Shown in Appearance → Addons and in the Marketplace's Themes tab.
  import Toggle from "../../components/Toggle.svelte";
  import { app, saveUi, toast } from "../../lib/state.svelte";
  import { ask } from "../../music/contextMenu.svelte";
  import { PACKS, SNIPPETS, inSafeMode } from "./themes";

  let { wide = false }: { wide?: boolean } = $props();
  const t = $derived(app.ui.themes);

  function pickTheme(id: string | null) {
    app.ui.themes.theme = id;
    saveUi();
  }
  function snippet(id: string, on: boolean) {
    app.ui.themes.snippets = on ? [...t.snippets.filter((x) => x !== id), id] : t.snippets.filter((x) => x !== id);
    saveUi();
  }
  // The CSS applies as you type (the look effect picks it up); saving to
  // disk waits for a pause.
  let cssTimer: ReturnType<typeof setTimeout> | undefined;
  function typed(v: string) {
    app.ui.themes.css = v;
    clearTimeout(cssTimer);
    cssTimer = setTimeout(saveUi, 600);
  }
  function keys(e: KeyboardEvent) {
    const ta = e.currentTarget as HTMLTextAreaElement;
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      clearTimeout(cssTimer);
      saveUi();
      toast("CSS saved");
    } else if (e.key === "Tab") {
      // Indent instead of leaving the editor.
      e.preventDefault();
      const { selectionStart: a, selectionEnd: b, value } = ta;
      ta.value = value.slice(0, a) + "  " + value.slice(b);
      ta.selectionStart = ta.selectionEnd = a + 2;
      typed(ta.value);
    }
  }
  async function saveAsSnippet() {
    if (!t.css.trim()) return;
    const name = await ask("Name this snippet", "My snippet");
    if (!name) return;
    app.ui.themes.mine = [...t.mine, { id: crypto.randomUUID(), name, css: t.css, on: true }];
    app.ui.themes.css = "";
    saveUi();
    toast(`Saved “${name}” as a snippet`);
  }
  function mineToggle(id: string, on: boolean) {
    const m = app.ui.themes.mine.find((x) => x.id === id);
    if (m) m.on = on;
    saveUi();
  }
  function mineRemove(id: string) {
    app.ui.themes.mine = t.mine.filter((x) => x.id !== id);
    saveUi();
  }
  function mineEdit(id: string) {
    const m = t.mine.find((x) => x.id === id);
    if (!m) return;
    app.ui.themes.css = m.css;
    mineRemove(id);
  }
</script>

{#if inSafeMode()}
  <p class="safe">Safe mode: you held Shift at startup, so snippets and your CSS are off until Arcadia restarts.</p>
{/if}

<div class="themes" class:wide>
  <section aria-label="Themes">
    <h4>Theme packs</h4>
    <p class="note">Recolour the whole app on top of your Look. Your Look's accent, if you picked one, still wins.</p>
    <div class="packs">
      <button class="pack" class:on={t.theme === null} aria-pressed={t.theme === null} onclick={() => pickTheme(null)}>
        <span class="sw none" aria-hidden="true">Aa</span><b>Your Look</b>
      </button>
      {#each PACKS as p}
        <button class="pack" class:on={t.theme === p.id} aria-pressed={t.theme === p.id} onclick={() => pickTheme(p.id)}>
          <span class="sw" style:background={p.c.panel} aria-hidden="true">
            <i style:background={p.c.raised}></i><i class="ln" style:background={p.c.text}></i><i class="ac" style:background={p.c.accent}></i>
          </span>
          <b>{p.name}</b>
          <span class="dots" aria-hidden="true"><i style:background={p.c.accent}></i><i style:background={p.c.alt}></i></span>
        </button>
      {/each}
    </div>
  </section>

  <section aria-label="Snippets">
    <h4>Snippets</h4>
    <p class="note">Small tweaks, one click each.</p>
    <div class="snips">
      {#each SNIPPETS as s}
        <div class="snip"><span><b>{s.name}</b><span class="muted">{s.note}</span></span><Toggle label={s.name} checked={t.snippets.includes(s.id)} onchange={(v) => snippet(s.id, v)} /></div>
      {/each}
      {#each t.mine as m (m.id)}
        <div class="snip mine"><span><b>{m.name}</b><span class="muted">yours · <button class="link" onclick={() => mineEdit(m.id)}>edit</button> · <button class="link" onclick={() => mineRemove(m.id)}>delete</button></span></span><Toggle label={m.name} checked={m.on} onchange={(v) => mineToggle(m.id, v)} /></div>
      {/each}
    </div>
  </section>

  <section class="css" aria-label="Your CSS">
    <div class="css-head">
      <h4>Your CSS</h4>
      <span class="grow"></span>
      <label for="css-scope" class="muted small">Applies to</label>
      <select id="css-scope" value={t.scope} onchange={(e) => { app.ui.themes.scope = (e.currentTarget as HTMLSelectElement).value as "app" | "music"; saveUi(); }}>
        <option value="app">Whole app</option>
        <option value="music">Music only</option>
      </select>
    </div>
    <textarea spellcheck="false" value={t.css} oninput={(e) => typed((e.currentTarget as HTMLTextAreaElement).value)} onkeydown={keys}
      aria-label="Your CSS" placeholder={"/* Changes show as you type. Ctrl+S saves. */\n.music .tabs button {\n  font-weight: 700;\n}\n\n:root {\n  --peach: #f5a97f !important;\n}"}></textarea>
    <div class="css-foot">
      <button onclick={saveAsSnippet} disabled={!t.css.trim()}>Save as snippet</button>
      <button onclick={() => { app.ui.themes.css = ""; saveUi(); }} disabled={!t.css.trim()}>Clear</button>
      <span class="grow"></span>
      <span class="muted small">Something broke? Hold Shift while Arcadia starts.</span>
    </div>
  </section>
</div>

<style>
  .safe { padding: 0.6rem 0.8rem; border-radius: var(--r-md); background: color-mix(in srgb, var(--lilac) 12%, var(--sunk)); margin-bottom: 0.8rem; font-size: 0.9rem; }
  .themes { display: flex; flex-direction: column; gap: 1.2rem; }
  .themes.wide { display: grid; grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr) minmax(0, 1.2fr); gap: 1.2rem; align-items: start; }
  h4 { font-size: 0.95rem; margin-bottom: 0.2rem; }
  .note { color: var(--muted); font-size: 0.84rem; margin-bottom: 0.6rem; }
  .small { font-size: 0.8rem; }
  .grow { flex: 1; }
  .packs { display: grid; grid-template-columns: repeat(auto-fill, minmax(130px, 1fr)); gap: 0.6rem; }
  .pack { display: flex; flex-direction: column; gap: 0.45rem; padding: 0.55rem; border-radius: var(--r-md); text-align: left; background: transparent; position: relative; }
  .pack.on { border-color: var(--peach); box-shadow: 0 0 0 1px var(--peach); }
  .sw { height: 58px; border-radius: calc(var(--r-md) - 2px); position: relative; overflow: hidden; display: block; border: 1px solid var(--line-soft); }
  .sw.none { display: grid; place-items: center; font-weight: 800; background: var(--sunk); color: var(--muted); }
  .sw i { position: absolute; display: block; }
  .sw i:first-child { left: 8px; top: 8px; width: 30%; bottom: 8px; border-radius: 6px; }
  .sw i.ln { left: 42%; top: 12px; width: 46%; height: 7px; border-radius: 9px; }
  .sw i.ac { left: 42%; top: 27px; width: 28%; height: 7px; border-radius: 9px; }
  .dots { position: absolute; right: 0.6rem; bottom: 0.7rem; display: flex; }
  .dots i { width: 14px; height: 14px; border-radius: 50%; box-shadow: 0 0 0 2px var(--raised); }
  .dots i + i { margin-left: -4px; }
  .snips { display: flex; flex-direction: column; gap: 0.35rem; }
  .snip { display: flex; align-items: center; justify-content: space-between; gap: 0.8rem; padding: 0.5rem 0.7rem; border-radius: var(--r-md); border: 1px solid var(--line-soft); }
  .snip > span { display: flex; flex-direction: column; min-width: 0; }
  .snip > span span { font-size: 0.78rem; }
  .snip.mine { border-style: dashed; }
  .css { display: flex; flex-direction: column; border: 1px solid var(--line); border-radius: var(--r-md); overflow: hidden; }
  .css-head, .css-foot { display: flex; align-items: center; gap: 0.5rem; padding: 0.55rem 0.7rem; flex-wrap: wrap; }
  .css-head { border-bottom: 1px solid var(--line-soft); }
  .css-foot { border-top: 1px solid var(--line-soft); }
  select { padding: 0.25rem 0.5rem; }
  textarea { min-height: 260px; resize: vertical; border: 0; border-radius: 0; padding: 0.8rem; font-family: "JetBrains Mono Variable", Consolas, monospace; font-size: 0.82rem; line-height: 1.6; background: var(--sunk); tab-size: 2; }
  @media (max-width: 1100px) { .themes.wide { grid-template-columns: 1fr; } }
</style>
