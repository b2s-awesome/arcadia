<script lang="ts">
  import { onMount } from "svelte";
  import { mc } from "../lib/api";
  import { app, toast } from "../lib/state.svelte";
  import { mcs, refreshInstances } from "../lib/mc.svelte";
  import type { Hit } from "../lib/types";

  let source = $state<"modrinth" | "curseforge">("modrinth");
  let q = $state("");
  let hits: Hit[] = $state([]);
  let searching = $state(false);
  let installing: string | null = $state(null);
  const needsKey = $derived(source === "curseforge" && !app.settings?.curseforge_api_key);

  async function search() {
    if (needsKey) {
      hits = [];
      return;
    }
    searching = true;
    try {
      hits = await mc.search(source, q, "modpack", null);
    } catch (e) {
      toast(String(e), "error");
      hits = [];
    } finally {
      searching = false;
    }
  }
  onMount(search);

  async function install(h: Hit) {
    installing = h.id;
    try {
      const r = await mc.installPack(h.source, h.id);
      await refreshInstances();
      mcs.selected = r.instance.id;
      mcs.tab = "instances";
      toast(`Installed ${r.instance.name}`);
      r.warnings.forEach((w) => toast(w, "error"));
    } catch (e) {
      toast(String(e), "error");
    } finally {
      installing = null;
    }
  }
</script>

<div class="page">
  <div class="page-inner">
    <div class="row tools">
      <div class="seg" role="radiogroup" aria-label="Source">
        <button role="radio" aria-checked={source === "modrinth"} onclick={() => { source = "modrinth"; search(); }}>Modrinth</button>
        <button role="radio" aria-checked={source === "curseforge"} onclick={() => { source = "curseforge"; search(); }}>CurseForge</button>
      </div>
      <input class="grow" placeholder="Search modpacks" bind:value={q} onkeydown={(e) => e.key === "Enter" && search()} aria-label="Search modpacks" />
      <button onclick={search} disabled={searching || needsKey}>Search</button>
    </div>

    {#if needsKey}
      <div class="notice">
        <p>CurseForge only lets apps search its site with an API key. Get a free key from CurseForge's developer console, then paste it under Accounts and setup.</p>
        <button onclick={() => (mcs.tab = "accounts")}>Add a CurseForge key</button>
      </div>
    {:else if searching}
      <p class="none">Searching…</p>
    {:else if hits.length === 0}
      <p class="none">No modpacks found. Try a different search.</p>
    {:else}
      <ul class="packs">
        {#each hits as h (h.id)}
          <li>
            {#if h.iconUrl}<img src={h.iconUrl} alt="" width="64" height="64" loading="lazy" />{:else}<span class="noicon"></span>{/if}
            <div class="txt">
              <span class="t">{h.title}</span>
              <span class="faint by">by {h.author}, {Intl.NumberFormat(undefined, { notation: "compact" }).format(h.downloads)} downloads</span>
              <span class="muted d">{h.description}</span>
            </div>
            <button class:play={installing === null} onclick={() => install(h)} disabled={installing !== null}>
              {installing === h.id ? "Installing…" : "Install"}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</div>

<style>
  .tools { margin-bottom: 1rem; }
  .grow { flex: 1; }
  .none { color: var(--muted); padding: 1.5rem 0; }
  .notice { display: grid; gap: 0.8rem; justify-items: start; padding: 1rem 1.1rem; border-radius: var(--r-md); border: 1px solid var(--line); background: var(--group); max-width: 640px; }
  .packs { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(380px, 1fr)); gap: 0.7rem; }
  .packs li { display: flex; align-items: center; gap: 0.9rem; padding: 0.8rem; border: 1px solid var(--line-soft); border-radius: var(--r-lg); background: var(--group); }
  img, .noicon { width: 64px; height: 64px; border-radius: var(--r-md); flex-shrink: 0; background: var(--sunk); object-fit: cover; }
  .txt { flex: 1; min-width: 0; display: grid; gap: 0.1rem; }
  .t { font-family: var(--display); font-weight: 700; font-size: 1.05rem; }
  .by { font-size: 0.8rem; }
  .d { font-size: 0.85rem; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  button.play { font-size: 0.95rem; padding: 0.45rem 1.1rem; }
</style>
