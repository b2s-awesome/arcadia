<script lang="ts">
  // Everything about the Music page that can be changed, in one panel.
  import { onMount } from "svelte";
  import Icon from "../components/Icon.svelte";
  import Toggle from "../components/Toggle.svelte";
  import { music, pickFolder } from "../lib/api";
  import { app, saveUi, toast } from "../lib/state.svelte";
  import { MUSIC_SECTIONS, MUSIC_TABS, PRESETS, type MusicPrefs, type MusicSection, type MusicTab, type MusicTheme } from "../lib/theme";
  import { STYLE_INFO, type GradientStyle } from "../lib/gradient";
  import GradientPicker from "../components/GradientPicker.svelte";
  import { ms, reloadLibrary } from "./store.svelte";

  const p = $derived(app.ui.music);
  function setTheme<K extends keyof MusicTheme>(k: K, v: MusicTheme[K]) {
    app.ui.music.theme[k] = v;
    saveUi();
  }
  function set<K extends keyof MusicPrefs>(k: K, v: MusicPrefs[K]) {
    app.ui.music[k] = v;
    saveUi();
  }
  const label = (id: MusicSection) => MUSIC_SECTIONS.find((s) => s.id === id)?.label ?? id;
  function move(id: MusicSection, d: number) {
    const v = [...p.sections];
    const i = v.indexOf(id);
    const j = i + d;
    if (j < 0 || j >= v.length) return;
    [v[i], v[j]] = [v[j], v[i]];
    set("sections", v);
  }
  function toggleSection(id: MusicSection) {
    set("hidden", p.hidden.includes(id) ? p.hidden.filter((x) => x !== id) : [...p.hidden, id]);
  }

  const tabLabel = (id: MusicTab) => MUSIC_TABS.find((t) => t.id === id)?.label ?? id;
  const tabAvailable = (id: MusicTab) => {
    const a = MUSIC_TABS.find((t) => t.id === id)?.addon;
    return !a || app.ui.addons.installed.includes(a);
  };
  const shownTabs = $derived(p.tabs.filter(tabAvailable));
  function moveTab(id: MusicTab, d: number) {
    // Move among the tabs that are shown, keeping unavailable ones in place.
    const vis = shownTabs;
    const i = vis.indexOf(id);
    const j = i + d;
    if (j < 0 || j >= vis.length) return;
    const all = [...p.tabs];
    const a = all.indexOf(vis[i]);
    const b = all.indexOf(vis[j]);
    [all[a], all[b]] = [all[b], all[a]];
    set("tabs", all);
  }
  function toggleTab(id: MusicTab) {
    set("hiddenTabs", p.hiddenTabs.includes(id) ? p.hiddenTabs.filter((x) => x !== id) : [...p.hiddenTabs, id]);
  }

  // Folders and output
  let folders: string[] = $state([]);
  let devices: string[] = $state([]);
  let scanning = $state(false);
  onMount(async () => {
    [folders, devices] = await Promise.all([music.folders(), music.devices()]);
  });
  async function saveFolders(next: string[]) {
    folders = next;
    await music.setFolders(next);
    await rescan();
  }
  async function addFolder() {
    const f = await pickFolder();
    if (f && !folders.includes(f)) await saveFolders([...folders, f]);
  }
  async function rescan() {
    scanning = true;
    try {
      const r = await music.scan();
      await reloadLibrary();
      const bits = [r.added && `${r.added} new`, r.updated && `${r.updated} updated`, r.removed && `${r.removed} removed`].filter(Boolean);
      toast(bits.length ? `Library: ${bits.join(", ")} (${r.total.toLocaleString()} songs)` : `Library is up to date (${r.total.toLocaleString()} songs)`);
      if (r.missingFolders.length) toast(`Couldn't find ${r.missingFolders.join(", ")}. Its songs are kept until it's back.`, "error");
    } catch (e) {
      toast(String(e), "error");
    }
    scanning = false;
  }
  async function setDevice(name: string) {
    await music.setDevice(name || null);
    if (ms.status) ms.status.device = name || null;
  }
</script>

