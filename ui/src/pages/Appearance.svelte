<script lang="ts">
  import { api, fileUrl, pickFile } from "../lib/api";
  import { app, saveUi, toast, focus } from "../lib/state.svelte";
  import {
    ACCENTS, BUILTIN_WALLPAPERS, DEFAULT_BRAND, DEFAULT_HOME, PRESETS, defaultLook, resolve, wallpaperUrl,
    type FontChoice, type Look, type PresetId, type RadiusChoice,
  } from "../lib/theme";
  import Toggle from "../components/Toggle.svelte";
  import Icon from "../components/Icon.svelte";
  import { allNav } from "../lib/nav";
  import { addonById } from "../addons/registry";
  import TreeTabs, { type TreeNode } from "../components/TreeTabs.svelte";
  import Profiles from "./appearance/Profiles.svelte";
  import Feel from "./appearance/Feel.svelte";
  import Keys from "./appearance/Keys.svelte";

  const look = $derived(app.ui.look);
  const presetIds = Object.keys(PRESETS) as PresetId[];

  /** Any edit by hand means the saved look no longer matches exactly. */
  function changed() {
    app.ui.applied = null;
    saveUi();
  }
  function setPreset(id: PresetId) {
    // Keep a custom wallpaper; take the preset's for built-ins.
    const custom = look.wallpaper && !look.wallpaper.startsWith("builtin:") ? look.wallpaper : null;
    app.ui.look = { ...defaultLook(id), ...(custom ? { wallpaper: custom } : {}) };
    changed();
  }
  function set<K extends keyof Look>(k: K, v: Look[K]) {
    app.ui.look[k] = v;
    changed();
  }

  // Saved looks
  let newName = $state("");
  function saveLook() {
    const name = newName.trim();
    if (!name) return;
    const snap = $state.snapshot(app.ui.look) as Look;
    const i = app.ui.saved.findIndex((s) => s.name === name);
    if (i >= 0) app.ui.saved[i].look = snap;
    else app.ui.saved.push({ name, look: snap });
    app.ui.applied = name;
    newName = "";
    saveUi();
    toast(`Saved “${name}”`);
  }
  function applySaved(name: string) {
    const s = app.ui.saved.find((x) => x.name === name);
    if (!s) return;
    app.ui.look = { ...defaultLook(s.look.preset), ...s.look };
    app.ui.applied = name;
    saveUi();
  }
  function deleteSaved(name: string) {
    app.ui.saved = app.ui.saved.filter((s) => s.name !== name);
    if (app.ui.applied === name) app.ui.applied = null;
    saveUi();
  }

  async function chooseWallpaper() {
    const p = await pickFile([{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp"] }]);
    if (!p) return;
    try {
      set("wallpaper", await api.setWallpaper(p));
    } catch (e) {
      toast(String(e), "error");
    }
  }
  const customWall = $derived(look.wallpaper && !look.wallpaper.startsWith("builtin:") ? look.wallpaper : null);

  const fonts: { id: FontChoice; label: string }[] = [
    { id: "theme", label: "Look default" },
    { id: "grotesk", label: "Grotesk" },
    { id: "rounded", label: "Rounded" },
    { id: "display", label: "Display" },
    { id: "mono", label: "Mono" },
  ];
  const radii: { id: RadiusChoice; label: string }[] = [
    { id: "theme", label: "Look default" },
    { id: "sharp", label: "Sharp" },
    { id: "soft", label: "Soft" },
    { id: "round", label: "Round" },
  ];
  // ── Sidebar ──────────────────────────────────────────────
  const railItems = $derived(allNav(app.ui));
  const hiddenCount = $derived(railItems.filter((n) => app.ui.rail.hidden.includes(n.id)).length);
  function moveTo(id: string, to: number) {
    const ids = railItems.map((n) => n.id as string);
    const from = ids.indexOf(id);
    if (from < 0 || to < 0 || to >= ids.length || from === to) return;
    ids.splice(to, 0, ...ids.splice(from, 1));
    app.ui.rail.order = ids;
    saveUi();
  }
  function toggleHidden(id: string) {
    const h = app.ui.rail.hidden;
    app.ui.rail.hidden = h.includes(id) ? h.filter((x) => x !== id) : [...h, id];
    saveUi();
  }
  function resetRail() {
    app.ui.rail = { order: [], hidden: [], compact: false };
    saveUi();
  }
  let dragging: string | null = $state(null);
  let dropAt: number | null = $state(null);
  function onDrop(i: number) {
    if (dragging) moveTo(dragging, i);
    dragging = null;
    dropAt = null;
  }

  // ── Addons with settings here ─────────────────────────────
  const settingAddons = $derived(app.ui.addons.installed.map(addonById).filter((a) => a?.settings));

  // ── Splash ───────────────────────────────────────────────
  const motionOff = typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  const splashLengths = [
    { id: "off", label: "Off" },
    { id: "short", label: "Short" },
    { id: "normal", label: "Normal" },
    { id: "long", label: "Long" },
  ] as const;

  const opacity = $derived(look.panelOpacity ?? PRESETS[look.preset].panelOpacity);

  // ── Sections ─────────────────────────────────────────────
  const tree: TreeNode[] = $derived([
    { id: "look", label: "Look", icon: "palette", children: [
      { id: "look-presets", label: "Looks", badge: PRESETS[look.preset].name },
      { id: "look-saved", label: "Saved looks", badge: app.ui.saved.length || "" },
      { id: "look-wallpaper", label: "Wallpaper" },
      { id: "look-window", label: "Window & colour" },
    ] },
    { id: "feel", label: "Feel", icon: "sparkle", children: [
      { id: "feel-motion", label: "Motion & sound", badge: app.ui.feel.sounds !== "off" ? "♪" : "" },
      { id: "feel-type", label: "Fonts & icons" },
    ] },
    { id: "layout", label: "Layout", icon: "window", children: [
      { id: "layout-home", label: "Home" },
      { id: "layout-sidebar", label: "Sidebar", badge: hiddenCount ? `${hiddenCount} hidden` : "" },
      { id: "layout-bar", label: "Top bar" },
    ] },
    { id: "keys", label: "Keyboard", icon: "command" },
    { id: "profiles", label: "Profiles & backup", icon: "save", children: [
      { id: "profiles-list", label: "Profiles", badge: app.ui.profiles.list.length || "" },
      { id: "profiles-backup", label: "Backup & sync" },
    ] },
    { id: "app", label: "Name & startup", icon: "settings" },
    ...(settingAddons.length ? [{ id: "addons", label: "Addons", icon: "store", children: settingAddons.map((a) => ({ id: `addon:${a!.id}`, label: a!.name })) }] : []),
  ]);
  let tab = $state(lastTab);
  $effect(() => {
    lastTab = tab;
  });
  // Opened from the Marketplace: go straight to that addon's settings.
  $effect(() => {
    if (focus.addon) {
      tab = `addon:${focus.addon}`;
      focus.addon = null;
    }
  });
  const show = (id: string) => tab === id || id.startsWith(tab + "-") || (tab === "addons" && id === `addon:${settingAddons[0]?.id}`);
</script>

<script lang="ts" module>
  let lastTab = "look-presets";
</script>

<div class="page">
  <div class="page-inner">
    <header class="page-head">
      <div>
        <h2>Appearance</h2>
        <p>Pick a look, then make it yours. Changes apply as you go.</p>
      </div>
    </header>

    <div class="split">
    <TreeTabs key="appearance" label="Appearance sections" nodes={tree} bind:selected={tab} search />
    <div class="panes">

    {#if show("app")}
    <section class="section">
      <h3>Name</h3>
      <div class="setting">
        <div><div class="label">App name</div><div class="desc">Shown in the top bar, the sidebar and the window title.</div></div>
        <input class="name-in" maxlength="24" placeholder={DEFAULT_BRAND.name} bind:value={app.ui.brand.name} oninput={saveUi} aria-label="App name" />
      </div>
      <div class="setting">
        <div><div class="label">Tagline</div><div class="desc">The small line under the name in the sidebar. Leave it empty to hide it.</div></div>
        <input class="name-in" maxlength="40" bind:value={app.ui.brand.tagline} oninput={saveUi} aria-label="Tagline" />
      </div>
      {#if app.ui.brand.name !== DEFAULT_BRAND.name || app.ui.brand.tagline !== DEFAULT_BRAND.tagline}
        <div class="setting">
          <div class="desc">Back to “{DEFAULT_BRAND.name}” and “{DEFAULT_BRAND.tagline}”.</div>
          <button onclick={() => { app.ui.brand = { ...DEFAULT_BRAND }; saveUi(); }}>Reset name</button>
        </div>
      {/if}
    </section>

    {/if}

    {#if show("layout-sidebar")}
    <section class="section">
      <div class="row spread"><h3>Sidebar</h3>
        {#if app.ui.rail.order.length || app.ui.rail.hidden.length || app.ui.rail.compact}
          <button class="ghost small" onclick={resetRail}>Reset</button>
        {/if}
      </div>
      <p class="hint">Drag to reorder, or use the arrows. Hidden pages stay one Ctrl+K away; Settings and Search always stay at the bottom.</p>
      <ul class="rail-edit" aria-label="Sidebar entries">
        {#each railItems as n, i (n.id)}
          {@const hidden = app.ui.rail.hidden.includes(n.id)}
          <li
            class:hidden
            class:dragging={dragging === n.id}
            class:drop={dropAt === i && dragging !== n.id}
            draggable="true"
            ondragstart={(e) => { dragging = n.id; e.dataTransfer?.setData("text/plain", n.id); }}
            ondragover={(e) => { e.preventDefault(); dropAt = i; }}
            ondragleave={() => { if (dropAt === i) dropAt = null; }}
            ondrop={(e) => { e.preventDefault(); onDrop(i); }}
            ondragend={() => { dragging = null; dropAt = null; }}
          >
            <span class="grip" aria-hidden="true"><Icon name="grip" size={16} /></span>
            <Icon name={n.icon} size={17} />
            <span class="rail-label">{n.label}</span>
            {#if n.addon}<span class="pill">Addon</span>{/if}
            <span class="grow"></span>
            <button class="ghost icon" disabled={i === 0} onclick={() => moveTo(n.id, i - 1)} aria-label="Move {n.label} up"><Icon name="up" size={15} /></button>
            <button class="ghost icon" disabled={i === railItems.length - 1} onclick={() => moveTo(n.id, i + 1)} aria-label="Move {n.label} down"><Icon name="down" size={15} /></button>
            <button class="ghost icon" onclick={() => toggleHidden(n.id)} aria-label={hidden ? `Show ${n.label}` : `Hide ${n.label}`} title={hidden ? "Show" : "Hide"}>
              <Icon name={hidden ? "eye-off" : "eye"} size={16} />
            </button>
          </li>
        {/each}
      </ul>
      <div class="setting">
        <div><div class="label">Icons only</div><div class="desc">A narrow sidebar; hover an icon to see its name.{hiddenCount ? ` ${hiddenCount} hidden.` : ""}</div></div>
        <Toggle label="Icons only" checked={app.ui.rail.compact} onchange={(v) => { app.ui.rail.compact = v; saveUi(); }} />
      </div>
    </section>

    {/if}

    {#if show("app")}
    <section class="section">
      <h3>Startup</h3>
      <div class="setting">
        <div><div class="label">Splash screen</div><div class="desc">The logo builds itself while {app.ui.brand.name.trim() || "Arcadia"} opens. Click or press any key to skip it.</div></div>
        <div class="row">
          <div class="seg" role="radiogroup" aria-label="Splash length">
            {#each splashLengths as l}
              <button role="radio" aria-checked={app.ui.splash.length === l.id} onclick={() => { app.ui.splash.length = l.id; saveUi(); }}>{l.label}</button>
            {/each}
          </div>
          <button class="ghost small" disabled={app.ui.splash.length === "off"} onclick={() => app.splashRun++}>Preview</button>
        </div>
      </div>
      <div class="setting">
        <div>
          <div class="label">Animate even with Windows animations off</div>
          <div class="desc">
            {motionOff
              ? "Windows animation effects are off on this PC, so the logo shows without moving. Turn this on to see it build anyway."
              : "If you turn off animation effects in Windows, the logo shows without moving. Turn this on to keep the animation."}
          </div>
        </div>
        <Toggle label="Animate even with Windows animations off" checked={app.ui.splash.animateAlways} onchange={(v) => { app.ui.splash.animateAlways = v; saveUi(); }} />
      </div>
    </section>

    {/if}

    {#each settingAddons as a (a!.id)}
      {#if show(`addon:${a!.id}`)}
      <section class="section addon" id="addon-{a!.id}">
        <div class="row spread"><h3>{a!.name}</h3><span class="pill">Addon</span></div>
        {#await a!.settings!()}
          <p class="hint">Loading…</p>
        {:then mod}
          <mod.default />
        {/await}
      </section>
      {/if}
    {/each}

    {#if show("look-presets")}
    <section class="section">
      <h3>Look</h3>
      <div class="presets" role="radiogroup" aria-label="Look">
        {#each presetIds as id}
          {@const r = resolve(defaultLook(id))}
          {@const wall = wallpaperUrl(PRESETS[id].defaults.wallpaper, fileUrl)}
          <button
            class="preset"
            role="radio"
            aria-checked={look.preset === id}
            onclick={() => setPreset(id)}
          >
            <!-- A miniature of the app drawn with that look's own tokens. -->
            <div class="mini" style={Object.entries(r.vars).map(([k, v]) => `${k}:${v}`).join(";")} style:background-image={wall ? `url("${wall}")` : undefined}>
              <div class="mini-bar"><i></i><i class="c"></i><i></i></div>
              <div class="mini-win" class:flush={!PRESETS[id].defaults.floating}>
                <div class="mini-rail"><b></b><b class="on"></b><b></b><b></b></div>
                <div class="mini-main">
                  <div class="mini-group"><span></span><em></em></div>
                  <div class="mini-group"><span></span><em class="off"></em></div>
                  <div class="mini-play"></div>
                </div>
              </div>
            </div>
            <span class="preset-name">{PRESETS[id].name}</span>
            <span class="preset-blurb">{PRESETS[id].blurb}</span>
          </button>
        {/each}
      </div>
    </section>

    {/if}

    {#if show("look-saved")}
    <section class="section">
      <h3>Saved looks</h3>
      {#if app.ui.saved.length === 0}
        <p class="hint">Save the current look under a name to switch back to it in one click.</p>
      {:else}
        <ul class="saved">
          {#each app.ui.saved as s (s.name)}
            <li class:applied={app.ui.applied === s.name}>
              <button class="saved-name" onclick={() => applySaved(s.name)}>
                <span class="sdot"></span>
                <span>{s.name}</span>
                <span class="faint small">{PRESETS[s.look.preset]?.name ?? s.look.preset}</span>
              </button>
              {#if app.ui.applied === s.name}<span class="tag">Applied</span>{/if}
              <button class="ghost danger icon" onclick={() => deleteSaved(s.name)} aria-label="Delete {s.name}"><Icon name="trash" size={16} /></button>
            </li>
          {/each}
        </ul>
      {/if}
      <div class="row save-row">
        <input class="grow" placeholder="New look name" bind:value={newName} onkeydown={(e) => e.key === "Enter" && saveLook()} aria-label="New look name" />
        <button onclick={saveLook} disabled={!newName.trim()}>Save current look</button>
      </div>
    </section>

    {/if}

    {#if show("look-wallpaper")}
    <section class="section">
      <h3>Wallpaper</h3>
      <div class="walls" role="radiogroup" aria-label="Wallpaper">
        <button class="wall-opt none" role="radio" aria-checked={look.wallpaper === null} onclick={() => set("wallpaper", null)}>
          <span class="thumb plain"></span><span>None</span>
        </button>
        {#each BUILTIN_WALLPAPERS as w}
          <button class="wall-opt" role="radio" aria-checked={look.wallpaper === `builtin:${w.id}`} onclick={() => set("wallpaper", `builtin:${w.id}`)}>
            <span class="thumb" style:background-image={`url("/wallpapers/${w.id}.svg")`}></span><span>{w.name}</span>
          </button>
        {/each}
        {#if customWall}
          <button class="wall-opt" role="radio" aria-checked={true}>
            <span class="thumb" style:background-image={`url("${fileUrl(customWall)}")`}></span><span>Your image</span>
          </button>
        {/if}
        <button class="wall-opt add" onclick={chooseWallpaper}>
          <span class="thumb plus"><Icon name="plus" size={20} /></span><span>Choose image</span>
        </button>
      </div>
      <div class="setting">
        <div><div class="label">Blur</div><div class="desc">Soften the wallpaper behind the window.</div></div>
        <div class="slider">
          <input type="range" min="0" max="40" step="1" value={look.wallpaperBlur} oninput={(e) => set("wallpaperBlur", +e.currentTarget.value)} aria-label="Wallpaper blur" />
          <span class="mono val">{look.wallpaperBlur}px</span>
        </div>
      </div>
      <div class="setting">
        <div><div class="label">Dim</div><div class="desc">Darken a bright wallpaper so text stays readable.</div></div>
        <div class="slider">
          <input type="range" min="0" max="0.8" step="0.05" value={look.wallpaperDim} oninput={(e) => set("wallpaperDim", +e.currentTarget.value)} aria-label="Wallpaper dim" />
          <span class="mono val">{Math.round(look.wallpaperDim * 100)}%</span>
        </div>
      </div>
    </section>

    {/if}

    {#if show("look-window")}
    <section class="section">
      <h3>Window & colour</h3>
      <div class="setting">
        <div><div class="label">Float over the wallpaper</div><div class="desc">Leave a gap around the window, like a tiling window manager.</div></div>
        <Toggle label="Float over the wallpaper" checked={look.floating} onchange={(v) => set("floating", v)} />
      </div>
      <div class="setting">
        <div><div class="label">Panel opacity</div><div class="desc">Lower it to see the wallpaper through frosted glass.</div></div>
        <div class="slider">
          <input type="range" min="0.3" max="1" step="0.02" value={opacity} oninput={(e) => set("panelOpacity", +e.currentTarget.value)} aria-label="Panel opacity" />
          <span class="mono val">{Math.round(opacity * 100)}%</span>
        </div>
      </div>
      <div class="setting">
        <div><div class="label">Corners</div></div>
        <div class="seg" role="radiogroup" aria-label="Corners">
          {#each radii as r}<button role="radio" aria-checked={look.radius === r.id} onclick={() => set("radius", r.id)}>{r.label}</button>{/each}
        </div>
      </div>
      <div class="setting">
        <div><div class="label">Font</div></div>
        <div class="seg" role="radiogroup" aria-label="Font">
          {#each fonts as f}<button role="radio" aria-checked={look.font === f.id} onclick={() => set("font", f.id)}>{f.label}</button>{/each}
        </div>
      </div>
      <div class="setting">
        <div><div class="label">Accent</div><div class="desc">Used for Play, the active page and the launcher.</div></div>
        <div class="swatches" role="radiogroup" aria-label="Accent">
          <button class="sw default" role="radio" aria-checked={look.accent === null} onclick={() => set("accent", null)} title="Look default" aria-label="Look default"
            style:background={PRESETS[look.preset].colors.accent}></button>
          {#each ACCENTS as a}
            <button class="sw" role="radio" aria-checked={look.accent === a} style:background={a} onclick={() => set("accent", a)} aria-label="Accent {a}"></button>
          {/each}
          <label class="sw custom" title="Pick any color">
            <input type="color" value={look.accent ?? PRESETS[look.preset].colors.accent} oninput={(e) => set("accent", e.currentTarget.value)} aria-label="Custom accent" />
          </label>
        </div>
      </div>
    </section>

    {/if}

    {#if show("layout-bar")}
    <section class="section">
      <h3>Top bar</h3>
      <div class="setting">
        <div><div class="label">Show the top bar</div><div class="desc">Pages, clock, proxy status and window buttons.</div></div>
        <Toggle label="Show the top bar" bind:checked={app.ui.bar.show} onchange={saveUi} />
      </div>
      {#if app.ui.bar.show}
        <div class="setting">
          <div><div class="label">Style</div><div class="desc">Separate floating islands, or one flush strip.</div></div>
          <div class="seg" role="radiogroup" aria-label="Top bar style">
            <button role="radio" aria-checked={app.ui.bar.style === "island"} onclick={() => { app.ui.bar.style = "island"; saveUi(); }}>Island</button>
            <button role="radio" aria-checked={app.ui.bar.style === "strip"} onclick={() => { app.ui.bar.style = "strip"; saveUi(); }}>Strip</button>
          </div>
        </div>
        <div class="setting">
          <div><div class="label">24-hour time</div></div>
          <Toggle label="24-hour time" bind:checked={app.ui.bar.clock24} onchange={saveUi} />
        </div>
        <div class="setting">
          <div><div class="label">Show seconds</div></div>
          <Toggle label="Show seconds" bind:checked={app.ui.bar.seconds} onchange={saveUi} />
        </div>
        <div class="setting">
          <div><div class="label">Show date</div></div>
          <Toggle label="Show date" bind:checked={app.ui.bar.date} onchange={saveUi} />
        </div>
      {/if}
    </section>
    {/if}

    {#if show("layout-home")}
    <section class="section">
      <h3>Home</h3>
      <p class="hint">A dashboard of widgets: continue playing, now playing, Roblox and Minecraft, playtime, pinned games, notes and a clock. Arrange them on Home with <strong>Customise</strong>.</p>
      <div class="setting">
        <div><div class="label">Open Home when {app.ui.brand.name.trim() || "Arcadia"} starts</div></div>
        <Toggle label="Open Home on start" checked={app.ui.home.start} onchange={(v) => { app.ui.home.start = v; saveUi(); }} />
      </div>
      <div class="setting">
        <div><div class="label">Widgets</div><div class="desc">{app.ui.home.widgets.length} on Home.</div></div>
        <div class="row">
          <button class="ghost" onclick={() => { app.ui.home = { ...app.ui.home, widgets: structuredClone(DEFAULT_HOME.widgets) }; saveUi(); }}>Reset widgets</button>
          <button onclick={() => (app.page = "home")}>Go to Home</button>
        </div>
      </div>
    </section>
    {/if}

    {#if show("feel-motion")}<Feel view="motion" />{/if}
    {#if show("feel-type")}<Feel view="type" />{/if}
    {#if show("keys")}<Keys />{/if}
    {#if show("profiles-list")}<Profiles view="profiles" />{/if}
    {#if show("profiles-backup")}<Profiles view="backup" />{/if}
    </div>
    </div>
  </div>
</div>

<style>
  .split { display: grid; grid-template-columns: 210px minmax(0, 1fr); gap: 1.6rem; align-items: start; }
  .panes { min-width: 0; display: flex; flex-direction: column; gap: 1rem; }
  .panes :global(.section) { margin-top: 0; }
  @media (max-width: 860px) { .split { grid-template-columns: 1fr; } }
  .name-in { width: 16rem; }
  .presets { display: grid; grid-template-columns: repeat(4, 1fr); gap: 0.9rem; padding-bottom: 0.7rem; }
  .preset {
    display: grid; gap: 0.25rem; text-align: left; padding: 0.5rem 0.5rem 0.7rem;
    background: transparent; border: 1px solid var(--line-soft); border-radius: var(--r-lg);
  }
  .preset:hover { background: var(--raised); }
  .preset[aria-checked="true"] { border-color: var(--peach); box-shadow: 0 0 0 1px var(--peach); }
  .preset-name { font-family: var(--display); font-weight: 700; margin-top: 0.35rem; padding: 0 0.2rem; }
  .preset-blurb { color: var(--muted); font-size: 0.8rem; font-weight: 400; line-height: 1.35; padding: 0 0.2rem; }

  .mini {
    aspect-ratio: 16 / 10; border-radius: calc(var(--r-md) + 2px); overflow: hidden;
    background: var(--desk); background-size: cover; background-position: center;
    display: grid; grid-template-rows: 9px 1fr; gap: 4px; padding: 5px;
    font-family: var(--body);
  }
  .mini-bar { display: flex; justify-content: space-between; }
  .mini-bar i { width: 26%; border-radius: 99px; background: var(--panel); border: 1px solid var(--line-soft); }
  .mini-bar i.c { width: 16%; }
  .mini-win {
    display: grid; grid-template-columns: 26% 1fr; background: var(--panel);
    backdrop-filter: blur(6px); border: 1px solid var(--line-soft); border-radius: calc(var(--r-win) / 4); overflow: hidden;
  }
  .mini-win.flush { border-radius: 2px; }
  .mini-rail { display: grid; align-content: start; gap: 3px; padding: 6px 4px; border-right: 1px solid var(--line-soft); background: var(--rail); }
  .mini-rail b { height: 5px; border-radius: 2px; background: var(--line); }
  .mini-rail b.on { background: var(--peach); }
  .mini-main { display: grid; align-content: start; gap: 4px; padding: 6px; }
  .mini-group { display: flex; align-items: center; justify-content: space-between; height: 13px; padding: 0 4px; border-radius: 3px; background: var(--group); border: 1px solid var(--line-soft); }
  .mini-group span { width: 40%; height: 3px; background: var(--muted); border-radius: 2px; opacity: 0.7; }
  .mini-group em { width: 12px; height: 6px; border-radius: 9px; background: var(--mint); }
  .mini-group em.off { background: var(--line); }
  .mini-play { width: 34%; height: 9px; border-radius: 9px; background: var(--peach); margin-top: 2px; }

  .saved { list-style: none; margin: 0 0 0.6rem; padding: 0; }
  .saved li { display: flex; align-items: center; gap: 0.6rem; border-bottom: 1px solid var(--line-soft); }
  .saved-name { all: unset; cursor: pointer; flex: 1; display: flex; align-items: center; gap: 0.7rem; padding: 0.7rem 0.2rem; font-weight: 500; }
  .saved-name:focus-visible { outline: 2px solid var(--lilac); border-radius: 4px; }
  .sdot { width: 8px; height: 8px; border-radius: 50%; background: var(--line); }
  .applied .sdot { background: var(--peach); }
  .tag { color: var(--peach); font-size: 0.82rem; font-weight: 600; }
  .icon { padding: 0.35rem; }
  .small { font-size: 0.8rem; }
  .save-row { padding: 0.5rem 0 0.7rem; }

  .walls { display: grid; grid-template-columns: repeat(auto-fill, minmax(112px, 1fr)); gap: 0.7rem; padding-bottom: 0.6rem; }
  .wall-opt { display: grid; gap: 0.35rem; padding: 0; border: 0; background: transparent; text-align: left; font-size: 0.82rem; font-weight: 500; color: var(--muted); }
  .wall-opt:hover { background: transparent; color: var(--text); }
  .thumb {
    aspect-ratio: 16 / 10; border-radius: var(--r-md); background-size: cover; background-position: center;
    border: 1px solid var(--line-soft); display: grid; place-items: center;
  }
  .thumb.plain { background: var(--desk); }
  .thumb.plus { border-style: dashed; color: var(--muted); }
  .wall-opt[aria-checked="true"] { color: var(--text); }
  .wall-opt[aria-checked="true"] .thumb { outline: 2px solid var(--peach); outline-offset: 2px; }

  .slider { display: flex; align-items: center; gap: 0.7rem; }
  .slider input { width: 180px; }
  .val { width: 3.2rem; text-align: right; color: var(--muted); }

  .rail-edit { list-style: none; margin: 0 0 0.4rem; padding: 0; display: grid; gap: 2px; max-width: 520px; }
  .rail-edit li {
    display: flex; align-items: center; gap: 0.6rem;
    padding: 0.35rem 0.4rem 0.35rem 0.5rem; border-radius: var(--r-md);
    border: 1px solid transparent; background: transparent; cursor: grab;
  }
  .rail-edit li:hover { background: var(--raised); border-color: var(--line-soft); }
  .rail-edit li.hidden .rail-label, .rail-edit li.hidden > :global(svg) { opacity: 0.45; }
  .rail-edit li.dragging { opacity: 0.4; }
  .rail-edit li.drop { box-shadow: inset 0 2px 0 var(--peach); }
  .rail-label { font-weight: 600; font-size: 0.92rem; }
  .grip { color: var(--faint); display: grid; }
  button.icon { padding: 0.25rem; display: grid; place-items: center; border-radius: var(--r-sm); }
  button.small { padding: 0.25rem 0.6rem; font-size: 0.82rem; }
  .swatches { display: flex; flex-wrap: wrap; gap: 0.45rem; justify-content: flex-end; max-width: 360px; }
  .sw { width: 24px; height: 24px; padding: 0; border-radius: 50%; border: 1px solid var(--line); }
  .sw[aria-checked="true"] { box-shadow: 0 0 0 2px var(--ink), 0 0 0 4px var(--text); }
  .sw.default { position: relative; }
  .sw.default::after { content: ""; position: absolute; inset: 7px; border-radius: 50%; background: var(--ink); }
  .sw.custom { position: relative; overflow: hidden; background: conic-gradient(#ff6b6b, #ffd93d, #6bff95, #6bd9ff, #9b6bff, #ff6bd5, #ff6b6b); cursor: pointer; }
  .sw.custom input { position: absolute; inset: 0; opacity: 0; cursor: pointer; }
</style>
