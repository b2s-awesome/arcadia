<script lang="ts">
  // One reading. A null value renders as "—" and explains itself on hover
  // (and in `sub` when a reason is given), never as a zero.
  let { label, value, unit = "", sub = "", reason = "", big = false, tone = "" }: {
    label: string; value: string | null; unit?: string; sub?: string; reason?: string; big?: boolean; tone?: "" | "ok" | "warn";
  } = $props();
  const missing = $derived(value === null || value === "—");
</script>

<div class="stat" class:big class:missing title={missing && reason ? reason : undefined}>
  <span class="label">{label}</span>
  <span class="value {tone}">
    {missing ? "—" : value}{#if !missing && unit}<span class="unit">{unit}</span>{/if}
  </span>
  {#if missing && reason}
    <span class="sub why">{reason}</span>
  {:else if sub}
    <span class="sub">{sub}</span>
  {/if}
</div>

<style>
  .stat { display: grid; gap: 0.1rem; align-content: start; min-width: 0; }
  .label { font-size: 0.72rem; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; color: var(--faint); }
  .value { font-family: var(--display); font-weight: 700; font-size: 1.35rem; line-height: 1.15; font-variant-numeric: tabular-nums; }
  .big .value { font-size: clamp(3.2rem, 7vw, 5rem); letter-spacing: -0.04em; line-height: 0.95; }
  .unit { font-size: 0.55em; font-weight: 600; color: var(--muted); margin-left: 0.25em; letter-spacing: 0; }
  .missing .value { color: var(--faint); }
  .value.ok { color: var(--mint); }
  .value.warn { color: var(--rose); }
  .sub { font-size: 0.78rem; color: var(--muted); overflow: hidden; text-overflow: ellipsis; }
  .why { color: var(--faint); display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; white-space: normal; }
</style>
