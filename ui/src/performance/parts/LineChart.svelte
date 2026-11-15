<script lang="ts">
  // A plain line chart with a labelled scale. Gaps in the data (null) are
  // drawn as gaps, not joined or zeroed.
  let { values, label, unit = "", min = 0, max = null, height = 120, digits = 0, tone = "accent", reason = "" }: {
    values: (number | null)[]; label: string; unit?: string; min?: number; max?: number | null; height?: number; digits?: number;
    tone?: "accent" | "ok" | "info" | "bad"; reason?: string;
  } = $props();

  const W = 600;
  const known = $derived(values.filter((v): v is number => v !== null && Number.isFinite(v)));
  const top = $derived.by(() => {
    if (max !== null) return max;
    const m = known.length ? Math.max(...known) : 1;
    // Round the scale up to a tidy number so the axis labels are readable.
    const step = 10 ** Math.floor(Math.log10(Math.max(m, 1e-6)));
    return Math.max(min + 1e-6, Math.ceil((m * 1.1) / step) * step);
  });
  const path = $derived.by(() => {
    const n = values.length;
    if (n < 2) return "";
    let d = "";
    let pen = false;
    values.forEach((v, i) => {
      if (v === null || !Number.isFinite(v)) { pen = false; return; }
      const x = (i / (n - 1)) * W;
      const y = height - ((Math.min(top, Math.max(min, v)) - min) / (top - min)) * height;
      d += `${pen ? "L" : "M"}${x.toFixed(1)} ${y.toFixed(1)}`;
      pen = true;
    });
    return d;
  });
  const last = $derived(known.length ? known[known.length - 1] : null);
  const color = $derived({ accent: "var(--peach)", ok: "var(--mint)", info: "var(--lilac)", bad: "var(--rose)" }[tone]);
</script>

<figure class="chart">
  <figcaption>
    <span class="label">{label}</span>
    <span class="now" class:faint={last === null}>{last === null ? "—" : last.toFixed(digits)}{#if last !== null && unit}<span class="unit">{unit}</span>{/if}</span>
  </figcaption>
  <div class="plot" style:height="{height}px">
    <span class="tick t">{top.toFixed(top < 10 ? 1 : 0)}</span>
    <span class="tick m">{((top + min) / 2).toFixed(top < 10 ? 1 : 0)}</span>
    <span class="tick b">{min.toFixed(0)}</span>
    {#if known.length >= 2}
      <svg viewBox="0 0 {W} {height}" preserveAspectRatio="none" role="img" aria-label="{label} over time">
        <line x1="0" x2={W} y1={height / 2} y2={height / 2} class="grid" />
        <path d={path} fill="none" stroke={color} stroke-width="1.6" vector-effect="non-scaling-stroke" stroke-linejoin="round" />
      </svg>
    {:else}
      <div class="empty">{reason || "Waiting for data"}</div>
    {/if}
  </div>
</figure>

<style>
  .chart { margin: 0; display: grid; gap: 0.35rem; min-width: 0; }
  figcaption { display: flex; justify-content: space-between; align-items: baseline; gap: 0.75rem; }
  .label { font-size: 0.72rem; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; color: var(--faint); }
  .now { font-weight: 700; font-variant-numeric: tabular-nums; }
  .unit { font-size: 0.8em; color: var(--muted); margin-left: 0.2em; font-weight: 600; }
  .plot { position: relative; background: var(--sunk); border: 1px solid var(--line-soft); border-radius: var(--r-md); overflow: hidden; }
  svg { position: absolute; inset: 0; width: 100%; height: 100%; }
  .grid { stroke: var(--line-soft); stroke-width: 1; vector-effect: non-scaling-stroke; }
  .tick { position: absolute; left: 6px; font-size: 0.66rem; color: var(--faint); font-variant-numeric: tabular-nums; z-index: 1; pointer-events: none; }
  .tick.t { top: 3px; }
  .tick.m { top: calc(50% - 0.5em); }
  .tick.b { bottom: 3px; }
  .empty { position: absolute; inset: 0; display: grid; place-items: center; color: var(--faint); font-size: 0.82rem; padding: 0 2rem; text-align: center; }
</style>
