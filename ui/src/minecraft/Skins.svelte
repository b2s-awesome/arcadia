<script lang="ts">
  // Skins and capes: a library of skins, a 3D preview, and wearing them on
  // your Minecraft profile.
  import { onMount } from "svelte";
  import { mc, pickFile } from "../lib/api";
  import { toast } from "../lib/state.svelte";
  import { activeAccount, mcs } from "../lib/mc.svelte";
  import type { McProfile, McSkin } from "../lib/types";
  import Icon from "../components/Icon.svelte";
  import SkinViewer from "./SkinViewer.svelte";

  let skins = $state<McSkin[]>([]);
  let profile = $state<McProfile | null>(null);
  let profileError = $state<string | null>(null);
  let selected = $state<string | null>(null);
  let outer = $state(true);
  let walk = $state(false);
  let busy = $state<string | null>(null);
  let player = $state("");

  async function load() {
    skins = await mc.skins();
  }
  onMount(load);
  // Ask Mojang for the profile once accounts have loaded.
  let asked = false;
  $effect(() => {
    if (!mcs.loaded || asked) return;
    asked = true;
    if (!activeAccount()) {
      profileError = "Sign in under Accounts and setup to change the skin you wear in game.";
      return;
    }
    mc.profile().then((p) => (profile = p)).catch((e) => (profileError = String(e)));
  });

  const pick = $derived(skins.find((s) => s.id === selected) ?? null);
  const shown = $derived(pick ? { src: pick.data, variant: pick.variant } : profile?.skin ? { src: profile.skin, variant: profile.variant } : skins[0] ? { src: skins[0].data, variant: skins[0].variant } : null);
  const cape = $derived(profile?.capes.find((c) => c.active)?.data ?? null);

  async function addFile() {
    const f = await pickFile([{ name: "Skin pictures", extensions: ["png"] }]);
    if (!f) return;
    try {
      const s = await mc.skinAddFile(f);
      await load();
      selected = s.id;
    } catch (e) {
      toast(String(e), "error");
    }
  }
  async function addPlayer() {
    if (!player.trim()) return;
    busy = "player";
    try {
      const s = await mc.skinAddPlayer(player.trim());
      player = "";
      await load();
      selected = s.id;
      toast(`Saved ${s.name}'s skin`);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = null;
    }
  }
  async function wear(s: McSkin) {
    busy = s.id;
    try {
      profile = await mc.skinWear(s.id);
      toast(`You're wearing ${s.name} now. Other players see it the next time you join a server.`);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = null;
    }
  }
  async function setCape(id: string | null) {
    busy = "cape";
    try {
      profile = await mc.capeSet(id);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = null;
    }
  }
  async function edit(s: McSkin, name: string, variant: "classic" | "slim") {
    s.name = name;
    s.variant = variant;
    await mc.skinEdit(s.id, name, variant);
  }
  async function remove(s: McSkin) {
    await mc.skinRemove(s.id);
    if (selected === s.id) selected = null;
    await load();
  }
  const wearing = (s: McSkin) => !!profile?.skin && profile.skin === s.data;
</script>

