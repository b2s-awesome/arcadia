<script lang="ts">
  import type { Snippet } from "svelte";
  let { open = $bindable(false), title, children, actions, wide = false }: { open?: boolean; title: string; children: Snippet; actions?: Snippet; wide?: boolean } = $props();
  let dlg: HTMLDialogElement | undefined = $state();
  $effect(() => {
    if (!dlg) return;
    if (open && !dlg.open) dlg.showModal();
    if (!open && dlg.open) dlg.close();
  });
</script>

<dialog bind:this={dlg} class:wide onclose={() => (open = false)}>
  <h3>{title}</h3>
  <div class="body">{@render children()}</div>
  {#if actions}<div class="actions">{@render actions()}</div>{/if}
</dialog>

<style>
  dialog {
    width: min(520px, 92vw);
    background: var(--raised);
    color: var(--text);
    font-family: var(--body);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    padding: 1.5rem;
  }
  dialog.wide { width: min(860px, 94vw); }
  dialog::backdrop { background: rgba(10, 6, 18, 0.7); }
  h3 { font-size: 1.3rem; margin-bottom: 0.75rem; }
  .body { color: var(--muted); display: grid; gap: 0.8rem; }
  .actions { display: flex; justify-content: flex-end; gap: 0.6rem; margin-top: 1.4rem; }
</style>
