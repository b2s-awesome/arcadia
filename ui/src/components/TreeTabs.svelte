<script lang="ts" module>
  export interface TreeNode {
    id: string;
    label: string;
    icon?: string;
    /** A count or short note on the right. */
    badge?: string | number;
    /** Greyed out (a profile that's switched off). */
    dim?: boolean;
    children?: TreeNode[];
  }
  // Which groups are folded, per tree, for this run.
  const folded = new Map<string, Set<string>>();
</script>

<script lang="ts">
  // Vertical tabs that can nest: a group folds open and shut, and its
  // children are tabs too. Used where a page has grown too long to scroll.
  import { untrack, type Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let { key, nodes, selected = $bindable(), label, search = false, foot }: {
    key: string; nodes: TreeNode[]; selected: string; label: string; search?: boolean; foot?: Snippet;
  } = $props();

  // The tree's key never changes after mount.
  const store = untrack(() => key);
  let shut = $state(new Set(folded.get(store) ?? []));
  function fold(id: string) {
    const s = new Set(shut);
    if (s.has(id)) s.delete(id);
    else s.add(id);
    shut = s;
    folded.set(store, s);
  }

  let q = $state("");
  const match = (n: TreeNode): boolean => !q.trim() || n.label.toLowerCase().includes(q.trim().toLowerCase()) || !!n.children?.some(match);
  const shown = $derived(nodes.filter(match));

  // Arrow keys move between visible tabs, like a native tree.
  let list: HTMLElement | undefined = $state();
  function keys(e: KeyboardEvent) {
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key) || !list) return;
    const items = [...list.querySelectorAll<HTMLButtonElement>("button.tab")];
    const i = items.indexOf(document.activeElement as HTMLButtonElement);
    const next = e.key === "Home" ? 0 : e.key === "End" ? items.length - 1 : Math.max(0, Math.min(items.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)));
    items[next]?.focus();
    e.preventDefault();
  }
</script>

{#snippet node(n: TreeNode, depth: number)}
  {@const kids = n.children?.filter(match) ?? []}
  {@const open = !shut.has(n.id) || !!q.trim()}
  <li role="none">
    <div class="line" style:--d={depth}>
      {#if n.children}
        <button class="fold" aria-label="{open ? 'Fold' : 'Unfold'} {n.label}" aria-expanded={open} onclick={() => fold(n.id)}><span class="chev" class:open>›</span></button>
      {:else}
        <span class="fold-pad"></span>
      {/if}
      <button class="tab" class:on={selected === n.id} class:dim={n.dim} role="tab" aria-selected={selected === n.id} onclick={() => (selected = n.id)}>
        {#if n.icon}<Icon name={n.icon} size={15} />{/if}
        <span class="lbl">{n.label}</span>
        {#if n.badge !== undefined && n.badge !== ""}<span class="badge">{n.badge}</span>{/if}
      </button>
    </div>
    {#if n.children && open && kids.length}
      <ul role="group">{#each kids as c (c.id)}{@render node(c, depth + 1)}{/each}</ul>
    {/if}
  </li>
{/snippet}

<nav class="tree" aria-label={label}>
  {#if search}
    <label class="find"><Icon name="search" size={14} /><input bind:value={q} placeholder="Find" aria-label="Find in {label}" /></label>
  {/if}
  <ul role="tablist" aria-orientation="vertical" bind:this={list} onkeydown={keys}>
    {#each shown as n (n.id)}{@render node(n, 0)}{/each}
  </ul>
  {#if foot}<div class="foot">{@render foot()}</div>{/if}
</nav>

<style>
  .tree { display: flex; flex-direction: column; gap: 0.4rem; position: sticky; top: 0; align-self: start; max-height: calc(100vh - 140px); overflow-y: auto; padding-right: 0.2rem; }
  ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 2px; }
  .line { display: flex; align-items: center; padding-left: calc(var(--d) * 0.9rem); }
  .fold, .fold-pad { width: 1.4rem; flex-shrink: 0; }
  .fold { border: 0; background: transparent; padding: 0.2rem 0; color: var(--faint); display: grid; place-items: center; border-radius: var(--r-sm); }
  .fold:hover { color: var(--text); background: transparent; }
  .chev { display: inline-block; transition: transform 0.15s; font-size: 1.05rem; line-height: 1; }
  .chev.open { transform: rotate(90deg); }
  .tab { flex: 1; min-width: 0; display: flex; align-items: center; gap: 0.5rem; border: 0; background: transparent; text-align: left; padding: 0.42rem 0.6rem; border-radius: var(--r-sm); color: var(--muted); font-weight: 600; font-size: 0.88rem; }
  .tab:hover { background: color-mix(in srgb, var(--text) 6%, transparent); color: var(--text); }
  .tab.on { background: color-mix(in srgb, var(--peach) 16%, transparent); color: var(--text); box-shadow: inset 2px 0 0 var(--peach); }
  .tab.dim .lbl { opacity: 0.5; }
  .lbl { flex: 1; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .badge { font-size: 0.72rem; font-weight: 600; color: var(--faint); font-variant-numeric: tabular-nums; }
  .find { display: flex; align-items: center; gap: 0.4rem; padding: 0.3rem 0.6rem; border: 1px solid var(--line); border-radius: var(--r-sm); color: var(--faint); margin-bottom: 0.2rem; }
  .find input { border: 0; background: transparent; padding: 0; flex: 1; min-width: 0; outline: none; font-size: 0.86rem; }
  .foot { margin-top: 0.4rem; padding-top: 0.6rem; border-top: 1px solid var(--line-soft); }
</style>
