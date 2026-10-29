<script lang="ts">
  import Icon from "../../components/Icon.svelte";
  import { app } from "../../lib/state.svelte";
  import Art from "../Art.svelte";
  import AlbumGrid from "../AlbumGrid.svelte";
  import TrackList from "../TrackList.svelte";
  import Playlists from "./Playlists.svelte";
  import { go, lib, ms, play, shuffleAll, toggle } from "../store.svelte";

  const prefs = $derived(app.ui.music);
  const sections = $derived(prefs.sections.filter((s) => !prefs.hidden.includes(s)));
  const recent = $derived(lib.albums.filter((a) => a.lastPlayed > 0).sort((a, b) => b.lastPlayed - a.lastPlayed).slice(0, 14));
  const added = $derived([...lib.albums].sort((a, b) => b.addedAt - a.addedAt).slice(0, 14));
  const most = $derived([...lib.tracks].filter((t) => t.playCount > 0).sort((a, b) => b.playCount - a.playCount).slice(0, 8));
  const favs = $derived(ms.tracks.filter((t) => t.favourite).slice(0, 8));
  const cur = $derived(ms.status?.track ?? null);
</script>

{#if !lib.tracks.length}
  <div class="empty">
    <Icon name="music" size={34} />
    <h2>No music yet</h2>
    <p>Arcadia plays the music on your PC. Point it at your music folders and it'll find every song in them.</p>
    <button class="primary" onclick={() => (ms.customise = true)}>Choose music folders</button>
  </div>
{:else}
  {#each sections as s (s)}
    {#if s === "hero"}
      <section class="hero">
        {#if cur}
          <button class="hero-art" onclick={() => (ms.nowPlaying = true)} aria-label="Open Now Playing"><Art src={cur.art} name={cur.album} color={cur.artColor} radius="var(--r-lg)" /></button>
          <div class="hero-text">
            <span class="eyebrow">{ms.status?.playing ? "Now playing" : "Paused"}</span>
            <h2>{cur.title}</h2>
            <p>{cur.artist} · {cur.album}</p>
            <div class="row">
              <button class="primary" onclick={toggle}><Icon name={ms.status?.playing ? "pause" : "play"} size={16} filled /> {ms.status?.playing ? "Pause" : "Resume"}</button>
              <button class="ghosty" onclick={() => shuffleAll(lib.tracks)}><Icon name="shuffle" size={16} /> Shuffle everything</button>
            </div>
          </div>
        {:else}
          <div class="hero-text">
            <span class="eyebrow">{lib.tracks.length.toLocaleString()} songs · {lib.albums.length.toLocaleString()} albums</span>
            <h2>What are we listening to?</h2>
            <div class="row"><button class="primary" onclick={() => shuffleAll(lib.tracks)}><Icon name="shuffle" size={16} /> Shuffle everything</button></div>
          </div>
        {/if}
      </section>
    {:else if s === "recent" && recent.length}
      <section><h3>Jump back in</h3><AlbumGrid albums={recent} row /></section>
    {:else if s === "added"}
      <section><div class="row spread"><h3>Recently added</h3><button class="more-link" onclick={() => go({ kind: "albums" })}>All albums</button></div><AlbumGrid albums={added} row /></section>
    {:else if s === "playlists" && ms.playlists.length}
      <section><div class="row spread"><h3>Playlists</h3><button class="more-link" onclick={() => go({ kind: "playlists" })}>All playlists</button></div><Playlists row /></section>
    {:else if s === "most" && most.length}
      <section><h3>Most played</h3><TrackList tracks={most} numbered onplay={(i) => play(most, i)} /></section>
    {:else if s === "favourites" && favs.length}
      <section><div class="row spread"><h3>Favourites</h3><button class="more-link" onclick={() => go({ kind: "favourites" })}>All favourites</button></div><TrackList tracks={favs} onplay={(i) => play(favs, i)} /></section>
    {/if}
  {/each}
{/if}

<style>
  section { margin-bottom: 2rem; }
  h3 { font-size: 1.1rem; margin-bottom: 0.8rem; }
  :global(:root[data-caps="true"]) section h3 { font-family: var(--body); font-size: 0.74rem; font-weight: 700; letter-spacing: 0.14em; text-transform: uppercase; color: var(--muted); }
  .row.spread { margin-bottom: 0.8rem; }
  .row.spread h3 { margin-bottom: 0; }
  .more-link { border: 0; background: transparent; color: var(--muted); font-size: 0.85rem; padding: 0.2rem 0.4rem; }
  .more-link:hover { color: var(--text); background: transparent; }
  .hero {
    display: flex; align-items: center; gap: 1.6rem; padding: 1.4rem; border-radius: calc(var(--r-lg) + 6px);
    background: linear-gradient(120deg, color-mix(in srgb, var(--m-accent, var(--peach)) 26%, var(--raised)), var(--raised) 70%);
    border: 1px solid var(--line-soft);
  }
  .hero-art { width: 150px; flex-shrink: 0; border: 0; padding: 0; background: transparent; border-radius: var(--r-lg); box-shadow: 0 18px 40px rgba(0, 0, 0, 0.35); }
  .hero-text { display: flex; flex-direction: column; gap: 0.35rem; min-width: 0; }
  .hero-text h2 { font-size: 2rem; letter-spacing: -0.02em; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .hero-text p { color: var(--muted); }
  .hero-text .row { margin-top: 0.6rem; }
  .eyebrow { font-size: 0.74rem; font-weight: 700; letter-spacing: 0.12em; text-transform: uppercase; color: var(--m-accent, var(--peach)); }
  .primary { display: inline-flex; align-items: center; gap: 0.45rem; background: var(--m-accent, var(--peach)); border-color: transparent; color: var(--m-ink, var(--peach-ink)); border-radius: 999px; padding: 0.6rem 1.2rem; }
  .primary:hover { background: var(--m-accent, var(--peach)); filter: brightness(1.08); border-color: transparent; }
  .ghosty { display: inline-flex; align-items: center; gap: 0.45rem; border-radius: 999px; padding: 0.6rem 1.1rem; background: transparent; }
  .empty { display: grid; justify-items: center; text-align: center; gap: 0.7rem; padding: 5rem 1rem; color: var(--muted); }
  .empty h2 { color: var(--text); }
  .empty p { max-width: 46ch; }
</style>
