<script lang="ts">
  // Full App Display: three drawings of the same thing, picked by the
  // addon's setting or Music's layout.
  import { onMount } from "svelte";
  import Icon from "../../components/Icon.svelte";
  import { app } from "../../lib/state.svelte";
  import { skinFor } from "../../lib/theme";
  import Art from "../../music/Art.svelte";
  import Seek from "../../music/Seek.svelte";
  import { musicKeys } from "../../music/keys";
  import { artUrl, initMusic, ms, next, prev, quality, setShuffle, cycleRepeat, time, toggle } from "../../music/store.svelte";
  import { LYRICS, currentLine, useLyrics } from "../lyrics/lyrics.svelte";
  import Visualiser from "./Visualiser.svelte";
  import { closeFad, fad, toggleFullscreen } from "./state.svelte";

  const p = $derived(app.ui.fad);
  const skin = $derived(skinFor(p.skin, app.ui.music.layout));
  const t = $derived(ms.status?.track ?? null);
  const s = $derived(ms.status);
  const upNext = $derived.by(() => {
    const i = ms.queue.index;
    return i === null ? null : (ms.queue.tracks[i + 1] ?? null);
  });
  const withLyrics = $derived(p.lyric && app.ui.addons.installed.includes(LYRICS));
  const line = $derived(withLyrics ? currentLine() : null);
  const cover = $derived(artUrl(t?.art ?? null));

  let now = $state(new Date());
  onMount(() => {
    initMusic();
    const stop = useLyrics();
    const tick = setInterval(() => (now = new Date()), 5000);
    return () => {
      stop();
      clearInterval(tick);
    };
  });
  const clock = $derived(now.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" }));
  const date = $derived(now.toLocaleDateString([], { weekday: "long", day: "numeric", month: "long" }));

  // Controls fade out when the mouse rests, like a video player.
  let idle = $state(false);
  let idleTimer: ReturnType<typeof setTimeout> | undefined;
  function wake() {
    idle = false;
    clearTimeout(idleTimer);
    idleTimer = setTimeout(() => (idle = true), 2600);
  }
  onMount(() => {
    wake();
    return () => clearTimeout(idleTimer);
  });

  function keys(e: KeyboardEvent) {
    wake();
    if (e.key === "Escape") {
      e.preventDefault();
      closeFad();
      return;
    }
    musicKeys(e, (ev) => {
      if (ev.key === "f") {
        closeFad();
        return true;
      }
      return false;
    });
  }
</script>

<svelte:window onkeydown={keys} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fad {skin}" class:idle onmousemove={wake} ondblclick={toggleFullscreen} role="dialog" aria-modal="true" aria-label="Full App Display" tabindex="-1">
  {#if skin === "tonal"}
    <div class="ambient" style:background-image={cover ? `url("${cover}")` : undefined} style:filter="blur({p.blur}px) saturate(1.3)" style:--c={t?.artColor ?? "#6a2c91"} aria-hidden="true"></div>
    <div class="shade" aria-hidden="true"></div>
  {:else if skin === "glass"}
    <div class="wall" style:background-image={cover ? `url("${cover}")` : undefined} style:--c={t?.artColor ?? "#6a2c91"} aria-hidden="true"></div>
  {/if}

  {#if skin !== "tiled"}
  <header class="top">
    <button class="pill" onclick={closeFad}><Icon name="collapse" size={15} /> Exit · Esc</button>
    <span class="grow"></span>
    {#if p.clock && skin !== "glass"}<span class="clock">{clock}</span>{/if}
    <button class="round" onclick={toggleFullscreen} aria-label={fad.fullscreen ? "Leave full screen" : "Full screen"} title="Double-click anywhere too"><Icon name="fullscreen" size={18} /></button>
  </header>
  {/if}

  {#if !t}
    <p class="nothing">Nothing playing. Pick something in Music.</p>
  {:else if skin === "tonal"}
    <main class="t-main" class:centred={p.layout === "centred"}>
      <div class="t-cover"><Art src={t.art} name={t.album} color={t.artColor} radius="22px" /></div>
      <div class="t-text">
        <span class="kicker">{s?.playing ? "Now playing" : "Paused"}</span>
        <h1>{t.title}</h1>
        <span class="by">{t.artist} · {t.album}</span>
        {#if line}<p class="lyric">{line.text || "♪"}</p>{/if}
      </div>
    </main>
    <footer class="t-foot">
      <div class="t-ctrl">
        <Seek />
        <div class="row">
          <button class="ic" class:on={s?.shuffle} aria-label="Shuffle" onclick={() => setShuffle(!s?.shuffle)}><Icon name="shuffle" size={20} /></button>
          <button class="ic" aria-label="Previous" onclick={prev}><Icon name="skip-prev" size={24} filled /></button>
          <button class="big" aria-label={s?.playing ? "Pause" : "Play"} onclick={toggle}><Icon name={s?.playing ? "pause" : "play"} size={26} filled /></button>
          <button class="ic" aria-label="Next" onclick={next}><Icon name="skip-next" size={24} filled /></button>
          <button class="ic" class:on={s?.repeat !== "off"} aria-label="Repeat" onclick={cycleRepeat}><Icon name={s?.repeat === "one" ? "repeat-one" : "repeat"} size={20} /></button>
        </div>
      </div>
      {#if p.upNext && upNext}
        <div class="next">
          <Art src={upNext.art} name={upNext.album} color={upNext.artColor} size={52} radius="10px" />
          <div><span class="kicker">Up next</span><b>{upNext.title}</b><span class="by">{upNext.artist}</span></div>
        </div>
      {/if}
    </footer>
  {:else if skin === "tiled"}
    <div class="g-grid">
      <section class="g-pane g-focus g-viz" aria-label="Visualiser">
        <div class="g-ph"><b>visualiser</b><span>{p.visualiser ? `spectrum · ${p.bands} bands` : "off"}</span><span class="grow"></span><span>[esc] exit · [f] toggle</span></div>
        <div class="g-bars">
          {#if p.visualiser}<Visualiser bands={p.bands} style={p.vizStyle} color={t.artColor ?? "#fe8019"} peak="#fabd2f" gap={p.bands > 64 ? 2 : 4} />{/if}
        </div>
        <div class="g-title"><h1>{t.title}</h1><span>{t.artist} — {t.album}{t.year ? ` (${t.year})` : ""}</span></div>
        {#if line}<p class="g-lyric">› {line.text || "♪"}</p>{/if}
      </section>
      <aside class="g-pane" aria-label="Track">
        <div class="g-ph"><b>now</b><span class="grow"></span>{#if p.clock}<span>{clock}</span>{/if}<button class="g-btn" onclick={toggleFullscreen} aria-label={fad.fullscreen ? "Leave full screen" : "Full screen"}><Icon name="fullscreen" size={13} /></button><button class="g-btn" onclick={closeFad}>exit</button></div>
        <div class="g-now">
          <Art src={t.art} name={t.album} color={t.artColor} radius="3px" />
          <dl>
            <dt>format</dt><dd>{quality(t)}</dd>
            {#if s?.output}<dt>output</dt><dd>{+(s.output.rate / 1000).toFixed(1)} kHz{t.sampleRate === s.output.rate ? " · native" : ""}</dd>{/if}
            <dt>plays</dt><dd>{t.playCount}</dd>
            <dt>source</dt><dd>{t.origin}</dd>
            {#if s?.loopAb}<dt>loop</dt><dd>{time(s.loopAb[0])}–{time(s.loopAb[1])}</dd>{/if}
          </dl>
        </div>
      </aside>
      <section class="g-pane" aria-label="Queue">
        <div class="g-ph"><b>queue</b><span class="grow"></span><span>{(ms.queue.index ?? 0) + 1} of {ms.queue.tracks.length}</span></div>
        <ol class="g-q">
          {#each ms.queue.tracks.slice(ms.queue.index ?? 0, (ms.queue.index ?? 0) + 6) as q, i (q.id + ":" + i)}
            <li class:cur={i === 0}><span>{i === 0 ? "▶" : (ms.queue.index ?? 0) + i + 1}</span><span class="tt">{q.title}</span><span class="dim">{q.artist}</span><span class="dim r">{time(q.duration)}</span></li>
          {/each}
        </ol>
      </section>
      <section class="g-pane g-tp" aria-label="Transport">
        <Seek />
        <div class="row">
          <button class="g-btn" aria-label="Shuffle" class:on={s?.shuffle} onclick={() => setShuffle(!s?.shuffle)}>⇄</button>
          <button class="g-btn" aria-label="Previous" onclick={prev}>⏮</button>
          <button class="g-btn g-play" onclick={toggle}>{s?.playing ? "⏸ pause" : "▶ play"}</button>
          <button class="g-btn" aria-label="Next" onclick={next}>⏭</button>
          <button class="g-btn" aria-label="Repeat" class:on={s?.repeat !== "off"} onclick={cycleRepeat}>↻</button>
        </div>
      </section>
    </div>
  {:else}
    {#if p.clock}
      <div class="l-clock"><span>{clock}</span><small>{date}</small></div>
    {/if}
    {#if p.upNext && upNext}
      <div class="lg l-next"><span class="sheen"></span><Art src={upNext.art} name={upNext.album} color={upNext.artColor} size={44} radius="12px" /><div><small>UP NEXT</small><b>{upNext.title} · {upNext.artist}</b></div></div>
    {/if}
    <section class="lg l-card" aria-label="Now playing">
      <span class="sheen"></span>
      <div class="l-cover"><Art src={t.art} name={t.album} color={t.artColor} radius="24px" /></div>
      <div class="l-body">
        <div><h1>{t.title}</h1><span class="by">{t.artist} · {t.album}</span></div>
        {#if line}<p class="l-lyric">{line.text || "♪"}</p>{/if}
        <Seek />
        <div class="row">
          <button class="ic" aria-label="Previous" onclick={prev}><Icon name="skip-prev" size={22} filled /></button>
          <button class="lg l-play" aria-label={s?.playing ? "Pause" : "Play"} onclick={toggle}><span class="sheen"></span><Icon name={s?.playing ? "pause" : "play"} size={22} filled /></button>
          <button class="ic" aria-label="Next" onclick={next}><Icon name="skip-next" size={22} filled /></button>
        </div>
      </div>
    </section>
  {/if}
</div>

<style>
  .fad { position: fixed; inset: 0; z-index: 900; overflow: hidden; color: #fff; background: #120a1f; outline: none; animation: in 0.35s cubic-bezier(0.2, 0.8, 0.2, 1); }
  @keyframes in { from { opacity: 0; transform: scale(1.02); } }
  .grow { flex: 1; }
  .row { display: flex; align-items: center; gap: 0.5rem; }
  .top { position: absolute; left: 2rem; right: 2rem; top: 1.5rem; display: flex; align-items: center; gap: 0.8rem; z-index: 3; transition: opacity 0.4s; }
  .idle .top, .idle .t-ctrl .row { opacity: 0; }
  .idle { cursor: none; }
  .pill { display: inline-flex; align-items: center; gap: 0.4rem; border: 0; border-radius: 999px; padding: 0.55rem 0.95rem; font-weight: 700; font-size: 0.82rem; color: inherit; background: rgba(255, 255, 255, 0.14); }
  .round { border: 0; width: 44px; height: 44px; border-radius: 50%; display: grid; place-items: center; color: inherit; background: rgba(255, 255, 255, 0.12); }
  .pill:hover, .round:hover { background: rgba(255, 255, 255, 0.24); }
  .clock { font-weight: 600; opacity: 0.85; }
  .nothing { position: absolute; inset: 0; display: grid; place-items: center; font-size: 1.2rem; opacity: 0.8; }
  .kicker { font-size: 0.8rem; font-weight: 700; letter-spacing: 0.12em; text-transform: uppercase; opacity: 0.75; }
  .by { opacity: 0.85; }
  .ic { border: 0; background: transparent; color: inherit; width: 44px; height: 44px; border-radius: 50%; display: grid; place-items: center; }
  .ic:hover { background: rgba(255, 255, 255, 0.12); }
  .ic.on { color: var(--c-on, #ffd9c2); }
  .fad :global(.seek .t) { color: rgba(255, 255, 255, 0.8); }
  .fad :global(.seek input::-webkit-slider-runnable-track) { background: linear-gradient(90deg, #fff var(--pct), rgba(255, 255, 255, 0.22) var(--pct)); }

  /* Ambient (Tonal) */
  .ambient { position: absolute; inset: -100px; background: var(--c) center / cover; transform: scale(1.1); }
  .shade { position: absolute; inset: 0; background: linear-gradient(180deg, rgba(10, 6, 18, 0.15), rgba(10, 6, 18, 0.7)); }
  .t-main { position: absolute; left: 6vw; right: 6vw; top: 18vh; display: grid; grid-template-columns: minmax(200px, 32vh) minmax(0, 1fr); gap: 4vw; align-items: end; }
  .t-cover { box-shadow: 0 40px 90px rgba(0, 0, 0, 0.45); border-radius: 22px; }
  .t-text { display: flex; flex-direction: column; gap: 0.7rem; min-width: 0; padding-bottom: 0.4rem; }
  .t-text h1 { font-size: clamp(2.6rem, 6.5vw, 6rem); font-weight: 900; letter-spacing: -0.045em; line-height: 0.92; overflow-wrap: anywhere; }
  .t-text .by { font-size: clamp(1rem, 1.8vw, 1.5rem); font-weight: 600; }
  .lyric { font-size: clamp(1.1rem, 1.7vw, 1.4rem); font-weight: 700; opacity: 0.7; margin-top: 0.4rem; transition: opacity 0.3s; }
  .t-main.centred { top: 12vh; grid-template-columns: 1fr; justify-items: center; text-align: center; }
  .t-main.centred .t-cover { width: min(38vh, 380px); }
  .t-main.centred h1 { font-size: clamp(2.2rem, 4.5vw, 4rem); }
  .t-foot { position: absolute; left: 6vw; right: 6vw; bottom: 5vh; display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 3vw; align-items: end; }
  .t-ctrl { display: flex; flex-direction: column; gap: 0.6rem; }
  .t-ctrl .row { transition: opacity 0.4s; }
  .big { width: 60px; height: 60px; border-radius: 50%; border: 0; background: #fff; color: #1a0f2c; display: grid; place-items: center; }
  .next { display: flex; align-items: center; gap: 0.8rem; padding: 0.75rem; border-radius: 16px; background: rgba(255, 255, 255, 0.1); min-width: 260px; }
  .next div { display: flex; flex-direction: column; }

  /* Tiled */
  .tiled { background: var(--sunk); color: var(--text); font-family: "JetBrains Mono Variable", Consolas, monospace; font-size: 0.85rem; }
  .tiled .top { left: auto; right: 1.4rem; top: 0.9rem; }
  .tiled .pill, .tiled .round { background: var(--raised); color: var(--text); border-radius: 4px; }
  .g-grid { position: absolute; inset: 14px; display: grid; gap: 12px; grid-template-columns: minmax(0, 1fr) 330px; grid-template-rows: minmax(0, 1fr) 210px; }
  .g-pane { border: 1px solid var(--line); border-radius: 4px; background: var(--panel, var(--ink)); display: flex; flex-direction: column; min-height: 0; min-width: 0; }
  .g-focus { border: 2px solid var(--peach); }
  .g-ph { display: flex; align-items: center; gap: 0.9rem; padding: 0.5rem 0.8rem; border-bottom: 1px solid var(--line-soft); color: var(--muted); }
  .g-ph b { color: var(--peach); }
  .g-bars { flex: 1; min-height: 0; padding: 1.4rem 1.6rem 1rem; }
  .g-title { padding: 0 1.6rem 1rem; display: flex; align-items: baseline; gap: 1.2rem; flex-wrap: wrap; }
  .g-title h1 { font-size: 2.6rem; letter-spacing: -0.02em; color: var(--text); }
  .g-title span { color: var(--muted); font-size: 1rem; }
  .g-lyric { padding: 0 1.6rem 1.2rem; color: var(--peach); }
  .g-now { padding: 0.9rem; display: grid; gap: 0.8rem; overflow: auto; }
  .g-now dl { display: grid; grid-template-columns: auto 1fr; gap: 0.25rem 0.9rem; }
  .g-now dt { color: var(--muted); }
  .g-q { list-style: none; padding: 0.5rem 0; overflow: hidden; }
  .g-q li { display: grid; grid-template-columns: 34px minmax(0, 1fr) minmax(0, 0.7fr) 56px; gap: 0.6rem; padding: 0.25rem 0.8rem; }
  .g-q li.cur { color: var(--peach); font-weight: 700; }
  .g-q .tt, .g-q .dim { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .dim { color: var(--muted); }
  .r { text-align: right; }
  .g-tp { padding: 0.9rem; justify-content: space-between; }
  .g-tp .row { justify-content: space-between; }
  .g-ph .g-btn { padding: 0.1rem 0.45rem; display: inline-flex; align-items: center; }
  .g-btn { font: inherit; border: 1px solid var(--line); background: transparent; color: var(--text); padding: 0.35rem 0.65rem; border-radius: 2px; }
  .g-btn.on { color: var(--peach); border-color: var(--peach); }
  .g-play { background: var(--peach); color: var(--peach-ink); border-color: transparent; font-weight: 800; }
  .tiled :global(.seek .t) { color: var(--muted); }
  .tiled :global(.seek input::-webkit-slider-runnable-track) { background: linear-gradient(90deg, var(--peach) var(--pct), var(--line) var(--pct)); }

  /* Liquid Glass */
  .wall { position: absolute; inset: 0; background: var(--c) center / cover; }
  .glass { font-family: "Figtree Variable", "Segoe UI", sans-serif; }
  .lg { position: relative; backdrop-filter: blur(22px) saturate(1.9) brightness(1.06); background: rgba(24, 14, 34, 0.22); text-shadow: 0 2px 12px rgba(0, 0, 0, 0.25);
    box-shadow: inset 0 1.5px 0.5px rgba(255, 255, 255, 0.75), inset 0 -1px 0.5px rgba(255, 255, 255, 0.22), inset 0 0 0 1px rgba(255, 255, 255, 0.18), 0 0 0 1px rgba(0, 0, 0, 0.22), 0 14px 34px rgba(0, 0, 0, 0.28); }
  .sheen { position: absolute; inset: 0; border-radius: inherit; pointer-events: none; background: linear-gradient(135deg, rgba(255, 255, 255, 0.42), rgba(255, 255, 255, 0) 38%, rgba(255, 255, 255, 0) 72%, rgba(255, 255, 255, 0.14)); mix-blend-mode: screen; }
  .l-clock { position: absolute; left: 5vw; top: 8vh; display: flex; flex-direction: column; text-shadow: 0 8px 40px rgba(20, 8, 40, 0.4); }
  .l-clock span { font-size: clamp(5rem, 12vw, 10rem); font-weight: 800; letter-spacing: -0.06em; line-height: 0.9; }
  .l-clock small { font-size: 1.4rem; font-weight: 700; opacity: 0.9; padding-left: 0.5rem; }
  .l-next { position: absolute; right: 4vw; top: 8vh; border-radius: 22px; padding: 0.6rem 0.9rem 0.6rem 0.6rem; display: flex; align-items: center; gap: 0.8rem; }
  .l-next div { display: flex; flex-direction: column; }
  .l-next small { font-size: 0.7rem; font-weight: 800; letter-spacing: 0.1em; opacity: 0.8; }
  .l-card { position: absolute; left: 50%; bottom: 6vh; transform: translateX(-50%); width: min(780px, 90vw); border-radius: 38px; padding: 20px; display: grid; grid-template-columns: 140px minmax(0, 1fr); gap: 22px; }
  .l-cover { box-shadow: 0 10px 30px rgba(0, 0, 0, 0.3); border-radius: 24px; }
  .l-body { display: flex; flex-direction: column; justify-content: space-between; gap: 0.4rem; min-width: 0; }
  .l-body h1 { font-size: 1.9rem; font-weight: 900; letter-spacing: -0.03em; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .l-body .by { font-weight: 700; }
  .l-lyric { font-weight: 800; opacity: 0.85; }
  .l-play { width: 52px; height: 52px; border-radius: 50%; border: 0; color: #fff; display: grid; place-items: center; }
  @media (max-width: 900px) { .t-main { grid-template-columns: 1fr; } .t-cover { width: 30vh; } .g-grid { grid-template-columns: 1fr; grid-template-rows: minmax(0, 1fr) auto; } .g-grid > aside, .g-grid > section:nth-child(3) { display: none; } }
</style>
