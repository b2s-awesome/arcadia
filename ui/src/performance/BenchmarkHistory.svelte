<script lang="ts">
  import { onMount } from "svelte";
  import { ff, pickSavePath } from "../lib/api";
  import { toast } from "../lib/state.svelte";
  import { ffs } from "../lib/ff.svelte";
  import type { BenchComparison, BenchMetric, BenchSession, MonitorSnapshot } from "../lib/fftypes";
  import Icon from "../components/Icon.svelte";
  import Modal from "../components/Modal.svelte";

  let sessions: BenchSession[] = $state([]);
  let loading = $state(true);
  let failed = $state<string | null>(null);
  let snap = $state<MonitorSnapshot | null>(null);
  let label = $state("");
  let recordStarted = $state(0);
  let nowMs = $state(Date.now());

  let selected: string[] = $state([]);
  let openId = $state<string | null>(null);
  let comparison = $state<BenchComparison | null>(null);
  let renameOpen = $state(false);
  let renaming = $state<BenchSession | null>(null);
  let renameText = $state("");

  const monitoring = $derived(snap?.state === "running");
  const head = (s: BenchSession) => (s.displayed.frames > 0 ? s.displayed : s.presented);
  const title = (s: BenchSession) => s.label || `${s.gameName} run`;
  const fmt = (v: number | null | undefined, d = 1) => (v == null ? "—" : v.toFixed(d));
  const secs = (ms: number) => (ms >= 60000 ? `${Math.floor(ms / 60000)} min ${Math.round((ms % 60000) / 1000)} s` : `${Math.round(ms / 1000)} s`);
  const date = (s: number) => new Date(s * 1000).toLocaleString([], { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
  const short = (s: BenchSession) => s.durationMs < 30000;

  async function load() {
    try {
      sessions = await ff.benchList();
      selected = selected.filter((id) => sessions.some((s) => s.id === id));
      failed = null;
    } catch (e) {
      failed = String(e);
    } finally {
      loading = false;
    }
  }

  async function poll() {
    nowMs = Date.now();
    try {
      const was = snap?.recording;
      snap = await ff.monitorSnapshot();
      if (was && !snap.recording) load(); // The recording ended elsewhere (the game exited, or monitoring stopped).
    } catch {
      snap = null;
    }
  }

  onMount(() => {
    load();
    poll();
    const t = setInterval(poll, 1000);
    return () => clearInterval(t);
  });

  async function start(name = label) {
    try {
      await ff.benchStart(name);
      recordStarted = Date.now();
      await poll();
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function stop() {
    try {
      const b = await ff.benchStop();
      label = "";
      toast(`Saved "${title(b)}": ${secs(b.durationMs)}, ${fmt(head(b).avgFps)} FPS average`);
      openId = b.id;
      await Promise.all([load(), poll()]);
    } catch (e) {
      toast(String(e), "error");
      poll();
    }
  }

  /** Record again under the same name, for a second sample of the same test. */
  function repeat(s: BenchSession) {
    if (!monitoring) return toast(`Start Live Monitor on ${s.exe} first, then choose Repeat again.`, "error");
    if (snap?.process && snap.process.toLowerCase() !== s.exe.toLowerCase() && snap.process !== s.gameName) return toast(`Live Monitor is measuring ${snap.process}, not ${s.exe}.`, "error");
    start(`${title(s).replace(/ \(repeat( \d+)?\)$/, "")} (repeat)`);
  }

  function toggle(id: string, on: boolean) {
    selected = on ? [...selected, id] : selected.filter((x) => x !== id);
    comparison = null;
  }

  /** Oldest first: the earliest run is the baseline the others are measured against. */
  const ordered = () => sessions.filter((s) => selected.includes(s.id)).sort((a, b) => a.startedAt - b.startedAt).map((s) => s.id);

  async function compare() {
    try {
      comparison = await ff.benchCompare(ordered());
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function exportAs(ext: "csv" | "json") {
    const ids = selected.length ? ordered() : sessions.map((s) => s.id);
    if (!ids.length) return;
    const path = await pickSavePath(`arcadia-benchmarks.${ext}`, [{ name: ext === "csv" ? "CSV table" : "JSON report", extensions: [ext] }]);
    if (!path) return;
    try {
      await ff.benchExport(ids, path);
      toast(`Exported ${ids.length} benchmark(s) as ${ext.toUpperCase()}`);
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function remove(s: BenchSession) {
    try {
      await ff.benchDelete(s.id);
      comparison = null;
      toast(`Deleted "${title(s)}"`);
      await load();
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function rename() {
    if (!renaming) return;
    try {
      await ff.benchRename(renaming.id, renameText);
      renameOpen = false;
      comparison = null;
      await load();
    } catch (e) {
      toast(String(e), "error");
    }
  }

  /** Frame-time chart: points for an SVG polyline in a 600 x 120 box. */
  function chart(times: number[]): { points: string; max: number } {
    if (times.length < 2) return { points: "", max: 0 };
    const max = Math.max(...times) * 1.05;
    const points = times.map((t, i) => `${((i / (times.length - 1)) * 600).toFixed(1)},${(120 - (t / max) * 120).toFixed(1)}`).join(" ");
    return { points, max };
  }

  function deltaClass(m: BenchMetric, i: number): string {
    const p = m.deltaPct[i];
    if (p == null || m.higherIsBetter === null || Math.abs(p) < 3) return "";
    return p > 0 === m.higherIsBetter ? "better" : "worse";
  }
  const signed = (v: number | null, d = 1) => (v == null ? "" : `${v >= 0 ? "+" : ""}${v.toFixed(d)}`);
</script>

<div class="ff-page">
  <div class="intro">
    <div>
      <h3 class="page-title">Benchmark History</h3>
      <p class="ff-note">A benchmark is a recording of the frames PresentMon measured for a game, with the conditions it ran under. Compare runs to see what changed; Arcadia flags comparisons that are not like for like.</p>
    </div>
  </div>

  {#if failed}
    <div class="ff-banner warn"><Icon name="warning" size={18} /><span><strong>The benchmark history could not be read.</strong> {failed}</span></div>
  {/if}

  <section class="ff-card">
    <header><h3>Record</h3>{#if monitoring}<span class="pill live">Monitoring {snap?.process}</span>{/if}</header>
    {#if snap?.recording}
      <div class="rec-row">
        <span class="dot"></span>
        <span>Recording <strong>{snap.process}</strong>: {secs(recordStarted ? nowMs - recordStarted : 0)}, {snap.recordedFrames} frames</span>
        <div class="grow"></div>
        <button class="play" onclick={stop}>Stop and save</button>
      </div>
      <p class="ff-note">Run at least 30 seconds of the same scene, route or built-in benchmark each time. Shorter runs are saved but marked unreliable.</p>
    {:else if monitoring}
      <div class="rec-row">
        <input bind:value={label} maxlength="120" placeholder="Name this run, e.g. Baseline or After tuning" aria-label="Benchmark name" />
        <button class="play" onclick={() => start()}>Start recording</button>
      </div>
    {:else}
      <div class="rec-row">
        <span class="ff-note">Start Live Monitor on a game first. A benchmark records the frames it measures.</span>
        <div class="grow"></div>
        <button onclick={() => (ffs.tab = "monitor")}>Open Live Monitor</button>
      </div>
    {/if}
  </section>

  <section class="ff-card">
    <header>
      <h3>Sessions</h3>
      <div class="row">
        <button disabled={selected.length < 2} title={selected.length < 2 ? "Tick two or more sessions" : ""} onclick={compare}>Compare{selected.length >= 2 ? ` (${selected.length})` : ""}</button>
        <button class="ghost" disabled={!sessions.length} onclick={() => exportAs("csv")}>Export CSV</button>
        <button class="ghost" disabled={!sessions.length} onclick={() => exportAs("json")}>Export JSON</button>
      </div>
    </header>
    {#if !loading && !sessions.length}
      <p class="ff-note">No benchmarks yet. Record one above, or from Live Monitor.</p>
    {:else}
      <div class="table" role="table" aria-label="Benchmark sessions">
        <div class="tr th" role="row">
          <span></span><span>Run</span><span>When</span><span>Length</span><span class="num">Average</span><span class="num">1% low</span><span class="num">0.1% low</span><span></span>
        </div>
        {#each sessions as s (s.id)}
          {@const h = head(s)}
          <div class="tr" role="row" class:open={openId === s.id}>
            <span><input type="checkbox" aria-label={`Select ${title(s)}`} checked={selected.includes(s.id)} onchange={(e) => toggle(s.id, e.currentTarget.checked)} /></span>
            <button class="run" onclick={() => (openId = openId === s.id ? null : s.id)}>
              <strong>{title(s)}</strong>
              <span class="ff-note">{s.gameName}{s.profile ? ` · profile ${s.profile}` : ""}</span>
            </button>
            <span class="ff-note">{date(s.startedAt)}</span>
            <span>{secs(s.durationMs)} {#if short(s)}<span class="pill warn" title="Under 30 seconds: not reliable enough to compare">Short</span>{/if}</span>
            <span class="num mono">{fmt(h.avgFps)}</span>
            <span class="num mono">{fmt(h.low1pctFps)}</span>
            <span class="num mono">{fmt(h.low01pctFps)}</span>
            <span class="row end">
              <button class="ghost" title="Record this test again" onclick={() => repeat(s)}>Repeat</button>
              <button class="ghost" onclick={() => { renaming = s; renameText = s.label; renameOpen = true; }}>Rename</button>
              <button class="ghost danger" onclick={() => remove(s)}>Delete</button>
            </span>
          </div>
          {#if openId === s.id}
            {@const c = chart(s.frameTimesMs)}
            <div class="detail">
              {#if c.points}
                <figure>
                  <svg viewBox="0 0 600 120" preserveAspectRatio="none" role="img" aria-label="Frame times over the run"><polyline points={c.points} /></svg>
                  <figcaption class="ff-note">Frame time over the run (top of the chart: {c.max.toFixed(1)} ms). Spikes are stutters.</figcaption>
                </figure>
              {/if}
              <div class="facts">
                <div><span class="ff-note">Frame time</span><span class="mono">avg {fmt(h.avgFrameTimeMs, 2)} · p95 {fmt(h.p95Ms, 2)} · p99 {fmt(h.p99Ms, 2)} · p99.9 {fmt(h.p999Ms, 2)} ms</span></div>
                <div><span class="ff-note">Consistency</span><span class="mono">{fmt(h.consistencyPct, 0)}%{h.stutterFrames != null ? ` · ${h.stutterFrames} stutter frame(s)` : ""}</span></div>
                <div><span class="ff-note">Per frame</span><span class="mono">CPU busy {fmt(s.cpuBusyMs, 2)} ms · GPU busy {fmt(s.gpuBusyMs, 2)} ms{s.displayLatencyMs != null ? ` · latency ${s.displayLatencyMs.toFixed(0)} ms` : ""}</span></div>
                <div><span class="ff-note">Hardware load</span><span class="mono">GPU {fmt(s.gpuUtilPct, 0)}% · {fmt(s.gpuTempAvgC, 0)} °C (max {fmt(s.gpuTempMaxC, 0)}) · {fmt(s.gpuPowerAvgW, 0)} W · CPU {fmt(s.cpuUtilPct, 0)}% · {fmt(s.cpuTempAvgC, 0)} °C</span></div>
                <div><span class="ff-note">Display</span><span class="mono">{s.width && s.height ? `${s.width}x${s.height}` : "unknown"}{s.refreshHz ? ` at ${s.refreshHz.toFixed(0)} Hz` : ""}</span></div>
                <div><span class="ff-note">System</span><span>{s.cpu} · {s.gpu ?? "unknown GPU"}{s.gpuDriver ? ` (driver ${s.gpuDriver})` : ""} · {s.os}</span></div>
                <div><span class="ff-note">Measured by</span><span>{s.source} · {h === s.displayed ? "frames as shown on screen" : "frames as presented by the game"} · {h.frames} frames</span></div>
                <div><span class="ff-note">Optimisations active</span><span>{s.activeOptimizations.length ? s.activeOptimizations.join(", ") : "None applied by Arcadia"}</span></div>
              </div>
              {#each s.notes as n}<div class="ff-banner"><Icon name="info" size={18} /><span>{n}</span></div>{/each}
            </div>
          {/if}
        {/each}
      </div>
    {/if}
  </section>

  {#if comparison}
    <section class="ff-card" aria-label="Comparison">
      <header><h3>Comparison</h3><span class="ff-note">Baseline: {comparison.labels[0]}</span></header>
      {#each comparison.flags.filter((f) => f.severity === "warning") as f}
        <div class="ff-banner warn"><Icon name="warning" size={18} /><span>{f.text}</span></div>
      {/each}
      <div class="cmp" style={`--cols: ${comparison.labels.length}`}>
        <div class="cr th"><span>Measurement</span>{#each comparison.labels as l, i}<span class="num">{l}{i === 0 ? " (baseline)" : ""}</span>{/each}</div>
        {#each comparison.metrics as m (m.key)}
          <div class="cr">
            <span>{m.name}</span>
            {#each m.values as v, i}
              <span class="num">
                <span class="mono">{fmt(v, m.unit === "ms" ? 2 : 1)} {m.unit}</span>
                {#if i > 0 && m.delta[i] != null}
                  <span class={`delta mono ${deltaClass(m, i)}`}>{signed(m.delta[i], m.unit === "ms" ? 2 : 1)} {m.unit}{m.deltaPct[i] != null ? ` (${signed(m.deltaPct[i])}%)` : ""}</span>
                {/if}
              </span>
            {/each}
          </div>
        {/each}
      </div>
      <ul class="summary">{#each comparison.summary as line}<li>{line}</li>{/each}</ul>
      {#each comparison.flags.filter((f) => f.severity === "info") as f}
        <p class="ff-note">{f.text}</p>
      {/each}
    </section>
  {/if}
</div>

<Modal bind:open={renameOpen} title="Rename benchmark">
  <label class="field">Name<input bind:value={renameText} maxlength="120" /></label>
  {#snippet actions()}
    <button class="ghost" onclick={() => (renameOpen = false)}>Cancel</button>
    <button class="play" onclick={rename}>Rename</button>
  {/snippet}
</Modal>

<style>
  .intro { margin-bottom: 1rem; }
  .page-title { font-family: var(--display); font-size: 1.15rem; margin-bottom: 0.3rem; }
  .intro .ff-note { max-width: 72ch; }
  section { margin-bottom: 1rem; }
  .row { display: flex; gap: 0.4rem; flex-wrap: wrap; align-items: center; }
  .row.end { justify-content: flex-end; flex-wrap: nowrap; }
  .row.end button { padding: 0.3rem 0.55rem; font-size: 0.8rem; }
  .grow { flex: 1; }
  .rec-row { display: flex; gap: 0.6rem; align-items: center; flex-wrap: wrap; }
  .rec-row input { flex: 1; min-width: 14rem; }
  .dot { width: 0.7rem; height: 0.7rem; border-radius: 50%; background: var(--rose); animation: pulse 1.4s ease-in-out infinite; }
  @keyframes pulse { 50% { opacity: 0.35; } }
  @media (prefers-reduced-motion: reduce) { .dot { animation: none; } }
  .table { display: grid; min-width: 0; overflow-x: auto; }
  .tr { display: grid; grid-template-columns: 1.6rem minmax(10rem, 2fr) 7.5rem 7.5rem 4.2rem 4.2rem 4.2rem 13rem; gap: 0.6rem; align-items: center; padding: 0.55rem 0; border-top: 1px solid var(--line-soft); min-width: 52rem; }
  .tr.th { border-top: 0; color: var(--muted); font-size: 0.75rem; font-weight: 600; padding-top: 0; }
  .tr.open { background: var(--sunk); }
  .num { text-align: right; }
  .run { border: 0; background: transparent; padding: 0; text-align: left; display: grid; gap: 0.1rem; color: var(--text); min-width: 0; }
  .run strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .detail { padding: 0.75rem 0 1rem 2.35rem; display: grid; gap: 0.75rem; border-top: 1px solid var(--line-soft); }
  figure { margin: 0; display: grid; gap: 0.3rem; }
  svg { width: 100%; height: 7.5rem; background: var(--sunk); border-radius: var(--r-md); border: 1px solid var(--line-soft); }
  polyline { fill: none; stroke: var(--peach); stroke-width: 1.2; vector-effect: non-scaling-stroke; }
  .facts { display: grid; gap: 0.35rem; font-size: 0.86rem; }
  .facts > div { display: grid; grid-template-columns: 10rem 1fr; gap: 0.75rem; }
  .cmp { display: grid; overflow-x: auto; margin: 0.5rem 0; }
  .cr { display: grid; grid-template-columns: minmax(12rem, 1.4fr) repeat(var(--cols), minmax(9rem, 1fr)); gap: 0.75rem; padding: 0.45rem 0; border-top: 1px solid var(--line-soft); font-size: 0.86rem; align-items: start; }
  .cr.th { border-top: 0; color: var(--muted); font-size: 0.75rem; font-weight: 600; }
  .cr .num { display: grid; gap: 0.1rem; justify-items: end; }
  .delta { font-size: 0.78rem; color: var(--muted); }
  .delta.better { color: var(--mint); }
  .delta.worse { color: var(--rose); }
  .summary { margin: 0.75rem 0 0.5rem; padding-left: 1.1rem; display: grid; gap: 0.3rem; font-size: 0.9rem; }
  .field { display: grid; gap: 0.3rem; font-size: 0.82rem; color: var(--muted); }
  .field input { width: 100%; }
</style>
