<script lang="ts">
  import { mcs } from "../lib/mc.svelte";
  import { toast } from "../lib/state.svelte";
  import Modal from "../components/Modal.svelte";

  let open = $state(false);
  let pre: HTMLPreElement | undefined = $state();
  let follow = $state(true);

  $effect(() => {
    open = mcs.consoleFor !== null;
  });
  $effect(() => {
    if (!open) mcs.consoleFor = null;
  });

  const name = $derived(mcs.instances.find((i) => i.id === mcs.consoleFor)?.name ?? "Minecraft");
  const lines = $derived(mcs.consoleFor ? (mcs.logs[mcs.consoleFor] ?? []) : []);

  $effect(() => {
    void lines.length;
    if (follow && pre) queueMicrotask(() => pre && (pre.scrollTop = pre.scrollHeight));
  });

  async function copy() {
    await navigator.clipboard.writeText(lines.join("\n"));
    toast("Log copied");
  }
  const tone = (l: string) => (/\b(ERROR|FATAL|Exception)\b/.test(l) ? "err" : /\bWARN\b/.test(l) ? "warn" : "");
</script>

<Modal bind:open wide title="{name} console">
  {#if lines.length === 0}
    <p>No output yet. Output from the game appears here while it runs.</p>
  {:else}
    <pre bind:this={pre} onscroll={() => pre && (follow = pre.scrollTop + pre.clientHeight >= pre.scrollHeight - 20)}>{#each lines as l}<span class={tone(l)}>{l}
</span>{/each}</pre>
  {/if}
  {#snippet actions()}
    <button class="ghost" onclick={copy} disabled={lines.length === 0}>Copy log</button>
    <button onclick={() => (open = false)}>Close</button>
  {/snippet}
</Modal>

<style>
  pre {
    margin: 0; max-height: 55vh; overflow: auto; 
    background: var(--sunk); border: 1px solid var(--line-soft); border-radius: var(--r-sm);
    padding: 0.7rem; font-size: 0.75rem; line-height: 1.45; color: var(--text);
    white-space: pre-wrap; word-break: break-all; user-select: text;
  }
  .err { color: var(--rose); }
  .warn { color: var(--peach); }
</style>
