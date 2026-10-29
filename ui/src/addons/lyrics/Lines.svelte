<script lang="ts">
  // The scrolling lyric lines, shared by every lyrics style. Follows the
  // song, unless you've just scrolled yourself; click a line to go there.
  import { app } from "../../lib/state.svelte";
  import type { Lyrics } from "../../lib/types";
  import { ms, seek, time } from "../../music/store.svelte";
  import { lineAt, ly, lyricTime, offsetFor, wordProgress } from "./lyrics.svelte";

  let { lyrics, stamps = false, size = null, align = null, fade = true }: {
    lyrics: Lyrics; stamps?: boolean; size?: number | null; align?: "left" | "center" | null; fade?: boolean;
  } = $props();
  const p = $derived(app.ui.lyrics);
  const t = $derived(lyricTime());
  const cur = $derived(lyrics.synced ? lineAt(lyrics.lines, t) : -1);
  const big = $derived(size ?? p.size);

  let box: HTMLDivElement | undefined = $state();
  let userScrolled = 0;
  $effect(() => {
    const i = cur;
    if (!box || i < 0 || performance.now() - userScrolled < 3000) return;
    const el = box.querySelector<HTMLElement>(`[data-i="${i}"]`);
    if (!el) return;
    const top = el.offsetTop - box.clientHeight / 2 + el.offsetHeight / 2;
    box.scrollTo({ top, behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" });
  });

  function jump(lt: number | null) {
    if (lt === null || !ms.status?.track) return;
    userScrolled = 0;
    seek(Math.max(0, lt - offsetFor(ly.id) + 0.01));
  }
</script>

<div
  role="region" aria-label="Lyrics"
  class="lines" class:fade class:center={(align ?? p.align) === "center"} class:stamps bind:this={box}
  style:--big="{big}px" style:--small="{Math.round(big * 0.55)}px"
  onwheel={() => (userScrolled = performance.now())} ontouchmove={() => (userScrolled = performance.now())}
>
  <div class="pad" aria-hidden="true"></div>
  {#each lyrics.lines as l, i (i)}
    {@const now = i === cur}
    {@const words = now && p.words && l.words?.length ? wordProgress(l, lyrics.lines[i + 1], t) : null}
    <p data-i={i} class:now class:past={lyrics.synced && i < cur} class:gap={!l.text}>
      {#if stamps && l.t !== null}<span class="ts">[{time(l.t)}]</span>{/if}
      {#if lyrics.synced}
        <button class="ln" onclick={() => jump(l.t)} aria-label="Play from {l.text || 'here'}">
          {#if words}
            {#each l.words! as w, k}<span class="w" style:--w="{Math.round(words[k] * 100)}%">{w.text}</span>{/each}
          {:else}{l.text || (now ? "♪" : "")}{/if}
        </button>
      {:else}
        <span class="ln">{l.text}</span>
      {/if}
    </p>
  {/each}
  <div class="pad" aria-hidden="true"></div>
</div>

<style>
  .lines { height: 100%; overflow-y: auto; scrollbar-width: none; display: flex; flex-direction: column; gap: 0.35em; font-size: var(--small); }
  .lines::-webkit-scrollbar { display: none; }
  .lines.fade { mask-image: linear-gradient(180deg, transparent, #000 16%, #000 84%, transparent); }
  .pad { flex: 0 0 40%; }
  p { margin: 0; font-weight: 800; letter-spacing: -0.02em; line-height: 1.15; opacity: 0.38; transition: opacity 0.25s, font-size 0.25s; display: flex; gap: 0.8rem; align-items: baseline; }
  p.past { opacity: 0.24; }
  p.now { opacity: 1; font-size: var(--big); font-weight: 900; letter-spacing: -0.035em; line-height: 1.05; }
  p.gap { min-height: 0.6em; }
  .center p { justify-content: center; text-align: center; }
  .ln { font: inherit; letter-spacing: inherit; color: inherit; text-align: inherit; border: 0; background: transparent; padding: 0; cursor: pointer; }
  span.ln { cursor: text; }
  .ln:hover { background: transparent; text-decoration: underline; text-decoration-thickness: 2px; text-underline-offset: 0.15em; }
  p.now .ln:hover { text-decoration: none; }
  /* Word by word: each word fills with colour as it's sung. */
  .w { background: linear-gradient(90deg, currentColor var(--w), color-mix(in srgb, currentColor 40%, transparent) var(--w)); -webkit-background-clip: text; background-clip: text; -webkit-text-fill-color: transparent; }
  .ts { font-family: "JetBrains Mono Variable", Consolas, monospace; font-size: 0.75rem; font-weight: 500; color: var(--faint); letter-spacing: 0; flex-shrink: 0; width: 5.4rem; }
  .stamps p { font-weight: 600; letter-spacing: 0; }
  .stamps p.now { font-weight: 800; letter-spacing: -0.01em; }
</style>
