<script lang="ts">
  import { onMount } from "svelte";
  import { api, fileUrl, mc, on, win } from "./lib/api";
  import { mcs } from "./lib/mc.svelte";
  import { app, loadSettings, saveSettings, saveUi, toast, type Page } from "./lib/state.svelte";
  import { applyLook, defaultLook, PRESETS, wallpaperUrl, type PresetId } from "./lib/theme";
  import type { Activity, Progress } from "./lib/types";
  import Icon from "./components/Icon.svelte";
  import Logo from "./components/Logo.svelte";
  import AnimatedLogo from "./components/AnimatedLogo.svelte";
  import { fade } from "svelte/transition";
  import TopBar from "./components/TopBar.svelte";
  import Launcher, { type LauncherAction } from "./components/Launcher.svelte";
  import Library from "./pages/Library.svelte";
  import Roblox from "./pages/Roblox.svelte";
  import Proxy from "./pages/Proxy.svelte";
  import Inspector from "./pages/Inspector.svelte";
  import Appearance from "./pages/Appearance.svelte";
  import SettingsPage from "./pages/Settings.svelte";
  import Minecraft from "./minecraft/Minecraft.svelte";

  const nav: { id: Page; label: string; icon: string }[] = [
    { id: "library", label: "Library", icon: "library" },
    { id: "minecraft", label: "Minecraft", icon: "cube" },
    { id: "roblox", label: "Roblox", icon: "roblox" },
    { id: "proxy", label: "Asset proxy", icon: "proxy" },
    { id: "inspector", label: "Inspector", icon: "inspector" },
    { id: "appearance", label: "Appearance", icon: "brush" },
  ];

  // Apply the look whenever it changes.
  $effect(() => applyLook(app.ui.look));
  const brandName = $derived(app.ui.brand.name.trim() || "Arcadia");
  $effect(() => {
    win.setTitle(brandName).catch(() => {});
  });
  const wall = $derived(wallpaperUrl(app.ui.look.wallpaper, fileUrl));

  // Roblox launch progress, shown along the bottom of the window.
  let progress: Progress | null = $state(null);
  const progressText = $derived.by(() => {
    if (!progress) return "";
    switch (progress.stage) {
      case "checking": return "Checking for Roblox updates";
      case "downloading": return `Downloading ${progress.package}`;
      case "extracting": return `Installing ${progress.package} (${progress.index} of ${progress.count})`;
      case "finalizing": return "Finishing install";
      case "ready": return "Starting Roblox";
    }
  });
  const progressPct = $derived.by(() => {
    if (!progress) return 0;
    if (progress.stage === "downloading") return Math.min(100, (progress.done / Math.max(1, progress.total)) * 50);
    if (progress.stage === "extracting") return 50 + (progress.index / progress.count) * 45;
    if (progress.stage === "ready" || progress.stage === "finalizing") return 100;
    return 4;
  });

  function cycleLook() {
    const ids = Object.keys(PRESETS) as PresetId[];
    const next = ids[(ids.indexOf(app.ui.look.preset) + 1) % ids.length];
    app.ui.look = defaultLook(next);
    app.ui.applied = null;
    saveUi();
    toast(`Look changed to ${PRESETS[next].name}`);
  }

  const go = (p: Page) => () => { app.page = p; };
  const actions: LauncherAction[] = [
    { id: "play", label: "Play Roblox", hint: "Update if needed and launch", icon: "play", ring: true, run: () => { api.robloxLaunch(null).catch((e) => toast(String(e), "error")); } },
    { id: "library", label: "Library", hint: "All your games", icon: "library", ring: true, run: go("library") },
    { id: "proxy", label: "Asset proxy", hint: "Swap and remove assets", icon: "proxy", ring: true, run: go("proxy") },
    { id: "inspector", label: "Inspector", hint: "Live proxy requests", icon: "inspector", ring: true, run: go("inspector") },
    { id: "look", label: "Next look", hint: "Cycle through the presets", icon: "sparkle", ring: true, run: cycleLook },
    { id: "cache", label: "Clear Roblox cache", hint: "Re-download assets", icon: "refresh", ring: true, run: async () => {
      const r = await api.clearCache(false);
      toast(`Cleared ${r.removed} cache item${r.removed === 1 ? "" : "s"}`);
    } },
    { id: "mods", label: "Open mods folder", hint: "Files overlaid on Roblox", icon: "folder", ring: true, run: () => api.openFolder("mods") },
    { id: "appearance", label: "Appearance", hint: "Looks, wallpaper and bar", icon: "brush", ring: true, run: go("appearance") },
    { id: "minecraft", label: "Minecraft", hint: "Instances, modpacks, mods", icon: "cube", run: go("minecraft") },
    { id: "mc-popout", label: "Pop out Minecraft", hint: "Open the launcher in its own window", icon: "cube", run: () => mc.popOut() },
    { id: "mc-security", label: "Minecraft security check", hint: "Scan mods for known incidents", icon: "shield", run: () => { mcs.tab = "security"; app.page = "minecraft"; } },
    { id: "roblox", label: "Roblox settings", hint: "FastFlags, FPS, mods", icon: "roblox", run: go("roblox") },
    { id: "settings", label: "Settings", hint: "Startup, tray, folders", icon: "settings", run: go("settings") },
    { id: "proxy-toggle", label: "Toggle asset proxy", hint: "Use it on next Roblox launch", icon: "shield", run: () => {
      if (!app.settings) return;
      app.settings.proxy_enabled = !app.settings.proxy_enabled;
      saveSettings();
      toast(app.settings.proxy_enabled ? "Asset proxy will be used on launch" : "Asset proxy turned off");
    } },
    ...(Object.keys(PRESETS) as PresetId[]).map((id) => ({
      id: `look-${id}`, label: `Look: ${PRESETS[id].name}`, hint: PRESETS[id].blurb, icon: "brush",
      run: () => { app.ui.look = defaultLook(id); app.ui.applied = null; saveUi(); },
    })),
  ];

  // Splash: plays the animated logo once at startup, after settings load.
  let splash = $state(new URLSearchParams(location.search).get("splash") !== "0");
  const reduced = typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  $effect(() => {
    if (!splash || !app.settings) return;
    const t = setTimeout(() => (splash = false), reduced ? 500 : 1900);
    return () => clearTimeout(t);
  });

  onMount(() => {
    loadSettings();
    const key = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && (e.key.toLowerCase() === "k" || e.key === " ")) {
        e.preventDefault();
        app.launcher = !app.launcher;
      }
    };
    window.addEventListener("keydown", key);
    if (new URLSearchParams(location.search).has("launcher")) setTimeout(() => (app.launcher = true), 300);
    const offs = [
      on<Progress>("roblox-progress", (p) => (progress = p)),
      on<string>("roblox-launched", () => setTimeout(() => (progress = null), 900)),
      on<string>("roblox-error", (e) => {
        progress = null;
        toast(e, "error");
      }),
      on<Activity>("roblox-activity", (a) => {
        if (a.type === "joining") app.playing = { placeId: a.place_id };
        else if (a.type === "serverIp" && app.playing) app.playing = { ...app.playing, ip: a.ip };
        else if (a.type === "left") app.playing = null;
      }),
      on<number>("roblox-exited", (left) => {
        if (left === 0) app.playing = null;
      }),
      on<Page>("navigate", (p) => (app.page = p)),
      on<string>("proxy-notice", (m) => toast(m, "error")),
      on<{ removed: number }>("cache-cleared", (r) => toast(`Cleared ${r.removed} cache item${r.removed === 1 ? "" : "s"}`)),
    ];
    return () => {
      window.removeEventListener("keydown", key);
      offs.forEach((p) => p.then((f) => f()));
    };
  });
