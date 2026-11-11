<script lang="ts">
  import { onMount } from "svelte";
  import { ff, openUrl, pickFile } from "../lib/api";
  import { toast } from "../lib/state.svelte";
  import { duration, ffs, gb, loadOverview, num, saveConfig, useMonitor } from "../lib/ff.svelte";
  import type { GpuSample, ProcessInfo, Sample } from "../lib/fftypes";
  import Icon from "../components/Icon.svelte";
  import Stat from "./parts/Stat.svelte";
  import LineChart from "./parts/LineChart.svelte";

  const TILES: { id: string; label: string }[] = [
    { id: "fps", label: "FPS" },
    { id: "frameTimeGraph", label: "Frame-time graph" },
    { id: "avgFps", label: "Average FPS" },
    { id: "low1", label: "1% low" },
    { id: "low01", label: "0.1% low" },
    { id: "frameTime", label: "Frame time" },
    { id: "consistency", label: "Consistency" },
    { id: "cpuBusy", label: "CPU busy / wait" },
    { id: "gpuBusy", label: "GPU busy / wait" },
    { id: "gpuUtil", label: "GPU use" },
    { id: "cpuUtil", label: "CPU use" },
    { id: "gpuTemp", label: "GPU temperature" },
    { id: "gpuPower", label: "GPU power" },
    { id: "vram", label: "Video memory" },
    { id: "ram", label: "Memory" },
  ];

  let processes: ProcessInfo[] = $state([]);
  let pid: number | null = $state(null);
  let busy = $state(false);
  let choosing = $state(false);

  const pm = $derived(ffs.overview?.presentmon ?? null);
  const snap = $derived(ffs.snapshot);
  const active = $derived(snap?.state === "running");
  const show = (id: string) => ffs.config?.monitorTiles.includes(id) ?? true;
  const candidates = $derived(processes.filter((p) => p.category !== "protected").sort((a, b) => Number(b.category === "game") - Number(a.category === "game") || (b.gpuPct ?? 0) - (a.gpuPct ?? 0) || b.memoryBytes - a.memoryBytes));

  async function refreshProcesses() {
    try {
      processes = await ff.processes();
      if (pid === null || !processes.some((p) => p.pid === pid)) {
        const game = await ff.runningGame();
        pid = game?.pid ?? null;
      }
    } catch (e) {
      toast(String(e), "error");
    }
  }

  onMount(() => {
    refreshProcesses();
    return useMonitor();
  });

  async function start() {
    const p = processes.find((x) => x.pid === pid);
    if (!p) return;
    busy = true;
    try {
      await ff.monitorStart(p.pid, p.gameName ?? p.name);
      ffs.snapshot = await ff.monitorSnapshot();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function stop() {
    await ff.monitorStop().catch((e) => toast(String(e), "error"));
    ffs.snapshot = await ff.monitorSnapshot();
    refreshProcesses();
  }

  async function chooseCli() {
    const path = await pickFile([{ name: "PresentMon", extensions: ["exe"] }]);
    if (!path || !ffs.config) return;
    if (await saveConfig({ ...ffs.config, presentmonCli: path })) {
      await loadOverview();
      toast("PresentMon console application saved");
    }
  }

  function toggleTile(id: string) {
    if (!ffs.config) return;
    const on = ffs.config.monitorTiles.includes(id);
    const next = on ? ffs.config.monitorTiles.filter((t) => t !== id) : TILES.map((t) => t.id).filter((t) => t === id || ffs.config!.monitorTiles.includes(t));
    saveConfig({ ...ffs.config, monitorTiles: next });
  }

  // The GPU the monitor attaches PresentMon readings to: the busiest one in the latest sample.
  const pickGpu = (s: Sample | null): GpuSample | null => (s?.gpus.length ? [...s.gpus].sort((a, b) => (b.utilizationPct ?? -1) - (a.utilizationPct ?? -1))[0] : null);
  const gpu = $derived(pickGpu(snap?.telemetry ?? null));
  const hist = $derived(snap?.telemetryHistory ?? []);
  const gpuHist = (f: (g: GpuSample) => number | null) => hist.map((h) => { const g = gpu ? h.gpus.find((x) => x.id === gpu.id) : null; return g ? f(g) : null; });
  const fpsNow = $derived(snap?.displayedFpsNow ?? snap?.presentedFpsNow ?? null);
  const fpsKind = $derived(snap?.displayedFpsNow != null ? "displayed" : snap?.presentedFpsNow != null ? "presented" : "");
  const stats = $derived(snap ? (snap.displayed.frames >= 10 ? snap.displayed : snap.presented) : null);
  const statsKind = $derived(snap && snap.displayed.frames >= 10 ? "on-screen frames" : "presented frames");
  const note = (needle: string) => stats?.notes.find((n) => n.includes(needle)) ?? "Not enough frames yet";
</script>

<div class="ff-page">
  {#if pm && !pm.willUse}
    <section class="ff-card setup">
      <header><h3>PresentMon is not set up</h3></header>
      <p>Arcadia measures frames with Intel PresentMon. It is not installed with Arcadia, and nothing here is estimated without it.</p>
      <ol>{#each pm.setupSteps as step}<li>{step}</li>{/each}</ol>
      <dl class="ff-kv">
        <dt>Service</dt><dd>{pm.serviceDetail}</dd>
        <dt>Console application</dt><dd>{pm.cliDetail}</dd>
      </dl>
      <div class="row wrap">
        <button onclick={() => openUrl(pm.downloadUrl)}><Icon name="download" size={16} /> Open the PresentMon download page</button>
        <button onclick={chooseCli}>Choose console application…</button>
        <button class="ghost" disabled={ffs.loadingOverview} onclick={() => loadOverview()}><Icon name="refresh" size={15} /> Check again</button>
      </div>
    </section>
  {:else}
    <div class="controls">
      <label class="pick">
        <span class="ff-note">Process</span>
        <select bind:value={pid} disabled={active} aria-label="Process to monitor">
          <option value={null}>Choose a running game or app</option>
          {#each candidates as p (p.pid)}
            <option value={p.pid}>{p.gameName ? `${p.gameName} (${p.name})` : p.name} · PID {p.pid}{p.gpuPct != null && p.gpuPct >= 1 ? ` · GPU ${p.gpuPct.toFixed(0)}%` : ""}</option>
          {/each}
        </select>
      </label>
      {#if active}
        <button class="danger" onclick={stop}><Icon name="stop" size={15} /> Stop</button>
      {:else}
        <button class="ghost" onclick={refreshProcesses} title="Refresh the process list"><Icon name="refresh" size={15} /></button>
        <button class="play start" disabled={pid === null || busy} onclick={start}>{busy ? "Starting…" : "Start monitoring"}</button>
      {/if}
      <div class="grow"></div>
      <button class="ghost" aria-expanded={choosing} onclick={() => (choosing = !choosing)}><Icon name="sliders" size={15} /> Readings</button>
    </div>

    {#if choosing}
      <div class="ff-card tiles" role="group" aria-label="Visible readings">
        {#each TILES as t}
          <label><input type="checkbox" checked={show(t.id)} onchange={() => toggleTile(t.id)} /> {t.label}</label>
        {/each}
      </div>
    {/if}

    {#if snap?.state === "error"}
      <div class="ff-banner warn"><Icon name="warning" size={18} /><span><strong>Monitoring stopped.</strong> {snap.error}</span></div>
    {/if}

    {#if !snap || snap.state === "idle"}
      <div class="ff-banner">
        <Icon name="info" size={18} />
        <span>Start a game, pick it above and press Start monitoring. Using {pm?.willUse === "cli" ? "the PresentMon console application" : "the PresentMon Service"}{pm?.serviceReady ? ` (${pm.serviceDetail})` : ""}.</span>
      </div>
    {:else}
      <div class="status">
        <span class="pill" class:live={active}>{#if active}<span class="dot"></span>{/if}{snap.state === "running" ? "Monitoring" : snap.state === "ended" ? "Ended" : "Stopped"}</span>
        <strong>{snap.process}</strong>
        <span class="ff-note">PID {snap.pid} · {snap.totalFrames.toLocaleString()} frames · {duration(snap.elapsedMs)} · {snap.sourceDetail}</span>
      </div>
      {#each snap.notes as n}<p class="ff-note">{n}</p>{/each}

      <div class="hero">
        {#if show("fps")}
          <div class="fps">
            <Stat big label={fpsKind ? `FPS (${fpsKind})` : "FPS"} value={fpsNow === null ? null : num(fpsNow)} reason="Needs one second of frames" />
          </div>
        {/if}
        {#if show("frameTimeGraph")}
          <div class="graph">
            <LineChart label="Frame time, last frames" unit="ms" values={snap.frameTimesMs} digits={1} height={150} reason="Waiting for frames" />
          </div>
        {/if}
      </div>

      <section class="ff-card">
        <header><h3>Frames</h3><span class="ff-note">Last 30 seconds of {statsKind}</span></header>
        <div class="ff-stats">
          {#if show("avgFps")}<Stat label="Average FPS" value={stats?.avgFps == null ? null : num(stats.avgFps, 1)} reason={note("Not enough")} />{/if}
          {#if show("low1")}<Stat label="1% low" value={stats?.low1pctFps == null ? null : num(stats.low1pctFps, 1)} unit="FPS" reason={note("1% low")} />{/if}
          {#if show("low01")}<Stat label="0.1% low" value={stats?.low01pctFps == null ? null : num(stats.low01pctFps, 1)} unit="FPS" reason={note("0.1% low")} />{/if}
          {#if show("frameTime")}<Stat label="Frame time" value={stats?.avgFrameTimeMs == null ? null : num(stats.avgFrameTimeMs, 2)} unit="ms" sub={stats?.p99Ms != null ? `99th percentile ${num(stats.p99Ms, 2)} ms` : ""} reason={note("Not enough")} />{/if}
          {#if show("consistency")}<Stat label="Consistency" value={stats?.consistencyPct == null ? null : num(stats.consistencyPct)} unit="%" sub={stats?.stutterFrames != null ? `${stats.stutterFrames} stutter frame${stats.stutterFrames === 1 ? "" : "s"}` : ""} reason={note("Not enough")} />{/if}
          {#if show("cpuBusy")}<Stat label="CPU busy" value={snap.cpuBusyMs === null ? null : num(snap.cpuBusyMs, 2)} unit="ms" sub={snap.cpuWaitMs !== null ? `wait ${num(snap.cpuWaitMs, 2)} ms` : ""} reason="Not reported by this PresentMon source" />{/if}
          {#if show("gpuBusy")}<Stat label="GPU busy" value={snap.gpuBusyMs === null ? null : num(snap.gpuBusyMs, 2)} unit="ms" sub={snap.gpuWaitMs !== null ? `wait ${num(snap.gpuWaitMs, 2)} ms` : ""} reason="Not reported by this PresentMon source" />{/if}
          <Stat label="Dropped frames" value={snap.droppedPct === null ? null : num(snap.droppedPct, 1)} unit="%" reason="Needs on-screen timing" tone={snap.droppedPct !== null && snap.droppedPct > 5 ? "warn" : ""} />
          {#if snap.generatedPct}<Stat label="Generated frames" value={num(snap.generatedPct)} unit="%" sub="frame generation is on" />{/if}
        </div>
      </section>

      <div class="ff-grid charts">
        {#if show("gpuUtil")}<LineChart label="GPU use" unit="%" max={100} values={gpuHist((g) => g.utilizationPct)} reason={gpu?.unavailable.utilization ?? "Waiting for data"} />{/if}
        {#if show("cpuUtil")}<LineChart label="CPU use" unit="%" max={100} values={hist.map((h) => h.cpuPct)} tone="info" />{/if}
        {#if show("gpuTemp")}<LineChart label="GPU temperature" unit="°C" values={gpuHist((g) => g.temperatureC)} tone="bad" reason={gpu?.unavailable.temperature ?? "Waiting for data"} />{/if}
        {#if show("gpuPower")}<LineChart label="GPU power" unit="W" values={gpuHist((g) => g.powerW)} tone="ok" reason={gpu?.unavailable.power ?? "Waiting for data"} />{/if}
        {#if show("vram")}<LineChart label="Video memory" unit="GB" digits={1} values={gpuHist((g) => (g.vramUsedBytes === null ? null : g.vramUsedBytes / 1024 ** 3))} max={gpu?.vramTotalBytes ? Number(gb(gpu.vramTotalBytes)) : null} reason={gpu?.unavailable.vramUsed ?? "Waiting for data"} />{/if}
        {#if show("ram")}<LineChart label="Memory" unit="GB" digits={1} values={hist.map((h) => h.ramUsedBytes / 1024 ** 3)} max={hist.length ? hist[0].ramTotalBytes / 1024 ** 3 : null} tone="info" />{/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .setup p, .setup li { color: var(--muted); font-size: 0.9rem; }
  .setup ol { margin: 0; padding-left: 1.2rem; display: grid; gap: 0.3rem; }
  .controls { display: flex; align-items: flex-end; gap: 0.6rem; margin-bottom: 1rem; flex-wrap: wrap; }
  .pick { display: grid; gap: 0.2rem; min-width: min(420px, 100%); }
  .pick select { width: 100%; }
  .start { font-size: 0.95rem; padding: 0.5rem 1.3rem; }
  .tiles { grid-template-columns: repeat(auto-fill, minmax(170px, 1fr)); margin-bottom: 1rem; font-size: 0.88rem; }
  .tiles label { display: flex; gap: 0.5rem; align-items: center; }
  .status { display: flex; align-items: baseline; gap: 0.75rem; flex-wrap: wrap; margin-bottom: 0.6rem; }
  .hero { display: grid; grid-template-columns: minmax(180px, auto) 1fr; gap: 1.5rem; align-items: end; margin: 0.6rem 0 1.2rem; }
  .hero > :only-child { grid-column: 1 / -1; }
  .charts { margin-top: 1rem; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); }
  @media (max-width: 760px) { .hero { grid-template-columns: 1fr; } }
</style>
