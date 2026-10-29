<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "../components/Icon.svelte";
  import { app } from "../lib/state.svelte";
  import Customise from "./Customise.svelte";
  import Menu from "./Menu.svelte";
  import NowPlaying from "./NowPlaying.svelte";
  import PlayerBar from "./PlayerBar.svelte";
  import Tiled from "./Tiled.svelte";
  import ViewSwitch from "./ViewSwitch.svelte";
  import { inkFor } from "./colour";
  import { musicKeys } from "./keys";
  import { back, go, initMusic, ms, type MusicView } from "./store.svelte";
  import { initStreaming, streamingOn, sx } from "./streaming.svelte";

  onMount(() => {
    initMusic();
    initStreaming();
  });
  const busy = $derived(sx.jobs.filter((j) => j.state === "running" || j.state === "queued").length);

  const prefs = $derived(app.ui.music);
  // Colour from the playing cover, or the look's accent.
  const accent = $derived(prefs.artColours ? (ms.status?.track?.artColor ?? null) : null);

  const tabs: { kind: MusicView["kind"]; label: string }[] = $derived([
    { kind: "home", label: "Home" },
    ...(streamingOn() ? [{ kind: "online" as const, label: "Online" }] : []),
    { kind: "albums", label: "Albums" },
    { kind: "artists", label: "Artists" },
    { kind: "songs", label: "Songs" },
    { kind: "playlists", label: "Playlists" },
    { kind: "favourites", label: "Favourites" },
    ...(streamingOn() ? [{ kind: "downloads" as const, label: "Downloads" }] : []),
  ]);
  const tabFor = (k: MusicView["kind"]): MusicView["kind"] =>
    k === "album" ? "albums" : k === "artist" ? "artists" : k === "playlist" ? "playlists" : k;

  let q = $state("");
  let searchEl: HTMLInputElement | undefined = $state();
  let qTimer: ReturnType<typeof setTimeout> | undefined;
  function onSearch() {
    clearTimeout(qTimer);
    qTimer = setTimeout(() => {
      if (q.trim()) {
        if (ms.view.kind === "search") ms.view = { kind: "search", q };
        else go({ kind: "search", q });
      } else if (ms.view.kind === "search") back();
    }, 120);
  }
  $effect(() => {
    if (ms.view.kind !== "search") q = "";
  });

  let scroller: HTMLDivElement | undefined = $state();
  // Fade the tab row's edges only when it overflows.
  let tabsEl: HTMLElement | undefined = $state();
  let tabsOverflow = $state(false);
  $effect(() => {
    const el = tabsEl;
    if (!el) return;
    const check = () => (tabsOverflow = el.scrollWidth > el.clientWidth + 1);
    const ro = new ResizeObserver(check);
    ro.observe(el);
    check();
    return () => ro.disconnect();
  });
  $effect(() => {
    void ms.view;
    scroller?.scrollTo({ top: 0 });
  });

  function keys(e: KeyboardEvent) {
    musicKeys(e, (ev) => {
      if (ev.key === "/") {
        searchEl?.focus();
        return true;
      }
      if (ev.key === "Escape" && ms.nowPlaying) {
        ms.nowPlaying = false;
        return true;
      }
      return false;
    });
  }
</script>

<svelte:window onkeydown={keys} />