<div class="skins">
  <section class="preview">
    <div class="stagebox">
      {#if shown}
        <SkinViewer src={shown.src} variant={shown.variant} cape={pick ? null : cape} scale={9} {outer} {walk} />
      {:else}
        <p class="muted">Add a skin to see it here.</p>
      {/if}
    </div>
    <div class="row opts">
      <label class="chk"><input type="checkbox" bind:checked={outer} /> Outer layer</label>
      <label class="chk"><input type="checkbox" bind:checked={walk} /> Walk</label>
      <span class="faint">Drag to turn</span>
    </div>
    <div class="now">
      {#if pick}
        <strong>{pick.name}</strong>
        <span class="muted">{pick.variant === "slim" ? "Slim arms" : "Classic arms"}</span>
        <button class="play" disabled={busy !== null || !profile} onclick={() => wear(pick)}>{busy === pick.id ? "Changing…" : wearing(pick) ? "Wearing" : "Wear this skin"}</button>
      {:else if profile}
        <strong>{profile.name}</strong>
        <span class="muted">Your skin right now</span>
      {/if}
      {#if profileError}<p class="note">{profileError}</p>{/if}
    </div>

    {#if profile && profile.capes.length}
      <div class="capes">
        <div class="label">Cape</div>
        <div class="capelist" role="radiogroup" aria-label="Cape">
          <button class="capebtn" role="radio" aria-checked={!profile.capes.some((c) => c.active)} disabled={busy !== null} onclick={() => setCape(null)}>
            <span class="capeimg none"><Icon name="close" size={14} /></span><span>None</span>
          </button>
          {#each profile.capes as c (c.id)}
            <button class="capebtn" role="radio" aria-checked={c.active} disabled={busy !== null} onclick={() => setCape(c.id)}>
              <span class="capeimg" style:background-image={c.data ? `url('${c.data}')` : undefined}></span><span>{c.alias}</span>
            </button>
          {/each}
        </div>
      </div>
    {/if}
  </section>

  <section class="library">
    <div class="row spread head">
      <h3>Your skins</h3>
      <button onclick={addFile}><Icon name="upload" size={15} /> Add a skin file</button>
    </div>
    <div class="row copy">
      <input class="grow" placeholder="Copy a player's skin by name" bind:value={player} onkeydown={(e) => e.key === "Enter" && addPlayer()} aria-label="Player name" />
      <button disabled={!player.trim() || busy === "player"} onclick={addPlayer}>{busy === "player" ? "Fetching…" : "Copy skin"}</button>
    </div>
    {#if skins.length === 0}
      <p class="hint">No saved skins yet. Add a 64×64 PNG, or copy one from any player.</p>
    {:else}
      <div class="grid">
        {#each skins as s (s.id)}
          <div class="card" class:on={selected === s.id} class:wearing={wearing(s)}>
            <button class="pickme" aria-label="Preview {s.name}" onclick={() => (selected = selected === s.id ? null : s.id)}>
              <SkinViewer src={s.data} variant={s.variant} scale={3.4} spin={false} />
            </button>
            <input class="name" value={s.name} onchange={(e) => edit(s, e.currentTarget.value, s.variant)} aria-label="Skin name" />
            <div class="row meta">
              <div class="seg small" role="radiogroup" aria-label="Arms">
                <button role="radio" aria-checked={s.variant === "classic"} onclick={() => edit(s, s.name, "classic")}>Classic</button>
                <button role="radio" aria-checked={s.variant === "slim"} onclick={() => edit(s, s.name, "slim")}>Slim</button>
              </div>
              <button class="ghost danger icon" aria-label="Delete {s.name}" onclick={() => remove(s)}><Icon name="trash" size={15} /></button>
            </div>
            {#if wearing(s)}<span class="badge">Wearing</span>{/if}
          </div>
        {/each}
      </div>
    {/if}
  </section>
</div>

<style>
  .skins { display: grid; grid-template-columns: minmax(280px, 340px) minmax(0, 1fr); gap: 1.6rem; padding: 1.4rem 2.2rem 3rem; height: 100%; overflow: auto; align-items: start; }
  @media (max-width: 900px) { .skins { grid-template-columns: 1fr; } }
  .preview { position: sticky; top: 0; display: flex; flex-direction: column; gap: 0.8rem; }
  .stagebox {
    height: 380px; display: grid; place-items: center; border-radius: var(--r-lg); overflow: hidden;
    background: radial-gradient(120% 80% at 50% 20%, color-mix(in srgb, var(--lilac) 18%, var(--raised)), var(--sunk));
    border: 1px solid var(--line-soft);
  }
  .opts { gap: 1rem; font-size: 0.85rem; }
  .chk { display: inline-flex; gap: 0.35rem; align-items: center; cursor: pointer; }
  .now { display: flex; flex-direction: column; align-items: flex-start; gap: 0.25rem; }
  .now .play { margin-top: 0.5rem; }
  .note { font-size: 0.85rem; color: var(--muted); padding: 0.6rem 0.8rem; border-radius: var(--r-md); background: var(--sunk); border: 1px solid var(--line-soft); margin-top: 0.4rem; }
  .label { font-weight: 600; margin-bottom: 0.4rem; }
  .capelist { display: flex; flex-wrap: wrap; gap: 0.4rem; }
  .capebtn { display: flex; flex-direction: column; align-items: center; gap: 0.3rem; padding: 0.45rem 0.5rem; font-size: 0.75rem; font-weight: 500; background: var(--sunk); min-width: 64px; }
  .capebtn[aria-checked="true"] { border-color: color-mix(in srgb, var(--peach) 70%, transparent); background: color-mix(in srgb, var(--peach) 9%, var(--raised)); }
  .capeimg {
    width: 30px; height: 48px; border-radius: 3px; image-rendering: pixelated;
    background-color: color-mix(in srgb, var(--lilac) 30%, var(--raised));
    background-size: 192px 96px; background-position: -3px -3px;
  }
  .capeimg.none { display: grid; place-items: center; color: var(--faint); background: var(--raised); }

  .head { margin-bottom: 0.7rem; }
  .head button, .copy button { display: inline-flex; gap: 0.35rem; align-items: center; }
  .copy { margin-bottom: 1rem; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 0.8rem; }
  .card { position: relative; display: flex; flex-direction: column; gap: 0.4rem; padding: 0.6rem; border-radius: var(--r-lg); background: var(--sunk); border: 1px solid var(--line-soft); }
  .card.on { border-color: color-mix(in srgb, var(--peach) 70%, transparent); box-shadow: 0 0 0 3px color-mix(in srgb, var(--peach) 14%, transparent); }
  .pickme { height: 150px; padding: 0; display: grid; place-items: center; border: none; background: radial-gradient(90% 70% at 50% 30%, var(--raised), transparent); overflow: hidden; }
  .name { background: transparent; border-color: transparent; padding: 0.15rem 0.3rem; font-weight: 600; }
  .name:hover { border-color: var(--line-soft); }
  .meta { justify-content: space-between; }
  .seg.small button { padding: 0.2rem 0.5rem; font-size: 0.75rem; }
  .icon { padding: 0.3rem; }
  .badge { position: absolute; top: 0.5rem; left: 0.5rem; font-size: 0.7rem; font-weight: 700; padding: 0.1rem 0.45rem; border-radius: 99px; background: var(--peach); color: var(--peach-ink); }
</style>
