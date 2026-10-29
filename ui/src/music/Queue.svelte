<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import { music } from "../lib/api";
  import { toast } from "../lib/state.svelte";
  import Art from "./Art.svelte";
  import { openMenu, trackMenu } from "./contextMenu.svelte";
  import { ms, time } from "./store.svelte";
  import { app } from "../lib/state.svelte";
  import { SORT, queueTool } from "../addons/listening/listening.svelte";

  /** `upcoming`: only what's after the current track. */
  let { upcoming = false }: { upcoming?: boolean } = $props();
  const idx = $derived(ms.queue.index ?? -1);
  const items = $derived(ms.queue.tracks.map((t, i) => ({ t, i })).filter(({ i }) => !upcoming || i > idx));
  const left = $derived(ms.queue.tracks.slice(idx + 1).reduce((a, t) => a + t.duration, 0));
  const run = (p: Promise<unknown>) => p.catch((e) => toast(String(e), "error"));

  let dragFrom: number | null = $state(null);
  let dropAt: number | null = $state(null);
</script>

<div class="q">
  <div class="head">
    <span class="muted">{Math.max(0, ms.queue.tracks.length - idx - 1)} up next · {time(left)}</span>
    {#if app.ui.addons.installed.includes(SORT)}
      <button class="clear" aria-haspopup="menu" disabled={ms.queue.tracks.length <= 1} onclick={(e) => openMenu(e, [
        { label: "Remove duplicates", icon: "copy", run: () => queueTool("dedupe") },
        { label: "Clear what's played", icon: "undo", run: () => queueTool("clearPlayed") },
        { label: "Shuffle what's left", icon: "shuffle", run: () => queueTool("shuffleRest") },
      ])}>Tools</button>
    {/if}
    <button class="clear" onclick={() => run(music.queueClear())} disabled={ms.queue.tracks.length <= 1}>Clear</button>
  </div>
  {#if !items.length}
    <p class="muted empty">{upcoming ? "Nothing after this. Right-click a song and choose Play next." : "The queue is empty."}</p>
  {/if}
  <ol>
    {#each items as { t, i } (t.id + ":" + i)}
      <li
        class:now={i === idx}
        class:past={i < idx}
        class:drop={dropAt === i && dragFrom !== i}
        draggable="true"
        ondragstart={(e) => { dragFrom = i; e.dataTransfer?.setData("text/plain", String(i)); }}
        ondragover={(e) => { e.preventDefault(); dropAt = i; }}
        ondrop={(e) => { e.preventDefault(); if (dragFrom !== null && dragFrom !== i) run(music.queueMove(dragFrom, i)); dragFrom = dropAt = null; }}
        ondragend={() => (dragFrom = dropAt = null)}
        oncontextmenu={(e) => openMenu(e, trackMenu([t], [{ label: "", sep: true }, { label: "Remove from queue", danger: true, run: () => run(music.queueRemove(i)) }]))}
      >
        <button class="row" ondblclick={() => run(music.queueJump(i))} onclick={(e) => { if (e.detail === 0) run(music.queueJump(i)); }} aria-label="Play {t.title}">
          <Art src={t.art} name={t.album} color={t.artColor} size={36} radius="6px" />
          <span class="meta"><span class="t">{t.title}</span><span class="s">{t.artist}</span></span>
          <span class="d">{time(t.duration)}</span>
        </button>
        <button class="x" aria-label="Remove {t.title} from the queue" onclick={() => run(music.queueRemove(i))}><Icon name="close" size={14} /></button>
      </li>
    {/each}
  </ol>
</div>

<style>
  .q { display: flex; flex-direction: column; min-height: 0; }
  .head { display: flex; align-items: center; justify-content: space-between; font-size: 0.82rem; padding: 0 0.3rem 0.5rem; }
  .clear { border: 0; background: transparent; padding: 0.2rem 0.4rem; font-size: 0.82rem; color: var(--muted); }
  .clear:hover { color: var(--text); background: transparent; }
  .empty { font-size: 0.85rem; padding: 0.4rem 0.3rem; }
  ol { list-style: none; margin: 0; padding: 0; display: grid; gap: 1px; }
  li { position: relative; border-radius: var(--r-md); }
  li:hover { background: color-mix(in srgb, var(--text) 6%, transparent); }
  li.now { background: color-mix(in srgb, var(--m-accent, var(--peach)) 16%, transparent); }
  li.now .t { color: var(--m-accent, var(--peach)); }
  li.past { opacity: 0.5; }
  li.drop { box-shadow: inset 0 2px 0 var(--m-accent, var(--peach)); }
  .row { display: flex; align-items: center; gap: 0.6rem; width: 100%; border: 0; background: transparent; padding: 0.35rem 2rem 0.35rem 0.35rem; text-align: left; cursor: grab; }
  .row:hover { background: transparent; }
  .meta { flex: 1; min-width: 0; display: flex; flex-direction: column; line-height: 1.2; }
  .t { font-weight: 600; font-size: 0.86rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .s { font-size: 0.76rem; color: var(--muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .d { font-size: 0.76rem; color: var(--faint); font-variant-numeric: tabular-nums; }
  .x { position: absolute; right: 0.3rem; top: 50%; transform: translateY(-50%); border: 0; background: transparent; padding: 0.25rem; color: var(--faint); opacity: 0; display: grid; }
  li:hover .x, .x:focus-visible { opacity: 1; }
  .x:hover { color: var(--rose); background: transparent; }
</style>
