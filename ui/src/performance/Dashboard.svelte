<script lang="ts">
  import { onMount } from "svelte";
  import { DASH, ffs, gb, loadOverview, num, size, useTelemetry, yesNo } from "../lib/ff.svelte";
  import { VENDOR_LABEL, type ApiStatus, type GpuDevice, type GpuSample } from "../lib/fftypes";
  import Icon from "../components/Icon.svelte";
  import Meter from "./parts/Meter.svelte";
  import Stat from "./parts/Stat.svelte";
  import LineChart from "./parts/LineChart.svelte";

  onMount(() => useTelemetry());

  const inv = $derived(ffs.overview?.inventory ?? null);
  const s = $derived(ffs.sample);
  const live = (g: GpuDevice): GpuSample | null => s?.gpus.find((x) => x.id === g.id) ?? null;
  const why = (t: GpuSample | null, metric: string) => t?.unavailable[metric] ?? "Waiting for the first reading";
  const from = (t: GpuSample | null, metric: string) => (t?.sources[metric] ? `from ${t.sources[metric]}` : "");
  const apiText = (a: ApiStatus) =>
    a.state === "available" ? `${a.api} ready${a.version ? ` (${a.version})` : ""}` : a.state === "unavailable" ? `${a.api} unavailable` : "No vendor API";
  const kindText = (g: GpuDevice) => (g.kind === "unknown" ? "Type unknown" : g.kind[0].toUpperCase() + g.kind.slice(1)) + (g.kindSource === "heuristic" && g.kind !== "unknown" ? " (estimated)" : "");
  const plan = $derived(inv?.system.powerPlans.find((p) => p.active) ?? null);
  const pm = $derived(ffs.overview?.presentmon ?? null);
  const cpuHistory = $derived(ffs.history.map((h) => h.cpuPct));
  const ramHistory = $derived(ffs.history.map((h) => h.ramPct));
</script>

