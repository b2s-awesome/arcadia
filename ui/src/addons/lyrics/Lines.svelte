<script lang="ts">
  // The lyric lines, shared by every lyrics style.
  //
  // Synced lyrics move like Riff's and Apple Music's: the page doesn't
  // scroll, each line slides on its own, and lines further down set off a
  // little later, so a new line rolls in as a wave. The line being sung
  // fills as it's sung (word by word with word timings, otherwise weighted
  // by vowels), lines further away blur, backing vocals in brackets are
  // smaller, and instrumental breaks show three dots that fill up.
  //
  // Follows the song unless you've just scrolled yourself; click a line to
  // play from there.
  import { app } from "../../lib/state.svelte";
  import type { Lyrics } from "../../lib/types";
  import { ms, seek, time } from "../../music/store.svelte";
  import { lineAt, ly, lyricTime, offsetFor, wordProgress } from "./lyrics.svelte";
  import { EASES, splitBacking, sweep } from "./sweep";

  let { lyrics, stamps = false, size = null, align = null, fade = true, under = null }: {
    lyrics: Lyrics; stamps?: boolean; size?: number | null; align?: "left" | "center" | null; fade?: boolean;
    /** A second line under each (romanisation, translation, or both). */
    under?: (string | null)[] | null;
  } = $props();
  const p = $derived(app.ui.lyrics);
  const t = $derived(lyricTime());
  const cur = $derived(lyrics.synced ? lineAt(lyrics.lines, t) : -1);
  const big = $derived(size ?? p.size);

  /** When a line ends: the next line's start, or a few seconds on. */
  const endOf = (i: number) => lyrics.lines[i + 1]?.t ?? (lyrics.lines[i].t ?? 0) + 4;

  // ── where the lines sit ──
  let box: HTMLDivElement | undefined = $state();
  let height = $state(400);
  let y = $state(0);
  let manual = $state(false);
  let userAt = 0;

  $effect(() => {
    if (!box) return;
    const ro = new ResizeObserver(() => (height = box!.clientHeight));
    ro.observe(box);
    return () => ro.disconnect();
  });

  // Keep the sung line about two-fifths of the way down.
  $effect(() => {
    const i = Math.max(0, cur);
    void height;
    void under;
    void follow;
    if (!box || !lyrics.synced || performance.now() - userAt < 3000) return;
    const el = box.querySelector<HTMLElement>(`[data-i="${i}"]`);
    if (!el) return;
    manual = false;
    y = el.offsetTop + el.offsetHeight / 2 - height * 0.4;
  });

  let follow = $state(0);
  let resume: ReturnType<typeof setTimeout> | undefined;
  function wheel(e: WheelEvent) {
    if (!lyrics.synced || !box) return;
    e.preventDefault();
    userAt = performance.now();
    manual = true;
    const max = box.scrollHeight - height * 0.4;
    y = Math.max(-height * 0.4, Math.min(max, y + e.deltaY));
    // Back to following the song after a pause.
    clearTimeout(resume);
    resume = setTimeout(() => {
      userAt = 0;
      follow++;
    }, 3000);
  }

  function jump(lt: number | null) {
    if (lt === null || !ms.status?.track) return;
    userAt = 0;
    seek(Math.max(0, lt - offsetFor(ly.id) + 0.01));
  }

  const motion = $derived(stamps ? "plain" : "full");
</script>

<div
  role="region" aria-label="Lyrics"
  class="lines {motion}" class:synced={lyrics.synced} class:fade class:center={(align ?? p.align) === "center"} class:stamps class:manual class:blur={p.blur && !stamps}
  bind:this={box}
  style:--big="{Math.round(big * 0.82)}px"
  style:--y="{-y}px"
  style:--ease={EASES[p.ease]}
  style:--dur="{(0.5 / p.speed).toFixed(2)}s"
  onwheel={wheel}
