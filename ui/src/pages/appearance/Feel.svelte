<script lang="ts">
  // How Arcadia moves, sounds and reads: motion, interface sounds, text
  // size, font and icon style.
  import { app, saveUi } from "../../lib/state.svelte";
  import { DEFAULT_FEEL, type FeelPrefs } from "../../lib/theme";
  import { play, type Cue } from "../../lib/sounds";
  import Icon from "../../components/Icon.svelte";

  let { view }: { view: "motion" | "type" } = $props();
  const f = $derived(app.ui.feel);
  function set<K extends keyof FeelPrefs>(k: K, v: FeelPrefs[K]) {
    app.ui.feel[k] = v;
    saveUi();
  }

  const MOTION = [
    { id: "full", label: "Full", desc: "Everything animates." },
    { id: "calm", label: "Calm", desc: "Slower, and looping animations hold still." },
    { id: "off", label: "Off", desc: "Nothing moves." },
  ] as const;
  const SOUNDS = [
    { id: "off", label: "Off" },
    { id: "soft", label: "Soft" },
    { id: "clicky", label: "Clicky" },
    { id: "retro", label: "Retro" },
  ] as const;
  const demo = (c: Cue) => play(c, { ...app.ui.feel, sounds: app.ui.feel.sounds === "off" ? "soft" : app.ui.feel.sounds });

  const FONTS = ["Segoe UI Variable", "Segoe UI", "Bahnschrift", "Cascadia Code", "Consolas", "Georgia", "Trebuchet MS", "Verdana", "Comic Sans MS", "Inter", "Roboto", "Nunito"];
  let fontInput = $state("");
  $effect(() => {
    fontInput = app.ui.feel.fontFamily;
  });
  // Is a font installed? Text measures differently when it is (the usual
  // trick: compare against two generic fallbacks).
  let canvas: CanvasRenderingContext2D | null = null;
  function available(name: string): boolean {
    if (typeof document === "undefined") return false;
    canvas ??= document.createElement("canvas").getContext("2d");
    if (!canvas) return true;
    const text = "mmmmmmmmmlli1WQ@#";
    return ["monospace", "serif"].some((g) => {
      canvas!.font = `32px ${g}`;
      const base = canvas!.measureText(text).width;
      canvas!.font = `32px "${name}", ${g}`;
      return canvas!.measureText(text).width !== base;
    });
  }
  const iconSample = ["library", "music", "cube", "roblox", "star", "settings", "play", "heart"];
</script>

