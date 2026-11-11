<script lang="ts">
  // A labelled bar for a 0–100% reading. No bar is drawn when the value is unknown.
  let { label, pct, text = "", reason = "", warnAt = 101 }: { label: string; pct: number | null; text?: string; reason?: string; warnAt?: number } = $props();
  const known = $derived(pct !== null && Number.isFinite(pct));
  const clamped = $derived(known ? Math.min(100, Math.max(0, pct as number)) : 0);
</script>

<div class="meter" title={!known && reason ? reason : undefined}>
  <div class="head">
    <span class="label">{label}</span>
    <span class="text" class:faint={!known}>{known ? text || `${clamped.toFixed(0)}%` : "—"}</span>
  </div>
  <div class="track" role="meter" aria-label={label} aria-valuemin="0" aria-valuemax="100" aria-valuenow={known ? clamped : undefined}>
    {#if known}<div class="fill" class:warn={clamped >= warnAt} style:width="{clamped}%"></div>{/if}
  </div>
  {#if !known && reason}<div class="why">{reason}</div>{/if}
</div>

<style>
  .meter { display: grid; gap: 0.3rem; min-width: 0; }
  .head { display: flex; justify-content: space-between; gap: 0.75rem; font-size: 0.85rem; }
  .label { color: var(--muted); font-weight: 600; }
  .text { font-variant-numeric: tabular-nums; font-weight: 600; }
  .track { height: 6px; border-radius: 6px; background: var(--sunk); border: 1px solid var(--line-soft); overflow: hidden; }
  .fill { height: 100%; background: var(--peach); transition: width 0.4s ease; }
  .fill.warn { background: var(--rose); }
  .why { font-size: 0.75rem; color: var(--faint); }
</style>
