<script lang="ts" module>
  // The open section, kept while switching pages.
  let lastView = "all";
</script>

<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { api, pickFile } from "../lib/api";
  import { toast } from "../lib/state.svelte";
  import { SOURCE_LABEL, type Game } from "../lib/types";
  import {
    lib, startLibrary, refreshLibrary, metaOf, saveMeta, saveCollections, nameOf, secondsOf, lastOf,
    coverOf, heroOf, logoOf, playtime, when, launch,
  } from "../lib/games.svelte";
  import Icon from "../components/Icon.svelte";
  import Modal from "../components/Modal.svelte";
  import TreeTabs, { type TreeNode } from "../components/TreeTabs.svelte";
  import GameEditor from "./library/GameEditor.svelte";

  onMount(startLibrary);

  let view = $state(lastView);
  $effect(() => {
    lastView = view;
  });
  let query = $state("");
  let sort = $state<"name" | "recent" | "played">(read("arcadia.lib.sort", "name") as "name" | "recent" | "played");
  let size = $state(Number(read("arcadia.lib.size", "150")) || 150);
  function read(k: string, d: string) {
    try {
      return localStorage.getItem(k) ?? d;
    } catch {
      return d;
    }
  }
  $effect(() => {
    try {
      localStorage.setItem("arcadia.lib.sort", sort);
      localStorage.setItem("arcadia.lib.size", String(size));
    } catch {
      /* private window */
    }
  });

  const counts = $derived(lib.games.reduce<Record<string, number>>((m, g) => ((m[g.source] = (m[g.source] ?? 0) + 1), m), {}));
  const shown = (g: Game) => !metaOf(g.id).hidden;
  const tree: TreeNode[] = $derived([
    { id: "all", label: "All games", icon: "library", badge: lib.games.filter(shown).length || "" },
    { id: "favourites", label: "Favourites", icon: "star", badge: lib.games.filter((g) => metaOf(g.id).favourite).length || "" },
    { id: "recent", label: "Recently played", icon: "clock" },
    { id: "collections", label: "Collections", icon: "bundle", children: lib.collections.map((c) => ({
      id: `c:${c.name}`, label: `${c.icon ? c.icon + " " : ""}${c.name}`, badge: lib.games.filter((g) => metaOf(g.id).collections.includes(c.name)).length || "",
    })) },
    { id: "launchers", label: "Launchers", icon: "gamepad", children: lib.launchers.filter((l) => counts[l]).map((l) => ({ id: `l:${l}`, label: SOURCE_LABEL[l], badge: counts[l] })) },
    ...(lib.games.some((g) => metaOf(g.id).hidden) ? [{ id: "hidden", label: "Hidden", icon: "eye-off", badge: lib.games.filter((g) => metaOf(g.id).hidden).length }] : []),
  ]);

  const visible = $derived.by(() => {
    const q = query.trim().toLowerCase();
    let list = lib.games.filter((g) => {
      const m = metaOf(g.id);
      if (view === "hidden") return m.hidden;
      if (m.hidden) return false;
      if (view === "favourites") return m.favourite;
      if (view === "recent") return lastOf(g) > 0;
      if (view.startsWith("c:")) return m.collections.includes(view.slice(2));
      if (view.startsWith("l:")) return g.source === view.slice(2);
      return true;
    });
    if (q) list = list.filter((g) => nameOf(g).toLowerCase().includes(q) || metaOf(g.id).notes.toLowerCase().includes(q));
    const by = view === "recent" ? "recent" : sort;
    return [...list].sort((a, b) =>
      by === "recent" ? lastOf(b) - lastOf(a) : by === "played" ? secondsOf(b) - secondsOf(a) : nameOf(a).localeCompare(nameOf(b)),
    );
  });

  let selectedId: string | null = $state(null);
  const selected = $derived(lib.games.find((g) => g.id === selectedId) ?? visible[0] ?? null);
  let broken = $state(new Set<string>());

  let launching = $state(false);
  async function play(g: Game) {
    launching = true;
    await launch(g);
    setTimeout(() => (launching = false), 1200);
  }

  // Editing
  let editing = $state(false);
  let editTab = $state("art");
  function edit(tab = "art") {
    editTab = tab;
    editing = true;
  }

  // Collections
  let newCol = $state("");
  async function addCollection() {
    const n = newCol.trim();
    if (!n || lib.collections.some((c) => c.name.toLowerCase() === n.toLowerCase())) return;
    await saveCollections([...lib.collections, { name: n, icon: "" }]);
    newCol = "";
    view = `c:${n}`;
  }
  const colName = $derived(view.startsWith("c:") ? view.slice(2) : null);
  let renaming = $state(false);
  let colEdit = $state({ name: "", icon: "" });
  function startRename() {
    const c = lib.collections.find((x) => x.name === colName);
    if (!c) return;
    colEdit = { ...c };
    renaming = true;
  }
  async function saveRename() {
    const old = colName!;
    const name = colEdit.name.trim();
    if (!name) return;
    // Move games across to the new name before saving the list.
    for (const g of lib.games) {
      const m = metaOf(g.id);
      if (m.collections.includes(old)) await saveMeta(g.id, { collections: m.collections.map((c) => (c === old ? name : c)) });
    }
    await saveCollections(lib.collections.map((c) => (c.name === old ? { name, icon: colEdit.icon.trim() } : c)));
    view = `c:${name}`;
    renaming = false;
  }
  async function deleteCollection() {
    await saveCollections(lib.collections.filter((c) => c.name !== colName));
    view = "all";
    renaming = false;
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
    await api.addCustomGame(newName.trim(), isUri ? { kind: "uri", uri: newPath } : { kind: "exe", path: newPath, args: [], cwd: null }, null);
    adding = false;
    newName = newPath = "";
    await refreshLibrary();
  }
  async function removeGame(g: Game) {
    await api.removeCustomGame(g.id);
    selectedId = null;
    await refreshLibrary();
  }

  const sm = $derived(selected ? metaOf(selected.id) : null);
  const secs = $derived(selected ? secondsOf(selected) : 0);
  const fromSteam = $derived(!!selected && (lib.steam[selected.id]?.[0] ?? 0) * 60 >= secs && secs > 0);
  const beatPct = $derived(sm?.beat.main ? Math.min(100, (secs / 3600 / sm.beat.main) * 100) : null);
  const label = $derived(
    view === "all" ? "All games" : view === "favourites" ? "Favourites" : view === "recent" ? "Recently played" : view === "hidden" ? "Hidden"
      : view.startsWith("c:") ? view.slice(2) : view.startsWith("l:") ? SOURCE_LABEL[view.slice(2) as Game["source"]] : "Games",
  );
  // Keep the selection inside what's shown.
  $effect(() => {
    void view;
    untrack(() => {
      if (selectedId && !visible.some((g) => g.id === selectedId)) selectedId = null;
    });
  });
