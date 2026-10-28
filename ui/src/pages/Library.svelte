<script lang="ts">
  import { onMount } from "svelte";
  import { api, fileUrl, pickFile } from "../lib/api";
  import { toast } from "../lib/state.svelte";
  import { SOURCE_LABEL, type Game, type Source } from "../lib/types";
  import Icon from "../components/Icon.svelte";
  import Modal from "../components/Modal.svelte";

  let games: Game[] = $state([]);
  let launchers: Source[] = $state([]);
  let loading = $state(true);
  let filter: Source | "all" = $state("all");
  let query = $state("");
  let selectedId: string | null = $state(null);
  let broken = $state(new Set<string>()); // covers that failed to load

  const visible = $derived(
    games.filter(
      (g) => (filter === "all" || g.source === filter) && g.name.toLowerCase().includes(query.trim().toLowerCase()),
    ),
  );
  const selected = $derived(games.find((g) => g.id === selectedId) ?? visible[0] ?? null);
  const counts = $derived(
    games.reduce<Record<string, number>>((m, g) => ((m[g.source] = (m[g.source] ?? 0) + 1), m), {}),
  );

  async function refresh() {
    loading = true;
    try {
      const r = await api.scanLibrary();
      games = r.games;
      launchers = r.launchers;
      r.warnings.forEach((w) => toast(w, "error"));
    } catch (e) {
      toast(String(e), "error");
    } finally {
      loading = false;
    }
  }
  onMount(refresh);

  /** Steam covers may be local files from Steam's own cache. */
  const art = (u: string) => (/^https?:/.test(u) ? u : fileUrl(u));

  let launching = $state(false);
  async function play(g: Game) {
    launching = true;
    try {
      await api.launchGame(g.id);
      if (g.source !== "roblox") toast(`Opening ${g.name} through ${SOURCE_LABEL[g.source]}`);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setTimeout(() => (launching = false), 1200);
    }
  }

  // Add a game by hand.
  let adding = $state(false);
  let newName = $state("");
  let newPath = $state("");
  async function chooseExe() {
    const p = await pickFile([{ name: "Programs", extensions: ["exe", "lnk", "bat", "url"] }]);
    if (p) {
      newPath = p;
      if (!newName) newName = p.split(/[\\/]/).pop()!.replace(/\.(exe|lnk|bat|url)$/i, "");
    }
  }
  async function addGame() {
    const isUri = /^[a-z][a-z0-9+.-]*:\/\//i.test(newPath) && !/^[a-z]:\\/i.test(newPath);
    await api.addCustomGame(
      newName.trim(),
      isUri ? { kind: "uri", uri: newPath } : { kind: "exe", path: newPath, args: [], cwd: null },
      null,
    );
    adding = false;
    newName = newPath = "";
    await refresh();
  }
  async function removeGame(g: Game) {
    await api.removeCustomGame(g.id);
    selectedId = null;
    await refresh();
  }

</script>

