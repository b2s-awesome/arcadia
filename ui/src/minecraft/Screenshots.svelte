<script lang="ts">
  // Screenshots taken in an instance (F2 in game).
  import { fileUrl, mc } from "../lib/api";
  import { bytes } from "../lib/state.svelte";
  import type { McInstance, McShot } from "../lib/types";
  import Icon from "../components/Icon.svelte";

  let { inst }: { inst: McInstance } = $props();
  let shots = $state<McShot[]>([]);
  $effect(() => {
    void inst.id;
    mc.screenshots(inst.id).then((s) => (shots = s)).catch(() => (shots = []));
  });

  let view = $state<number | null>(null);
  const cur = $derived(view === null ? null : shots[view]);
  const date = (s: number) => new Date(s * 1000).toLocaleString(undefined, { day: "numeric", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit" });
  function key(e: KeyboardEvent) {
    const v = view;
    if (v === null) return;
    if (e.key === "Escape") view = null;
    if (e.key === "ArrowRight") view = Math.min(shots.length - 1, v + 1);
    if (e.key === "ArrowLeft") view = Math.max(0, v - 1);
  }
  async function del(i: number) {
    await mc.screenshotDelete(inst.id, shots[i].file);
    shots = shots.filter((_, j) => j !== i);
    if (view !== null) view = shots.length ? Math.min(view, shots.length - 1) : null;
  }
</script>

<svelte:window onkeydown={key} />

<div class="row spread bar">
  <h3>Screenshots <span class="n">{shots.length}</span></h3>
  <button class="ghost" onclick={() => mc.openFolder(inst.id, "screenshots")}><Icon name="folder" size={15} /> Folder</button>
</div>
{#if shots.length === 0}
  <p class="none">No screenshots yet. Press F2 in game to take one.</p>
{:else}
  <div class="grid">
    {#each shots as s, i (s.file)}
      <button class="shot" onclick={() => (view = i)} aria-label="Open screenshot from {date(s.taken)}">
        <img src={fileUrl(s.path)} alt="" loading="lazy" />
        <span class="when">{date(s.taken)}</span>
      </button>
    {/each}
  </div>
{/if}

{#if cur && view !== null}
  <div class="lightbox" role="dialog" aria-modal="true" aria-label="Screenshot">
    <button class="backdrop" aria-label="Close" onclick={() => (view = null)}></button>
    <img src={fileUrl(cur.path)} alt="" />
    <div class="cap">
      <span>{date(cur.taken)} · {bytes(cur.size)}</span>
      <span class="grow"></span>
      <button class="ghost" disabled={view === 0} onclick={() => (view = view! - 1)} aria-label="Previous"><Icon name="skip-prev" size={16} /></button>
      <button class="ghost" disabled={view === shots.length - 1} onclick={() => (view = view! + 1)} aria-label="Next"><Icon name="skip-next" size={16} /></button>
      <button class="ghost danger" onclick={() => del(view!)}><Icon name="trash" size={15} /> Delete</button>
      <button class="ghost" onclick={() => (view = null)} aria-label="Close"><Icon name="close" size={16} /></button>
    </div>
  </div>
{/if}

<style>
  .bar { margin-bottom: 0.7rem; }
  .bar button, .cap button { display: inline-flex; align-items: center; gap: 0.35rem; }
  .n { color: var(--faint); font-weight: 500; }
  .none { color: var(--muted); padding: 1.2rem 0; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 0.7rem; }
  .shot { position: relative; padding: 0; aspect-ratio: 16 / 9; overflow: hidden; border-radius: var(--r-md); border: 1px solid var(--line-soft); background: var(--sunk); }
  .shot img { width: 100%; height: 100%; object-fit: cover; transition: transform 0.25s ease; }
  .shot:hover img { transform: scale(1.04); }
  .when { position: absolute; left: 0.4rem; bottom: 0.35rem; font-size: 0.7rem; padding: 0.1rem 0.45rem; border-radius: 99px; background: rgb(0 0 0 / 0.55); color: #fff; font-weight: 500; }
  .lightbox { position: fixed; inset: 0; z-index: 100; display: grid; place-items: center; padding: 3rem 3rem 5rem; }
  .backdrop { position: absolute; inset: 0; border: none; border-radius: 0; background: rgb(6 6 10 / 0.86); cursor: zoom-out; }
  .lightbox > img { position: relative; max-width: 100%; max-height: 100%; border-radius: var(--r-md); box-shadow: 0 20px 60px rgb(0 0 0 / 0.5); }
  .cap { position: absolute; left: 50%; bottom: 1.2rem; transform: translateX(-50%); width: min(720px, 92vw); display: flex; align-items: center; gap: 0.3rem; padding: 0.4rem 0.6rem 0.4rem 1rem; border-radius: 999px; background: var(--raised); border: 1px solid var(--line); font-size: 0.85rem; color: var(--muted); }
  .grow { flex: 1; }
</style>