<aside class="drawer mscroll" aria-label="Customise Music">
  <header>
    <h3>Customise Music</h3>
    <button class="x" aria-label="Close" onclick={() => (ms.customise = false)}><Icon name="close" size={16} /></button>
  </header>

  <section>
    <h4>Layout</h4>
    <div class="layouts" role="radiogroup" aria-label="Layout">
      <button role="radio" aria-checked={p.layout === "tonal"} class:on={p.layout === "tonal"} onclick={() => set("layout", "tonal")}>
        <span class="thumb tonal" aria-hidden="true"><i></i><b></b><u></u></span>
        <span class="lbl">Tonal</span><span class="desc">Riff-style: covers, colour from the art, player bar.</span>
      </button>
      <button role="radio" aria-checked={p.layout === "tiled"} class:on={p.layout === "tiled"} onclick={() => set("layout", "tiled")}>
        <span class="thumb tiled" aria-hidden="true"><i></i><b></b><u></u></span>
        <span class="lbl">Tiled</span><span class="desc">Panes with gaps, like a tiling WM. Keyboard first.</span>
      </button>
    </div>
  </section>

  <section>
    <h4>Colours</h4>
    <p class="desc">Give Music its own colours, apart from the rest of {app.ui.brand.name.trim() || "Arcadia"}.</p>
    <div class="seg wrap" role="radiogroup" aria-label="Music colours">
      {#each [["app", "App's"], ["preset", "A look"], ["gradient", "Gradient"], ["cover", "Cover"]] as [id, label]}
        <button role="radio" aria-checked={p.theme.mode === id} onclick={() => setTheme("mode", id as MusicTheme["mode"])}>{label}</button>
      {/each}
    </div>
    {#if p.theme.mode === "preset"}
      <div class="looks">
        {#each Object.values(PRESETS) as pr (pr.id)}
          <button class="look" class:on={p.theme.preset === pr.id} aria-pressed={p.theme.preset === pr.id} onclick={() => setTheme("preset", pr.id)}>
            <span class="lk" style:background={pr.colors.panel} style:color={pr.colors.text}><i style:background={pr.colors.accent}></i>Aa</span>{pr.name}
          </button>
        {/each}
      </div>
    {:else if p.theme.mode === "gradient" || p.theme.mode === "cover"}
      <div class="setting">
        <div class="label">Style</div>
        <div class="seg" role="radiogroup" aria-label="Gradient style">
          {#each Object.entries(STYLE_INFO) as [id, s] (id)}
            <button role="radio" aria-checked={p.theme.gradient.style === id} onclick={() => { app.ui.music.theme.gradient.style = id as GradientStyle; saveUi(); }}>{s.name}</button>
          {/each}
        </div>
      </div>
      {#if p.theme.mode === "gradient"}
        <GradientPicker bind:top={app.ui.music.theme.gradient.top} bind:bottom={app.ui.music.theme.gradient.bottom} styles={false} onchange={saveUi} />
      {:else}
        <p class="desc">Changes with every song: a gradient made from the playing cover's main colour.</p>
      {/if}
    {/if}
  </section>

  <section>
    <h4>Look</h4>
    {#if p.layout === "tonal"}
      <div class="setting">
        <div class="label">Player bar</div>
        <div class="seg" role="radiogroup" aria-label="Player bar">
          <button role="radio" aria-checked={p.bar === "bottom"} onclick={() => set("bar", "bottom")}>Bottom</button>
          <button role="radio" aria-checked={p.bar === "top"} onclick={() => set("bar", "top")}>Top</button>
        </div>
      </div>
    {:else}
      <div class="setting">
        <div class="label">Queue pane</div>
        <Toggle label="Queue pane" checked={p.queuePane} onchange={(v) => set("queuePane", v)} />
      </div>
    {/if}
    <div class="setting">
      <div><div class="label">Colours from the cover</div><div class="desc">Tint the page with what's playing.</div></div>
      <Toggle label="Colours from the cover" checked={p.artColours} onchange={(v) => set("artColours", v)} />
    </div>
    <div class="setting">
      <div class="label">Cover size</div>
      <input type="range" min="110" max="260" step="2" value={p.cover} oninput={(e) => set("cover", Number((e.currentTarget as HTMLInputElement).value))} aria-label="Cover size" />
    </div>
    <div class="setting">
      <div class="label">Compact song rows</div>
      <Toggle label="Compact song rows" checked={p.compact} onchange={(v) => set("compact", v)} />
    </div>
  </section>

  <section>
    <h4>Tabs</h4>
    <ul class="sections">
      {#each shownTabs as id, i (id)}
        {@const hidden = p.hiddenTabs.includes(id)}
        <li class:hidden>
          <span>{tabLabel(id)}</span>
          <span class="grow"></span>
          <button class="ic" disabled={i === 0} aria-label="Move {tabLabel(id)} left" onclick={() => moveTab(id, -1)}><Icon name="up" size={14} /></button>
          <button class="ic" disabled={i === shownTabs.length - 1} aria-label="Move {tabLabel(id)} right" onclick={() => moveTab(id, 1)}><Icon name="down" size={14} /></button>
          <button class="ic" disabled={id === "home"} title={id === "home" ? "Home always shows" : undefined} aria-label={hidden ? `Show ${tabLabel(id)}` : `Hide ${tabLabel(id)}`} onclick={() => toggleTab(id)}><Icon name={hidden ? "eye-off" : "eye"} size={15} /></button>
        </li>
      {/each}
    </ul>
    <p class="desc">Hidden tabs are still a link away (e.g. from Home), and in Tiled they leave the tree too.</p>
  </section>

  <section>
    <h4>Home</h4>
    <ul class="sections">
      {#each p.sections as id, i (id)}
        {@const hidden = p.hidden.includes(id)}
        <li class:hidden>
          <span>{label(id)}</span>
          <span class="grow"></span>
          <button class="ic" disabled={i === 0} aria-label="Move {label(id)} up" onclick={() => move(id, -1)}><Icon name="up" size={14} /></button>
          <button class="ic" disabled={i === p.sections.length - 1} aria-label="Move {label(id)} down" onclick={() => move(id, 1)}><Icon name="down" size={14} /></button>
          <button class="ic" aria-label={hidden ? `Show ${label(id)}` : `Hide ${label(id)}`} onclick={() => toggleSection(id)}><Icon name={hidden ? "eye-off" : "eye"} size={15} /></button>
        </li>
      {/each}
    </ul>
  </section>

  <section>
    <h4>Library folders</h4>
    <ul class="folders">
      {#each folders as f (f)}
        <li><span class="path" title={f}>{f}</span><button class="ic" aria-label="Stop using {f}" onclick={() => saveFolders(folders.filter((x) => x !== f))}><Icon name="close" size={14} /></button></li>
      {:else}
        <li class="muted">No folders yet.</li>
      {/each}
    </ul>
    <div class="row">
      <button onclick={addFolder}><Icon name="plus" size={15} /> Add folder</button>
      <button class="ghost" onclick={rescan} disabled={scanning || !folders.length}><Icon name="refresh" size={15} /> {scanning ? "Scanning…" : "Rescan"}</button>
    </div>
    {#if ms.scan}<p class="desc scan">Reading {ms.scan.done.toLocaleString()} of {ms.scan.total.toLocaleString()}: {ms.scan.current}</p>{/if}
  </section>

  <section>
    <h4>Output</h4>
    <select value={ms.status?.device ?? ""} onchange={(e) => setDevice((e.currentTarget as HTMLSelectElement).value)} aria-label="Output device">
      <option value="">System default{ms.status?.output && !ms.status.device ? ` (${ms.status.output.device})` : ""}</option>
      {#each devices as d}<option value={d}>{d}</option>{/each}
    </select>
    {#if ms.status?.output}
      <p class="desc">{ms.status.output.device} · {+(ms.status.output.rate / 1000).toFixed(1)} kHz · {ms.status.output.channels} ch. Files at this rate play untouched; others are resampled.</p>
    {:else if ms.status?.outputError}
      <p class="desc err">{ms.status.outputError}</p>
    {/if}
  </section>
</aside>

<style>
  .drawer {
    position: absolute; top: 0.6rem; right: 0.6rem; bottom: 0.6rem; width: 340px; z-index: 20; overflow-y: auto;
    padding: 1rem 1.1rem; border-radius: var(--r-lg);
    background: var(--raised); border: 1px solid var(--line); box-shadow: 0 24px 60px rgba(0, 0, 0, 0.4);
    /* See-through styles (Glow, Faithful, Frost): frost what is behind. */
    backdrop-filter: blur(22px) saturate(1.3);
    animation: in 0.18s ease-out;
  }
  @keyframes in { from { opacity: 0; transform: translateX(12px); } }
  header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 0.6rem; }
  section { padding: 0.8rem 0; border-top: 1px solid var(--line-soft); display: grid; gap: 0.5rem; }
  h4 { font-size: 0.72rem; letter-spacing: 0.12em; text-transform: uppercase; color: var(--faint); margin: 0; }
  .x, .ic { border: 0; background: transparent; padding: 0.3rem; color: var(--muted); display: grid; }
  .x:hover, .ic:hover:not(:disabled) { color: var(--text); background: transparent; }
  .layouts { display: grid; grid-template-columns: 1fr 1fr; gap: 0.5rem; }
  .layouts button { display: flex; flex-direction: column; align-items: flex-start; gap: 0.25rem; text-align: left; padding: 0.55rem; border-radius: var(--r-md); background: var(--sunk); }
  .layouts button.on { border-color: var(--m-accent, var(--peach)); box-shadow: 0 0 0 1px var(--m-accent, var(--peach)); }
  .thumb { width: 100%; height: 56px; border-radius: 6px; position: relative; overflow: hidden; background: var(--ink); }
  .thumb i, .thumb b, .thumb u { position: absolute; display: block; }
  .tonal i { left: 6px; top: 6px; width: 18px; bottom: 6px; border-radius: 4px; background: var(--line); }
  .tonal b { left: 30px; right: 6px; top: 6px; height: 24px; border-radius: 4px; background: linear-gradient(90deg, color-mix(in srgb, var(--m-accent, var(--peach)) 60%, transparent), transparent); }
  .tonal u { left: 30px; right: 6px; bottom: 6px; height: 10px; border-radius: 6px; background: color-mix(in srgb, var(--m-accent, var(--peach)) 40%, var(--raised)); }
  .tiled i { left: 4px; top: 4px; width: 22px; bottom: 4px; border: 1px solid var(--line); }
  .tiled b { left: 30px; right: 30px; top: 4px; bottom: 16px; border: 1.5px solid var(--m-accent, var(--peach)); }
  .tiled u { right: 4px; top: 4px; width: 22px; bottom: 4px; border: 1px solid var(--line); }
  .lbl { font-weight: 700; font-size: 0.88rem; }
  .desc { font-size: 0.78rem; color: var(--muted); font-weight: 500; }
  .setting { display: flex; align-items: center; justify-content: space-between; gap: 1rem; padding: 0.2rem 0; }
  .label { font-weight: 600; font-size: 0.88rem; }
  .sections, .folders { list-style: none; margin: 0; padding: 0; display: grid; gap: 2px; }
  .sections li, .folders li { display: flex; align-items: center; gap: 0.2rem; font-size: 0.86rem; padding: 0.2rem 0.2rem 0.2rem 0.5rem; border-radius: var(--r-sm); background: var(--sunk); }
  .sections li.hidden span:first-child { opacity: 0.45; }
  .path { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 0.8rem; direction: rtl; text-align: left; }
  .row { display: flex; gap: 0.4rem; flex-wrap: wrap; }
  .row button { display: inline-flex; align-items: center; gap: 0.35rem; font-size: 0.84rem; padding: 0.4rem 0.75rem; }
  select { width: 100%; }
  .scan { font-variant-numeric: tabular-nums; }
  .err { color: var(--rose); }
  .seg.wrap { flex-wrap: wrap; }
  .looks { display: grid; grid-template-columns: 1fr 1fr; gap: 0.4rem; margin-top: 0.6rem; }
  .look { display: flex; align-items: center; gap: 0.5rem; padding: 0.3rem; border-radius: var(--r-md); background: transparent; font-size: 0.84rem; }
  .look.on { border-color: var(--peach); box-shadow: 0 0 0 1px var(--peach); }
  .lk { width: 38px; height: 28px; border-radius: 6px; display: grid; place-items: center; font-weight: 700; font-size: 0.8rem; position: relative; border: 1px solid rgba(127, 127, 127, 0.3); }
  .lk i { position: absolute; right: 3px; bottom: 3px; width: 7px; height: 7px; border-radius: 50%; }
  .drawer :global(.gp) { margin-top: 0.7rem; }
</style>