<div class="library">
  {#if selected}
    {@const hasArt = !!selected.hero && !broken.has(selected.id + "h")}
    <section class="hero" class:bare={!hasArt}>
      {#if hasArt}
        <img class="backdrop" src={art(selected.hero!)} alt="" onerror={() => (broken = new Set(broken).add(selected.id + "h"))} />
      {/if}
      <div class="hero-text">
        {#if SOURCE_LABEL[selected.source] !== selected.name}
          <span class="source src-{selected.source}">{SOURCE_LABEL[selected.source]}</span>
        {/if}
        <h1>{selected.name}</h1>
        {#if selected.install_dir}<p class="mono faint path">{selected.install_dir}</p>{/if}
        <div class="row hero-actions">
          <button class="play" disabled={launching} onclick={() => play(selected)}>
            {launching ? "Starting…" : "Play"}
          </button>
          {#if selected.source === "custom"}
            <button class="ghost danger" onclick={() => removeGame(selected)}>Remove from library</button>
          {/if}
        </div>
      </div>
    </section>
  {/if}

  <div class="toolbar">
    <div class="chips" role="tablist" aria-label="Filter by launcher">
      <button role="tab" class="chip" aria-selected={filter === "all"} onclick={() => (filter = "all")}>
        All <span class="n">{games.length}</span>
      </button>
      {#each launchers.filter((l) => counts[l]) as l}
        <button role="tab" class="chip" aria-selected={filter === l} onclick={() => (filter = l)}>
          <span class="swatch src-{l}"></span>{SOURCE_LABEL[l]} <span class="n">{counts[l]}</span>
        </button>
      {/each}
    </div>
    <label class="search">
      <Icon name="search" size={16} />
      <input placeholder="Search games" bind:value={query} aria-label="Search games" />
    </label>
    <button class="ghost" onclick={refresh} title="Scan launchers again" aria-label="Refresh library"><Icon name="refresh" size={18} /></button>
    <button onclick={() => (adding = true)}><Icon name="plus" size={16} /> Add a game</button>
  </div>

  {#if loading && games.length === 0}
    <p class="empty">Looking for installed games…</p>
  {:else if visible.length === 0}
    <p class="empty">No games match “{query}”. Clear the search or pick another launcher.</p>
  {:else}
    <ul class="grid">
      {#each visible as g (g.id)}
        <li>
          <button
            class="tile"
            class:sel={selected?.id === g.id}
            onclick={() => (selectedId = g.id)}
            ondblclick={() => play(g)}
            title="Double-click to play"
          >
            <div class="cover src-{g.source}">
              {#if g.cover && !broken.has(g.id)}
                <img src={art(g.cover)} alt="" loading="lazy" onerror={() => (broken = new Set(broken).add(g.id))} />
              {:else}
                <span class="mark">{g.name}</span>
              {/if}
            </div>
            <span class="name">{g.name}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<Modal bind:open={adding} title="Add a game">
  <p>Pick a program, or paste a launcher link such as <span class="mono">steam://rungameid/730</span>.</p>
  <label class="field">Name<input bind:value={newName} placeholder="Minecraft" /></label>
  <label class="field">
    Program or link
    <div class="row"><input class="grow" bind:value={newPath} placeholder="C:\Games\…\game.exe" /><button onclick={chooseExe}>Browse</button></div>
  </label>
  {#snippet actions()}
    <button class="ghost" onclick={() => (adding = false)}>Cancel</button>
    <button disabled={!newName.trim() || !newPath.trim()} onclick={addGame}>Add to library</button>
  {/snippet}
</Modal>

<style>
  .library { height: 100%; overflow-y: auto; padding-bottom: 4rem; }

  .hero {
    position: relative;
    min-height: 300px;
    display: flex; align-items: flex-end;
    padding: 2.5rem 2.5rem 2rem;
    overflow: hidden;
    border-bottom: 1px solid var(--line-soft);
  }
  .backdrop {
    position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover;
    opacity: 0.38;
    mask-image: linear-gradient(to right, transparent 0%, #000 55%);
  }
  .hero.bare { min-height: 230px; }
  .hero-text { position: relative; max-width: 760px; }
  .hero h1 {
    font-size: clamp(2.8rem, 6vw, 5.2rem);
    line-height: 0.92;
    letter-spacing: -0.035em;
    font-weight: 800;
    font-variation-settings: "wdth" 85;
    margin: 0.5rem 0 0.6rem;
    text-wrap: balance;
  }
  .source { font-weight: 700; font-size: 0.95rem; color: var(--src, var(--muted)); }
  .path { font-size: 0.8rem; margin-bottom: 1.2rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .hero-actions { margin-top: 1rem; }

  .toolbar { display: flex; align-items: center; gap: 0.6rem; padding: 1.2rem 2.5rem 0.4rem; flex-wrap: wrap; }
  .chips { display: flex; gap: 0.35rem; flex-wrap: wrap; flex: 1; }
  .chip {
    display: inline-flex; align-items: center; gap: 0.45rem;
    background: transparent; border: 1px solid var(--line-soft);
    border-radius: 999px; padding: 0.3rem 0.8rem; font-size: 0.88rem; color: var(--muted);
  }
  .chip[aria-selected="true"] { background: var(--text); color: var(--ink); border-color: var(--text); }
  .chip .n { opacity: 0.6; font-variant-numeric: tabular-nums; }
  .swatch { width: 8px; height: 8px; border-radius: 2px; background: var(--src); }
  .search { display: flex; align-items: center; gap: 0.4rem; color: var(--faint); }
  .search input { width: 180px; }

  .grid {
    list-style: none; margin: 0; padding: 1rem 2.5rem;
    display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 1.4rem 1.1rem;
  }
  .tile { all: unset; cursor: pointer; display: grid; gap: 0.5rem; width: 100%; }
  .tile:focus-visible .cover { outline: 2px solid var(--lilac); outline-offset: 3px; }
  .cover {
    aspect-ratio: 2 / 3; border-radius: var(--r-md); overflow: hidden;
    background: var(--raised); border: 1px solid var(--line-soft);
    display: grid; place-items: center; position: relative;
    box-shadow: inset 0 -4px 0 var(--src);
  }
  .cover img { width: 100%; height: 100%; object-fit: cover; }
  .tile.sel .cover { outline: 2px solid var(--peach); outline-offset: 3px; }
  .mark {
    position: absolute; left: 0.8rem; right: 0.8rem; bottom: 1rem;
    font-family: var(--display); font-weight: 800; font-size: 1.55rem; line-height: 1;
    letter-spacing: -0.03em; color: var(--src); text-wrap: balance; overflow-wrap: anywhere;
  }
  .name { font-weight: 600; font-size: 0.9rem; line-height: 1.25; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .empty { padding: 3rem 2.5rem; color: var(--muted); }
  .field { display: grid; gap: 0.35rem; color: var(--text); font-weight: 600; font-size: 0.9rem; }

  /* Each launcher gets one identifying hue, used only as a small marker. */
  .src-roblox { --src: #ff8fa8; }
  .src-minecraft { --src: #8fbf5a; }
  .src-steam { --src: #7fb8ff; }
  .src-epic { --src: #e6e0f5; }
  .src-battlenet { --src: #4fc3ff; }
  .src-gog { --src: #c79bff; }
  .src-ea { --src: #ff9a8a; }
  .src-ubisoft { --src: #8fd0ff; }
  .src-custom { --src: #8fe3c8; }
  /* Light looks: deepen the launcher hues so names stay readable. */
  :global(:root[data-scheme="light"]) .mark,
  :global(:root[data-scheme="light"]) .source { color: color-mix(in srgb, var(--src) 55%, #1a1f2e); }
  :global(:root[data-scheme="light"]) .cover { background: rgba(255, 255, 255, 0.55); }
</style>
