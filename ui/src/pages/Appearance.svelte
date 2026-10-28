<script lang="ts">
  import { api, fileUrl, pickFile } from "../lib/api";
  import { app, saveUi, toast } from "../lib/state.svelte";
  import {
    ACCENTS, BUILTIN_WALLPAPERS, DEFAULT_BRAND, PRESETS, defaultLook, resolve, wallpaperUrl,
    type FontChoice, type Look, type PresetId, type RadiusChoice,
  } from "../lib/theme";
  import Toggle from "../components/Toggle.svelte";
  import Icon from "../components/Icon.svelte";

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
  const opacity = $derived(look.panelOpacity ?? PRESETS[look.preset].panelOpacity);
</script>

<div class="page">
  <div class="page-inner">
    <header class="page-head">
      <div>
        <h2>Appearance</h2>
        <p>Pick a look, then make it yours. Changes apply as you go.</p>
      </div>
    </header>

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

    <section class="section">
      <h3>Window</h3>
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
  </div>
</div>

<style>
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

  .swatches { display: flex; flex-wrap: wrap; gap: 0.45rem; justify-content: flex-end; max-width: 360px; }
  .sw { width: 24px; height: 24px; padding: 0; border-radius: 50%; border: 1px solid var(--line); }
  .sw[aria-checked="true"] { box-shadow: 0 0 0 2px var(--ink), 0 0 0 4px var(--text); }
  .sw.default { position: relative; }
  .sw.default::after { content: ""; position: absolute; inset: 7px; border-radius: 50%; background: var(--ink); }
  .sw.custom { position: relative; overflow: hidden; background: conic-gradient(#ff6b6b, #ffd93d, #6bff95, #6bd9ff, #9b6bff, #ff6bd5, #ff6b6b); cursor: pointer; }
  .sw.custom input { position: absolute; inset: 0; opacity: 0; cursor: pointer; }
</style>
