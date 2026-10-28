<script lang="ts" module>
  export interface LauncherAction {
    id: string;
    label: string;
    hint: string;
    icon: string;
    run: () => void | Promise<void>;
    /** Shown on the ring when nothing is typed. */
    ring?: boolean;
  }
</script>

<script lang="ts">
  import { api } from "../lib/api";
  import { app, toast } from "../lib/state.svelte";
  import { SOURCE_LABEL, type Game } from "../lib/types";
  import Icon from "./Icon.svelte";

  let { actions }: { actions: LauncherAction[] } = $props();

  let query = $state("");
  let games: Game[] = $state([]);
  let cursor = $state(0);
  let input: HTMLInputElement | undefined = $state();

  const ring = $derived(actions.filter((a) => a.ring).slice(0, 8));

  type Item = { kind: "game"; game: Game } | { kind: "action"; action: LauncherAction };
  const results: Item[] = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return [];
    const score = (s: string) => {
      const t = s.toLowerCase();
      if (t.startsWith(q)) return 0;
      if (t.split(/\s+/).some((w) => w.startsWith(q))) return 1;
      return t.includes(q) ? 2 : 9;
    };
    const g: Item[] = games
      .map((game) => ({ s: score(game.name), item: { kind: "game", game } as Item }))
      .filter((x) => x.s < 9)
      .sort((a, b) => a.s - b.s)
      .map((x) => x.item);
    const a: Item[] = actions
      .map((action) => ({ s: Math.min(score(action.label), score(action.hint) + 1), item: { kind: "action", action } as Item }))
      .filter((x) => x.s < 9)
      .sort((a, b) => a.s - b.s)
      .map((x) => x.item);
    return [...g.slice(0, 6), ...a].slice(0, 10);
  });

  const focused = $derived(query.trim() ? null : ring[cursor % Math.max(1, ring.length)]);

  $effect(() => {
    if (app.launcher) {
      query = "";
      cursor = 0;
      queueMicrotask(() => input?.focus());
      if (games.length === 0) api.scanLibrary().then((r) => (games = r.games)).catch(() => {});
    }
  });
  $effect(() => {
    void query;
    cursor = 0;
  });

  async function run(item: Item | LauncherAction | undefined) {
    if (!item) return;
    app.launcher = false;
    try {
      if ("kind" in item && item.kind === "game") {
        await api.launchGame(item.game.id);
        if (item.game.source !== "roblox") toast(`Opening ${item.game.name}`);
      } else {
        const a = "kind" in item ? item.action : item;
        await a.run();
      }
    } catch (e) {
      toast(String(e), "error");
    }
  }

  function onkey(e: KeyboardEvent) {
    const n = query.trim() ? results.length : ring.length;
    if (e.key === "Escape") app.launcher = false;
    else if (e.key === "ArrowDown" || e.key === "ArrowRight" || (e.key === "Tab" && !e.shiftKey)) {
      e.preventDefault();
      cursor = (cursor + 1) % Math.max(1, n);
    } else if (e.key === "ArrowUp" || e.key === "ArrowLeft" || (e.key === "Tab" && e.shiftKey)) {
      e.preventDefault();
      cursor = (cursor - 1 + n) % Math.max(1, n);
    } else if (e.key === "Enter") {
      e.preventDefault();
      run(query.trim() ? results[cursor] : ring[cursor]);
    }
  }

  // Ring geometry: first item at the top, clockwise.
  const pos = (i: number, n: number) => {
    const a = (i / n) * Math.PI * 2 - Math.PI / 2;
    return { x: Math.cos(a) * 128, y: Math.sin(a) * 128 };
  };
</script>

