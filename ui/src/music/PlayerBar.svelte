<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import Art from "./Art.svelte";
  import Seek from "./Seek.svelte";
  import Transport from "./Transport.svelte";
  import Volume from "./Volume.svelte";
  import { albumKey, back, go, ms, setFavourite } from "./store.svelte";
  import { app } from "../lib/state.svelte";
  import { openFad } from "../addons/fad/state.svelte";
  const t = $derived(ms.status?.track ?? null);
</script>

<div class="bar">
  <div class="left">
    {#if t}
      <button class="art" onclick={() => (ms.nowPlaying = !ms.nowPlaying)} aria-label="Now Playing"><Art src={t.art} name={t.album} color={t.artColor} size={52} radius="10px" /></button>
      <div class="meta">
        <button class="title" onclick={() => go({ kind: "album", key: albumKey(t) })}>{t.title}</button>
        <button class="sub" onclick={() => go({ kind: "artist", name: t.albumArtist })}>{t.artist}</button>
      </div>
      <button class="ic" class:on={t.favourite} aria-label={t.favourite ? "Remove from favourites" : "Add to favourites"} onclick={() => setFavourite(t, !t.favourite)}><Icon name="heart" size={17} filled={t.favourite} /></button>
    {:else}
      <span class="muted idle">Nothing playing</span>
    {/if}
  </div>
  <div class="mid">
    <Transport />
    <Seek />
  </div>
  <div class="right">
    {#if app.ui.addons.installed.includes("lyrics-plus")}<button class="ic" class:on={ms.view.kind === "lyrics"} aria-label="Lyrics" title="Lyrics" onclick={() => (ms.view.kind === "lyrics" ? back() : go({ kind: "lyrics" }))}><Icon name="lyrics" size={16} /></button>{/if}
    <button class="ic" class:on={ms.nowPlaying} aria-pressed={ms.nowPlaying} aria-label="Now Playing" onclick={() => (ms.nowPlaying = !ms.nowPlaying)}><Icon name="expand" size={16} /></button>
    {#if app.ui.addons.installed.includes("full-app-display")}<button class="ic" aria-label="Full App Display (F)" title="Full App Display (F)" onclick={openFad} disabled={!t}><Icon name="fullscreen" size={16} /></button>{/if}
    <Volume />
  </div>
</div>

<style>
  .bar {
    display: grid; grid-template-columns: minmax(0, 1fr) minmax(320px, 1.3fr) minmax(0, 1fr); align-items: center; gap: 1rem;
    padding: 0.6rem 1rem; border-radius: calc(var(--r-lg) + 4px);
    background: color-mix(in srgb, var(--m-accent, var(--peach)) var(--m-tint, 14%), var(--raised));
    border: 1px solid var(--line-soft);
  }
  .left { display: flex; align-items: center; gap: 0.75rem; min-width: 0; }
  .art { border: 0; padding: 0; background: transparent; border-radius: 10px; flex-shrink: 0; }
  .meta { display: flex; flex-direction: column; min-width: 0; }
  .title, .sub { border: 0; background: transparent; padding: 0; text-align: left; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 100%; }
  .title { font-weight: 700; font-size: 0.92rem; }
  .sub { font-weight: 500; font-size: 0.8rem; color: var(--muted); }
  .title:hover, .sub:hover { text-decoration: underline; background: transparent; }
  .mid { display: flex; flex-direction: column; align-items: center; gap: 0.2rem; }
  .right { display: flex; justify-content: flex-end; align-items: center; gap: 0.4rem; }
  .ic { border: 0; background: transparent; padding: 0.35rem; color: var(--muted); display: grid; }
  .ic:hover { color: var(--text); background: transparent; }
  .ic.on { color: var(--m-accent, var(--peach)); }
  .idle { font-size: 0.88rem; padding-left: 0.4rem; }
  @media (max-width: 1020px) { .bar { grid-template-columns: minmax(0, 1fr) minmax(260px, 1.4fr) auto; } .right :global(.vol input) { width: 60px; } }
</style>
