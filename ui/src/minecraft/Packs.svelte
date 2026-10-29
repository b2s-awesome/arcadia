<script lang="ts">
  // Resource packs or shader packs in an instance: turn on/off, remove, add.
  import { mc, pickFiles } from "../lib/api";
  import { bytes, toast } from "../lib/state.svelte";
  import type { McInstance, McPack } from "../lib/types";
  import Toggle from "../components/Toggle.svelte";
  import Icon from "../components/Icon.svelte";

  let { inst, kind, onadd }: { inst: McInstance; kind: "resourcepacks" | "shaderpacks"; onadd: () => void } = $props();

  let packs = $state<McPack[]>([]);
  let loading = $state(true);
  async function load() {
    loading = true;
    packs = await mc.packs(inst.id, kind).catch(() => []);
    loading = false;
  }
  $effect(() => {
    void kind;
    void inst.id;
    load();
  });

  const label = $derived(kind === "resourcepacks" ? "Resource packs" : "Shader packs");
  async function toggle(p: McPack, on: boolean) {
    try {
      p.fileName = await mc.packToggle(inst.id, kind, p.fileName, on);
      p.enabled = on;
    } catch (e) {
      toast(String(e), "error");
    }
  }
  async function remove(p: McPack) {
    await mc.packRemove(inst.id, kind, p.fileName);
    packs = packs.filter((x) => x !== p);
  }
  async function add() {
    const files = await pickFiles([{ name: "Pack files", extensions: ["zip"] }]);
    if (!files.length) return;
    const n = await mc.packsAdd(inst.id, kind, files).catch((e) => (toast(String(e), "error"), 0));
    if (n) toast(`Added ${n} pack${n === 1 ? "" : "s"}`);
    load();
  }
</script>

<div class="row spread bar">
  <h3>{label} <span class="n">{packs.length}</span></h3>
  <div class="row">
    <button class="ghost" onclick={() => mc.openFolder(inst.id, kind)}><Icon name="folder" size={15} /> Folder</button>
    <button class="ghost" onclick={add}><Icon name="upload" size={15} /> Add .zip</button>
    <button onclick={onadd}><Icon name="search" size={15} /> Find {kind === "resourcepacks" ? "packs" : "shaders"}</button>
  </div>
</div>
{#if kind === "shaderpacks" && inst.loader !== "vanilla"}
  <p class="hint">Shaders need Iris (Fabric) or Oculus (Forge) in Mods. Pick the active one in game under Video Settings → Shader Packs.</p>
{:else if kind === "resourcepacks"}
  <p class="hint">Switched-off packs are hidden from the game. Choose the order of the rest in game under Options → Resource Packs.</p>
{/if}

{#if loading}
  <p class="none">Loading…</p>
{:else if packs.length === 0}
  <p class="none">No {label.toLowerCase()} yet.</p>
{:else}
  <ul class="packs">
    {#each packs as p (p.fileName)}
      <li class:off={!p.enabled}>
        <Toggle label="Use {p.name}" checked={p.enabled} onchange={(on) => toggle(p, on)} />
        {#if p.icon}<img src={p.icon} alt="" width="40" height="40" />{:else}<span class="noicon"><Icon name={kind === "shaderpacks" ? "sun" : "image"} size={18} /></span>{/if}
        <div class="txt">
          <span class="nm">{p.name}{#if p.folder}<span class="pill tiny">folder</span>{/if}</span>
          {#if p.description}<span class="muted desc">{p.description}</span>{/if}
        </div>
        <span class="faint size">{bytes(p.size)}</span>
        <button class="ghost danger icon" onclick={() => remove(p)} aria-label="Remove {p.name}"><Icon name="trash" size={16} /></button>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .bar { margin-bottom: 0.3rem; }
  .bar button { display: inline-flex; align-items: center; gap: 0.35rem; }
  .n { color: var(--faint); font-weight: 500; }
  .hint { font-size: 0.82rem; color: var(--faint); margin-bottom: 0.8rem; max-width: 70ch; }
  .none { color: var(--muted); padding: 1.2rem 0; }
  .packs { list-style: none; margin: 0; padding: 0; }
  .packs li { display: flex; align-items: center; gap: 0.8rem; padding: 0.55rem 0; border-bottom: 1px solid var(--line-soft); }
  .packs li.off .txt, .packs li.off img { opacity: 0.5; }
  img, .noicon { width: 40px; height: 40px; border-radius: var(--r-sm); flex: none; image-rendering: pixelated; object-fit: cover; }
  .noicon { display: grid; place-items: center; background: var(--sunk); color: var(--faint); }
  .txt { flex: 1; min-width: 0; display: grid; }
  .nm { font-weight: 600; display: flex; gap: 0.4rem; align-items: center; }
  .desc { font-size: 0.82rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tiny { font-size: 0.68rem; }
  .size { font-size: 0.8rem; width: 5rem; text-align: right; white-space: nowrap; }
  .icon { padding: 0.35rem; }
</style>
