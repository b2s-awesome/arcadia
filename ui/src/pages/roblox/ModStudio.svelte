<script lang="ts">
  // Mod studio: cursor, shift-lock crosshair, death sound and font, picked
  // from a few choices with a live preview. Saved as presets you can swap.
  import Icon from "../../components/Icon.svelte";
  import Modal from "../../components/Modal.svelte";
  import { api, pickFile } from "../../lib/api";
  import { app, saveSettings, toast } from "../../lib/state.svelte";
  import type { ModStudio } from "../../lib/types";

  const s = app.settings!;
  const m = $derived(s.roblox_mod_studio);

  const STYLES = [
    { id: "", label: "Roblox's own" },
    { id: "dot", label: "Dot" },
    { id: "ring", label: "Ring" },
    { id: "ring-dot", label: "Ring & dot" },
    { id: "cross", label: "Crosshair" },
    { id: "arrow", label: "Clean arrow" },
    { id: "custom", label: "Your picture" },
  ];
  const SWATCHES = ["#ffffff", "#b8bbff", "#7fdca0", "#ffd66b", "#ff7b86", "#5ad1ff", "#111114"];

  let previews = $state<Record<string, string>>({});
  $effect(() => {
    const colour = m.cursor_colour;
    const scale = m.cursor_scale;
    for (const st of STYLES) {
      if (!st.id || st.id === "custom") continue;
      api.robloxCursorPreview(st.id, colour, scale).then((u) => (previews[st.id] = u));
    }
  });

  function set<K extends keyof ModStudio>(k: K, v: ModStudio[K]) {
    s.roblox_mod_studio[k] = v;
    saveSettings();
  }
  async function pick(k: "cursor_arrow" | "cursor_far" | "shiftlock_file") {
    const f = await pickFile([{ name: "PNG pictures", extensions: ["png"] }]);
    if (f) set(k, f);
  }
  async function pickSound() {
    const f = await pickFile([{ name: "Sounds", extensions: ["ogg", "mp3", "wav"] }]);
    if (f) set("death_sound", f);
  }
  async function chooseFont() {
    const f = await pickFile([{ name: "Fonts", extensions: ["ttf", "otf"] }]);
    if (f) {
      s.roblox_custom_font = f;
      saveSettings();
    }
  }
  const base = (p: string | null) => (p ? p.split(/[\\/]/).pop() : null);

  // The preview scene's cursor: a built-in style, your picture or Roblox's.
  const cursorImg = $derived(m.cursor && m.cursor !== "custom" ? previews[m.cursor] : null);
  const lockImg = $derived(m.shiftlock && m.shiftlock !== "custom" ? previews[m.shiftlock] : null);
  let locked = $state(false);
  // The pointer follows the mouse over the preview and rests in the middle.
  let at = $state<{ x: number; y: number } | null>(null);
  function track(e: PointerEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    at = { x: ((e.clientX - r.left) / r.width) * 100, y: ((e.clientY - r.top) / r.height) * 100 };
  }

  // ── presets ──
  const presetNames = $derived(Object.keys(s.roblox_mod_presets));
  const same = (a: ModStudio, b: ModStudio) => JSON.stringify(a) === JSON.stringify(b);
  const activePreset = $derived(presetNames.find((n) => same(s.roblox_mod_presets[n], m)) ?? null);
  let naming = $state(false);
  let newName = $state("");
  function savePreset() {
    const n = newName.trim();
    if (!n) return;
    s.roblox_mod_presets[n] = structuredClone($state.snapshot(m));
    saveSettings();
    naming = false;
    newName = "";
    toast(`Saved “${n}”`);
  }
  function usePreset(n: string) {
    s.roblox_mod_studio = structuredClone($state.snapshot(s.roblox_mod_presets[n]));
    saveSettings();
  }
  function dropPreset(n: string) {
    delete s.roblox_mod_presets[n];
    saveSettings();
  }
  function reset() {
    s.roblox_mod_studio = { cursor: "", cursor_colour: "#ffffff", cursor_scale: 100, cursor_arrow: null, cursor_far: null, shiftlock: "", shiftlock_file: null, death_sound: null };
    saveSettings();
  }
</script>