{#if app.launcher}
  <div class="scrim" role="presentation" onclick={() => (app.launcher = false)}>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="launcher" role="dialog" aria-label="Launcher" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={onkey}>
      <label class="search">
        <Icon name="search" size={18} />
        <input bind:this={input} bind:value={query} placeholder="Search games and actions" aria-label="Search games and actions" />
        <span class="faint enter">Enter to run</span>
      </label>

      {#if !query.trim()}
        <div class="ring-wrap">
          <div class="ring">
            <div class="circle"></div>
            {#each ring as a, i (a.id)}
              {@const p = pos(i, ring.length)}
              <button
                class="node"
                class:on={i === cursor}
                style:transform="translate({p.x}px, {p.y}px)"
                aria-label={a.label}
                onmouseenter={() => (cursor = i)}
                onfocus={() => (cursor = i)}
                onclick={() => run(a)}
              >
                <Icon name={a.icon} size={18} />
              </button>
            {/each}
            {#if focused}
              <div class="core">
                <span class="core-icon"><Icon name={focused.icon} size={26} /></span>
                <strong>{focused.label}</strong>
                <span class="muted">{focused.hint}</span>
              </div>
            {/if}
          </div>
        </div>
      {:else if results.length === 0}
        <p class="none">Nothing called “{query}”. Try a game name or an action like “cache”.</p>
      {:else}
        <ul class="results" role="listbox" aria-label="Results">
          {#each results as r, i}
            <li>
              <button class="result" class:on={i === cursor} role="option" aria-selected={i === cursor} onmouseenter={() => (cursor = i)} onclick={() => run(r)}>
                {#if r.kind === "game"}
                  <span class="ico"><Icon name="play" size={15} /></span>
                  <span class="grow">{r.game.name}</span>
                  <span class="faint">{SOURCE_LABEL[r.game.source]}</span>
                {:else}
                  <span class="ico"><Icon name={r.action.icon} size={15} /></span>
                  <span class="grow">{r.action.label}</span>
                  <span class="faint">{r.action.hint}</span>
                {/if}
              </button>
            </li>
          {/each}
        </ul>
      {/if}

      <footer class="foot faint">
        <span><kbd>↑</kbd><kbd>↓</kbd> move</span>
        <span><kbd>Enter</kbd> run</span>
        <span><kbd>Esc</kbd> close</span>
      </footer>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed; inset: 0; z-index: 60;
    display: grid; place-items: center;
    background: color-mix(in srgb, var(--desk) 35%, rgba(0, 0, 0, 0.45));
    backdrop-filter: blur(3px);
    animation: fade 0.12s ease-out;
  }
  @keyframes fade { from { opacity: 0; } }
  @keyframes rise { from { opacity: 0; transform: translateY(8px) scale(0.98); } }

  .launcher {
    width: min(460px, 92vw);
    background: var(--panel);
    backdrop-filter: blur(calc(var(--glass-blur) + 10px)) saturate(1.4);
    border: 1px solid var(--line);
    border-radius: calc(var(--r-lg) + 6px);
    box-shadow: 0 30px 80px rgba(0, 0, 0, 0.45);
    overflow: hidden;
    animation: rise 0.16s ease-out;
  }
  .search { display: flex; align-items: center; gap: 0.7rem; padding: 0.85rem 1.1rem; border-bottom: 1px solid var(--line-soft); color: var(--muted); }
  .search input { flex: 1; background: transparent; border: 0; padding: 0; font-size: 1.1rem; color: var(--text); }
  .enter { font-size: 0.8rem; white-space: nowrap; }

  .ring-wrap { display: grid; place-items: center; padding: 1.6rem 0 1.4rem; }
  .ring { position: relative; width: 320px; height: 320px; display: grid; place-items: center; }
  .circle { position: absolute; inset: 32px; border-radius: 50%; border: 1px solid var(--line-soft); }
  .node {
    position: absolute; width: 50px; height: 50px; margin: auto; inset: 0;
    display: grid; place-items: center; padding: 0;
    border-radius: 50%; border: 1px solid var(--line-soft);
    background: var(--raised); color: var(--muted);
    transition: background 0.12s, color 0.12s, box-shadow 0.12s;
  }
  .node.on { background: var(--peach); color: var(--peach-ink); border-color: transparent; box-shadow: 0 0 0 6px color-mix(in srgb, var(--peach) 22%, transparent); }
  .core { display: grid; justify-items: center; gap: 0.2rem; text-align: center; max-width: 150px; font-size: 0.85rem; }
  .core-icon { color: var(--peach); margin-bottom: 0.2rem; }
  .core strong { font-family: var(--display); font-size: 1.1rem; }

  .results { list-style: none; margin: 0; padding: 0.4rem; max-height: 360px; overflow-y: auto; }
  .result {
    width: 100%; display: flex; align-items: center; gap: 0.7rem; text-align: left;
    border: 0; background: transparent; padding: 0.55rem 0.7rem; border-radius: var(--r-md); font-weight: 600; font-size: 0.92rem;
  }
  .result .faint { font-weight: 500; font-size: 0.82rem; }
  .result.on { background: var(--raised); }
  .result.on .ico { color: var(--peach); }
  .ico { color: var(--faint); display: grid; }
  .none { padding: 2rem 1.2rem; color: var(--muted); }

  .foot { display: flex; gap: 1.2rem; justify-content: center; padding: 0.6rem; border-top: 1px solid var(--line-soft); font-size: 0.78rem; }
  kbd { font: inherit; font-size: 0.72rem; padding: 0 0.3rem; margin-right: 0.15rem; border: 1px solid var(--line); border-radius: 4px; }
</style>
