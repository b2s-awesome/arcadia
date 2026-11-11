<script lang="ts">
  import { onMount } from "svelte";
  import { api, win } from "../lib/api";
  import { app, type Page } from "../lib/state.svelte";
  import { visibleNav } from "../lib/nav";
  import Icon from "./Icon.svelte";
  import Logo from "./Logo.svelte";

  // Same entries and order as the sidebar, then Settings.
  const pages: { id: Page; label: string }[] = $derived([...visibleNav(app.ui), { id: "settings", label: "Settings" }]);

  let now = $state(new Date());
  let proxyPort: number | null = $state(null);

  onMount(() => {
    const tick = setInterval(() => (now = new Date()), 1000);
    const poll = async () => {
      try {
        const s = await api.proxyStatus();
        proxyPort = s.running ? s.port : null;
      } catch {
        proxyPort = null;
      }
    };
    poll();
    const p = setInterval(poll, 5000);
    return () => {
      clearInterval(tick);
      clearInterval(p);
    };
  });

  const bar = $derived(app.ui.bar);
  const time = $derived(
    now.toLocaleTimeString([], {
      hour: bar.clock24 ? "2-digit" : "numeric",
      minute: "2-digit",
      second: bar.seconds ? "2-digit" : undefined,
      hour12: !bar.clock24,
    }),
  );
  const date = $derived(now.toLocaleDateString([], { weekday: "short", month: "short", day: "numeric" }));
</script>

<header class="bar style-{bar.style}" data-tauri-drag-region>
  <div class="island left">
    <Logo size={18} />
    <span class="name">{app.ui.brand.name || "Arcadia"}</span>
    <div class="dots" role="tablist" aria-label="Pages">
      {#each pages as p}
        <button
          role="tab"
          class="ws"
          class:on={app.page === p.id}
          aria-selected={app.page === p.id}
          aria-label={p.label}
          title={p.label}
          onclick={() => (app.page = p.id)}
        ></button>
      {/each}
    </div>
  </div>

  <div class="island center" data-tauri-drag-region>
    {#if bar.date}<span class="date" data-tauri-drag-region>{date}</span>{/if}
    <span class="time" data-tauri-drag-region>{time}</span>
  </div>

  <div class="island right">
    {#if app.playing}
      <span class="stat live" title="In a Roblox server"><Icon name="gamepad" size={15} /> place {app.playing.placeId}</span>
    {/if}
    <span class="stat" class:live={proxyPort} title={proxyPort ? `Asset proxy on port ${proxyPort}` : "Asset proxy is off"}>
      <Icon name="shield" size={15} />{proxyPort ? proxyPort : "off"}
    </span>
    <button class="stat key" onclick={() => (app.launcher = true)} title="Search games and actions (Ctrl+K)">
      <Icon name="search" size={15} /><kbd>Ctrl K</kbd>
    </button>
    <span class="sep"></span>
    <button class="wc" onclick={() => win.minimize()} aria-label="Minimize"><Icon name="minimize" size={14} /></button>
    <button class="wc" onclick={() => win.toggleMaximize()} aria-label="Maximize"><Icon name="maximize" size={13} /></button>
    <button class="wc close" onclick={() => win.close()} aria-label="Close"><Icon name="close" size={14} /></button>
  </div>
</header>

<style>
  .bar {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: 0.6rem;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text);
  }
  .island {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    height: 34px;
    padding: 0 0.7rem;
    background: var(--panel);
    backdrop-filter: blur(var(--glass-blur)) saturate(1.4);
    border: 1px solid var(--line-soft);
    border-radius: 999px;
  }
  :global(:root[data-preset="gruvbox"]) .island { border-radius: var(--r-md); }
  .left { justify-self: start; }
  .right { justify-self: end; padding-right: 0.3rem; }

  /* Strip: one flush bar instead of floating islands. */
  .style-strip { background: var(--panel); backdrop-filter: blur(var(--glass-blur)); border-bottom: 1px solid var(--line-soft); padding: 0 0.5rem; height: 38px; }
  .style-strip .island { background: transparent; border: 0; backdrop-filter: none; }

  .mark { color: var(--text); }
  .name { font-family: var(--display); font-weight: 700; }
  .dots { display: flex; align-items: center; gap: 5px; margin-left: 0.2rem; }
  .ws {
    width: 8px; height: 8px; padding: 0; border: 0; border-radius: 999px;
    background: var(--faint); opacity: 0.55;
    transition: width 0.2s ease, background 0.2s, opacity 0.2s;
  }
  .ws:hover { opacity: 1; background: var(--muted); }
  .ws.on { width: 26px; background: var(--peach); opacity: 1; }

  .center { gap: 0.8rem; }
  .date { color: var(--muted); }
  .time { font-variant-numeric: tabular-nums; }

  .stat { display: inline-flex; align-items: center; gap: 0.35rem; color: var(--muted); font-variant-numeric: tabular-nums; }
  .stat.live { color: var(--mint); }
  .key { background: transparent; border: 0; padding: 0.2rem 0.3rem; font-weight: 600; font-size: inherit; }
  .key:hover { background: var(--raised); color: var(--text); }
  kbd { font: inherit; font-size: 0.75rem; padding: 0 0.35rem; border: 1px solid var(--line); border-radius: 5px; color: var(--faint); }
  .sep { width: 1px; height: 16px; background: var(--line); margin: 0 0.1rem; }
  .wc { display: grid; place-items: center; width: 26px; height: 26px; padding: 0; border: 0; border-radius: 999px; background: transparent; color: var(--muted); }
  .wc:hover { background: var(--raised); color: var(--text); }
  .wc.close:hover { background: var(--rose); color: white; }
</style>
