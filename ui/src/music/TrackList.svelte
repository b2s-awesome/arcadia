<script lang="ts">
  // A list of songs. Long lists only render the rows on screen, so a
  // 20 000-song library scrolls as lightly as an album.
  import { onMount } from "svelte";
  import Icon from "../components/Icon.svelte";
  import { app } from "../lib/state.svelte";
  import type { Track } from "../lib/types";
  import Art from "./Art.svelte";
  import { openMenu, trackMenu, type MenuItem } from "./menu.svelte";
  import { albumKey, go, ms, play, setFavourite, time } from "./store.svelte";

  let {
    tracks,
    album = false,
    numbered = false,
    onplay,
    reorder,
    extra,
  }: {
    tracks: Track[];
    /** Inside an album: no cover or album column, track numbers instead. */
    album?: boolean;
    numbered?: boolean;
    onplay?: (index: number) => void;
    reorder?: (from: number, to: number) => void;
    extra?: (t: Track, index: number) => MenuItem[];
  } = $props();

  const compact = $derived(app.ui.music.compact);
  const rowH = $derived(compact ? 36 : 52);
  const VIRTUAL_FROM = 300;

  let el: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewH = $state(800);
  let scroller: HTMLElement | null = null;

  function measure() {
    if (!el || !scroller) return;
    const r = el.getBoundingClientRect();
    const s = scroller.getBoundingClientRect();
    scrollTop = s.top - r.top;
    viewH = s.height;
  }
  onMount(() => {
    scroller = el?.closest(".mscroll") ?? null;
    if (!scroller) return;
    measure();
    scroller.addEventListener("scroll", measure, { passive: true });
    const ro = new ResizeObserver(measure);
    ro.observe(scroller);
    return () => {
      scroller?.removeEventListener("scroll", measure);
      ro.disconnect();
    };
  });

  const virtual = $derived(tracks.length > VIRTUAL_FROM);
  const first = $derived(virtual ? Math.max(0, Math.floor(scrollTop / rowH) - 12) : 0);
  const last = $derived(virtual ? Math.min(tracks.length, Math.ceil((scrollTop + viewH) / rowH) + 12) : tracks.length);
  const rows = $derived(tracks.slice(first, last).map((t, i) => ({ t, i: first + i })));

  const playingId = $derived(ms.status?.track?.id ?? null);
  const isPlaying = $derived(!!ms.status?.playing);

  function start(i: number) {
    if (onplay) onplay(i);
    else play(tracks, i);
  }

  let dragFrom: number | null = $state(null);
  let dropAt: number | null = $state(null);
</script>