</script>

<div class="desktop" class:has-bar={app.ui.bar.show && app.mode === "hub"}>
  <div class="wall" style:background-image={wall ? `url("${wall}")` : undefined} aria-hidden="true"></div>

  {#if app.mode === "minecraft"}
    <!-- Standalone Minecraft launcher window ("popped out"). -->
    <div class="window solo">
      <header class="solo-bar" data-tauri-drag-region>
        <Logo size={18} />
        <span class="solo-title" data-tauri-drag-region>{brandName} Minecraft</span>
        <div class="grow" data-tauri-drag-region></div>
        <button class="wc" onclick={() => win.minimize()} aria-label="Minimize"><Icon name="minimize" size={14} /></button>
        <button class="wc" onclick={() => win.toggleMaximize()} aria-label="Maximize"><Icon name="maximize" size={13} /></button>
        <button class="wc close" onclick={() => win.close()} aria-label="Close"><Icon name="close" size={14} /></button>
      </header>
      <div class="solo-body">
        {#if app.settings}<Minecraft standalone />{/if}
      </div>
    </div>
  {:else}
  {#if app.ui.bar.show}
    <TopBar />
  {/if}

  <div class="window">
    <nav class="rail" aria-label="Main">
      <div class="brand" data-tauri-drag-region>
        {#if !app.ui.bar.show}
          <Logo size={26} />
          <span>{brandName}</span>
        {:else}
          <span class="rail-title">{brandName}</span>
          {#if app.ui.brand.tagline.trim()}<span class="rail-sub">{app.ui.brand.tagline}</span>{/if}
        {/if}
      </div>
      {#each nav as n}
        <button class="nav" class:active={app.page === n.id} aria-current={app.page === n.id ? "page" : undefined} onclick={() => (app.page = n.id)}>
          <Icon name={n.icon} size={18} />
          <span>{n.label}</span>
        </button>
      {/each}
      <div class="grow"></div>
      {#if app.playing && !app.ui.bar.show}
        <div class="playing">
          <span class="pill live"><span class="dot"></span>In game</span>
          <span class="mono faint">place {app.playing.placeId}</span>
        </div>
      {/if}
      <button class="nav" onclick={() => (app.launcher = true)}>
        <Icon name="search" size={18} />
        <span>Search</span>
        <kbd>Ctrl K</kbd>
      </button>
      <button class="nav" class:active={app.page === "settings"} onclick={() => (app.page = "settings")}>
        <Icon name="settings" size={18} />
        <span>Settings</span>
      </button>
    </nav>

    <main>
      {#if !app.ui.bar.show}
        <div class="wc-float">
          <button class="wc" onclick={() => win.minimize()} aria-label="Minimize"><Icon name="minimize" size={14} /></button>
          <button class="wc" onclick={() => win.toggleMaximize()} aria-label="Maximize"><Icon name="maximize" size={13} /></button>
          <button class="wc close" onclick={() => win.close()} aria-label="Close"><Icon name="close" size={14} /></button>
        </div>
      {/if}
      {#if app.settings}
        {#key app.page}
          <div class="view">
            {#if app.page === "library"}<Library />
            {:else if app.page === "minecraft"}<Minecraft />
            {:else if app.page === "roblox"}<Roblox />
            {:else if app.page === "proxy"}<Proxy />
            {:else if app.page === "inspector"}<Inspector />
            {:else if app.page === "appearance"}<Appearance />
            {:else}<SettingsPage />{/if}
          </div>
        {/key}
      {/if}

      {#if progress}
        <div class="launchbar" role="status" aria-live="polite">
          <span>{progressText}</span>
          <div class="track"><div class="fill" style:width="{progressPct}%"></div></div>
        </div>
      {/if}
    </main>
  </div>

  <Launcher {actions} />
  {/if}

  {#if splash}
    <div class="splash" out:fade={{ duration: reduced ? 0 : 350 }} aria-hidden="true">
      {#if app.settings}<AnimatedLogo size={112} name={brandName} />{/if}
    </div>
  {/if}

  <div class="toasts" aria-live="polite">
    {#each app.toasts as t (t.id)}
      <div class="toast" class:error={t.tone === "error"}>{t.text}</div>
    {/each}
  </div>
</div>

<style>
  .desktop {
    position: relative;
    height: 100%;
    display: grid;
    grid-template-rows: 1fr;
    gap: 10px;
    background: var(--desk);
    isolation: isolate;
  }
  .desktop.has-bar { grid-template-rows: auto 1fr; }
  .wall {
    position: absolute; inset: calc(var(--wall-blur) * -2);
    z-index: -1;
    background-size: cover;
    background-position: center;
    filter: blur(var(--wall-blur));
  }
  .wall::after { content: ""; position: absolute; inset: 0; background: rgba(0, 0, 0, var(--wall-dim)); }

  /* Floating: the window sits on the wallpaper like a tiled WM window. */
  :global(:root[data-floating="true"]) .desktop { padding: 10px 12px 12px; }
  :global(:root[data-floating="false"]) .desktop { gap: 0; }
  :global(:root[data-floating="false"]) .window { border-radius: 0; border-width: 0; box-shadow: none; }

  .window {
    min-height: 0;
    display: grid;
    grid-template-columns: 216px 1fr;
    background: var(--panel);
    backdrop-filter: blur(var(--glass-blur)) saturate(1.5);
    border: 1px solid var(--line-soft);
    border-radius: var(--r-win);
    box-shadow: 0 24px 70px rgba(0, 0, 0, 0.35);
    overflow: hidden;
  }

  .rail {
    display: flex; flex-direction: column; gap: 2px;
    padding: 1.2rem 0.75rem 0.9rem;
    background: var(--rail);
    border-right: 1px solid var(--line-soft);
    min-height: 0;
  }
  .brand {
    display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap;
    font-family: var(--display); font-weight: 800; font-size: 1.3rem;
    padding: 0 0.6rem 1.3rem;
  }
  .rail-title { overflow-wrap: anywhere; font-weight: 800; font-size: 1.05rem; letter-spacing: 0.04em; text-transform: uppercase; width: 100%; }
  .rail-sub { font-family: var(--body); font-size: 0.7rem; font-weight: 600; letter-spacing: 0.12em; text-transform: uppercase; color: var(--faint); margin-top: -0.55rem; }
  :global(:root[data-caps="false"]) .rail-title { text-transform: none; letter-spacing: -0.01em; font-size: 1.25rem; }
  :global(:root[data-caps="false"]) .rail-sub { text-transform: none; letter-spacing: 0; font-size: 0.8rem; }

  .nav {
    display: flex; align-items: center; gap: 0.75rem;
    background: transparent; border: 1px solid transparent; border-radius: var(--r-md);
    padding: 0.55rem 0.7rem; color: var(--muted); font-weight: 500; text-align: left; font-size: 0.93rem;
  }
  .nav:hover { background: var(--raised); color: var(--text); border-color: transparent; }
  .nav.active { background: var(--raised); color: var(--text); border-color: var(--line-soft); font-weight: 600; }
  :global(:root[data-preset="dusk"]) .nav.active { box-shadow: inset 3px 0 0 var(--peach); border-color: transparent; }
  :global(:root[data-preset="gruvbox"]) .nav.active { background: var(--peach); color: var(--peach-ink); }
  .nav kbd { margin-left: auto; font: inherit; font-size: 0.7rem; color: var(--faint); border: 1px solid var(--line); border-radius: 5px; padding: 0 0.3rem; }
  .grow { flex: 1; }
  .playing { display: grid; gap: 0.2rem; padding: 0.7rem; margin-bottom: 0.4rem; font-size: 0.8rem; }

  main { position: relative; min-width: 0; min-height: 0; overflow: hidden; }
  .view { height: 100%; animation: enter 0.18s ease-out; }
  @keyframes enter { from { opacity: 0; transform: translateY(4px); } }

  .window.solo { display: flex; flex-direction: column; }
  .solo-bar { display: flex; align-items: center; gap: 0.6rem; padding: 0.5rem 0.6rem 0.4rem 1rem; color: var(--muted); border-bottom: 1px solid var(--line-soft); }
  .solo-title { font-family: var(--display); font-weight: 700; color: var(--text); font-size: 0.92rem; }
  .solo-body { flex: 1; min-height: 0; }
  .wc-float { position: absolute; top: 10px; right: 12px; z-index: 5; display: flex; gap: 2px; }
  .wc { display: grid; place-items: center; width: 26px; height: 26px; padding: 0; border: 0; border-radius: 999px; background: transparent; color: var(--muted); }
  .wc:hover { background: var(--raised); color: var(--text); }
  .wc.close:hover { background: var(--rose); color: white; }

  .launchbar {
    position: absolute; left: 0; right: 0; bottom: 0;
    display: flex; align-items: center; gap: 1.2rem;
    padding: 0.85rem 2.2rem; background: var(--raised); border-top: 1px solid var(--line);
    font-weight: 600;
  }
  .track { flex: 1; height: 6px; background: var(--sunk); border-radius: 6px; overflow: hidden; }
  .fill { height: 100%; background: var(--peach); transition: width 0.3s ease; }

  .splash {
    position: fixed; inset: 0; z-index: 80;
    display: grid; place-items: center;
    background: var(--desk);
  }
  :global(:root[data-scheme="dark"]) .splash, :global(:root[data-preset="obsidian"]) .splash { background: var(--ink); }
  .toasts { position: fixed; right: 1.5rem; bottom: 1.5rem; display: grid; gap: 0.5rem; z-index: 70; }
  .toast {
    background: var(--panel); backdrop-filter: blur(calc(var(--glass-blur) + 8px));
    border: 1px solid var(--line); border-left: 3px solid var(--mint);
    padding: 0.65rem 1rem; border-radius: var(--r-md); max-width: 420px; font-size: 0.9rem;
  }
  .toast.error { border-left-color: var(--rose); }
</style>
