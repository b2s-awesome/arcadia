<script lang="ts">
  // The two lyrics windows: a small always-on-top pop-up (draggable, glass)
  // and the desktop bar (one line, click-through, over games).
  import { onMount } from "svelte";
  import Icon from "../../components/Icon.svelte";
  import { win } from "../../lib/api";
  import { app } from "../../lib/state.svelte";
  import { initPlayer, ms } from "../../music/store.svelte";
  import { currentLine, lineAt, ly, lyricTime, useLyrics } from "./lyrics.svelte";

  let { kind }: { kind: "popup" | "bar" } = $props();
  onMount(() => {
    // These windows show only the lyrics: no page behind them.
    document.documentElement.classList.add("lyrics-window");
    initPlayer();
    return useLyrics();
  });
  const p = $derived(app.ui.lyrics);
  const t = $derived(ms.status?.track ?? null);
  const l = $derived(ly.id === t?.id ? ly.lyrics : null);
  const i = $derived(l?.synced ? lineAt(l.lines, lyricTime()) : -1);
  const line = $derived(currentLine());
</script>

{#if kind === "popup"}
  <div class="pop" style:--tint={p.popup.tint} data-tauri-drag-region>
    <header data-tauri-drag-region>
      {#if t}
        <div class="who" data-tauri-drag-region><b>{t.title}</b><span>{t.artist}</span></div>
      {:else}
        <div class="who" data-tauri-drag-region><b>Lyrics</b><span>Nothing playing</span></div>
      {/if}
      <button class="x" onclick={() => win.close()} aria-label="Close lyrics window"><Icon name="close" size={14} /></button>
    </header>
    <div class="body" data-tauri-drag-region>
      {#if !t}
        <p class="now dim">Play something in Arcadia.</p>
      {:else if ly.loading}
        <p class="now dim">Looking for lyrics…</p>
      {:else if !l}
        <p class="now dim">No lyrics for this one.</p>
      {:else if !l.synced}
        <p class="now dim">These lyrics aren't timed. Open Lyrics in Arcadia to read them or tap along.</p>
      {:else}
        <p class="prev">{l.lines[i - 1]?.text ?? ""}</p>
        <p class="now">{(i >= 0 ? l.lines[i].text : "") || "♪"}</p>
        <p class="next">{l.lines[i + 1]?.text ?? ""}</p>
      {/if}
    </div>
  </div>
{:else}
  <div class="bar" class:top={p.bar.position === "top"} style:--tint={p.bar.tint} style:--size="{p.bar.size}px">
    {#if line && (line.text || line.next)}
      {#key line.text}<span class="pill">{line.text || "♪"}</span>{/key}
    {/if}
  </div>
{/if}

<style>
  /* No page colour, and no dark canvas from color-scheme, behind the lyrics. */
  :global(html.lyrics-window), :global(html.lyrics-window body) { background: transparent !important; color-scheme: normal !important; }
  .pop { position: fixed; inset: 0; display: flex; flex-direction: column; color: #fff; font-family: "Figtree Variable", "Segoe UI", sans-serif;
    background: rgba(20, 16, 30, var(--tint)); border-radius: 14px; overflow: hidden; user-select: none;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.45), inset 0 0 0 1px rgba(255, 255, 255, 0.14); }
  header { display: flex; align-items: center; gap: 0.6rem; padding: 0.7rem 0.8rem 0.2rem 1rem; }
  .who { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .who b { font-size: 0.86rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .who span { font-size: 0.76rem; opacity: 0.75; }
  .x { border: 0; background: rgba(255, 255, 255, 0.12); color: #fff; width: 28px; height: 28px; border-radius: 50%; display: grid; place-items: center; padding: 0; }
  .x:hover { background: rgba(255, 255, 255, 0.25); }
  .body { flex: 1; display: flex; flex-direction: column; justify-content: center; gap: 0.35rem; padding: 0 1.1rem 0.9rem; text-shadow: 0 2px 14px rgba(0, 0, 0, 0.35); }
  p { margin: 0; font-weight: 800; }
  .prev, .next { font-size: 0.95rem; opacity: 0.5; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .now { font-size: 1.45rem; font-weight: 900; letter-spacing: -0.02em; line-height: 1.12; }
  .now.dim { font-size: 1rem; font-weight: 700; opacity: 0.8; }
  .bar { position: fixed; inset: 0; display: flex; align-items: flex-end; justify-content: center; padding: 6px; pointer-events: none; }
  .bar.top { align-items: flex-start; }
  .pill { max-width: 100%; padding: 0.35em 1.1em; border-radius: 999px; background: rgba(12, 10, 20, var(--tint)); color: #fff;
    font-family: "Figtree Variable", "Segoe UI", sans-serif; font-size: var(--size); font-weight: 900; letter-spacing: -0.02em;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis; text-shadow: 0 2px 10px rgba(0, 0, 0, 0.6), 0 0 2px rgba(0, 0, 0, 0.8);
    animation: swap 0.25s ease-out; }
  @keyframes swap { from { opacity: 0.4; transform: translateY(4px); } }
</style>
