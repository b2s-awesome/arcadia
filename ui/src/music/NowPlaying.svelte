<script lang="ts">
  // Big cover on one side; up next and details on the other.
  import Icon from "../components/Icon.svelte";
  import Art from "./Art.svelte";
  import Queue from "./Queue.svelte";
  import Seek from "./Seek.svelte";
  import Transport from "./Transport.svelte";
  import Volume from "./Volume.svelte";
  import { albumKey, go, longTime, ms, quality, setFavourite } from "./store.svelte";

  const t = $derived(ms.status?.track ?? null);
  const out = $derived(ms.status?.output ?? null);
  import { app } from "../lib/state.svelte";
  import NowLyrics from "../addons/lyrics/NowLyrics.svelte";
  let tab: "next" | "lyrics" | "about" = $state("next");
  const hasLyrics = $derived(app.ui.addons.installed.includes("lyrics-plus"));
  const rateNote = $derived.by(() => {
    if (!t?.sampleRate || !out) return null;
    const k = (r: number) => `${+(r / 1000).toFixed(1)} kHz`;
    return t.sampleRate === out.rate ? `Plays at its own rate, ${k(out.rate)}` : `Resampled ${k(t.sampleRate)} → ${k(out.rate)} for ${out.device}`;
  });
</script>

<div class="np">
  <button class="close" onclick={() => (ms.nowPlaying = false)} aria-label="Close Now Playing"><Icon name="collapse" size={16} /> <span>Library</span></button>
  {#if t}
    <section class="left">
      <div class="cover"><Art src={t.art} name={t.album} color={t.artColor} radius="calc(var(--r-lg) + 10px)" /></div>
      <div class="titles">
        <div class="tt">
          <h1>{t.title}</h1>
          <p>
            <button class="lnk" onclick={() => go({ kind: "artist", name: t.albumArtist })}>{t.artist}</button><span class="sep">·</span><button class="lnk" onclick={() => go({ kind: "album", key: albumKey(t) })}>{t.album}</button>{#if t.year}<span class="sep">·</span>{t.year}{/if}
          </p>
        </div>
        <button class="fav" class:on={t.favourite} aria-label={t.favourite ? "Remove from favourites" : "Add to favourites"} onclick={() => setFavourite(t, !t.favourite)}><Icon name="heart" size={22} filled={t.favourite} /></button>
      </div>
      <Seek />
      <Transport big />
      <div class="under"><span class="badge">{quality(t)}</span><Volume /></div>
    </section>
    <section class="right">
      <div class="seg" role="radiogroup" aria-label="Now Playing">
        <button role="radio" aria-checked={tab === "next"} onclick={() => (tab = "next")}>Up next</button>
        {#if hasLyrics}<button role="radio" aria-checked={tab === "lyrics"} onclick={() => (tab = "lyrics")}>Lyrics</button>{/if}
        <button role="radio" aria-checked={tab === "about"} onclick={() => (tab = "about")}>Details</button>
      </div>
      <div class="panel mscroll">
        {#if tab === "next"}
          <Queue />
        {:else if tab === "lyrics" && hasLyrics}
          <NowLyrics />
        {:else}
          <dl>
            <dt>Format</dt><dd>{quality(t)}{t.channels ? ` · ${t.channels === 1 ? "mono" : t.channels === 2 ? "stereo" : `${t.channels} channels`}` : ""}{t.bitrate ? ` · ${t.bitrate} kbps` : ""}</dd>
            {#if rateNote}<dt>Output</dt><dd>{rateNote}</dd>{/if}
            <dt>Length</dt><dd>{longTime(t.duration)}</dd>
            <dt>Plays</dt><dd>{t.playCount}{t.lastPlayed ? `, last ${new Date(t.lastPlayed * 1000).toLocaleDateString()}` : ""}</dd>
            {#if t.genre}<dt>Genre</dt><dd>{t.genre}</dd>{/if}
            <dt>Added</dt><dd>{new Date(t.addedAt * 1000).toLocaleDateString()}</dd>
            <dt>File</dt><dd class="path">{t.path}</dd>
          </dl>
        {/if}
      </div>
    </section>
  {:else}
    <p class="muted nothing">Nothing playing yet.</p>
  {/if}
</div>

<style>
  .np {
    position: relative; height: 100%; display: grid; grid-template-columns: minmax(300px, 0.95fr) minmax(300px, 1fr); gap: 3rem;
    padding: 3.2rem 3rem 1.4rem; overflow: hidden; box-sizing: border-box;
    background: linear-gradient(160deg, color-mix(in srgb, var(--m-accent, var(--peach)) 30%, var(--ink)) 0%, var(--ink) 75%);
  }
  .close { position: absolute; left: 1.2rem; top: 1rem; display: inline-flex; align-items: center; gap: 0.35rem; border-radius: 999px; padding: 0.35rem 0.8rem; font-size: 0.82rem; background: color-mix(in srgb, var(--text) 8%, transparent); border-color: transparent; }
  .left { display: flex; flex-direction: column; justify-content: center; gap: 1.1rem; min-width: 0; max-width: 520px; justify-self: center; width: 100%; }
  .cover { width: 100%; max-width: min(420px, calc(100vh - 500px)); align-self: center; box-shadow: 0 30px 80px rgba(0, 0, 0, 0.5); border-radius: calc(var(--r-lg) + 10px); }
  .titles { display: flex; align-items: flex-end; justify-content: space-between; gap: 1rem; }
  .tt { min-width: 0; }
  h1 { font-size: clamp(1.5rem, 2.6vw, 2.2rem); letter-spacing: -0.02em; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  p { color: var(--muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sep { margin: 0 0.4em; }
  .lnk { border: 0; background: transparent; padding: 0; color: inherit; font: inherit; }
  .lnk:hover { text-decoration: underline; background: transparent; color: var(--text); }
  .fav { border: 0; background: transparent; padding: 0.3rem; color: var(--muted); display: grid; }
  .fav.on { color: var(--m-accent, var(--peach)); }
  .fav:hover { background: transparent; color: var(--text); }
  .under { display: flex; align-items: center; justify-content: space-between; }
  .badge { font-size: 0.7rem; font-weight: 700; letter-spacing: 0.08em; padding: 0.2rem 0.55rem; border-radius: 999px; background: color-mix(in srgb, var(--text) 10%, transparent); color: var(--muted); }
  .right { display: flex; flex-direction: column; gap: 1rem; min-height: 0; }
  .panel { flex: 1; min-height: 0; overflow-y: auto; padding-right: 0.3rem; }
  dl { display: grid; grid-template-columns: 6rem 1fr; gap: 0.6rem 1rem; margin: 0; font-size: 0.9rem; }
  dt { color: var(--muted); }
  dd { margin: 0; overflow-wrap: anywhere; }
  .path { font-family: "JetBrains Mono Variable", Consolas, monospace; font-size: 0.78rem; color: var(--muted); user-select: text; }
  .nothing { grid-column: 1 / -1; text-align: center; align-self: center; }
  @media (max-width: 980px) { .np { grid-template-columns: 1fr; overflow-y: auto; } .right { min-height: 320px; } }
</style>
