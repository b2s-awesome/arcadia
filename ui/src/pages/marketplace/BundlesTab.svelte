<script lang="ts">
  // Marketplace → Bundles: official bundles, and your own to share.
  import Icon from "../../components/Icon.svelte";
  import Modal from "../../components/Modal.svelte";
  import { app, saveUi, toast } from "../../lib/state.svelte";
  import { BUNDLES, addonById, type Bundle } from "../../addons/registry";
  import { SETTING_NAMES, apply, decode, encode, fromUser, installAddons, preview, snapshotSettings, type SharedBundle } from "../../addons/bundles.svelte";

  const installed = $derived(app.ui.addons.installed);
  const featured = BUNDLES[0];
  const others = BUNDLES.slice(1);

  // Ticks in the featured bundle; optional ones start unticked.
  let ticks: Record<string, boolean> = $state(Object.fromEntries(featured.addons.map((id) => [id, !featured.optional?.includes(id)])));
  const ticked = $derived(featured.addons.filter((id) => ticks[id] && !installed.includes(id)));
  let busy = $state(false);

  async function install(ids: string[], name: string) {
    busy = true;
    try {
      const added = await installAddons(ids);
      toast(added.length ? `${name}: added ${added.length} addon${added.length === 1 ? "" : "s"}. Their settings are in Appearance; Music has the rest.` : `Everything in ${name} is already installed`);
    } finally {
      busy = false;
    }
  }
  const missing = (b: Bundle) => b.addons.filter((id) => !installed.includes(id));

  // ── your bundles ──
  let makeOpen = $state(false);
  let makeName = $state("My setup");
  let makePick: Record<string, boolean> = $state({});
  let makeSettings = $state(true);
  let makeTheme = $state(false);
  function startMake() {
    makeName = "My setup";
    makePick = Object.fromEntries(installed.map((id) => [id, true]));
    makeSettings = true;
    makeTheme = installed.includes("themes");
    makeOpen = true;
  }
  function make() {
    const addons = installed.filter((id) => makePick[id]);
    if (!addons.length) return;
    app.ui.bundles = [...app.ui.bundles, { id: crypto.randomUUID(), name: makeName.trim() || "My setup", addons, settings: makeSettings ? snapshotSettings(addons, makeTheme) : null }];
    saveUi();
    makeOpen = false;
  }
  function removeMine(id: string) {
    app.ui.bundles = app.ui.bundles.filter((b) => b.id !== id);
    saveUi();
  }
  let codes: Record<string, string> = $state({});
  $effect(() => {
    for (const b of app.ui.bundles) if (!codes[b.id]) encode(fromUser($state.snapshot(b) as typeof b)).then((c) => (codes[b.id] = c));
  });
  async function copy(id: string) {
    await navigator.clipboard.writeText(codes[id] ?? "");
    toast("Code copied. Anyone with Arcadia can paste it under Add a bundle.");
  }
  function saveFile(id: string) {
    const b = app.ui.bundles.find((x) => x.id === id);
    if (!b) return;
    const blob = new Blob([JSON.stringify(fromUser($state.snapshot(b) as typeof b), null, 2)], { type: "application/json" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = `${b.name.replace(/[^\w\- ]+/g, "").trim() || "bundle"}.arcbundle`;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 2000);
  }

  // ── adding a shared bundle ──
  let code = $state("");
  let incoming: SharedBundle | null = $state(null);
  let incomingOpen = $state(false);
  let withSettings = $state(true);
  async function read(text: string) {
    try {
      incoming = await decode(text);
      withSettings = true;
      incomingOpen = true;
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e), "error");
    }
  }
  async function readFile(e: Event) {
    const f = (e.currentTarget as HTMLInputElement).files?.[0];
    (e.currentTarget as HTMLInputElement).value = "";
    if (!f) return;
    if (f.size > 512 * 1024) {
      toast("That file is too big to be a bundle.", "error");
      return;
    }
    await read(await f.text());
  }
  async function applyIncoming() {
    if (!incoming) return;
    busy = true;
    try {
      const added = await apply(incoming, withSettings);
      toast(`${incoming.name}: ${added.length ? `added ${added.length} addon${added.length === 1 ? "" : "s"}` : "nothing new to add"}${withSettings && incoming.settings ? ", settings applied" : ""}`);
      incomingOpen = false;
      code = "";
    } finally {
      busy = false;
    }
  }
  const pv = $derived(incoming ? preview(incoming) : null);
  const name = (id: string) => addonById(id)?.name ?? id;
</script>

