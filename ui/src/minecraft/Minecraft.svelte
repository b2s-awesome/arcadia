<script lang="ts">
  import { onMount } from "svelte";
  import { mc } from "../lib/api";
  import { activeAccount, initMinecraft, mcs, type McTab } from "../lib/mc.svelte";
  import Icon from "../components/Icon.svelte";
  import Instances from "./Instances.svelte";
  import Modpacks from "./Modpacks.svelte";
  import Import from "./Import.svelte";
  import Security from "./Security.svelte";
  import Accounts from "./Accounts.svelte";
  import Console from "./Console.svelte";
  import NewInstance from "./NewInstance.svelte";

  let { standalone = false }: { standalone?: boolean } = $props();

  const tabs: { id: McTab; label: string }[] = [
    { id: "instances", label: "Instances" },
    { id: "modpacks", label: "Modpacks" },
    { id: "import", label: "Import" },
    { id: "security", label: "Security" },
    { id: "accounts", label: "Accounts and setup" },
  ];

  let creating = $state(false);
  let avatarOk = $state(true);
  onMount(() => {
    initMinecraft();
  });

  const account = $derived(activeAccount());
  const pct = $derived(mcs.progress && mcs.progress.total > 0 ? Math.min(100, (mcs.progress.done / mcs.progress.total) * 100) : null);
</script>

<div class="mc">
  <header class="head">
    <div class="title">
      <h2>Minecraft</h2>
      <button class="acct" onclick={() => (mcs.tab = "accounts")}>
        {#if account}
          {#if avatarOk}
            <img src="https://mc-heads.net/avatar/{account.uuid}/20" alt="" width="20" height="20" onerror={() => (avatarOk = false)} />
          {:else}
            <span class="initial">{account.name[0]}</span>
          {/if}
          <span>{account.name}</span>
        {:else}
          <span class="dot-warn"></span><span>Not signed in</span>
        {/if}
      </button>
    </div>
    <div class="row">
      <button onclick={() => (creating = true)}><Icon name="plus" size={16} /> New instance</button>
      {#if standalone}
        <button class="ghost" onclick={() => mc.dock()} title="Close this window and show the launcher inside Arcadia">Back to Arcadia</button>
      {:else}
        <button class="ghost" onclick={() => mc.popOut()} title="Open the launcher in its own window">Pop out</button>
      {/if}
    </div>
  </header>

  <div class="tabs" role="tablist" tabindex="-1" aria-label="Minecraft">
    {#each tabs as t}
      <button role="tab" aria-selected={mcs.tab === t.id} class:on={mcs.tab === t.id} onclick={() => (mcs.tab = t.id)}>{t.label}</button>
    {/each}
  </div>

  <div class="body">
    {#if mcs.tab === "instances"}<Instances onnew={() => (creating = true)} />
    {:else if mcs.tab === "modpacks"}<Modpacks />
    {:else if mcs.tab === "import"}<Import />
    {:else if mcs.tab === "security"}<Security />
    {:else}<Accounts />{/if}
  </div>

  {#if mcs.progress}
    <div class="progress" role="status" aria-live="polite">
      <span class="msg">{mcs.progress.message}</span>
      <div class="track">
        <div class="fill" class:indet={pct === null} style:width={pct === null ? undefined : `${pct}%`}></div>
      </div>
      {#if pct !== null}<span class="mono faint">{mcs.progress.done} / {mcs.progress.total}</span>{/if}
    </div>
  {/if}
</div>

<NewInstance bind:open={creating} />
<Console />

<style>
  .mc { height: 100%; display: flex; flex-direction: column; min-height: 0; position: relative; }
  .head { display: flex; align-items: center; justify-content: space-between; gap: 1rem; padding: 1.6rem 2.2rem 0.6rem; }
  .title { display: flex; align-items: center; gap: 1rem; }
  .acct {
    display: inline-flex; align-items: center; gap: 0.5rem;
    background: var(--sunk); border: 1px solid var(--line-soft); border-radius: 999px;
    padding: 0.2rem 0.75rem 0.2rem 0.3rem; font-size: 0.85rem; font-weight: 600; color: var(--muted);
  }
  .acct img { border-radius: 4px; image-rendering: pixelated; }
  .initial { width: 20px; height: 20px; border-radius: 4px; display: grid; place-items: center; background: var(--peach); color: var(--peach-ink); font-size: 0.75rem; font-weight: 800; }
  .dot-warn { width: 8px; height: 8px; border-radius: 50%; background: var(--rose); margin-left: 0.4rem; }

  .tabs { display: flex; gap: 0.2rem; padding: 0 2.2rem; border-bottom: 1px solid var(--line-soft); }
  .tabs button {
    border: 0; background: transparent; border-radius: 0; padding: 0.7rem 0.8rem;
    color: var(--muted); font-weight: 600; font-size: 0.9rem; box-shadow: inset 0 -2px 0 transparent;
  }
  .tabs button:hover { color: var(--text); background: transparent; }
  .tabs button.on { color: var(--text); box-shadow: inset 0 -2px 0 var(--peach); }

  .body { flex: 1; min-height: 0; overflow: hidden; }

  .progress {
    display: flex; align-items: center; gap: 1rem;
    padding: 0.8rem 2.2rem; background: var(--raised); border-top: 1px solid var(--line);
    font-weight: 600; font-size: 0.9rem;
  }
  .msg { max-width: 45%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .track { flex: 1; height: 6px; background: var(--sunk); border-radius: 6px; overflow: hidden; }
  .fill { height: 100%; background: var(--peach); transition: width 0.25s ease; }
  .fill.indet { width: 30%; animation: slide 1.2s ease-in-out infinite; }
  @keyframes slide { from { transform: translateX(-100%); } to { transform: translateX(340%); } }
</style>