{#if view === "motion"}
  <section class="section">
    <h3>Motion</h3>
    <div class="cards" role="radiogroup" aria-label="Motion">
      {#each MOTION as m (m.id)}
        <button class="card" role="radio" aria-checked={f.motion === m.id} onclick={() => set("motion", m.id)}>
          <span class="demo {m.id}"><i></i></span>
          <strong>{m.label}</strong>
          <span class="muted small">{m.desc}</span>
        </button>
      {/each}
    </div>
    {#if f.motion !== "off"}
      <div class="setting">
        <div><div class="label">Animation speed</div><div class="desc">How quickly things slide, fade and pop.</div></div>
        <div class="slider">
          <span class="faint small">Slower</span>
          <input type="range" min="0.5" max="2" step="0.1" value={f.speed} oninput={(e) => (app.ui.feel.speed = +e.currentTarget.value)} onchange={saveUi} aria-label="Animation speed" />
          <span class="faint small">Faster</span>
          <span class="mono val">{f.speed.toFixed(1)}×</span>
        </div>
      </div>
    {/if}
  </section>

  <section class="section">
    <h3>Sounds</h3>
    <p class="hint">Little sounds for clicks, switches, launches and messages. Made on the fly, so there's nothing to download.</p>
    <div class="setting">
      <div class="label">Sound set</div>
      <div class="seg" role="radiogroup" aria-label="Sound set">
        {#each SOUNDS as s (s.id)}
          <button role="radio" aria-checked={f.sounds === s.id} onclick={() => { set("sounds", s.id); if (s.id !== "off") play("ok", { ...app.ui.feel, sounds: s.id }); }}>{s.label}</button>
        {/each}
      </div>
    </div>
    {#if f.sounds !== "off"}
      <div class="setting">
        <div class="label">Volume</div>
        <div class="slider">
          <input type="range" min="0" max="1" step="0.05" value={f.volume} oninput={(e) => (app.ui.feel.volume = +e.currentTarget.value)} onchange={() => { saveUi(); demo("click"); }} aria-label="Sound volume" />
          <span class="mono val">{Math.round(f.volume * 100)}%</span>
        </div>
      </div>
      <div class="setting">
        <div class="label">Try them</div>
        <div class="row">
          {#each [["click", "Click"], ["on", "Switch on"], ["ok", "Done"], ["error", "Problem"], ["launch", "Launch"]] as [c, l] (c)}
            <button class="ghost small" onclick={() => demo(c as Cue)}>{l}</button>
          {/each}
        </div>
      </div>
    {/if}
  </section>
{:else}
  <section class="section">
    <h3>Text</h3>
    <div class="setting">
      <div><div class="label">Text size</div><div class="desc">Makes everything bigger or smaller, not just the words.</div></div>
      <div class="slider">
        <span class="small">A</span>
        <input type="range" min="85" max="130" step="5" value={f.scale} oninput={(e) => (app.ui.feel.scale = +e.currentTarget.value)} onchange={saveUi} aria-label="Text size" />
        <span class="big">A</span>
        <span class="mono val">{f.scale}%</span>
      </div>
    </div>
    <div class="setting col">
      <div><div class="label">Font</div><div class="desc">Any font installed on this PC. Leave empty for the look's own.</div></div>
      <div class="fonts">
        <button class="font" class:on={!f.fontFamily} onclick={() => set("fontFamily", "")}>Look's own</button>
        {#each FONTS.filter(available) as name (name)}
          <button class="font" class:on={f.fontFamily === name} style:font-family={`"${name}"`} onclick={() => set("fontFamily", name)}>{name}</button>
        {/each}
      </div>
      <div class="row">
        <input class="grow" placeholder="Or type a font name" bind:value={fontInput} onkeydown={(e) => e.key === "Enter" && set("fontFamily", fontInput.trim())} aria-label="Font name" />
        <button disabled={!fontInput.trim() || fontInput.trim() === f.fontFamily} onclick={() => set("fontFamily", fontInput.trim())}>Use font</button>
      </div>
      {#if f.fontFamily && !available(f.fontFamily)}<p class="warn small">“{f.fontFamily}” doesn't seem to be installed, so another font is showing.</p>{/if}
      <p class="sample" style:font-family={f.fontFamily ? `"${f.fontFamily}"` : undefined}>The quick brown fox jumps over the lazy dog · 0123456789</p>
    </div>
  </section>

  <section class="section">
    <h3>Icons</h3>
    <div class="icons">
      {#each iconSample as n}<Icon name={n} size={26} />{/each}
    </div>
    <div class="setting">
      <div class="label">Line weight</div>
      <div class="slider">
        <span class="faint small">Thin</span>
        <input type="range" min="1.2" max="2.6" step="0.1" value={f.iconStroke} oninput={(e) => (app.ui.feel.iconStroke = +e.currentTarget.value)} onchange={saveUi} aria-label="Icon line weight" />
        <span class="faint small">Bold</span>
      </div>
    </div>
    <div class="setting">
      <div class="label">Line ends</div>
      <div class="seg" role="radiogroup" aria-label="Line ends">
        <button role="radio" aria-checked={f.iconCaps === "round"} onclick={() => set("iconCaps", "round")}>Round</button>
        <button role="radio" aria-checked={f.iconCaps === "square"} onclick={() => set("iconCaps", "square")}>Square</button>
      </div>
    </div>
    {#if f.iconStroke !== DEFAULT_FEEL.iconStroke || f.iconCaps !== DEFAULT_FEEL.iconCaps || f.scale !== DEFAULT_FEEL.scale || f.fontFamily}
      <div class="setting">
        <div class="desc">Back to the defaults for text and icons.</div>
        <button class="ghost" onclick={() => { Object.assign(app.ui.feel, { iconStroke: DEFAULT_FEEL.iconStroke, iconCaps: DEFAULT_FEEL.iconCaps, scale: DEFAULT_FEEL.scale, fontFamily: "" }); saveUi(); }}>Reset</button>
      </div>
    {/if}
  </section>
{/if}

<style>
  .cards { display: grid; grid-template-columns: repeat(3, 1fr); gap: 0.6rem; margin-bottom: 0.6rem; }
  .card { display: flex; flex-direction: column; align-items: flex-start; gap: 0.25rem; padding: 0.8rem; text-align: left; font-weight: 500; background: var(--sunk); border-color: var(--line-soft); }
  .card[aria-checked="true"] { border-color: color-mix(in srgb, var(--peach) 70%, transparent); background: color-mix(in srgb, var(--peach) 8%, var(--raised)); }
  .demo { width: 100%; height: 36px; border-radius: var(--r-sm); background: var(--raised); position: relative; overflow: hidden; margin-bottom: 0.3rem; }
  .demo i { position: absolute; top: 9px; left: 8px; width: 18px; height: 18px; border-radius: 50%; background: var(--peach); }
  .demo.full i { animation: glide 1.6s ease-in-out infinite alternate; }
  .demo.calm i { animation: glide 3.2s ease-in-out infinite alternate; opacity: 0.8; }
  @keyframes glide { to { left: calc(100% - 26px); } }
  .small { font-size: 0.78rem; }
  .big { font-size: 1.2rem; font-weight: 700; }
  .slider { display: flex; align-items: center; gap: 0.6rem; }
  .slider input { width: 170px; }
  .val { width: 3.4rem; text-align: right; color: var(--muted); }
  .setting.col { flex-direction: column; align-items: stretch; gap: 0.6rem; }
  .fonts { display: flex; flex-wrap: wrap; gap: 0.35rem; }
  .font { padding: 0.35rem 0.75rem; border-radius: 999px; font-weight: 500; background: var(--sunk); }
  .font.on { border-color: color-mix(in srgb, var(--peach) 70%, transparent); background: color-mix(in srgb, var(--peach) 10%, var(--raised)); }
  .sample { font-size: 1.15rem; padding: 0.7rem 0.9rem; border-radius: var(--r-md); background: var(--sunk); }
  .warn { color: var(--peach); }
  .icons { display: flex; gap: 1.2rem; padding: 0.9rem 1rem; border-radius: var(--r-md); background: var(--sunk); margin-bottom: 0.6rem; color: var(--text); }
  .row button { display: inline-flex; align-items: center; gap: 0.35rem; }
</style>
