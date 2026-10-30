<script lang="ts">
  // Pick a two-colour gradient, Discord-style, and pull one in from
  // anywhere: a screenshot, two pasted hex codes, a saved Profile Studio
  // profile, or a Discord user's accent colour.
  import { onMount, type Snippet } from "svelte";
  import { addons } from "../lib/api";
  import { app, toast } from "../lib/state.svelte";
  import { STYLE_INFO, fromAccent, isHex, parsePair, pullFromImage, readability, type GradientStyle } from "../lib/gradient";
  import Icon from "./Icon.svelte";

  let {
    top = $bindable(),
    bottom = $bindable(),
    style = $bindable("tonal"),
    styles = true,
    onchange,
    extra,
  }: {
    top: string;
    bottom: string;
    style?: GradientStyle;
    /** Offer the Tonal / Faithful / Glow switch. */
    styles?: boolean;
    onchange?: () => void;
    /** More ways to pull colours (Profile Studio adds banner and avatar). */
    extra?: Snippet;
  } = $props();

  const PRESETS: { name: string; top: string; bottom: string }[] = [
    { name: "Blush", top: "#f7cfcf", bottom: "#f19ba7" },
    { name: "Blurple", top: "#5865f2", bottom: "#eb459e" },
    { name: "Midnight", top: "#2b2d6e", bottom: "#7a3fa0" },
    { name: "Matcha", top: "#e8f3d6", bottom: "#7fb685" },
    { name: "Sunset", top: "#ff9a62", bottom: "#6b2fa3" },
    { name: "Deep sea", top: "#0f3443", bottom: "#34e89e" },
    { name: "Mono", top: "#2b2b30", bottom: "#121214" },
  ];

  function set(a: string | null, b: string | null) {
    if (a && isHex(a)) top = a.toLowerCase();
    if (b && isHex(b)) bottom = b.toLowerCase();
    onchange?.();
  }
  function hexIn(which: "top" | "bottom", e: Event) {
    const el = e.currentTarget as HTMLInputElement;
    const v = el.value.trim().startsWith("#") ? el.value.trim() : `#${el.value.trim()}`;
    if (isHex(v)) set(which === "top" ? v : null, which === "bottom" ? v : null);
    else el.value = which === "top" ? top : bottom;
  }

  // ── paste two colours ──
  let pasted = $state("");
  function usePasted() {
    const p = parsePair(pasted);
    if (!p) {
      toast("Couldn't find two colours there. Try “#f7cfcf #f19ba7”.", "error");
      return;
    }
    set(p[0], p[1]);
    pasted = "";
  }

  // ── screenshot ──
  let unblend = $state(true);
  let shot: string | null = $state(null);
  let seen: [string, string] | null = $state(null);
  let over = $state(false);
  let fileEl: HTMLInputElement | undefined = $state();
  async function readShot(f: File | null | undefined) {
    if (!f || !f.type.startsWith("image/")) return;
    if (shot) URL.revokeObjectURL(shot);
    shot = URL.createObjectURL(f);
    await pullShot();
  }
  async function pullShot() {
    if (!shot) return;
    try {
      const r = await pullFromImage(shot, unblend);
      seen = r.seen;
      set(r.top, r.bottom);
    } catch (e) {
      toast(String((e as Error).message ?? e), "error");
    }
  }
  function pasteShot(e: ClipboardEvent) {
    const f = [...(e.clipboardData?.files ?? [])].find((x) => x.type.startsWith("image/"));
    if (f) {
      e.preventDefault();
      readShot(f);
    }
  }

  // ── saved Profile Studio profiles, and Discord accent colours ──
  const studio = $derived(app.ui.addons.installed.includes("discord-profile"));
  let shelf: { label: string; theme: { primary: string; accent: string } | null }[] = $state([]);
  onMount(async () => {
    if (!studio) return;
    const idx = await addons.get<typeof shelf>("discord-profile", "index").catch(() => null);
    shelf = (idx ?? []).filter((s) => s.theme);
  });
  let who = $state("");
  let looking = $state(false);
  async function fromUser() {
    if (!who.trim() || looking) return;
    looking = true;
    try {
      const f = await addons.discordLookup(who.trim());
      if (!f.accentColor) {
        toast(`${f.displayName ?? f.username} has no accent colour Arcadia can see. Discord only shares profile colours with the person themselves, so a screenshot works best.`, "error");
        return;
      }
      const [a, b] = fromAccent(f.accentColor);
      set(a, b);
      toast(`Made a gradient from ${f.displayName ?? f.username}'s accent colour`);
      who = "";
    } catch (e) {
      toast(String(e), "error");
    } finally {
      looking = false;
    }
  }

  const ratio = $derived(readability({ top, bottom, style }));