<div class="music" style:--m-accent={accent ?? undefined} style:--m-ink={accent ? inkFor(accent) : undefined} style:--m-tint={prefs.artColours ? "16%" : "0%"}>
  {#if prefs.layout === "tiled"}
    <Tiled />
  {:else}
    <div class="tonal" class:bar-top={prefs.bar === "top"}>
      <header class="head">
        <button class="back" onclick={back} disabled={!ms.history.length} aria-label="Back"><Icon name="up" size={16} /></button>
        <h1>Music</h1>
        <nav class="tabs" class:fade={tabsOverflow} aria-label="Music sections" bind:this={tabsEl}>
          {#each tabs as t}
            <button class:on={tabFor(ms.view.kind) === t.kind} aria-current={tabFor(ms.view.kind) === t.kind ? "page" : undefined} onclick={() => go({ kind: t.kind } as MusicView)}>{t.label}{#if t.kind === "downloads" && busy}<span class="badge">{busy}</span>{/if}</button>
          {/each}
        </nav>
        <span class="grow"></span>
        <label class="search">
          <Icon name="search" size={15} />
          <input bind:this={searchEl} bind:value={q} oninput={onSearch} placeholder="Search your music" aria-label="Search your music" />
          <kbd>/</kbd>
        </label>
        <button class="cust" class:on={ms.customise} aria-pressed={ms.customise} aria-label="Customise Music" title="Customise Music" onclick={() => (ms.customise = !ms.customise)}><Icon name="sliders" size={17} /></button>
      </header>

      <div class="body">
        <div class="scroll mscroll" bind:this={scroller}>
          {#if ms.loaded}<ViewSwitch />{/if}
        </div>
        {#if ms.customise}<Customise />{/if}
      </div>

      <div class="player"><PlayerBar /></div>
      {#if ms.nowPlaying}
        <!-- Covers the library and the player bar: it has its own controls. -->
        <div class="np-layer"><NowPlaying /></div>
      {/if}
    </div>
  {/if}
  <Menu />
</div>

<style>
  .music { height: 100%; min-height: 0; position: relative; }
  .tonal {
    height: 100%; display: grid; grid-template-rows: auto minmax(0, 1fr) auto; grid-template-columns: minmax(0, 1fr); min-height: 0;
    background: linear-gradient(180deg, color-mix(in srgb, var(--m-accent, var(--peach)) calc(var(--m-tint) * 0.6), transparent), transparent 340px);
  }
  /* Explicit rows, so Now Playing can lie over both the library and the bar. */
  .head { grid-row: 1; grid-column: 1; }
  .body { grid-row: 2; grid-column: 1; }
  .player { grid-row: 3; grid-column: 1; }
  .tonal.bar-top { grid-template-rows: auto auto minmax(0, 1fr); }
  .tonal.bar-top .player { grid-row: 2; padding: 0 1.4rem 0.6rem; }
  .tonal.bar-top .body { grid-row: 3; }
  .head { display: flex; align-items: center; gap: 0.8rem; padding: 1.3rem 1.6rem 0.8rem; min-width: 0; }
  .head h1 { font-size: 1.6rem; margin-right: 0.4rem; }
  .back { border: 0; background: transparent; padding: 0.3rem; color: var(--muted); transform: rotate(-90deg); display: grid; }
  .back:disabled { opacity: 0.3; }
  .back:hover:not(:disabled) { color: var(--text); background: transparent; }
  /* One row: tabs scroll sideways when there are many, rather than wrapping. */
  .tabs { display: flex; gap: 0.3rem; min-width: 0; overflow-x: auto; scrollbar-width: none; flex: 0 1 auto; }
  .tabs::-webkit-scrollbar { display: none; }
  .tabs button { flex-shrink: 0; }
  .tabs.fade { mask-image: linear-gradient(90deg, transparent, #000 14px, #000 calc(100% - 14px), transparent); padding: 0 10px; }
  .tabs button { border-radius: 999px; padding: 0.4rem 0.8rem; font-size: 0.85rem; border: 1px solid var(--line); background: transparent; color: var(--muted); }
  .tabs button:hover { color: var(--text); }
  .badge { margin-left: 0.35rem; font-size: 0.7rem; padding: 0 0.4rem; border-radius: 999px; background: var(--m-accent, var(--peach)); color: var(--m-ink, var(--peach-ink)); }
  .tabs button.on .badge { background: var(--m-ink, var(--peach-ink)); color: var(--m-accent, var(--peach)); }
  .tabs button.on { background: var(--m-accent, var(--peach)); border-color: transparent; color: var(--m-ink, var(--peach-ink)); }
  .search { display: flex; align-items: center; gap: 0.5rem; padding: 0.4rem 0.7rem; border-radius: 999px; background: var(--sunk); border: 1px solid var(--line); color: var(--faint); width: clamp(150px, 17vw, 250px); flex-shrink: 0; }
  .search:focus-within { border-color: var(--lilac); }
  .search input { border: 0; background: transparent; padding: 0; flex: 1; min-width: 0; outline: none; }
  .search kbd { font: inherit; font-size: 0.7rem; border: 1px solid var(--line); border-radius: 5px; padding: 0 0.35rem; }
  .cust { flex-shrink: 0; border-radius: 50%; width: 36px; height: 36px; padding: 0; display: grid; place-items: center; }
  .cust.on { background: var(--m-accent, var(--peach)); color: var(--m-ink, var(--peach-ink)); border-color: transparent; }
  .body { position: relative; min-height: 0; min-width: 0; }
  .scroll { height: 100%; overflow-y: auto; padding: 0.4rem 1.6rem 2rem; }
  .np-layer { grid-row: 2 / 4; grid-column: 1; position: relative; z-index: 10; min-height: 0; animation: rise 0.22s cubic-bezier(0.2, 0.8, 0.2, 1); }
  @keyframes rise { from { opacity: 0; transform: translateY(16px); } }
  .player { padding: 0.6rem 1rem 0.9rem; }
</style>
