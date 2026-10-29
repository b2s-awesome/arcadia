<script lang="ts">
  // The tiled layout: panes with gaps, like a tiling window manager.
  import Icon from "../components/Icon.svelte";
  import { app, saveUi } from "../lib/state.svelte";
  import Art from "./Art.svelte";
  import Customise from "./Customise.svelte";
  import Queue from "./Queue.svelte";
  import Seek from "./Seek.svelte";
  import Transport from "./Transport.svelte";
  import ViewSwitch from "./ViewSwitch.svelte";
  import Volume from "./Volume.svelte";
  import { musicKeys } from "./keys";
  import { back, cycleRepeat, go, lib, ms, next, prev, quality, setShuffle, type MusicView } from "./store.svelte";
  import { streamingOn } from "./streaming.svelte";

  const prefs = $derived(app.ui.music);
  const t = $derived(ms.status?.track ?? null);
  let open: Record<string, boolean> = $state({});
  let q = $state("");
  let filterEl: HTMLInputElement | undefined = $state();

  const lists: { kind: MusicView["kind"]; label: string; icon: string }[] = $derived([
    { kind: "home", label: "home", icon: "library" },
    ...(streamingOn() ? [{ kind: "online" as const, label: "online", icon: "search" }] : []),
    { kind: "songs", label: "songs", icon: "music" },
    { kind: "albums", label: "albums", icon: "cube" },
    { kind: "artists", label: "artists", icon: "profile" },
    { kind: "favourites", label: "favourites", icon: "heart" },
    ...(streamingOn() ? [{ kind: "downloads" as const, label: "downloads", icon: "download" }] : []),
  ]);
  const title = $derived.by(() => {
    const v = ms.view;
    if (v.kind === "album") return lib.albumByKey.get(v.key)?.title ?? "album";
    if (v.kind === "artist") return v.name;
    if (v.kind === "playlist") return ms.playlists.find((p) => p.id === v.id)?.name ?? "playlist";
    if (v.kind === "search") return `/${v.q}`;
    return v.kind;
  });
  const isOn = (v: MusicView) => JSON.stringify(v) === JSON.stringify(ms.view);

  function filter() {
    if (q.trim()) {
      if (ms.view.kind === "search") ms.view = { kind: "search", q };
      else go({ kind: "search", q });
    } else if (ms.view.kind === "search") back();
  }

  function keys(e: KeyboardEvent) {
    musicKeys(e, (ev) => {
      switch (ev.key) {
        case "n": next(); return true;
        case "p": prev(); return true;
        case "s": setShuffle(!ms.status?.shuffle); return true;
        case "r": cycleRepeat(); return true;
        case "q": app.ui.music.queuePane = !prefs.queuePane; saveUi(); return true;
        case "/": filterEl?.focus(); return true;
        case "Backspace": back(); return true;
      }
      return false;
    });
  }
</script>

<svelte:window onkeydown={keys} />