<section class="section">
  <div class="row spread">
    <h3>Mod studio</h3>
    <div class="row">
      <button class="ghost" onclick={reset}>Reset all</button>
      <button onclick={() => (naming = true)}><Icon name="save" size={15} /> Save as preset</button>
    </div>
  </div>
  <p class="hint">Applied each time Roblox starts and undone when you change them. Nothing here touches your account.</p>

  {#if presetNames.length}
    <div class="presets" role="radiogroup" aria-label="Mod presets">
      {#each presetNames as n (n)}
        <span class="preset" class:on={activePreset === n}>
          <button role="radio" aria-checked={activePreset === n} onclick={() => usePreset(n)}>{n}</button>
          <button class="x" aria-label="Delete {n}" onclick={() => dropPreset(n)}><Icon name="close" size={12} /></button>
        </span>
      {/each}
    </div>
  {/if}

  <div class="studio">
    <!-- Live preview -->
    <div
      class="scene"
      role="button"
      tabindex="0"
      aria-label="Preview. Click to toggle shift-lock."
      onclick={() => (locked = !locked)}
      onkeydown={(e) => (e.key === " " || e.key === "Enter") && (locked = !locked)}
      onpointermove={track}
      onpointerleave={() => (at = null)}
    >
      <div class="bg" aria-hidden="true"><span class="blob a"></span><span class="blob b"></span><span class="blob c"></span></div>
      <div class="pointer" class:free={at !== null && !locked} style:left="{locked || !at ? 50 : at.x}%" style:top="{locked || !at ? 50 : at.y}%">
        {#if locked}
          {#if lockImg}<img src={lockImg} alt="" />
          {:else if m.shiftlock === "custom" && m.shiftlock_file}<span class="tag">Your picture</span>
          {:else}<span class="default-lock"></span>{/if}
        {:else if cursorImg}<img src={cursorImg} alt="" />
        {:else if m.cursor === "custom" && m.cursor_arrow}<span class="tag">Your picture</span>
        {:else}<Icon name="cursor" size={26} filled />{/if}
      </div>
      <span class="mode">{locked ? "Shift-lock" : "Pointer"} · move over it, click to switch</span>
    </div>

    <div class="opts">
      <div class="group">
        <div class="label">Cursor</div>
        <div class="styles" role="radiogroup" aria-label="Cursor style">
          {#each STYLES as st (st.id)}
            <button class="style" role="radio" aria-checked={m.cursor === st.id} onclick={() => { set("cursor", st.id); locked = false; }} title={st.label}>
              <span class="sw">
                {#if st.id === ""}<Icon name="cursor" size={20} filled />
                {:else if st.id === "custom"}<Icon name="image" size={20} />
                {:else if previews[st.id]}<img class:arrow={st.id === "arrow"} src={previews[st.id]} alt="" />{/if}
              </span>
              <span class="nm">{st.label}</span>
            </button>
          {/each}
        </div>
        {#if m.cursor === "custom"}
          <div class="files">
            <button onclick={() => pick("cursor_arrow")}>{base(m.cursor_arrow) ?? "Choose pointer…"}</button>
            <button class="ghost" onclick={() => pick("cursor_far")}>{base(m.cursor_far) ?? "Far pointer (optional)"}</button>
          </div>
          <p class="hint">64×64 PNGs work best. Roblox centres the picture on the pointer.</p>
        {/if}
      </div>

      <div class="group">
        <div class="label">Shift-lock crosshair</div>
        <div class="styles small" role="radiogroup" aria-label="Shift-lock style">
          {#each STYLES.filter((x) => x.id !== "arrow") as st (st.id)}
            <button class="style" role="radio" aria-checked={m.shiftlock === st.id} onclick={() => { set("shiftlock", st.id); locked = true; }} title={st.label}>
              <span class="sw">
                {#if st.id === ""}<span class="default-lock mini"></span>
                {:else if st.id === "custom"}<Icon name="image" size={18} />
                {:else if previews[st.id]}<img src={previews[st.id]} alt="" />{/if}
              </span>
            </button>
          {/each}
        </div>
        {#if m.shiftlock === "custom"}
          <div class="files"><button onclick={() => pick("shiftlock_file")}>{base(m.shiftlock_file) ?? "Choose picture…"}</button></div>
        {/if}
      </div>

      {#if (m.cursor && m.cursor !== "custom") || (m.shiftlock && m.shiftlock !== "custom")}
        <div class="group two">
          <div>
            <div class="label">Colour</div>
            <div class="swatches">
              {#each SWATCHES as c (c)}
                <button class="swatch" style:background={c} aria-label={c} aria-pressed={m.cursor_colour === c} onclick={() => set("cursor_colour", c)}></button>
              {/each}
              <label class="swatch pick" title="Any colour" style:background={m.cursor_colour}>
                <input type="color" value={m.cursor_colour} onchange={(e) => set("cursor_colour", e.currentTarget.value)} aria-label="Any colour" />
              </label>
            </div>
          </div>
          <div>
            <div class="label">Size <span class="faint">{m.cursor_scale}%</span></div>
            <input type="range" min="50" max="200" step="10" value={m.cursor_scale} oninput={(e) => (s.roblox_mod_studio.cursor_scale = +e.currentTarget.value)} onchange={saveSettings} aria-label="Cursor size" />
          </div>
        </div>
      {/if}

      <div class="group row-set">
        <div>
          <div class="label">Death sound</div>
          <div class="desc mono">{base(m.death_sound) ?? "Roblox's own"}</div>
        </div>
        <div class="row">
          {#if m.death_sound}<button class="ghost" onclick={() => set("death_sound", null)}>Reset</button>{/if}
          <button onclick={pickSound}>Choose sound</button>
        </div>
      </div>
      <div class="group row-set">
        <div>
          <div class="label">Font</div>
          <div class="desc mono">{base(s.roblox_custom_font) ?? "Roblox's own"}</div>
        </div>
        <div class="row">
          {#if s.roblox_custom_font}<button class="ghost" onclick={() => { s.roblox_custom_font = null; saveSettings(); }}>Reset</button>{/if}
          <button onclick={chooseFont}>Choose font</button>
        </div>
      </div>
    </div>
  </div>
</section>

<Modal bind:open={naming} title="Save mod preset">
  <p>Keeps the cursor, crosshair, colour, size and sound so you can switch back in one click.</p>
  <input placeholder="Preset name" bind:value={newName} onkeydown={(e) => e.key === "Enter" && savePreset()} aria-label="Preset name" />
  {#snippet actions()}
    <button class="ghost" onclick={() => (naming = false)}>Cancel</button>
    <button disabled={!newName.trim()} onclick={savePreset}>Save preset</button>
  {/snippet}
</Modal>

<style>
  .row button { display: inline-flex; align-items: center; gap: 0.35rem; }
  .presets { display: flex; flex-wrap: wrap; gap: 0.4rem; margin: 0.2rem 0 1rem; }
  .preset { display: inline-flex; align-items: center; border: 1px solid var(--line); border-radius: 999px; background: var(--sunk); }
  .preset.on { border-color: color-mix(in srgb, var(--peach) 60%, transparent); background: color-mix(in srgb, var(--peach) 10%, var(--raised)); }
  .preset button { border: none; background: transparent; padding: 0.3rem 0.4rem 0.3rem 0.8rem; font-weight: 500; font-size: 0.88rem; }
  .preset .x { padding: 0.3rem 0.6rem 0.3rem 0.2rem; color: var(--faint); line-height: 0; }
  .preset .x:hover { color: var(--rose); }

  .studio { display: grid; grid-template-columns: minmax(240px, 0.9fr) minmax(0, 1.4fr); gap: 1.4rem; align-items: start; }
  @media (max-width: 980px) { .studio { grid-template-columns: 1fr; } }

  .scene {
    position: sticky; top: 0; aspect-ratio: 4 / 3; border-radius: var(--r-lg); overflow: hidden; cursor: pointer;
    border: 1px solid var(--line); isolation: isolate;
  }
  /* A slowly drifting background that passes through light, dark and
     colourful patches, so you can see the cursor against all of them. */
  .scene { cursor: none; }
  .bg {
    position: absolute; inset: -20%;
    background: linear-gradient(120deg, #7cc4ff, #f4f7fb, #6fcf8f, #1b1d2a, #ffcf6b, #8f6bff, #0e1016, #7cc4ff);
    background-size: 400% 400%;
    animation: drift 28s ease-in-out infinite alternate;
  }
  .blob { position: absolute; border-radius: 50%; filter: blur(28px); opacity: 0.75; mix-blend-mode: soft-light; }
  .blob.a { width: 55%; height: 55%; left: 5%; top: 10%; background: #fff; animation: float 17s ease-in-out infinite alternate; }
  .blob.b { width: 45%; height: 45%; right: 5%; bottom: 10%; background: #000; animation: float 23s ease-in-out infinite alternate-reverse; }
  .blob.c { width: 35%; height: 35%; left: 40%; top: 40%; background: #ff7bd5; animation: float 19s ease-in-out infinite alternate; }
  @keyframes drift { 0% { background-position: 0% 50%; } 100% { background-position: 100% 50%; } }
  @keyframes float { 0% { transform: translate(0, 0) scale(1); } 100% { transform: translate(30%, -20%) scale(1.25); } }
  /* Holding still is handled app-wide (data-still). */
  .pointer { position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%); display: grid; place-items: center; color: #fff; filter: drop-shadow(0 1px 1px rgb(0 0 0 / 0.5)); transition: left 0.35s ease, top 0.35s ease; pointer-events: none; }
  .pointer.free { transition: none; }
  .pointer img { width: 64px; height: 64px; image-rendering: auto; }
  .pointer :global(svg) { transform: translate(9px, 9px); }
  .default-lock { width: 22px; height: 22px; border-radius: 50%; border: 3px solid rgb(255 255 255 / 0.9); box-shadow: 0 0 0 1px rgb(0 0 0 / 0.4); }
  .default-lock.mini { width: 16px; height: 16px; border-width: 2px; }
  .tag { font-size: 0.75rem; padding: 0.2rem 0.5rem; border-radius: 99px; background: rgb(0 0 0 / 0.55); }
  .mode { position: absolute; left: 0.6rem; bottom: 0.5rem; font-size: 0.72rem; color: rgb(255 255 255 / 0.9); background: rgb(0 0 0 / 0.35); padding: 0.15rem 0.5rem; border-radius: 99px; }

  .opts { display: flex; flex-direction: column; gap: 1.1rem; min-width: 0; }
  .label { font-weight: 600; margin-bottom: 0.45rem; }
  .desc { color: var(--muted); font-size: 0.85rem; }
  .styles { display: grid; grid-template-columns: repeat(auto-fill, minmax(84px, 1fr)); gap: 0.45rem; }
  .styles.small { grid-template-columns: repeat(auto-fill, 56px); }
  .style { display: flex; flex-direction: column; align-items: center; gap: 0.3rem; padding: 0.45rem 0.3rem; border-radius: var(--r-md); background: var(--sunk); border-color: var(--line-soft); font-weight: 500; }
  .style[aria-checked="true"] { border-color: color-mix(in srgb, var(--peach) 70%, transparent); background: color-mix(in srgb, var(--peach) 9%, var(--raised)); }
  .sw {
    width: 40px; height: 40px; display: grid; place-items: center; border-radius: 8px; color: #fff;
    background: linear-gradient(135deg, #6a8fbf, #3e5f86);
  }
  .sw { overflow: hidden; position: relative; }
  /* Zoom in on the middle of the 64px picture, where the shape is. */
  .sw img { position: absolute; left: 50%; top: 50%; width: 88px; height: 88px; max-width: none; transform: translate(-50%, -50%); }
  .sw img.arrow { transform: translate(-36%, -36%); }
  .sw :global(svg) { filter: drop-shadow(0 1px 1px rgb(0 0 0 / 0.5)); }
  .nm { font-size: 0.75rem; color: var(--muted); white-space: nowrap; }
  .files { display: flex; gap: 0.4rem; flex-wrap: wrap; margin-top: 0.55rem; }
  .group.two { display: grid; grid-template-columns: 1fr 1fr; gap: 1.2rem; }
  .swatches { display: flex; gap: 0.35rem; flex-wrap: wrap; }
  .swatch { width: 26px; height: 26px; padding: 0; border-radius: 50%; border: 2px solid var(--line); }
  .swatch[aria-pressed="true"] { box-shadow: 0 0 0 2px var(--raised), 0 0 0 4px var(--lilac); }
  .swatch.pick { position: relative; overflow: hidden; cursor: pointer; background-image: conic-gradient(red, yellow, lime, cyan, blue, magenta, red) !important; }
  .swatch.pick input { position: absolute; inset: 0; opacity: 0; cursor: pointer; }
  input[type="range"] { width: 100%; }
  .row-set { display: flex; align-items: center; justify-content: space-between; gap: 1rem; padding-top: 0.9rem; border-top: 1px solid var(--line-soft); }
  .row-set .label { margin-bottom: 0.1rem; }
</style>