</script>

<div class="library">
  <aside class="side">
    <TreeTabs key="library" label="Library" nodes={tree} bind:selected={view}>
      {#snippet foot()}
        <div class="newcol">
          <input bind:value={newCol} placeholder="New collection" onkeydown={(e) => e.key === "Enter" && addCollection()} aria-label="New collection name" />
          <button class="ghost" aria-label="Add collection" disabled={!newCol.trim()} onclick={addCollection}><Icon name="plus" size={15} /></button>
        </div>
        <button class="bp" onclick={() => (lib.bigPicture = true)}><Icon name="fullscreen" size={16} /> Big Picture</button>
      {/snippet}
    </TreeTabs>
  </aside>

  <div class="main">
    {#if selected && sm}
      {@const hero = heroOf(selected)}
      {@const logo = logoOf(selected)}
      {@const hasArt = !!hero && !broken.has(selected.id + "h")}
      <section class="hero" class:bare={!hasArt}>
        {#if hasArt}
          <img class="backdrop" src={hero} alt="" onerror={() => (broken = new Set(broken).add(selected.id + "h"))} />
        {/if}
        <div class="hero-text">
          {#if SOURCE_LABEL[selected.source] !== selected.name}
            <span class="source src-{selected.source}">{SOURCE_LABEL[selected.source]}</span>
          {/if}
          {#if logo && !broken.has(selected.id + "l")}
            <img class="logo" src={logo} alt={nameOf(selected)} onerror={() => (broken = new Set(broken).add(selected.id + "l"))} />
          {:else}
            <h1>{nameOf(selected)}</h1>
          {/if}
          <div class="stats">
            <span class="stat"><Icon name="clock" size={14} /> {playtime(secs)}{#if fromSteam}<span class="faint"> on Steam</span>{/if}</span>
            {#if lastOf(selected)}<span class="stat">Last played {when(lastOf(selected))}</span>{/if}
            {#if sm.sessions}<span class="stat">{sm.sessions} session{sm.sessions === 1 ? "" : "s"}</span>{/if}
            {#if sm.collections.length}<span class="stat">{sm.collections.join(" · ")}</span>{/if}
          </div>
          {#if beatPct !== null}
            <div class="beat" title="Main story about {sm.beat.main} h">
              <div class="track"><span style:width="{beatPct}%"></span></div>
              <span class="faint">{Math.round(beatPct)}% of the main story's {sm.beat.main} h{#if sm.beat.complete} · everything {sm.beat.complete} h{/if}</span>
            </div>
          {/if}
          {#if sm.notes}<button class="note" onclick={() => edit("notes")}><Icon name="lyrics" size={13} /> {sm.notes.split("\n")[0]}</button>{/if}
          <div class="row hero-actions">
            <button class="play" disabled={launching} onclick={() => play(selected)}>{launching ? "Starting…" : "Play"}</button>
            <button class="round" class:fav={sm.favourite} aria-pressed={sm.favourite} aria-label="Favourite" title="Favourite" onclick={() => saveMeta(selected.id, { favourite: !sm.favourite })}><Icon name="star" size={18} filled={sm.favourite} /></button>
            <button class="round" aria-label="Edit {nameOf(selected)}" title="Art, launch options, notes" onclick={() => edit("art")}><Icon name="brush" size={18} /></button>
            {#if selected.source === "custom"}
              <button class="ghost danger" onclick={() => removeGame(selected)}>Remove from library</button>
            {/if}
          </div>
        </div>
      </section>
    {/if}

    <div class="toolbar">
      <h3 class="title">{label} <span class="n">{visible.length}</span></h3>
      {#if colName}
        <button class="ghost" onclick={startRename}>Edit collection</button>
      {/if}
      <span class="grow"></span>
      <label class="search">
        <Icon name="search" size={16} />
        <input placeholder="Search games and notes" bind:value={query} aria-label="Search games" />
      </label>
      {#if view !== "recent"}
        <div class="seg" role="radiogroup" aria-label="Sort">
          <button role="radio" aria-checked={sort === "name"} onclick={() => (sort = "name")}>A–Z</button>
          <button role="radio" aria-checked={sort === "recent"} onclick={() => (sort = "recent")}>Recent</button>
          <button role="radio" aria-checked={sort === "played"} onclick={() => (sort = "played")}>Most played</button>
        </div>
      {/if}
      <input class="size" type="range" min="110" max="230" step="10" bind:value={size} aria-label="Cover size" title="Cover size" />
      <button class="ghost" onclick={refreshLibrary} title="Scan launchers again" aria-label="Refresh library"><Icon name="refresh" size={18} /></button>
      <button onclick={() => (adding = true)}><Icon name="plus" size={16} /> Add a game</button>
    </div>

    {#if lib.loading && lib.games.length === 0}
      <p class="empty">Looking for installed games…</p>
    {:else if visible.length === 0}
      <p class="empty">
        {#if query}No games match “{query}”.{:else if view === "favourites"}Star a game to keep it here.{:else if colName}Add games to {colName} from their edit menu (the brush) → Details.{:else}Nothing here yet.{/if}
      </p>
    {:else}
      <ul class="grid" style:--tile="{size}px">
        {#each visible as g (g.id)}
          {@const c = coverOf(g)}
          {@const m = metaOf(g.id)}
          <li>
            <button class="tile" class:sel={selected?.id === g.id} onclick={() => (selectedId = g.id)} ondblclick={() => play(g)} title="Double-click to play">
              <div class="cover src-{g.source}">
                {#if c && !broken.has(g.id)}
                  <img src={c} alt="" loading="lazy" onerror={() => (broken = new Set(broken).add(g.id))} />
                {:else}
                  <span class="mark">{nameOf(g)}</span>
                {/if}
                {#if m.favourite}<span class="star"><Icon name="star" size={12} filled /></span>{/if}
              </div>
              <span class="name">{nameOf(g)}</span>
              {#if secondsOf(g) >= 60}<span class="pt">{playtime(secondsOf(g))}</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</div>

{#if selected}<GameEditor game={selected} bind:open={editing} bind:tab={editTab} />{/if}

<Modal bind:open={renaming} title="Edit collection">
  <div class="row"><input style="width: 4rem; text-align: center" bind:value={colEdit.icon} placeholder="🎮" aria-label="Icon" /><input class="grow" bind:value={colEdit.name} aria-label="Name" /></div>
  {#snippet actions()}
    <button class="ghost danger" onclick={deleteCollection}>Delete collection</button>
    <span style="flex: 1"></span>
    <button class="ghost" onclick={() => (renaming = false)}>Cancel</button>
    <button onclick={saveRename}>Save</button>
  {/snippet}
</Modal>

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
  .library { height: 100%; display: grid; grid-template-columns: 220px minmax(0, 1fr); min-height: 0; }
  .side { padding: 1.4rem 0.6rem 1rem 1.2rem; overflow-y: auto; border-right: 1px solid var(--line-soft); }
  .newcol { display: flex; gap: 0.2rem; margin-top: 0.4rem; }
  .newcol input { flex: 1; min-width: 0; padding: 0.3rem 0.5rem; font-size: 0.82rem; }
  .newcol button { padding: 0.3rem 0.4rem; }
  .bp { margin-top: 0.8rem; width: 100%; display: inline-flex; align-items: center; justify-content: center; gap: 0.45rem; }
  .main { height: 100%; overflow-y: auto; padding-bottom: 4rem; min-width: 0; }

  .hero { position: relative; min-height: 300px; display: flex; align-items: flex-end; padding: 2.5rem 2.5rem 1.8rem; overflow: hidden; border-bottom: 1px solid var(--line-soft); }
  .backdrop { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; opacity: 0.4; mask-image: linear-gradient(to right, transparent 0%, #000 55%); }
  .hero.bare { min-height: 230px; }
  .hero-text { position: relative; max-width: 780px; }
  .hero h1 { font-size: clamp(2.6rem, 5.5vw, 4.8rem); line-height: 0.92; letter-spacing: -0.035em; font-weight: 800; font-variation-settings: "wdth" 85; margin: 0.5rem 0 0.7rem; text-wrap: balance; }
  .logo { display: block; max-width: min(420px, 70%); max-height: 130px; object-fit: contain; object-position: left bottom; margin: 0.6rem 0 0.8rem; filter: drop-shadow(0 4px 18px rgb(0 0 0 / 0.45)); }
  .source { font-weight: 700; font-size: 0.95rem; color: var(--src, var(--muted)); }
  .stats { display: flex; flex-wrap: wrap; gap: 0.45rem; font-size: 0.85rem; }
  .stat { display: inline-flex; align-items: center; gap: 0.35rem; padding: 0.2rem 0.65rem; border-radius: 999px; background: color-mix(in srgb, var(--raised) 75%, transparent); border: 1px solid var(--line-soft); backdrop-filter: blur(8px); }
  .beat { display: flex; align-items: center; gap: 0.7rem; margin-top: 0.7rem; font-size: 0.8rem; }
  .beat .track { width: 180px; height: 6px; border-radius: 99px; background: var(--line-soft); overflow: hidden; }
  .beat .track span { display: block; height: 100%; background: linear-gradient(90deg, var(--mint), var(--peach)); border-radius: inherit; }
  .note { margin-top: 0.7rem; max-width: 560px; display: flex; align-items: center; gap: 0.45rem; padding: 0.35rem 0.75rem; font-weight: 500; font-size: 0.85rem; color: var(--muted); background: color-mix(in srgb, var(--raised) 70%, transparent); text-align: left; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .hero-actions { margin-top: 1.1rem; gap: 0.5rem; }
  .round { width: 46px; height: 46px; padding: 0; border-radius: 50%; display: grid; place-items: center; background: color-mix(in srgb, var(--raised) 80%, transparent); }
  .round.fav { color: #ffd66b; }

  .toolbar { display: flex; align-items: center; gap: 0.6rem; padding: 1.2rem 2.5rem 0.4rem; flex-wrap: wrap; }
  .toolbar button { display: inline-flex; align-items: center; gap: 0.35rem; }
  .title { font-size: 1.1rem; }
  .n { color: var(--faint); font-weight: 500; }
  .grow { flex: 1; }
  .search { display: flex; align-items: center; gap: 0.4rem; color: var(--faint); }
  .search input { width: 190px; }
  .size { width: 90px; }

  .grid { list-style: none; margin: 0; padding: 1rem 2.5rem; display: grid; grid-template-columns: repeat(auto-fill, minmax(var(--tile, 150px), 1fr)); gap: 1.4rem 1.1rem; }
  .tile { all: unset; cursor: pointer; display: grid; gap: 0.35rem; width: 100%; }
  .tile:focus-visible .cover { outline: 2px solid var(--lilac); outline-offset: 3px; }
  .cover { aspect-ratio: 2 / 3; border-radius: var(--r-md); overflow: hidden; background: var(--raised); border: 1px solid var(--line-soft); display: grid; place-items: center; position: relative; box-shadow: inset 0 -4px 0 var(--src); transition: transform 0.15s ease; }
  .tile:hover .cover { transform: translateY(-2px); }
  .cover img { width: 100%; height: 100%; object-fit: cover; }
  .tile.sel .cover { outline: 2px solid var(--peach); outline-offset: 3px; }
  .star { position: absolute; top: 0.4rem; right: 0.4rem; width: 22px; height: 22px; border-radius: 50%; display: grid; place-items: center; color: #ffd66b; background: rgb(0 0 0 / 0.5); }
  .mark { position: absolute; left: 0.8rem; right: 0.8rem; bottom: 1rem; font-family: var(--display); font-weight: 800; font-size: 1.45rem; line-height: 1; letter-spacing: -0.03em; color: var(--src); text-wrap: balance; overflow-wrap: anywhere; }
  .name { font-weight: 600; font-size: 0.9rem; line-height: 1.25; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-top: 0.15rem; }
  .pt { font-size: 0.75rem; color: var(--faint); }

  .empty { padding: 3rem 2.5rem; color: var(--muted); }
  .field { display: grid; gap: 0.35rem; color: var(--text); font-weight: 600; font-size: 0.9rem; }
  @media (max-width: 860px) { .library { grid-template-columns: 1fr; } .side { display: none; } }

  .src-roblox { --src: #ff8fa8; }
  .src-minecraft { --src: #8fbf5a; }
  .src-steam { --src: #7fb8ff; }
  .src-epic { --src: #e6e0f5; }
  .src-battlenet { --src: #4fc3ff; }
  .src-gog { --src: #c79bff; }
  .src-ea { --src: #ff9a8a; }
  .src-ubisoft { --src: #8fd0ff; }
  .src-custom { --src: #8fe3c8; }
  :global(:root[data-scheme="light"]) .mark,
  :global(:root[data-scheme="light"]) .source { color: color-mix(in srgb, var(--src) 55%, #1a1f2e); }
  :global(:root[data-scheme="light"]) .cover { background: rgba(255, 255, 255, 0.55); }
</style>