<div class="tiled" class:noqueue={!prefs.queuePane}>
  <nav class="pane tree mscroll" aria-label="Library">
    <div class="ph"><span class="name">library</span><span class="faint">{ms.tracks.length.toLocaleString()}</span></div>
    <div class="items">
      {#each lists as l}
        <button class:sel={isOn({ kind: l.kind } as MusicView)} onclick={() => go({ kind: l.kind } as MusicView)}><Icon name={l.icon} size={14} /> {l.label}</button>
      {/each}
      <div class="group">playlists</div>
      {#each ms.playlists as p (p.id)}
        <button class:sel={isOn({ kind: "playlist", id: p.id })} onclick={() => go({ kind: "playlist", id: p.id })}>≡ {p.name}</button>
      {/each}
      <div class="group">artists</div>
      {#each lib.artists as a (a.name)}
        <button class="ar" class:sel={isOn({ kind: "artist", name: a.name })} onclick={() => { open[a.name] = !open[a.name]; go({ kind: "artist", name: a.name }); }}>
          <span class="tw">{open[a.name] ? "▾" : "▸"}</span> {a.name}
        </button>
        {#if open[a.name]}
          {#each a.albums as al (al.key)}
            <button class="al" class:sel={isOn({ kind: "album", key: al.key })} onclick={() => go({ kind: "album", key: al.key })}>{t && al.tracks.some((x) => x.id === t.id) ? "◉" : "○"} {al.title}</button>
          {/each}
        {/if}
      {/each}
    </div>
  </nav>

  <section class="pane main focus" aria-label={title}>
    <div class="ph">
      <button class="ic" onclick={back} disabled={!ms.history.length} aria-label="Back">←</button>
      <span class="name">{title}</span>
      <span class="grow"></span>
      <label class="filter"><span class="faint">/</span><input bind:this={filterEl} bind:value={q} oninput={filter} placeholder="filter" aria-label="Filter your music" /></label>
      <button class="ic" class:on={ms.customise} aria-label="Customise Music" onclick={() => (ms.customise = !ms.customise)}><Icon name="sliders" size={15} /></button>
    </div>
    <div class="content mscroll">
      {#if ms.loaded}<ViewSwitch />{/if}
    </div>
    {#if ms.customise}<div class="slot"><Customise /></div>{/if}
  </section>

  <aside class="pane side" aria-label="Now playing">
    <div class="ph"><span class="name">now</span>{#if t}<span class="faint">{quality(t)}</span>{/if}</div>
    {#if t}
      <div class="now">
        <button class="cover" onclick={() => go({ kind: "album", key: `${t.albumArtist}\u0000${t.album}` })} aria-label="Go to {t.album}"><Art src={t.art} name={t.album} color={t.artColor} radius="var(--r-sm)" /></button>
        <div class="tt">{t.title}</div>
        <div class="faint">{t.artist} — {t.album}{t.year ? ` (${t.year})` : ""}</div>
        <div class="chips">
          <span class:on={ms.status?.shuffle}>shuf</span>
          <span class:on={ms.status?.repeat !== "off"}>rep:{ms.status?.repeat}</span>
          {#if ms.status?.output && t.sampleRate}
            <span class:on={t.sampleRate === ms.status.output.rate}>{t.sampleRate === ms.status.output.rate ? "native rate" : `→ ${+(ms.status.output.rate / 1000).toFixed(1)}k`}</span>
          {/if}
        </div>
      </div>
    {:else}
      <p class="faint pad">nothing playing</p>
    {/if}
    {#if prefs.queuePane}
      <div class="ph sub"><span class="name">queue</span></div>
      <div class="qwrap mscroll"><Queue upcoming /></div>
    {/if}
  </aside>

  <footer class="pane bottom" aria-label="Transport">
    <Transport />
    <div class="seekw"><Seek /></div>
    <Volume />
    <div class="hints faint">
      <span><b>space</b> play</span><span><b>n/p</b> next/prev</span><span><b>←/→</b> seek</span><span><b>s</b> shuffle</span><span><b>r</b> repeat</span><span><b>q</b> queue</span><span><b>/</b> filter</span>
    </div>
  </footer>
</div>

<style>
  .tiled {
    height: 100%; min-height: 0; padding: 10px; box-sizing: border-box;
    display: grid; gap: 10px;
    grid-template-columns: 220px minmax(0, 1fr) 290px;
    grid-template-rows: minmax(0, 1fr) auto;
    grid-template-areas: "tree main side" "bottom bottom bottom";
    font-family: "JetBrains Mono Variable", Consolas, monospace; font-size: 0.82rem;
  }
  .tiled.noqueue { grid-template-columns: 220px minmax(0, 1fr) 250px; }
  .pane { background: var(--sunk); border: 1px solid var(--line); border-radius: var(--r-sm); min-height: 0; display: flex; flex-direction: column; position: relative; }
  .pane.focus { border-color: var(--m-accent, var(--peach)); box-shadow: inset 0 0 0 1px var(--m-accent, var(--peach)); }
  .tree { grid-area: tree; overflow-y: auto; }
  .main { grid-area: main; }
  .side { grid-area: side; }
  .bottom { grid-area: bottom; flex-direction: row; align-items: center; gap: 1rem; padding: 0.5rem 0.9rem; flex-wrap: wrap; }
  .ph { display: flex; align-items: center; gap: 0.6rem; padding: 0.45rem 0.7rem; border-bottom: 1px solid var(--line-soft); }
  .ph.sub { border-top: 1px solid var(--line-soft); }
  .name { color: var(--m-accent, var(--peach)); font-weight: 700; }
  .items { display: flex; flex-direction: column; padding: 0.3rem 0; }
  .items button { display: flex; align-items: center; gap: 0.45rem; border: 0; border-radius: 0; background: transparent; padding: 0.22rem 0.7rem; font: inherit; font-weight: 500; color: var(--text); text-align: left; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .items button:hover { background: color-mix(in srgb, var(--text) 7%, transparent); }
  .items button.sel { background: var(--m-accent, var(--peach)); color: var(--m-ink, var(--peach-ink)); font-weight: 700; }
  .items .al { padding-left: 1.9rem; color: var(--muted); }
  .tw { width: 0.8rem; color: var(--faint); }
  .sel .tw { color: inherit; }
  .group { color: var(--faint); padding: 0.6rem 0.7rem 0.15rem; }
  .content { flex: 1; overflow-y: auto; padding: 0.9rem 1rem 1.6rem; font-family: var(--body); font-size: 15px; }
  .filter { display: flex; align-items: center; gap: 0.35rem; border: 1px solid var(--line); padding: 0.1rem 0.45rem; }
  .filter input { border: 0; background: transparent; padding: 0.1rem 0; width: 140px; font: inherit; outline: none; }
  .ic { border: 0; background: transparent; padding: 0.15rem 0.35rem; color: var(--muted); font: inherit; display: grid; place-items: center; }
  .ic:hover:not(:disabled) { color: var(--text); background: transparent; }
  .ic.on { color: var(--m-accent, var(--peach)); }
  .now { padding: 0.8rem; display: grid; gap: 0.45rem; }
  .cover { border: 0; padding: 0; background: transparent; border-radius: var(--r-sm); }
  .tt { font-weight: 800; font-size: 0.95rem; color: var(--text); }
  .chips { display: flex; flex-wrap: wrap; gap: 0.35rem; margin-top: 0.2rem; }
  .chips span { border: 1px solid var(--line); padding: 0 0.4rem; color: var(--muted); }
  .chips span.on { border-color: var(--m-accent, var(--peach)); color: var(--m-accent, var(--peach)); }
  .qwrap { flex: 1; overflow-y: auto; padding: 0.5rem; font-family: var(--body); font-size: 14px; }
  .seekw { flex: 1; min-width: 240px; font-family: var(--body); }
  .hints { display: flex; gap: 0.9rem; flex-wrap: wrap; width: 100%; font-size: 0.74rem; }
  .hints b { color: var(--text); font-weight: 600; }
  .pad { padding: 0.8rem; }
  /* The panel sits under the pane's title bar, so its toggle stays reachable. */
  .slot { position: absolute; inset: 38px 0 0 0; pointer-events: none; font-family: var(--body); font-size: 15px; }
  .slot :global(.drawer) { pointer-events: auto; }
  @media (max-width: 1100px) { .tiled { grid-template-columns: 190px minmax(0, 1fr); grid-template-areas: "tree main" "bottom bottom"; } .side { display: none; } }
</style>