>
  {#if !lyrics.synced}<div class="pad" aria-hidden="true"></div>{/if}
  {#each lyrics.lines as l, i (i)}
    {@const now = i === cur}
    {@const d = cur < 0 ? i : i - cur}
    {@const gap = lyrics.synced && !l.text.trim()}
    {@const share = now && l.t !== null ? (t - l.t) / Math.max(0.3, endOf(i) - l.t) : i < cur ? 1 : 0}
    {@const words = now && p.words && l.words?.length ? wordProgress(l, lyrics.lines[i + 1], t) : null}
    {@const [main, backing] = splitBacking(l.text)}
    <p
      data-i={i}
      class:now class:past={lyrics.synced && i < cur} class:gap class:long-gap={gap && endOf(i) - (l.t ?? 0) >= 3}
      style:--d={Math.max(-2, Math.min(8, d))}
      style:--ad={Math.min(5, Math.abs(d))}
    >
      {#if stamps && l.t !== null}<span class="ts">[{time(l.t)}]</span>{/if}
      <span class="col">
        {#if gap}
          <button class="ln dots" onclick={() => jump(l.t)} aria-label="Instrumental break" style:--k={now ? Math.min(1, Math.max(0, share)) : 0}>
            <i style:--n="0"></i><i style:--n="1"></i><i style:--n="2"></i>
          </button>
        {:else if lyrics.synced}
          <button class="ln" onclick={() => jump(l.t)} aria-label="Play from {l.text}">
            {#if words}
              {#each l.words! as w, k}<span class="w" style:--k={words[k]}>{w.text}</span>{/each}
            {:else if now && p.sweep}
              <span class="fill" style:--k={sweep(main, share)}>{main}</span>{#if backing} <span class="fill back" style:--k={Math.max(0, (share - 0.6) / 0.4)}>{backing}</span>{/if}
            {:else}
              {main}{#if backing} <span class="back">{backing}</span>{/if}
            {/if}
          </button>
        {:else}
          <span class="ln">{l.text}</span>
        {/if}
        {#if under?.[i]}<span class="sub">{under[i]}</span>{/if}
      </span>
    </p>
  {/each}
  {#if !lyrics.synced}<div class="pad" aria-hidden="true"></div>{/if}
</div>

<style>
  /* Lets the fill's position glide between position updates. */
  @property --k { syntax: "<number>"; inherits: false; initial-value: 0; }

  .lines { position: relative; height: 100%; overflow-y: auto; scrollbar-width: none; display: flex; flex-direction: column; gap: 0.1em; font-size: var(--big); }
  .lines.synced { overflow: clip; }
  .lines::-webkit-scrollbar { display: none; }
  .lines.fade { mask-image: linear-gradient(180deg, transparent, #000 14%, #000 86%, transparent); }
  .pad { flex: 0 0 40%; }

  p {
    margin: 0; font-weight: 800; letter-spacing: -0.02em; line-height: 1.12; display: flex; gap: 0.8rem; align-items: baseline;
    padding: 0.14em 0; opacity: 0.42; transform-origin: left center;
    transform: translateY(var(--y, 0)) scale(0.93);
    transition:
      transform var(--dur) var(--ease) calc(max(0, var(--d) + 1) * 42ms),
      opacity var(--dur) var(--ease),
      filter var(--dur) var(--ease);
  }
  .synced p.past { opacity: 0.3; }
  p.now { opacity: 1; transform: translateY(var(--y, 0)) scale(1); }
  .center p { justify-content: center; text-align: center; transform-origin: center; }
  .lines:not(.synced) p { transform: none; opacity: 0.85; font-size: 0.62em; }
  /* While you scroll, everything moves together and nothing blurs. */
  .manual p { transition-delay: 0s; transition-duration: 0.12s; filter: none !important; opacity: 0.6; }
  .blur.synced p:not(.now) { filter: blur(calc(var(--ad) * 0.7px)); }
  .lines p:hover { opacity: 0.9; filter: none; }

  .ln { font: inherit; letter-spacing: inherit; color: inherit; text-align: inherit; border: 0; background: transparent; padding: 0; cursor: pointer; }
  span.ln { cursor: text; }
  .ln:hover { background: transparent; }

  /* The fill: sung text in full colour, the rest faint, with a soft edge. */
  .fill, .w {
    --edge: 6%;
    background-image: linear-gradient(90deg, currentColor calc(var(--k) * (100% + var(--edge)) - var(--edge)), color-mix(in srgb, currentColor 36%, transparent) calc(var(--k) * (100% + var(--edge))));
    -webkit-background-clip: text; background-clip: text; -webkit-text-fill-color: transparent; color: inherit;
    -webkit-box-decoration-break: slice; box-decoration-break: slice;
    transition: --k 0.09s linear;
  }
  .w { --edge: 14%; display: inline; }
  .back { font-size: 0.66em; font-weight: 600; opacity: 0.7; }

  /* Instrumental break: three dots that fill in turn. */
  p.gap { min-height: 0.4em; }
  p.gap.past { opacity: 0; transform: translateY(var(--y, 0)) scale(0.6); }
  .dots { display: none; gap: 0.18em; align-items: center; height: 0.7em; padding: 0.1em 0; }
  .long-gap .dots { display: inline-flex; }
  .dots i {
    width: 0.24em; height: 0.24em; border-radius: 50%;
    background: color-mix(in srgb, currentColor calc(30% + 70% * min(1, max(0, var(--k) * 3 - var(--n)))), transparent);
    transition: --k 0.12s linear, background 0.2s;
  }
  .now .dots { animation: breathe 2.4s ease-in-out infinite; }
  @keyframes breathe { 50% { transform: scale(1.18); } }

  .col { display: flex; flex-direction: column; min-width: 0; }
  .center .col { align-items: center; }
  .sub { font-size: max(13px, 0.42em); font-weight: 600; letter-spacing: 0; line-height: 1.3; opacity: 0.72; margin-top: 0.15em; white-space: pre-line; }

  /* Tiled: timestamps, plain moves. */
  .ts { font-family: "JetBrains Mono Variable", Consolas, monospace; font-size: 0.75rem; font-weight: 500; color: var(--faint); letter-spacing: 0; flex-shrink: 0; width: 5.4rem; }
  .stamps p { font-weight: 600; letter-spacing: 0; transform: translateY(var(--y, 0)); transition-delay: 0s; }
  .stamps p.now { font-weight: 800; letter-spacing: -0.01em; transform: translateY(var(--y, 0)); }
</style>
