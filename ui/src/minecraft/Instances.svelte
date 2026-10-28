<script lang="ts">
  import { mcs, ago, play } from "../lib/mc.svelte";
  import { LOADER_LABEL } from "../lib/types";
  import Icon from "../components/Icon.svelte";
  import InstanceDetail from "./InstanceDetail.svelte";

  let { onnew }: { onnew: () => void } = $props();
  const BADGE = { vanilla: "V", fabric: "Fa", forge: "Fo", neoforge: "Ne" } as const;
  let query = $state("");
  const list = $derived(mcs.instances.filter((i) => i.name.toLowerCase().includes(query.trim().toLowerCase())));
  const selected = $derived(mcs.instances.find((i) => i.id === mcs.selected) ?? null);
</script>

{#if mcs.loaded && mcs.instances.length === 0}
  <div class="empty">
    <h3>No instances yet</h3>
    <p class="muted">An instance is one Minecraft setup with its own version, mods and worlds. Make a new one, install a modpack, or bring one over from another launcher.</p>
    <div class="row">
      <button class="play" onclick={onnew}>New instance</button>
      <button onclick={() => (mcs.tab = "modpacks")}>Browse modpacks</button>
      <button onclick={() => (mcs.tab = "import")}>Import</button>
    </div>
  </div>
{:else}
  <div class="split">
    <aside class="list">
      <label class="search">
        <Icon name="search" size={15} />
        <input placeholder="Find an instance" bind:value={query} aria-label="Find an instance" />
      </label>
      <ul>
        {#each list as i (i.id)}
          <li>
            <button class="item" class:sel={i.id === mcs.selected} onclick={() => (mcs.selected = i.id)} ondblclick={() => play(i.id)} title="Double-click to play">
              <span class="badge l-{i.loader}" title={LOADER_LABEL[i.loader]}>{BADGE[i.loader]}</span>
              <span class="txt">
                <span class="name">{i.name}</span>
                <span class="meta">{i.mcVersion} {LOADER_LABEL[i.loader]}, {ago(i.lastPlayed)}</span>
              </span>
              {#if mcs.running.includes(i.id)}<span class="live" title="Running"></span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    </aside>
    <section class="detail">
      {#if selected}
        {#key selected.id}<InstanceDetail inst={selected} />{/key}
      {/if}
    </section>
  </div>
{/if}

<style>
  .empty { padding: 3rem 2.2rem; max-width: 640px; display: grid; gap: 0.8rem; }
  .split { display: grid; grid-template-columns: 270px 1fr; height: 100%; min-height: 0; }
  .list { border-right: 1px solid var(--line-soft); display: flex; flex-direction: column; min-height: 0; }
  .search { display: flex; align-items: center; gap: 0.4rem; color: var(--faint); padding: 0.9rem 0.9rem 0.5rem; }
  .search input { flex: 1; min-width: 0; }
  ul { list-style: none; margin: 0; padding: 0.2rem 0.6rem 1rem; overflow-y: auto; flex: 1; }
  .item {
    width: 100%; display: flex; align-items: center; gap: 0.7rem; text-align: left;
    border: 1px solid transparent; background: transparent; padding: 0.55rem 0.6rem; border-radius: var(--r-md);
  }
  .item:hover { background: var(--raised); border-color: transparent; }
  .item.sel { background: var(--raised); border-color: var(--line-soft); }
  .badge {
    width: 32px; height: 32px; flex-shrink: 0; border-radius: var(--r-sm);
    display: grid; place-items: center; font-family: var(--display); font-weight: 800; font-size: 0.85rem; letter-spacing: -0.02em;
    color: #1a1712; background: var(--l);
  }
  .l-vanilla { --l: #8fbf5a; }
  .l-fabric { --l: #dbd0b4; }
  .l-forge { --l: #e0a46a; }
  .l-neoforge { --l: #f0a36b; }
  .txt { display: grid; min-width: 0; flex: 1; }
  .name { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta { color: var(--faint); font-size: 0.78rem; font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .live { width: 8px; height: 8px; border-radius: 50%; background: var(--mint); box-shadow: 0 0 0 3px color-mix(in srgb, var(--mint) 25%, transparent); }
  .detail { min-height: 0; overflow-y: auto; }
</style>
