<script lang="ts">
  import { mc } from "../lib/api";
  import { app, toast } from "../lib/state.svelte";
  import { mcs, refreshInstances } from "../lib/mc.svelte";
  import { LOADER_LABEL, type LoaderVersion, type McLoader, type McVersion } from "../lib/types";
  import Modal from "../components/Modal.svelte";
  import Toggle from "../components/Toggle.svelte";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  let versions: McVersion[] = $state([]);
  let snapshots = $state(false);
  let version = $state("");
  let loader = $state<McLoader>("fabric");
  let loaderVersions: LoaderVersion[] = $state([]);
  let loaderVersion = $state("");
  let name = $state("");
  let nameTouched = $state(false);
  let memory = $state(4096);
  let creating = $state(false);
  let loadingLoaders = $state(false);

  $effect(() => {
    if (!open) return;
    memory = app.settings?.minecraft_default_memory_mb || 4096;
    mc.versions(snapshots)
      .then((v) => {
        versions = v;
        if (!version || !v.some((x) => x.id === version)) version = v[0]?.id ?? "";
      })
      .catch((e) => toast(`Couldn't load Minecraft versions: ${e}`, "error"));
  });

  // Loader builds for the chosen Minecraft version.
  $effect(() => {
    const l = loader;
    const v = version;
    loaderVersions = [];
    loaderVersion = "";
    if (!open || !v || l === "vanilla") return;
    loadingLoaders = true;
    mc.loaderVersions(l, v)
      .then((r) => {
        if (l !== loader || v !== version) return;
        loaderVersions = r;
        loaderVersion = (r.find((x) => x.stable) ?? r[0])?.version ?? "";
      })
      .catch(() => (loaderVersions = []))
      .finally(() => (loadingLoaders = false));
  });

  $effect(() => {
    if (!nameTouched) name = version ? `${version}${loader === "vanilla" ? "" : ` ${LOADER_LABEL[loader]}`}` : "";
  });

  const noLoaderBuild = $derived(loader !== "vanilla" && !loadingLoaders && loaderVersions.length === 0);

  async function create() {
    creating = true;
    try {
      const i = await mc.create(name.trim() || version, version, loader, loader === "vanilla" ? null : loaderVersion || null, memory);
      await refreshInstances();
      mcs.selected = i.id;
      mcs.tab = "instances";
      open = false;
      nameTouched = false;
      toast(`Created ${i.name}. Files download the first time you press Play.`);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      creating = false;
    }
  }

  const loaders: McLoader[] = ["vanilla", "fabric", "forge", "neoforge"];
</script>

<Modal bind:open title="New instance">
  <label class="field">
    Minecraft version
    <div class="row">
      <select class="grow" bind:value={version} aria-label="Minecraft version">
        {#each versions as v}<option value={v.id}>{v.id}{v.type === "snapshot" ? " (snapshot)" : ""}</option>{/each}
      </select>
      <span class="row small"><Toggle label="Show snapshots" bind:checked={snapshots} /> Snapshots</span>
    </div>
  </label>

  <div class="field">
    Mod loader
    <div class="seg full" role="radiogroup" aria-label="Mod loader">
      {#each loaders as l}
        <button role="radio" aria-checked={loader === l} onclick={() => (loader = l)}>{LOADER_LABEL[l]}</button>
      {/each}
    </div>
  </div>

  {#if loader !== "vanilla"}
    <label class="field">
      {LOADER_LABEL[loader]} version
      {#if loadingLoaders}
        <span class="faint small">Looking up {LOADER_LABEL[loader]} builds…</span>
      {:else if noLoaderBuild}
        <span class="warn small">{LOADER_LABEL[loader]} has no build for Minecraft {version}. Pick another version or loader.</span>
      {:else}
        <select bind:value={loaderVersion} aria-label="Loader version">
          {#each loaderVersions as lv}<option value={lv.version}>{lv.version}{lv.stable ? "" : " (beta)"}</option>{/each}
        </select>
      {/if}
    </label>
  {/if}

  <label class="field">
    Name
    <input bind:value={name} oninput={() => (nameTouched = true)} placeholder="My world" />
  </label>

  <label class="field">
    Memory <span class="faint small">{(memory / 1024).toFixed(1)} GB</span>
    <input type="range" min="1024" max="16384" step="512" bind:value={memory} />
  </label>

  {#snippet actions()}
    <button class="ghost" onclick={() => (open = false)}>Cancel</button>
    <button disabled={creating || !version || noLoaderBuild || loadingLoaders} onclick={create}>{creating ? "Creating…" : "Create instance"}</button>
  {/snippet}
</Modal>

<style>
  .field { display: grid; gap: 0.35rem; color: var(--text); font-weight: 600; font-size: 0.9rem; }
  .small { font-size: 0.8rem; font-weight: 500; }
  .warn { color: var(--rose); }
  .seg.full { display: flex; }
  .seg.full button { flex: 1; }
  .grow { flex: 1; min-width: 0; }
</style>
