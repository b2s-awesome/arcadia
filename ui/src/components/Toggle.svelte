<script lang="ts">
  let { checked = $bindable(false), label, onchange }: { checked?: boolean; label: string; onchange?: (v: boolean) => void } = $props();
</script>

<button
  class="toggle"
  role="switch"
  aria-checked={checked}
  aria-label={label}
  onclick={() => {
    checked = !checked;
    onchange?.(checked);
  }}
>
  <span class="knob"></span>
</button>

<style>
  .toggle {
    width: 44px;
    height: 26px;
    padding: 0;
    border-radius: 999px;
    background: var(--sunk);
    border: 1px solid var(--line);
    position: relative;
    flex-shrink: 0;
  }
  .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--faint);
    transition: transform 0.15s ease, background 0.15s;
  }
  .toggle[aria-checked="true"] { background: color-mix(in srgb, var(--mint) 22%, var(--sunk)); border-color: color-mix(in srgb, var(--mint) 50%, transparent); }
  .toggle[aria-checked="true"] .knob { transform: translateX(18px); background: var(--mint); }
</style>
