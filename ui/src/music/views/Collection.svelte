<script lang="ts">
  // The header above an album, artist or playlist: cover, title, actions.
  import type { Snippet } from "svelte";
  import Icon from "../../components/Icon.svelte";
  import type { Track } from "../../lib/types";
  import Art from "../Art.svelte";
  import { openMenu, trackMenu, type MenuItem } from "../menu.svelte";
  import { longTime, play, shuffleAll } from "../store.svelte";
  import { app } from "../../lib/state.svelte";
  import SortPlay from "../../addons/listening/SortPlay.svelte";

  let { kind, title, sub, art, color, tracks, round = false, more = [], children }: {
    kind: string; title: string; sub?: string; art: string | null; color: string | null; tracks: Track[];
    round?: boolean; more?: MenuItem[]; children?: Snippet;
  } = $props();
  const total = $derived(tracks.reduce((a, t) => a + t.duration, 0));
  const line = $derived([sub, `${tracks.length} ${tracks.length === 1 ? "song" : "songs"}`, longTime(total)].filter(Boolean).join(" · "));
</script>

<header class="head" style:--c={color ?? "var(--m-accent, var(--peach))"}>
  <div class="cover"><Art src={art} name={title} {color} {round} radius="var(--r-lg)" /></div>
  <div class="text">
    <span class="kind">{kind}</span>
    <h1>{title}</h1>
    <p>{line}</p>
    <div class="row wrap">
      <button class="primary" disabled={!tracks.length} onclick={() => play(tracks, 0)}><Icon name="play" size={16} filled /> Play</button>
      <button class="ghosty" disabled={!tracks.length} onclick={() => shuffleAll(tracks)}><Icon name="shuffle" size={16} /> Shuffle</button>
      {#if app.ui.addons.installed.includes("sort-play")}<SortPlay {tracks} />{/if}
      <button class="ghosty icon" aria-label="More" disabled={!tracks.length && !more.length} onclick={(e) => openMenu(e, [...trackMenu(tracks), ...more])}><Icon name="more" size={18} /></button>
      {@render children?.()}
    </div>
  </div>
</header>

<style>
  .head {
    display: flex; align-items: flex-end; gap: 1.6rem; margin: -0.4rem -0.6rem 1.4rem; padding: 1.2rem 0.6rem 1.2rem;
    border-radius: calc(var(--r-lg) + 6px);
    background: linear-gradient(180deg, color-mix(in srgb, var(--c) 22%, transparent), transparent);
  }
  .cover { width: 200px; flex-shrink: 0; box-shadow: 0 20px 50px rgba(0, 0, 0, 0.35); border-radius: var(--r-lg); }
  .text { display: flex; flex-direction: column; gap: 0.4rem; min-width: 0; }
  .kind { font-size: 0.74rem; font-weight: 700; letter-spacing: 0.12em; text-transform: uppercase; color: var(--muted); }
  h1 { font-size: clamp(1.8rem, 3.2vw, 3rem); letter-spacing: -0.03em; line-height: 1.02; overflow-wrap: anywhere; }
  p { color: var(--muted); }
  .row { margin-top: 0.5rem; gap: 0.5rem; }
  .primary { display: inline-flex; align-items: center; gap: 0.45rem; background: var(--m-accent, var(--peach)); border-color: transparent; color: var(--m-ink, var(--peach-ink)); border-radius: 999px; padding: 0.6rem 1.3rem; }
  .primary:hover { background: var(--m-accent, var(--peach)); filter: brightness(1.08); border-color: transparent; }
  .ghosty { display: inline-flex; align-items: center; gap: 0.45rem; border-radius: 999px; padding: 0.6rem 1.1rem; background: transparent; }
  .ghosty.icon { padding: 0.55rem; }
  @media (max-width: 900px) { .cover { width: 140px; } }
</style>