<div class="bundles">
  <div class="main">
    <section class="featured" aria-label="Featured bundle">
      <span class="kicker">Official bundle</span>
      <div class="head">
        <div>
          <h3>{featured.name}</h3>
          <p>{featured.description}</p>
        </div>
        <button class="primary" disabled={busy || !ticked.length} onclick={() => install(ticked, featured.name)}>
          {ticked.length ? `Install ${ticked.length} addon${ticked.length === 1 ? "" : "s"}` : "All installed"}
        </button>
      </div>
      <div class="ticks">
        {#each featured.addons as id}
          {@const a = addonById(id)}
          {@const have = installed.includes(id)}
          {#if a}
            <label class="tick" class:have>
              {#if have}<input type="checkbox" checked disabled />{:else}<input type="checkbox" bind:checked={ticks[id]} />{/if}
              <span><b>{a.name}</b><span>{have ? "Installed" : a.tagline}</span></span>
            </label>
          {/if}
        {/each}
      </div>
    </section>

    <div class="others">
      {#each others as b}
        {@const left = missing(b)}
        <article class="card">
          <div class="row"><span class="kicker">Official</span><span class="grow"></span><span class="muted small">{b.addons.length} addons</span></div>
          <h4>{b.name}</h4>
          <p class="muted small">{b.description}</p>
          <div class="chips">{#each b.addons as id}<span class="chip" class:have={installed.includes(id)}>{name(id)}</span>{/each}</div>
          <button disabled={busy || !left.length} onclick={() => install(left, b.name)}>{left.length ? `Install ${left.length === b.addons.length ? "all" : `the ${left.length} missing`}` : "Installed"}</button>
        </article>
      {/each}
    </div>
  </div>

  <aside class="side" aria-label="Your bundles">
    <h3>Your bundles</h3>
    {#each app.ui.bundles as b (b.id)}
      <div class="mine">
        <div class="row"><b>{b.name}</b><span class="grow"></span><button class="x" aria-label="Delete {b.name}" onclick={() => removeMine(b.id)}><Icon name="trash" size={14} /></button></div>
        <span class="muted small">{b.addons.length} addon{b.addons.length === 1 ? "" : "s"}{b.settings ? ` · with settings${b.settings.themes ? " and theme" : ""}` : ""}</span>
        <span class="kicker">Share code</span>
        <code>{codes[b.id] ?? "…"}</code>
        <div class="row"><button onclick={() => copy(b.id)} disabled={!codes[b.id]}><Icon name="copy" size={14} /> Copy code</button><button onclick={() => saveFile(b.id)}><Icon name="save" size={14} /> Save file</button></div>
      </div>
    {/each}
    <button class="dashed" onclick={startMake} disabled={!installed.length}><Icon name="plus" size={15} /> New bundle from your addons</button>

    <div class="mine add">
      <span class="kicker">Add a bundle</span>
      <label for="bundle-code" class="small">Paste a code, or open a .arcbundle file</label>
      <form class="row" onsubmit={(e) => { e.preventDefault(); if (code.trim()) read(code); }}>
        <input id="bundle-code" bind:value={code} placeholder="arc-bundle:v1:…" spellcheck="false" />
        <button type="submit" disabled={!code.trim()}>Open</button>
      </form>
      <label class="file"><input type="file" accept=".arcbundle,.json,application/json" onchange={readFile} /> <Icon name="folder" size={14} /> Open a file…</label>
      <span class="muted small">You'll see every addon and setting it changes before anything installs. Bundles only list addons from this Marketplace; they can't carry code.</span>
    </div>
  </aside>
</div>

<Modal bind:open={makeOpen} title="New bundle">
  <label class="field"><span>Name</span><input bind:value={makeName} maxlength="60" /></label>
  <div class="picks">
    {#each installed as id}<label class="pick"><input type="checkbox" bind:checked={makePick[id]} /> {name(id)}</label>{/each}
  </div>
  <label class="pick"><input type="checkbox" bind:checked={makeSettings} /> Include my addon settings</label>
  <label class="pick"><input type="checkbox" bind:checked={makeTheme} disabled={!makeSettings || !makePick["themes"]} /> Include my theme, snippets and CSS</label>
  {#snippet actions()}
    <button onclick={() => (makeOpen = false)}>Cancel</button>
    <button class="primary" onclick={make} disabled={!installed.some((id) => makePick[id])}>Make bundle</button>
  {/snippet}
</Modal>

<Modal bind:open={incomingOpen} title={incoming?.name ?? "Bundle"}>
  {#if incoming && pv}
    {#if pv.add.length}
      <h4>Adds</h4>
      <ul>{#each pv.add as id}<li><b>{name(id)}</b> <span class="muted">{addonById(id)?.tagline}</span></li>{/each}</ul>
    {/if}
    {#if pv.have.length}<p class="muted small">Already installed: {pv.have.map(name).join(", ")}</p>{/if}
    {#if pv.settings.length}
      <label class="pick"><input type="checkbox" bind:checked={withSettings} /> Also use its settings for: {pv.settings.map((k) => SETTING_NAMES[k]).join(", ")}</label>
      {#if withSettings && pv.settings.includes("themes")}<p class="muted small">It includes CSS, which changes how Arcadia looks. If it goes wrong, hold Shift while Arcadia starts.</p>{/if}
    {/if}
    {#if !pv.add.length && !(pv.settings.length && withSettings)}<p class="muted">Nothing here you don't already have.</p>{/if}
  {/if}
  {#snippet actions()}
    <button onclick={() => (incomingOpen = false)}>Cancel</button>
    <button class="primary" disabled={busy} onclick={applyIncoming}>Add bundle</button>
  {/snippet}
</Modal>

<style>
  .bundles { display: grid; grid-template-columns: minmax(0, 1fr) 320px; gap: 1.4rem; align-items: start; }
  .main { display: flex; flex-direction: column; gap: 1rem; min-width: 0; }
  .kicker { font-size: 0.72rem; font-weight: 700; letter-spacing: 0.1em; text-transform: uppercase; color: var(--muted); }
  .small { font-size: 0.82rem; }
  .grow { flex: 1; }
  .row { display: flex; align-items: center; gap: 0.5rem; }
  .featured { padding: 1.4rem; border-radius: var(--r-lg); border: 1px solid var(--line-soft); background: linear-gradient(150deg, color-mix(in srgb, var(--peach) 30%, var(--group)), var(--group) 70%); display: flex; flex-direction: column; gap: 0.7rem; }
  .head { display: flex; align-items: flex-end; gap: 1rem; flex-wrap: wrap; }
  .head > div { flex: 1; min-width: 240px; }
  .head h3 { font-size: 2rem; letter-spacing: -0.03em; }
  .head p { color: var(--muted); max-width: 60ch; }
  .primary { background: var(--peach); color: var(--peach-ink); border-color: transparent; font-weight: 700; padding: 0.6rem 1.1rem; border-radius: 999px; }
  .primary:hover:not(:disabled) { background: var(--peach); filter: brightness(1.08); }
  .ticks { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 0.5rem; }
  .tick { display: flex; gap: 0.6rem; align-items: flex-start; padding: 0.6rem 0.7rem; border-radius: var(--r-md); background: color-mix(in srgb, var(--text) 5%, transparent); cursor: pointer; }
  .tick input { margin-top: 0.2rem; accent-color: var(--peach); }
  .tick > span { display: flex; flex-direction: column; min-width: 0; }
  .tick > span span { font-size: 0.78rem; color: var(--muted); }
  .tick.have { opacity: 0.6; cursor: default; }
  .others { display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); gap: 1rem; }
  .card { padding: 1rem; border-radius: var(--r-lg); border: 1px solid var(--line-soft); background: var(--group); display: flex; flex-direction: column; gap: 0.5rem; }
  .card h4 { font-size: 1.15rem; text-transform: none; letter-spacing: -0.01em; color: var(--text); }
  .card > button { align-self: flex-start; border-radius: 999px; }
  .chips { display: flex; flex-wrap: wrap; gap: 0.3rem; }
  .chip { font-size: 0.74rem; padding: 0.15rem 0.55rem; border-radius: 999px; background: var(--sunk); color: var(--muted); }
  .chip.have { color: var(--mint); }
  .side { display: flex; flex-direction: column; gap: 0.8rem; }
  .side h3 { font-size: 1.05rem; }
  .mine { display: flex; flex-direction: column; gap: 0.45rem; padding: 0.9rem; border-radius: var(--r-md); border: 1px solid var(--line-soft); background: var(--group); }
  code { font-size: 0.72rem; word-break: break-all; padding: 0.5rem; border-radius: var(--r-sm); background: var(--sunk); color: var(--muted); max-height: 5.4rem; overflow: hidden; }
  .mine button { display: inline-flex; align-items: center; gap: 0.35rem; font-size: 0.82rem; padding: 0.35rem 0.7rem; }
  .x { border: 0; background: transparent; color: var(--muted); }
  .dashed { border-style: dashed; display: inline-flex; gap: 0.4rem; align-items: center; justify-content: center; }
  .add form input { flex: 1; min-width: 0; }
  .file { position: relative; display: inline-flex; align-items: center; gap: 0.35rem; font-size: 0.84rem; color: var(--lilac); cursor: pointer; }
  .file input { position: absolute; width: 1px; height: 1px; opacity: 0; }
  .file:focus-within { outline: 2px solid var(--lilac); outline-offset: 2px; border-radius: 4px; }
  .field { display: grid; gap: 0.3rem; margin-bottom: 0.8rem; }
  .picks { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 0.3rem; margin-bottom: 0.8rem; }
  .pick { display: flex; gap: 0.5rem; align-items: center; font-size: 0.9rem; margin: 0.2rem 0; }
  ul { margin: 0.3rem 0 0.8rem; padding-left: 1.1rem; display: grid; gap: 0.3rem; }
  h4 { font-size: 0.78rem; letter-spacing: 0.1em; text-transform: uppercase; color: var(--faint); }
  @media (max-width: 1000px) { .bundles { grid-template-columns: 1fr; } }
</style>
