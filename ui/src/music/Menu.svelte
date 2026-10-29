<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import { menu, prompt, type MenuItem } from "./menu.svelte";

  let sub: { items: MenuItem[]; y: number } | null = $state(null);
  let box: HTMLDivElement | undefined = $state();
  let pos = $state({ x: 0, y: 0 });

  $effect(() => {
    if (!menu.open) {
      sub = null;
      return;
    }
    // Keep it on screen.
    const w = box?.offsetWidth ?? 220;
    const h = box?.offsetHeight ?? 200;
    pos = { x: Math.max(8, Math.min(menu.x - (menu.x + w > innerWidth - 8 ? w : 0), innerWidth - w - 8)), y: Math.max(8, Math.min(menu.y, innerHeight - h - 8)) };
  });

  function close() {
    menu.open = false;
  }
  function pick(it: MenuItem) {
    if (it.sub) return;
    close();
    it.run?.();
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && menu.open) close();
  }

  let input: HTMLInputElement | undefined = $state();
  $effect(() => {
    if (prompt.open) setTimeout(() => input?.select(), 30);
  });
  function answer(v: string | null) {
    prompt.open = false;
    prompt.resolve?.(v === null ? null : v.trim());
    prompt.resolve = null;
  }
</script>

<svelte:window onkeydown={onKey} />

{#if menu.open}
  <div class="scrim" role="presentation" onclick={close} oncontextmenu={(e) => { e.preventDefault(); close(); }}></div>
  <div class="menu" role="menu" bind:this={box} style:left="{pos.x}px" style:top="{pos.y}px">
    {#each menu.items as it}
      {#if it.sep}
        <div class="sep" role="separator"></div>
      {:else}
        <button role="menuitem" class:danger={it.danger} onclick={() => pick(it)} onmouseenter={(e) => (sub = it.sub ? { items: it.sub, y: (e.currentTarget as HTMLElement).offsetTop } : null)}>
          {#if it.icon}<Icon name={it.icon} size={15} />{:else}<span class="pad"></span>{/if}
          <span class="lbl">{it.label}</span>
          {#if it.sub}<span class="chev">›</span>{/if}
        </button>
      {/if}
    {/each}
    {#if sub}
      <div class="menu submenu" role="menu" style:top="{sub.y - 4}px">
        {#each sub.items as it}
          <button role="menuitem" onclick={() => pick(it)}>
            {#if it.icon}<Icon name={it.icon} size={15} />{:else}<span class="pad"></span>{/if}
            <span class="lbl">{it.label}</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>
{/if}

{#if prompt.open}
  <div class="scrim dim" role="presentation" onclick={() => answer(null)}></div>
  <form class="prompt" onsubmit={(e) => { e.preventDefault(); answer(prompt.value); }}>
    <h3>{prompt.title}</h3>
    <input bind:this={input} bind:value={prompt.value} maxlength="80" aria-label={prompt.title} />
    <div class="row">
      <span class="grow"></span>
      <button type="button" onclick={() => answer(null)}>Cancel</button>
      <button type="submit" class="ok">Save</button>
    </div>
  </form>
{/if}

<style>
  .scrim { position: fixed; inset: 0; z-index: 60; }
  .scrim.dim { background: rgba(0, 0, 0, 0.35); }
  .menu {
    position: fixed; z-index: 61; min-width: 210px; max-height: 70vh; overflow: visible;
    padding: 5px; border-radius: var(--r-md);
    background: var(--raised); border: 1px solid var(--line);
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.35);
  }
  .submenu { position: absolute; left: calc(100% - 4px); max-height: 50vh; overflow-y: auto; }
  .menu button {
    display: flex; align-items: center; gap: 0.6rem; width: 100%;
    border: 0; background: transparent; border-radius: var(--r-sm);
    padding: 0.42rem 0.6rem; font-weight: 500; font-size: 0.88rem; text-align: left;
  }
  .menu button:hover { background: color-mix(in srgb, var(--text) 8%, transparent); }
  .menu button.danger { color: var(--rose); }
  .lbl { flex: 1; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 260px; }
  .pad { width: 15px; }
  .chev { color: var(--faint); }
  .sep { height: 1px; background: var(--line-soft); margin: 4px 2px; }
  .prompt {
    position: fixed; z-index: 62; left: 50%; top: 30%; transform: translateX(-50%);
    width: min(400px, 90vw); padding: 1.1rem; border-radius: var(--r-lg);
    background: var(--raised); border: 1px solid var(--line); display: grid; gap: 0.8rem;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.4);
  }
  .prompt input { width: 100%; }
  .ok { background: var(--peach); border-color: var(--peach); color: var(--peach-ink); }
</style>
