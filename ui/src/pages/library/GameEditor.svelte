<script lang="ts">
  // Edit one game: art (SteamGridDB or your own files), launch options and
  // scripts, notes, name, how long to beat, collections.
  import { untrack } from "svelte";
  import { api, openUrl, pickFile } from "../../lib/api";
  import { app, toast } from "../../lib/state.svelte";
  import { art, lib, metaOf, nameOf, saveCollections, saveMeta } from "../../lib/games.svelte";
  import type { Game, GameMeta, SgdbArt, SgdbFound } from "../../lib/types";
  import Modal from "../../components/Modal.svelte";
  import Icon from "../../components/Icon.svelte";

  let { game, open = $bindable(false), tab = $bindable("art") }: { game: Game; open?: boolean; tab?: string } = $props();

  // A working copy, saved with the button.
  let m = $state<GameMeta>(untrack(() => structuredClone($state.snapshot(metaOf(game.id)))));
  // Start fresh each time it opens (the selected game may have changed).
  $effect(() => {
    if (!open) return;
    const g = game;
    untrack(() => {
      m = structuredClone($state.snapshot(metaOf(g.id)));
      term = g.name;
      found = [];
      picked = null;
      results = [];
      artError = null;
      broken = false;
    });
  });

  // ── art ──
  type Kind = "cover" | "hero" | "logo";
  let kind = $state<Kind>("cover");
  let term = $state(untrack(() => game.name));
  let found = $state<SgdbFound[]>([]);
  let picked = $state<{ id: number; steam: boolean } | null>(null);
  let results = $state<SgdbArt[]>([]);
  let busy = $state(false);
  let artError = $state<string | null>(null);
  let broken = $state(false);
  const steamId = $derived(game.source === "steam" ? Number(game.id.split(":")[1]) : null);

  async function find() {
    busy = true;
    artError = null;
    try {
      found = await api.libraryArtSearch(term);
      if (found[0] && !picked) await choose({ id: found[0].id, steam: false });
    } catch (e) {
      artError = String(e);
    } finally {
      busy = false;
    }
  }
  async function choose(p: { id: number; steam: boolean }) {
    picked = p;
    busy = true;
    artError = null;
    try {
      results = await api.libraryArt(kind, p.id, p.steam);
    } catch (e) {
      artError = String(e);
      results = [];
    } finally {
      busy = false;
    }
  }
  $effect(() => {
    // Reload art when switching cover/hero/logo.
    void kind;
    const p = untrack(() => picked);
    if (p) choose(p);
  });
  $effect(() => {
    if (open && tab === "art" && !picked && !found.length && app.settings?.steamgriddb_key) {
      untrack(() => (steamId ? choose({ id: steamId, steam: true }) : find()));
    }
  });
  async function fromFile() {
    const f = await pickFile([{ name: "Pictures", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }]);
    if (!f) return;
    try {
      m[kind] = await api.libraryArtImport(f);
    } catch (e) {
      toast(String(e), "error");
    }
  }
  const current = $derived(art(m[kind] ?? (kind === "cover" ? game.cover : kind === "hero" ? game.hero : null)));
  $effect(() => {
    void current;
    broken = false;
  });

  // ── collections ──
  let newCol = $state("");
  async function addCollection() {
    const n = newCol.trim();
    if (!n) return;
    if (!lib.collections.some((c) => c.name.toLowerCase() === n.toLowerCase())) await saveCollections([...lib.collections, { name: n, icon: "" }]);
    if (!m.collections.includes(n)) m.collections = [...m.collections, n];
    newCol = "";
  }
  function toggleCol(n: string, on: boolean) {
    m.collections = on ? [...m.collections, n] : m.collections.filter((c) => c !== n);
  }

  async function save() {
    await saveMeta(game.id, $state.snapshot(m) as GameMeta);
    open = false;
    toast(`Saved ${nameOf(game)}`);
  }
  const hltb = () => openUrl(`https://howlongtobeat.com/?q=${encodeURIComponent(game.name)}`);
  const num = (v: string) => (v.trim() === "" ? null : Math.max(0, Number(v)) || null);
  const TABS = [
    { id: "art", label: "Art", icon: "image" },
    { id: "launch", label: "Launch", icon: "play" },
    { id: "notes", label: "Notes", icon: "lyrics" },
    { id: "details", label: "Details", icon: "sliders" },
  ];
</script>

