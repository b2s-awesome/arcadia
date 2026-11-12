<script lang="ts">
  import { onMount } from "svelte";
  import { ffs, loadConfig, loadOverview, type PerfTab } from "../lib/ff.svelte";
  import { VENDOR_LABEL } from "../lib/fftypes";
  import Dashboard from "./Dashboard.svelte";
  import LiveMonitor from "./LiveMonitor.svelte";
  import Optimizer from "./Optimizer.svelte";
  import NvidiaProfiles from "./NvidiaProfiles.svelte";
  import GpuControl from "./GpuControl.svelte";

  const tabs: { id: PerfTab; label: string }[] = [
    { id: "dashboard", label: "Dashboard" },
    { id: "optimizer", label: "Optimizer" },
    { id: "monitor", label: "Live Monitor" },
    { id: "gpu", label: "GPU Control" },
    { id: "nvidia", label: "NVIDIA Profiles" },
  ];

  onMount(() => {
    if (!ffs.overview) loadOverview();
    if (!ffs.config) loadConfig();
  });

  const gpu = $derived(ffs.overview?.inventory.gpus.find((g) => g.kind === "discrete") ?? ffs.overview?.inventory.gpus[0] ?? null);
</script>

<div class="perf">
  <header class="head">
    <div class="title">
      <h2>Performance</h2>
      {#if gpu}
        <span class="pill" title="Primary graphics adapter">{VENDOR_LABEL[gpu.vendor]} · {gpu.name}</span>
      {/if}
    </div>
  </header>

  <div class="tabs" role="tablist" tabindex="-1" aria-label="Performance">
    {#each tabs as t}
      <button role="tab" aria-selected={ffs.tab === t.id} class:on={ffs.tab === t.id} onclick={() => (ffs.tab = t.id)}>{t.label}</button>
    {/each}
  </div>

  <div class="body">
    {#if ffs.tab === "monitor"}<LiveMonitor />
    {:else if ffs.tab === "optimizer"}<Optimizer />
    {:else if ffs.tab === "gpu"}<GpuControl />
    {:else if ffs.tab === "nvidia"}<NvidiaProfiles />
    {:else}<Dashboard />{/if}
  </div>
</div>

<style>
  .perf { height: 100%; display: flex; flex-direction: column; min-height: 0; }
  .head { display: flex; align-items: center; justify-content: space-between; gap: 1rem; padding: 1.6rem 2.2rem 0.6rem; }
  .title { display: flex; align-items: center; gap: 1rem; min-width: 0; }
  .title .pill { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tabs { display: flex; gap: 0.2rem; padding: 0 2.2rem; border-bottom: 1px solid var(--line-soft); overflow-x: auto; }
  .tabs button {
    border: 0; background: transparent; border-radius: 0; padding: 0.7rem 0.8rem; white-space: nowrap;
    color: var(--muted); font-weight: 600; font-size: 0.9rem; box-shadow: inset 0 -2px 0 transparent;
  }
  .tabs button:hover { color: var(--text); background: transparent; }
  .tabs button.on { color: var(--text); box-shadow: inset 0 -2px 0 var(--peach); }
  .body { flex: 1; min-height: 0; overflow: hidden; }

  /* Shared by every Performance page. */
  .body :global(.ff-page) { height: 100%; overflow-y: auto; padding: 1.4rem 2.2rem 4rem; }
  .body :global(.ff-grid) { display: grid; gap: 1rem; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); align-items: start; }
  .body :global(.ff-card) {
    border: 1px solid var(--line-soft); border-radius: var(--r-lg); background: var(--group);
    padding: 1rem 1.15rem 1.1rem; display: grid; gap: 0.85rem; min-width: 0;
  }
  .body :global(.ff-card > header) { display: flex; align-items: baseline; justify-content: space-between; gap: 0.75rem; min-width: 0; }
  .body :global(.ff-card h3) { font-size: 0.72rem; font-family: var(--body); font-weight: 700; letter-spacing: 0.14em; text-transform: uppercase; color: var(--muted); }
  .body :global(.ff-card .lead) { font-family: var(--display); font-weight: 700; font-size: 1.05rem; line-height: 1.25; overflow-wrap: anywhere; }
  .body :global(.ff-stats) { display: grid; grid-template-columns: repeat(auto-fit, minmax(96px, 1fr)); gap: 0.9rem 1rem; }
  .body :global(.ff-kv) { display: grid; grid-template-columns: auto 1fr; gap: 0.3rem 1rem; font-size: 0.88rem; margin: 0; }
  .body :global(.ff-kv dt) { color: var(--muted); }
  .body :global(.ff-kv dd) { margin: 0; text-align: right; font-variant-numeric: tabular-nums; overflow-wrap: anywhere; }
  .body :global(.ff-note) { font-size: 0.82rem; color: var(--muted); }
  .body :global(.ff-banner) {
    display: flex; gap: 0.75rem; align-items: flex-start; padding: 0.8rem 1rem; border-radius: var(--r-md);
    border: 1px solid var(--line-soft); background: var(--sunk); font-size: 0.88rem; color: var(--muted);
  }
  .body :global(.ff-banner.warn) { border-color: color-mix(in srgb, var(--rose) 40%, transparent); }
  .body :global(.ff-banner strong) { color: var(--text); }
</style>
