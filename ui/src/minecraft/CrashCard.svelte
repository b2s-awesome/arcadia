<script lang="ts">
  // What went wrong, in plain words, with the fix.
  import type { McCrashInfo } from "../lib/types";
  import Icon from "../components/Icon.svelte";

  let { info, compact = false }: { info: McCrashInfo; compact?: boolean } = $props();
  const ICON: Record<string, string> = {
    "missing-dependency": "bundle", incompatible: "close", duplicate: "copy", memory: "sliders", heap: "sliders",
    java: "code", graphics: "image", mixin: "code", "mod-error": "cube", suspect: "search", "missing-class": "bundle",
  };
</script>

<div class="crash" class:compact>
  {#if info.diagnoses.length}
    {#each info.diagnoses as d, i (i)}
      <div class="dx">
        <span class="ico"><Icon name={ICON[d.kind] ?? "shield"} size={18} /></span>
        <div class="body">
          <strong>{d.title}</strong>
          <p class="muted">{d.detail}</p>
          <p class="fix"><Icon name="check" size={14} /> {d.fix}</p>
        </div>
      </div>
    {/each}
  {:else}
    <div class="dx">
      <span class="ico"><Icon name="search" size={18} /></span>
      <div class="body">
        <strong>Arcadia doesn't recognise this crash</strong>
        <p class="muted">{info.lastError ?? "There's no error message in the log."}</p>
        <p class="fix"><Icon name="check" size={14} /> If you added mods recently, turn the newest ones off and try again, or check updates.</p>
      </div>
    </div>
  {/if}
  {#if info.lastError && info.diagnoses.length && !compact}
    <p class="last mono">{info.lastError}</p>
  {/if}
  <p class="src faint">From <span class="mono">{info.path}</span></p>
</div>

<style>
  .crash { display: flex; flex-direction: column; gap: 0.55rem; }
  .dx { display: flex; gap: 0.8rem; padding: 0.8rem 0.9rem; border-radius: var(--r-lg); background: color-mix(in srgb, var(--rose) 7%, var(--sunk)); border: 1px solid color-mix(in srgb, var(--rose) 22%, var(--line-soft)); }
  .ico { width: 34px; height: 34px; flex: none; display: grid; place-items: center; border-radius: 50%; color: var(--rose); background: color-mix(in srgb, var(--rose) 14%, transparent); }
  .body { display: flex; flex-direction: column; gap: 0.2rem; min-width: 0; }
  .body p { font-size: 0.88rem; }
  .fix { color: var(--text); display: flex; gap: 0.35rem; align-items: baseline; }
  .fix :global(svg) { color: var(--mint); flex: none; position: relative; top: 2px; }
  .last { font-size: 0.75rem; color: var(--muted); padding: 0.5rem 0.7rem; border-radius: var(--r-sm); background: var(--sunk); overflow-wrap: anywhere; user-select: text; }
  .src { font-size: 0.75rem; }
  .compact .dx { padding: 0.6rem 0.7rem; }
</style>