<div class="list" class:compact class:album bind:this={el} role="list" style:height={virtual ? `${tracks.length * rowH}px` : undefined}>
  {#if virtual}<div style:height="{first * rowH}px"></div>{/if}
  {#each rows as { t, i } (t.id + ":" + i)}
    {@const now = t.id === playingId}
    <div
      class="row"
      class:now
      class:drop={dropAt === i && dragFrom !== i}
      role="listitem"
      style:height="{rowH}px"
      draggable={reorder ? "true" : undefined}
      ondragstart={reorder ? (e) => { dragFrom = i; e.dataTransfer?.setData("text/plain", String(i)); } : undefined}
      ondragover={reorder ? (e) => { e.preventDefault(); dropAt = i; } : undefined}
      ondrop={reorder ? (e) => { e.preventDefault(); if (dragFrom !== null && dragFrom !== i) reorder(dragFrom, i); dragFrom = dropAt = null; } : undefined}
      ondragend={() => (dragFrom = dropAt = null)}
      ondblclick={() => start(i)}
      oncontextmenu={(e) => openMenu(e, trackMenu([t], extra?.(t, i) ?? []))}
    >
      <button class="num" onclick={() => start(i)} aria-label="Play {t.title}">
        {#if now && isPlaying}
          <span class="eq" aria-hidden="true"><i></i><i></i><i></i></span>
        {:else}
          <span class="n">{album || numbered ? (album ? (t.trackNo ?? i + 1) : i + 1) : ""}</span>
          <span class="p"><Icon name="play" size={14} filled /></span>
        {/if}
      </button>
      {#if !album}<Art src={t.art} name={t.album} color={t.artColor} size={compact ? 26 : 38} radius="6px" />{/if}
      <div class="main">
        <span class="title">{t.title}</span>
        {#if !compact || !album}
          <span class="sub">
            <button class="link" onclick={() => go({ kind: "artist", name: t.albumArtist })}>{t.artist}</button>
          </span>
        {/if}
      </div>
      {#if !album}
        <button class="link album-col" onclick={() => go({ kind: "album", key: albumKey(t) })}>{t.album}</button>
      {/if}
      <button class="fav" class:on={t.favourite} aria-label={t.favourite ? "Remove from favourites" : "Add to favourites"} onclick={() => setFavourite(t, !t.favourite)}>
        <Icon name="heart" size={15} filled={t.favourite} />
      </button>
      <span class="dur">{time(t.duration)}</span>
      <button class="more" aria-label="More for {t.title}" onclick={(e) => openMenu(e, trackMenu([t], extra?.(t, i) ?? []))}><Icon name="more" size={18} /></button>
    </div>
  {/each}
</div>

<style>
  .list { position: relative; }
  .row {
    display: grid; align-items: center; gap: 0.8rem;
    grid-template-columns: 34px 38px minmax(0, 1.4fr) minmax(0, 1fr) 28px 52px 30px;
    padding: 0 0.6rem; border-radius: var(--r-md);
  }
  .compact .row { grid-template-columns: 30px 26px minmax(0, 1.4fr) minmax(0, 1fr) 26px 48px 28px; gap: 0.6rem; }
  .album .row, .album.compact .row { grid-template-columns: 34px minmax(0, 1fr) 28px 52px 30px; }
  .row:hover, .row:focus-within { background: color-mix(in srgb, var(--text) 6%, transparent); }
  .row.now .title { color: var(--m-accent, var(--peach)); }
  .row.drop { box-shadow: inset 0 2px 0 var(--m-accent, var(--peach)); }
  .num { border: 0; background: transparent; padding: 0; display: grid; place-items: center; color: var(--faint); font-variant-numeric: tabular-nums; font-size: 0.85rem; height: 100%; }
  .num .p { display: none; color: var(--text); }
  .row:hover .num .n { display: none; }
  .row:hover .num .p { display: grid; }
  .main { min-width: 0; display: flex; flex-direction: column; line-height: 1.25; }
  .compact .main { flex-direction: row; gap: 0.6rem; align-items: baseline; }
  .title { font-weight: 600; font-size: 0.92rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sub { font-size: 0.8rem; color: var(--muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .link { border: 0; background: transparent; padding: 0; color: inherit; font: inherit; font-weight: inherit; text-align: left; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 100%; }
  .link:hover { text-decoration: underline; background: transparent; color: var(--text); }
  .album-col { font-size: 0.84rem; color: var(--muted); }
  .dur { font-size: 0.82rem; color: var(--muted); text-align: right; font-variant-numeric: tabular-nums; }
  .fav, .more { border: 0; background: transparent; padding: 0.2rem; display: grid; place-items: center; color: var(--faint); opacity: 0; }
  .fav.on { opacity: 1; color: var(--m-accent, var(--peach)); }
  .row:hover .fav, .row:hover .more, .fav:focus-visible, .more:focus-visible { opacity: 1; }
  .fav:hover, .more:hover { color: var(--text); background: transparent; }
  .eq { display: flex; gap: 2px; align-items: flex-end; height: 14px; }
  .eq i { width: 3px; background: var(--m-accent, var(--peach)); border-radius: 1px; animation: eq 0.9s ease-in-out infinite; }
  .eq i:nth-child(2) { animation-delay: -0.3s; }
  .eq i:nth-child(3) { animation-delay: -0.6s; }
  @keyframes eq { 0%, 100% { height: 4px; } 50% { height: 14px; } }
</style>