<div class="ff-page">
  {#if !inv}
    <p class="ff-note">{ffs.loadingOverview ? "Scanning hardware…" : "Hardware has not been scanned yet."}</p>
  {:else}
    <div class="bar">
      <span class="ff-note">Scanned {new Date(inv.scannedAt * 1000).toLocaleTimeString()}. Live readings refresh every 2 seconds while this page is open.</span>
      <button class="ghost" disabled={ffs.loadingOverview} onclick={() => loadOverview(true)}><Icon name="refresh" size={15} /> {ffs.loadingOverview ? "Scanning…" : "Scan again"}</button>
    </div>

    {#each inv.warnings as w}
      <div class="ff-banner warn"><Icon name="warning" size={18} /><span>{w}</span></div>
    {/each}

    <div class="ff-grid">
      <section class="ff-card">
        <header><h3>Processor</h3><span class="pill">{inv.cpu.manufacturer || "Unknown maker"}</span></header>
        <div class="lead">{inv.cpu.model || "Unknown processor"}</div>
        <div class="ff-stats">
          <Stat label="Cores" value={inv.cpu.physicalCores === null ? null : String(inv.cpu.physicalCores)} reason="Not reported by Windows" />
          <Stat label="Threads" value={String(inv.cpu.logicalCores)} />
          <Stat label="Base clock" value={inv.cpu.baseFrequencyMhz === null ? null : (inv.cpu.baseFrequencyMhz / 1000).toFixed(2)} unit="GHz" reason="Not reported" />
          <Stat label="Now" value={s?.cpuFrequencyMhz == null ? null : (s.cpuFrequencyMhz / 1000).toFixed(2)} unit="GHz" reason="Not reported" />
          <Stat label="Temperature" value={s?.cpuTemperatureC == null ? null : num(s.cpuTemperatureC)} unit="°C" reason={s?.cpuTemperatureNote ?? "Waiting for the first reading"} />
          <Stat label="Architecture" value={inv.cpu.architecture || null} />
        </div>
        <Meter label="Use, all cores" pct={s?.cpuPct ?? null} reason="Waiting for the first reading" />
        <Meter label="Busiest core" pct={s?.cpuMaxCorePct ?? null} warnAt={95} reason="Waiting for the first reading" />
        <LineChart label="CPU use" unit="%" values={cpuHistory} max={100} height={70} />
      </section>

      {#each inv.gpus as g (g.id)}
        {@const t = live(g)}
        <section class="ff-card">
          <header>
            <h3>Graphics · {VENDOR_LABEL[g.vendor]}</h3>
            <span class="pill" class:live={g.api.state === "available"} class:warn={g.api.state === "unavailable"} title={g.api.state === "unavailable" ? g.api.reason : undefined}>{apiText(g.api)}</span>
          </header>
          <div class="lead">{g.name}</div>
          <dl class="ff-kv">
            <dt>Type</dt><dd>{kindText(g)}</dd>
            <dt>Device ID</dt><dd class="mono">{g.vendorId.toString(16).toUpperCase().padStart(4, "0")}:{g.deviceId.toString(16).toUpperCase().padStart(4, "0")}</dd>
            <dt>Driver</dt><dd>{g.driverVersionVendor ?? g.driverVersion ?? DASH}{#if g.driverVersionVendor && g.driverVersion} <span class="faint">({g.driverVersion})</span>{/if}</dd>
            <dt>Video memory</dt><dd>{size(g.vramBytes)}</dd>
            {#if g.outputs.length}<dt>Displays</dt><dd>{g.outputs.length}</dd>{/if}
          </dl>
          {#if g.api.state === "unavailable"}<p class="ff-note">{g.api.reason}</p>{/if}
          <Meter label="Use" pct={t?.utilizationPct ?? null} reason={why(t, "utilization")} />
          <Meter
            label="Video memory in use"
            pct={t?.vramUsedBytes != null && t?.vramTotalBytes ? (t.vramUsedBytes / t.vramTotalBytes) * 100 : null}
            text={t?.vramUsedBytes != null ? `${gb(t.vramUsedBytes)} of ${gb(t.vramTotalBytes)} GB` : ""}
            warnAt={92}
            reason={why(t, "vramUsed")}
          />
          <div class="ff-stats">
            <Stat label="Temperature" value={t?.temperatureC == null ? null : num(t.temperatureC)} unit="°C" sub={from(t, "temperature")} reason={why(t, "temperature")} tone={t?.thermalLimited ? "warn" : ""} />
            <Stat label="Power" value={t?.powerW == null ? null : num(t.powerW)} unit="W" sub={from(t, "power")} reason={why(t, "power")} />
            <Stat label="Fan" value={t?.fanRpm != null ? num(t.fanRpm) : t?.fanPct != null ? num(t.fanPct) : null} unit={t?.fanRpm != null ? "RPM" : "%"} sub={from(t, t?.fanRpm != null ? "fanRpm" : "fanPct")} reason={why(t, "fanRpm")} />
            <Stat label="Core clock" value={t?.coreClockMhz == null ? null : num(t.coreClockMhz)} unit="MHz" sub={from(t, "coreClock")} reason={why(t, "coreClock")} />
            <Stat label="Memory clock" value={t?.memoryClockMhz == null ? null : num(t.memoryClockMhz)} unit="MHz" sub={from(t, "memoryClock")} reason={why(t, "memoryClock")} />
          </div>
          {#if t?.thermalLimited}<div class="ff-banner warn"><Icon name="warning" size={18} /><span>The driver reports this GPU is being slowed down to control its temperature.</span></div>{/if}
          {#if g.features.length}<div class="chips">{#each g.features as f}<span class="pill">{f}</span>{/each}</div>{/if}
        </section>
      {/each}

      <section class="ff-card">
        <header><h3>Memory</h3><span class="pill">{size(inv.memory.totalBytes)}</span></header>
        <Meter label="In use" pct={s?.ramPct ?? null} text={s ? `${gb(s.ramUsedBytes)} of ${gb(s.ramTotalBytes)} GB` : ""} warnAt={90} reason="Waiting for the first reading" />
        <div class="ff-stats">
          <Stat label="Available" value={s ? gb(s.ramAvailableBytes) : null} unit="GB" />
          <Stat label="Page file used" value={s && s.swapTotalBytes > 0 ? gb(s.swapUsedBytes) : null} unit="GB" reason="No page file reported" />
        </div>
        {#if inv.memory.modules.length}
          <table class="mini">
            <thead><tr><th>Slot</th><th>Size</th><th>Type</th><th>Speed</th></tr></thead>
            <tbody>
              {#each inv.memory.modules as m}
                <tr><td>{m.slot ?? DASH}</td><td>{size(m.capacityBytes)}</td><td>{m.kind ?? DASH}</td><td>{m.speedMhz ? `${m.speedMhz} MHz` : DASH}</td></tr>
              {/each}
            </tbody>
          </table>
        {:else}
          <p class="ff-note">{inv.memory.modulesNote ?? "Module type and speed were not reported."}</p>
        {/if}
        <LineChart label="Memory use" unit="%" values={ramHistory} max={100} height={70} tone="info" />
      </section>

      <section class="ff-card">
        <header><h3>Displays</h3><span class="pill">{inv.displays.length}</span></header>
        {#if !inv.displays.length}<p class="ff-note">No display information is available on this system.</p>{/if}
        {#each inv.displays as d (d.device)}
          <div class="block">
            <div class="lead">{d.name || d.device}{#if d.primary} <span class="pill live">Primary</span>{/if}</div>
            <dl class="ff-kv">
              <dt>Resolution</dt><dd>{d.width} × {d.height}{#if d.nativeWidth && (d.nativeWidth !== d.width || d.nativeHeight !== d.height)} <span class="faint">(native {d.nativeWidth} × {d.nativeHeight})</span>{/if}</dd>
              <dt>Refresh rate</dt><dd>{d.refreshHz === null ? DASH : `${num(d.refreshHz, d.refreshHz % 1 ? 2 : 0)} Hz`}{#if d.availableRefreshHz.length && d.refreshHz !== null && Math.max(...d.availableRefreshHz) > Math.round(d.refreshHz)} <span class="pill warn" title="This display offers a higher refresh rate at this resolution">up to {Math.max(...d.availableRefreshHz)} Hz available</span>{/if}</dd>
              <dt>HDR</dt><dd>{d.hdrSupported === null ? DASH : d.hdrSupported ? (d.hdrEnabled ? "On" : "Supported, off") : "Not supported"}</dd>
              <dt>Variable refresh</dt><dd title={d.vrrNote ?? undefined}>{d.vrrSupported === null ? "Not detectable" : d.vrrSupported ? "Supported" : "Not supported"}</dd>
              {#if d.adapter}<dt>Connected to</dt><dd>{d.adapter}</dd>{/if}
            </dl>
          </div>
        {/each}
      </section>

      <section class="ff-card">
        <header><h3>Storage</h3></header>
        <table class="mini">
          <thead><tr><th>Drive</th><th>Type</th><th>Free</th><th>Size</th></tr></thead>
          <tbody>
            {#each inv.drives as d}
              <tr title={d.model ?? undefined}>
                <td>{d.mount}{#if d.label} <span class="faint">{d.label}</span>{/if}</td>
                <td>{[d.bus, d.kind].filter(Boolean).join(" ") || DASH}</td>
                <td class:low={d.totalBytes > 0 && d.freeBytes / d.totalBytes < 0.1}>{size(d.freeBytes)}</td>
                <td>{size(d.totalBytes)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </section>

      <section class="ff-card">
        <header><h3>Windows</h3></header>
        <div class="lead">{inv.system.osName} {inv.system.osVersion}</div>
        <dl class="ff-kv">
          <dt>Build</dt><dd>{inv.system.osBuild ?? DASH}</dd>
          <dt>Power plan</dt><dd>{plan?.name ?? DASH}</dd>
          <dt>On battery</dt><dd>{yesNo(inv.system.gaming.onBattery, "Yes", "No")}</dd>
          <dt>Game Mode</dt><dd>{yesNo(inv.system.gaming.gameMode)}</dd>
          <dt>Background recording</dt><dd>{yesNo(inv.system.gaming.backgroundRecording)}</dd>
          <dt>GPU scheduling (HAGS)</dt><dd>{yesNo(inv.system.gaming.hags)}</dd>
          <dt>Windowed-game optimisations</dt><dd>{yesNo(inv.system.gaming.windowedOptimizations)}</dd>
          <dt>Startup apps</dt><dd>{inv.system.startupApps.filter((a) => a.enabled).length} on</dd>
        </dl>
      </section>

      <section class="ff-card">
        <header><h3>Measurement and driver interfaces</h3></header>
        <dl class="ff-kv">
          {#each ffs.overview?.vendorApis ?? [] as v}
            <dt>{VENDOR_LABEL[v.vendor]} {v.api}</dt>
            <dd title={v.status.state === "unavailable" ? v.status.reason : undefined}>
              {#if v.status.state === "available"}<span class="pill live">Ready</span>
              {:else if !v.hardwarePresent}<span class="faint">No {VENDOR_LABEL[v.vendor]} GPU</span>
              {:else}<span class="pill warn">Unavailable</span>{/if}
            </dd>
          {/each}
          <dt>PresentMon</dt>
          <dd>{#if pm?.willUse}<span class="pill live">{pm.willUse === "service" ? "Service" : "Console app"}</span>{:else}<span class="pill warn">Not set up</span>{/if}</dd>
        </dl>
        {#each (ffs.overview?.vendorApis ?? []).filter((v) => v.hardwarePresent && v.status.state === "unavailable") as v}
          <p class="ff-note"><strong>{v.api}:</strong> {v.status.state === "unavailable" ? v.status.reason : ""}</p>
        {/each}
        {#if pm && !pm.willUse}
          <p class="ff-note">Frame rate measurement needs PresentMon. <button class="link" onclick={() => (ffs.tab = "monitor")}>Set it up in Live Monitor</button></p>
        {/if}
      </section>
    </div>
  {/if}
</div>

<style>
  .bar { display: flex; justify-content: space-between; align-items: center; gap: 1rem; margin-bottom: 1rem; }
  .chips { display: flex; flex-wrap: wrap; gap: 0.35rem; }
  .block { display: grid; gap: 0.5rem; }
  .block + .block { border-top: 1px solid var(--line-soft); padding-top: 0.85rem; }
  .mini { width: 100%; border-collapse: collapse; font-size: 0.85rem; font-variant-numeric: tabular-nums; }
  .mini th { text-align: left; font-size: 0.7rem; letter-spacing: 0.08em; text-transform: uppercase; color: var(--faint); font-weight: 700; padding: 0 0.5rem 0.3rem 0; }
  .mini td { padding: 0.3rem 0.5rem 0.3rem 0; border-top: 1px solid var(--line-soft); }
  .mini td.low { color: var(--rose); font-weight: 600; }
  .link { border: 0; background: transparent; padding: 0; color: var(--lilac); font-weight: 600; text-decoration: underline; }
  .link:hover { background: transparent; }
  :global(.ff-page) .ff-banner { margin-bottom: 0.75rem; }
</style>