<Modal bind:open title={nameOf(game)} wide>
  <div class="tabs" role="tablist" tabindex="-1">
    {#each TABS as t}
      <button role="tab" aria-selected={tab === t.id} class:on={tab === t.id} onclick={() => (tab = t.id)}><Icon name={t.icon} size={15} /> {t.label}</button>
    {/each}
  </div>

  {#if tab === "art"}
    <div class="art">
      <div class="left">
        <div class="seg" role="radiogroup" aria-label="Which picture">
          <button role="radio" aria-checked={kind === "cover"} onclick={() => (kind = "cover")}>Cover</button>
          <button role="radio" aria-checked={kind === "hero"} onclick={() => (kind = "hero")}>Banner</button>
          <button role="radio" aria-checked={kind === "logo"} onclick={() => (kind = "logo")}>Logo</button>
        </div>
        <div class="now {kind}">
          {#if current && !broken}<img src={current} alt="" onerror={() => (broken = true)} />{:else}<span class="faint">{kind === "logo" ? "No logo: the name is shown" : "Launcher's own"}</span>{/if}
        </div>
        <div class="row">
          <button onclick={fromFile}><Icon name="upload" size={14} /> Use a file</button>
          {#if m[kind]}<button class="ghost" onclick={() => (m[kind] = null)}>Reset</button>{/if}
        </div>
      </div>
      <div class="right">
        {#if !app.settings?.steamgriddb_key}
          <div class="key">
            <strong>Find art on SteamGridDB</strong>
            <p class="muted">Thousands of community covers, banners and logos. Add a free API key from steamgriddb.com (Preferences → API) under Settings → Library.</p>
            <button class="ghost" onclick={() => openUrl("https://www.steamgriddb.com/profile/preferences/api")}>Get a key</button>
          </div>
        {:else}
          <div class="row search">
            <input class="grow" bind:value={term} onkeydown={(e) => e.key === "Enter" && find()} placeholder="Search SteamGridDB" aria-label="Search SteamGridDB" />
            <button onclick={find} disabled={busy}><Icon name="search" size={14} /> Search</button>
          </div>
          {#if found.length || steamId}
            <div class="found">
              {#if steamId}<button class="chip" class:on={picked?.steam} onclick={() => choose({ id: steamId, steam: true })}>This Steam game</button>{/if}
              {#each found.slice(0, 6) as f (f.id)}
                <button class="chip" class:on={picked && !picked.steam && picked.id === f.id} onclick={() => choose({ id: f.id, steam: false })}>{f.name}{#if f.year} <span class="faint">{f.year}</span>{/if}</button>
              {/each}
            </div>
          {/if}
          {#if artError}<p class="err">{artError}</p>{/if}
          <div class="results {kind}">
            {#each results as r (r.url)}
              <button class="pick" class:on={m[kind] === r.url} onclick={() => (m[kind] = r.url)} title="{r.width}×{r.height} {r.style}">
                <img src={r.thumb} alt="" loading="lazy" />
              </button>
            {:else}
              {#if busy}<p class="faint">Looking…</p>{:else if picked}<p class="faint">Nothing here yet for this game.</p>{/if}
            {/each}
          </div>
        {/if}
      </div>
    </div>
  {:else if tab === "launch"}
    <div class="form">
      <label class="field">
        <span class="label">Launch options</span>
        <input class="mono" bind:value={m.launchArgs} placeholder={game.source === "steam" ? "-novid -high" : "-windowed"} />
        <span class="desc">{game.source === "steam" ? "Passed to the game through Steam." : game.launch.kind === "exe" ? "Added after the program." : "This launcher doesn't take options from other apps, so these only apply to games Arcadia starts directly."}</span>
      </label>
      <label class="field">
        <span class="label">Before the game starts</span>
        <input class="mono" bind:value={m.before} placeholder={"e.g. start \"\" \"C:\\Tools\\ds4windows.exe\""} />
        <span class="desc">A command run first (Arcadia waits up to a minute for it). Runs in the game's folder.</span>
      </label>
      <label class="field">
        <span class="label">After it closes</span>
        <input class="mono" bind:value={m.after} placeholder="e.g. taskkill /IM ds4windows.exe" />
        <span class="desc">Run once Arcadia sees the game has closed.</span>
      </label>
    </div>
  {:else if tab === "notes"}
    <textarea class="notes" rows="14" bind:value={m.notes} placeholder="Where you left off, codes, builds, anything."></textarea>
  {:else}
    <div class="form">
      <label class="field">
        <span class="label">Name</span>
        <input bind:value={() => m.rename ?? "", (v) => (m.rename = v.trim() ? v : null)} placeholder={game.name} />
      </label>
      <div class="field">
        <span class="label">How long to beat <button class="ghost link" onclick={hltb}>Look it up <Icon name="link" size={12} /></button></span>
        <div class="row beat">
          <label>Main story<input type="number" min="0" step="0.5" value={m.beat.main ?? ""} oninput={(e) => (m.beat.main = num(e.currentTarget.value))} /> h</label>
          <label>+ Extras<input type="number" min="0" step="0.5" value={m.beat.extra ?? ""} oninput={(e) => (m.beat.extra = num(e.currentTarget.value))} /> h</label>
          <label>Everything<input type="number" min="0" step="0.5" value={m.beat.complete ?? ""} oninput={(e) => (m.beat.complete = num(e.currentTarget.value))} /> h</label>
        </div>
        <span class="desc">Shown beside your playtime as progress.</span>
      </div>
      <div class="field">
        <span class="label">Collections</span>
        <div class="cols">
          {#each lib.collections as c (c.name)}
            <label class="col"><input type="checkbox" checked={m.collections.includes(c.name)} onchange={(e) => toggleCol(c.name, e.currentTarget.checked)} /> {c.icon} {c.name}</label>
          {/each}
          <span class="row newcol"><input bind:value={newCol} placeholder="New collection" onkeydown={(e) => e.key === "Enter" && addCollection()} /><button class="ghost" disabled={!newCol.trim()} onclick={addCollection}><Icon name="plus" size={14} /></button></span>
        </div>
      </div>
      <label class="col"><input type="checkbox" bind:checked={m.hidden} /> Hide from the library (find it under Hidden)</label>
    </div>
  {/if}

  {#snippet actions()}
    <button class="ghost" onclick={() => (open = false)}>Cancel</button>
    <button onclick={save}>Save</button>
  {/snippet}
</Modal>

<style>
  .tabs { display: flex; gap: 0.2rem; border-bottom: 1px solid var(--line-soft); margin: -0.2rem 0 0.4rem; }
  .tabs button { display: inline-flex; align-items: center; gap: 0.4rem; border: 0; border-radius: 0; background: transparent; padding: 0.5rem 0.7rem; color: var(--muted); box-shadow: inset 0 -2px 0 transparent; }
  .tabs button.on { color: var(--text); box-shadow: inset 0 -2px 0 var(--peach); }
  .art { display: grid; grid-template-columns: 230px minmax(0, 1fr); gap: 1.2rem; min-height: 420px; }
  .left { display: flex; flex-direction: column; gap: 0.6rem; }
  .left button, .search button, .row button { display: inline-flex; align-items: center; gap: 0.35rem; }
  .now { display: grid; place-items: center; border-radius: var(--r-md); background: var(--sunk); border: 1px solid var(--line-soft); overflow: hidden; }
  .now.cover { aspect-ratio: 2 / 3; }
  .now.hero { aspect-ratio: 3.1 / 1; }
  .now.logo { aspect-ratio: 5 / 2; background: repeating-conic-gradient(var(--sunk) 0 25%, var(--raised) 0 50%) 0 0 / 16px 16px; }
  .now img { width: 100%; height: 100%; object-fit: cover; }
  .now.logo img { object-fit: contain; padding: 0.6rem; }
  .now span { font-size: 0.8rem; text-align: center; padding: 1rem; }
  .key { padding: 1.2rem; border-radius: var(--r-lg); background: var(--sunk); border: 1px dashed var(--line); display: flex; flex-direction: column; gap: 0.5rem; align-items: flex-start; color: var(--text); }
  .found { display: flex; flex-wrap: wrap; gap: 0.35rem; margin: 0.6rem 0; }
  .chip { padding: 0.25rem 0.65rem; border-radius: 999px; font-size: 0.8rem; font-weight: 500; background: var(--sunk); }
  .chip.on { background: color-mix(in srgb, var(--lilac) 20%, var(--raised)); border-color: color-mix(in srgb, var(--lilac) 50%, transparent); }
  .err { color: var(--rose); font-size: 0.85rem; }
  .results { display: grid; gap: 0.5rem; max-height: 380px; overflow: auto; padding: 2px; }
  .results.cover { grid-template-columns: repeat(auto-fill, minmax(96px, 1fr)); }
  .results.hero { grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); }
  .results.logo { grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); }
  .pick { padding: 0; overflow: hidden; border-radius: var(--r-sm); border: 2px solid transparent; background: var(--sunk); }
  .results.cover .pick { aspect-ratio: 2 / 3; }
  .results.hero .pick { aspect-ratio: 3.1 / 1; }
  .results.logo .pick { aspect-ratio: 5 / 2; background: repeating-conic-gradient(var(--sunk) 0 25%, var(--raised) 0 50%) 0 0 / 12px 12px; }
  .pick img { width: 100%; height: 100%; object-fit: cover; }
  .results.logo .pick img { object-fit: contain; padding: 0.4rem; }
  .pick.on { border-color: var(--peach); }
  .form { display: flex; flex-direction: column; gap: 1rem; color: var(--text); }
  .field { display: flex; flex-direction: column; gap: 0.3rem; }
  .label { font-weight: 600; display: flex; align-items: center; gap: 0.5rem; }
  .desc { font-size: 0.8rem; color: var(--faint); }
  .link { padding: 0.1rem 0.4rem; font-size: 0.8rem; display: inline-flex; gap: 0.3rem; align-items: center; }
  .beat label { display: inline-flex; align-items: center; gap: 0.4rem; font-size: 0.85rem; color: var(--muted); }
  .beat input { width: 4.8rem; }
  .cols { display: flex; flex-wrap: wrap; gap: 0.5rem 1rem; align-items: center; }
  .col { display: inline-flex; gap: 0.4rem; align-items: center; font-size: 0.9rem; color: var(--text); cursor: pointer; }
  .newcol input { width: 10rem; }
  .notes { width: 100%; font-size: 0.92rem; line-height: 1.5; color: var(--text); }
</style>
