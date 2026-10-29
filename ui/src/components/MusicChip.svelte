<script lang="ts">
  // What's playing, in the top bar: click to open Music.
  import Icon from "./Icon.svelte";
  import { app } from "../lib/state.svelte";
  import { artUrl, ms, next, toggle } from "../music/store.svelte";
  const t = $derived(ms.status?.track ?? null);
  const pct = $derived(ms.status?.duration ? Math.min(100, (ms.pos / ms.status.duration) * 100) : 0);
</script>

{#if t}
  <div class="chip" style:--pct="{pct}%">
    <button class="open" onclick={() => (app.page = "music")} title="{t.title} — {t.artist}">
      {#if t.art}<img src={artUrl(t.art)} alt="" />{:else}<span class="dot"></span>{/if}
      <span class="tt">{t.title}</span>
      {#if ms.status?.playing}<span class="eq" aria-hidden="true"><i></i><i></i><i></i></span>{/if}
    </button>
    <button class="pp" onclick={toggle} aria-label={ms.status?.playing ? "Pause" : "Play"}><Icon name={ms.status?.playing ? "pause" : "play"} size={12} filled /></button>
    <button class="nx" onclick={next} aria-label="Next"><Icon name="skip-next" size={13} filled /></button>
  </div>
{/if}

<style>
  .chip { position: relative; display: flex; align-items: center; gap: 2px; max-width: 280px; padding-bottom: 2px; }
  .chip::after { content: ""; position: absolute; left: 6px; right: 6px; bottom: 0; height: 2px; border-radius: 2px; background: linear-gradient(90deg, var(--peach) var(--pct), transparent var(--pct)); opacity: 0.8; }
  .open { display: flex; align-items: center; gap: 0.45rem; border: 0; background: transparent; padding: 0 0.3rem; min-width: 0; font-size: 0.8rem; font-weight: 600; color: inherit; }
  .open:hover { background: transparent; }
  img, .dot { width: 18px; height: 18px; border-radius: 5px; object-fit: cover; flex-shrink: 0; }
  .dot { background: var(--peach); }
  .tt { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .pp, .nx { border: 0; background: transparent; padding: 0.2rem; display: grid; place-items: center; color: inherit; opacity: 0.8; }
  .pp:hover, .nx:hover { opacity: 1; background: transparent; }
  .eq { display: flex; gap: 2px; align-items: flex-end; height: 10px; flex-shrink: 0; }
  .eq i { width: 2px; background: var(--peach); border-radius: 1px; animation: eq 0.9s ease-in-out infinite; }
  .eq i:nth-child(2) { animation-delay: -0.3s; }
  .eq i:nth-child(3) { animation-delay: -0.6s; }
  @keyframes eq { 0%, 100% { height: 3px; } 50% { height: 10px; } }
</style>