</script>

<svelte:window onpaste={pasteShot} />

<div class="gp">
  <div class="row top-row">
    <span class="preview" style:background="linear-gradient(180deg, {top}, {bottom})" aria-hidden="true"></span>
    <label class="pick"><input type="color" value={top} oninput={(e) => set(e.currentTarget.value, null)} aria-label="Top colour" /><span><small>Top</small><input class="hex" value={top} onchange={(e) => hexIn("top", e)} maxlength="7" aria-label="Top colour hex" spellcheck="false" /></span></label>
    <label class="pick"><input type="color" value={bottom} oninput={(e) => set(null, e.currentTarget.value)} aria-label="Bottom colour" /><span><small>Bottom</small><input class="hex" value={bottom} onchange={(e) => hexIn("bottom", e)} maxlength="7" aria-label="Bottom colour hex" spellcheck="false" /></span></label>
    <button class="ghost small" onclick={() => set(bottom, top)} title="Swap top and bottom"><Icon name="refresh" size={14} /> Swap</button>
    <span class="pill" class:warn={ratio < 4.5} title="Contrast of text on the panels">{ratio >= 7 ? "Very easy to read" : ratio >= 4.5 ? "Easy to read" : ratio >= 3 ? "Readable" : "Low contrast"}</span>
  </div>

  {#if styles}
    <div class="styles" role="radiogroup" aria-label="Gradient style">
      {#each Object.entries(STYLE_INFO) as [id, s] (id)}
        <button role="radio" aria-checked={style === id} onclick={() => { style = id as GradientStyle; onchange?.(); }}>
          <span class="sw-demo {id}" style:--a={top} style:--b={bottom}><i></i><i></i></span>
          <strong>{s.name}</strong>
          <span class="muted tiny">{s.blurb}</span>
        </button>
      {/each}
    </div>
  {/if}

  <div class="chips" aria-label="Ready-made gradients">
    {#each PRESETS as p (p.name)}
      <button class="chip" aria-pressed={p.top === top && p.bottom === bottom} onclick={() => set(p.top, p.bottom)}><i style:background="linear-gradient(180deg, {p.top}, {p.bottom})"></i>{p.name}</button>
    {/each}
    {#each shelf as s, i (i)}
      <button class="chip" title="From Profile Studio" onclick={() => set(s.theme!.primary, s.theme!.accent)}><i style:background="linear-gradient(180deg, {s.theme!.primary}, {s.theme!.accent})"></i>{s.label}</button>
    {/each}
  </div>

  <div class="pull">
    <div
      class="drop" class:over role="button" tabindex="0"
      aria-label="Drop, paste or choose a screenshot of a Discord profile"
      onclick={() => fileEl?.click()}
      onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); fileEl?.click(); } }}
      ondragover={(e) => { e.preventDefault(); over = true; }}
      ondragleave={() => (over = false)}
      ondrop={(e) => { e.preventDefault(); over = false; readShot(e.dataTransfer?.files[0]); }}
    >
      {#if shot}<img src={shot} alt="" />{:else}<Icon name="image" size={20} />{/if}
      <span>
        <strong>{seen ? `Pulled ${top} → ${bottom}` : "Pull from a screenshot"}</strong>
        <span class="muted tiny">Drop, paste (Ctrl+V) or click to choose a screenshot of a Discord profile.</span>
      </span>
    </div>
    <input type="file" accept="image/*" hidden bind:this={fileEl} onchange={(e) => { readShot(e.currentTarget.files?.[0]); e.currentTarget.value = ""; }} />
    <label class="check"><input type="checkbox" bind:checked={unblend} onchange={pullShot} /> Undo Discord's see-through layer{#if seen}<span class="muted tiny"> (seen {seen[0]} → {seen[1]})</span>{/if}</label>

    <form class="row" onsubmit={(e) => { e.preventDefault(); usePasted(); }}>
      <input class="grow" bind:value={pasted} placeholder="Paste two colours, e.g. #f7cfcf #f19ba7" aria-label="Paste two colours" spellcheck="false" />
      <button type="submit" disabled={!pasted.trim()}>Use</button>
    </form>

    {#if studio}
      <form class="row" onsubmit={(e) => { e.preventDefault(); fromUser(); }}>
        <input class="grow" bind:value={who} placeholder="Discord user ID, for their accent colour" aria-label="Discord user for an accent colour" spellcheck="false" />
        <button type="submit" disabled={!who.trim() || looking}>{looking ? "Looking…" : "Use accent"}</button>
      </form>
    {/if}
    {#if extra}{@render extra()}{/if}
  </div>
</div>

<style>
  .gp { display: grid; gap: 0.8rem; }
  .row { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; }
  .grow { flex: 1; min-width: 0; }
  .preview { width: 34px; height: 46px; border-radius: 8px; border: 1px solid var(--line); flex-shrink: 0; }
  .pick { display: flex; align-items: center; gap: 0.45rem; padding: 0.25rem 0.6rem 0.25rem 0.3rem; border: 1px solid var(--line); border-radius: 999px; }
  .pick input[type="color"] { width: 30px; height: 30px; padding: 0; border: 0; border-radius: 50%; background: none; cursor: pointer; }
  .pick input[type="color"]::-webkit-color-swatch-wrapper { padding: 0; }
  .pick input[type="color"]::-webkit-color-swatch { border: 1px solid rgba(0, 0, 0, 0.18); border-radius: 50%; }
  .pick > span { display: grid; line-height: 1.1; }
  .pick small { font-size: 0.7rem; color: var(--faint); font-weight: 600; }
  .hex { width: 5.6rem; border: 0; padding: 0; background: transparent; font-family: "JetBrains Mono Variable", Consolas, monospace; font-size: 0.82rem; }
  .styles { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 0.5rem; }
  .styles button { display: grid; gap: 0.25rem; text-align: left; padding: 0.6rem; border-radius: var(--r-md); background: transparent; align-content: start; }
  .styles button[aria-checked="true"] { border-color: var(--peach); box-shadow: 0 0 0 1px var(--peach); }
  .sw-demo { height: 46px; border-radius: 8px; position: relative; overflow: hidden; display: block; margin-bottom: 0.2rem; }
  .sw-demo i { position: absolute; left: 8px; right: 8px; height: 10px; border-radius: 4px; }
  .sw-demo i:first-child { top: 10px; }
  .sw-demo i:last-child { top: 26px; right: 30px; }
  .sw-demo.faithful { background: linear-gradient(180deg, var(--a), var(--b)); }
  .sw-demo.faithful i { background: rgba(255, 255, 255, 0.5); }
  .sw-demo.glow { background: radial-gradient(90% 80% at 10% 0%, color-mix(in srgb, var(--a) 60%, white), transparent 60%), radial-gradient(90% 90% at 100% 100%, var(--b), transparent 70%), linear-gradient(160deg, var(--a), var(--b)); }
  .sw-demo.glow i { background: rgba(255, 255, 255, 0.4); box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.8); }
  .sw-demo.tonal { background: linear-gradient(180deg, color-mix(in srgb, var(--a) 45%, var(--raised)), var(--raised) 40%); border: 1px solid var(--line); }
  .sw-demo.tonal i { background: color-mix(in srgb, var(--b) 18%, var(--sunk)); }
  .sw-demo.tonal i:last-child { background: var(--b); }
  .tiny { font-size: 0.76rem; line-height: 1.35; }
  .chips { display: flex; flex-wrap: wrap; gap: 0.4rem; }
  .chip { display: inline-flex; align-items: center; gap: 0.4rem; padding: 0.22rem 0.7rem 0.22rem 0.25rem; border-radius: 999px; font-size: 0.82rem; background: transparent; }
  .chip i { width: 18px; height: 18px; border-radius: 50%; }
  .chip[aria-pressed="true"] { border-color: var(--peach); }
  .pull { display: grid; gap: 0.55rem; }
  .drop { display: flex; align-items: center; gap: 0.8rem; padding: 0.7rem; border: 1.5px dashed var(--line); border-radius: var(--r-md); cursor: pointer; color: var(--faint); }
  .drop.over { border-color: var(--peach); background: color-mix(in srgb, var(--peach) 8%, transparent); }
  .drop img { width: 40px; height: 64px; object-fit: cover; border-radius: 6px; }
  .drop > span { display: grid; color: var(--text); }
  .check { display: flex; align-items: center; gap: 0.45rem; font-size: 0.84rem; color: var(--muted); }
  .small { font-size: 0.8rem; padding: 0.3rem 0.6rem; display: inline-flex; align-items: center; gap: 0.3rem; }
  @media (max-width: 720px) { .styles { grid-template-columns: 1fr; } }
</style>
