<script lang="ts">
  import { addons as addonApi } from "../lib/api";
  import { app, saveSettingsNow, toast } from "../lib/state.svelte";
  import { CATALOG, type AddonMeta } from "../addons/registry";
  import { seeThroughRemoved } from "../lib/seeThrough.svelte";
  import Icon from "../components/Icon.svelte";
  import Modal from "../components/Modal.svelte";

  type Filter = "all" | "installed" | "official";
  let filter: Filter = $state("all");
  let detail: AddonMeta | null = $state(null);
  let detailOpen = $state(false);
  let removing: AddonMeta | null = $state(null);
  let removeOpen = $state(false);
  let wipeData = $state(false);
  let busy = $state(false);

  const isInstalled = (id: string) => app.ui.addons.installed.includes(id);
  const shown = $derived(
    CATALOG.filter((a) => (filter === "installed" ? isInstalled(a.id) : filter === "official" ? a.official : true)),
  );
  const brand = $derived(app.ui.brand.name.trim() || "Arcadia");

  async function install(a: AddonMeta) {
    if (isInstalled(a.id)) return;
    busy = true;
    app.ui.addons.installed = [...app.ui.addons.installed, a.id];
    // Show it in the sidebar straight away, even if the user hid it before.
    const page = `addon:${a.id}`;
    app.ui.rail.hidden = app.ui.rail.hidden.filter((h) => h !== page);
    if (app.settings) app.settings.ui = $state.snapshot(app.ui);
    // The backend checks the installed list, so save before opening it.
    await saveSettingsNow();
    busy = false;
    detailOpen = false;
    toast(a.load ? `${a.name} added to the sidebar` : a.opens === "music" ? `${a.name} added: see Online and Downloads in Music` : `${a.name} added. Its settings are in Appearance.`);
    openAddon(a);
  }

  function openAddon(a: AddonMeta) {
    app.page = a.load ? `addon:${a.id}` : (a.opens ?? "appearance");
  }

  function askRemove(a: AddonMeta) {
    removing = a;
    wipeData = false;
    removeOpen = true;
  }

  async function remove() {
    const a = removing;
    if (!a) return;
    busy = true;
    app.ui.addons.installed = app.ui.addons.installed.filter((x) => x !== a.id);
    app.ui.rail.order = app.ui.rail.order.filter((x) => x !== `addon:${a.id}`);
    if (app.settings) app.settings.ui = $state.snapshot(app.ui);
    await saveSettingsNow();
    if (wipeData) {
      try {
        await addonApi.clear(a.id);
      } catch (e) {
        toast(String(e), "error");
      }
    }
    busy = false;
    removeOpen = false;
    toast(wipeData ? `Removed ${a.name} and its saved data` : `Removed ${a.name}. Its saved data is kept if you add it again.`);
    if (a.id === "see-through") seeThroughRemoved();
  }

  function open(a: AddonMeta) {
    detail = a;
    detailOpen = true;
  }
</script>

{#snippet art(a: AddonMeta, big = false)}
  <div class="art" class:big style:--a={a.art[0]} style:--b={a.art[1]} style:--c={a.art[2]} aria-hidden="true">
    <span class="art-icon"><Icon name={a.icon} size={big ? 40 : 30} /></span>
  </div>
{/snippet}

<div class="page">
  <div class="page-inner">
    <header class="page-head">
      <div>
        <h2>Marketplace</h2>
        <p>Addons bring whole new tools into {brand}. Each one runs on your PC, keeps its data in its own folder, and can be removed in a click.</p>
      </div>
      <div class="seg" role="radiogroup" aria-label="Show">
        <button role="radio" aria-checked={filter === "all"} onclick={() => (filter = "all")}>All</button>
        <button role="radio" aria-checked={filter === "official"} onclick={() => (filter = "official")}>Official</button>
        <button role="radio" aria-checked={filter === "installed"} onclick={() => (filter = "installed")}>Installed</button>
      </div>
    </header>

    {#if shown.length}
      <div class="grid">
        {#each shown as a (a.id)}
          {@const on = isInstalled(a.id)}
          <article class="card">
            <button class="card-hit" onclick={() => open(a)} aria-label="Details for {a.name}"></button>
            {@render art(a)}
            <div class="card-body">
              <div class="row name-row">
                <h3>{a.name}</h3>
                {#if a.official}<span class="pill official"><Icon name="check" size={12} /> Official</span>{/if}
              </div>
              <p class="tagline">{a.tagline}</p>
              <div class="row meta">
                <span class="faint">by {a.author} · v{a.version}</span>
                <span class="grow"></span>
                {#if on}
                  <button class="primary-sm" onclick={() => openAddon(a)}>{a.load ? "Open" : "Settings"}</button>
                {:else}
                  <button class="primary-sm" disabled={busy} onclick={() => install(a)}><Icon name="plus" size={15} /> Add</button>
                {/if}
              </div>
            </div>
          </article>
        {/each}
      </div>
    {:else}
      <div class="empty">
        <Icon name="store" size={28} />
        <p>Nothing installed yet. Pick something from <button class="link" onclick={() => (filter = "all")}>All</button>.</p>
      </div>
    {/if}

    <p class="footnote faint">This is Arcadia's first marketplace. Community addons and a shared catalog are on the way; for now every addon here is made and reviewed by the Arcadia project.</p>
  </div>
</div>

<Modal bind:open={detailOpen} title={detail?.name ?? ""} wide>
  {#if detail}
    {@const on = isInstalled(detail.id)}
    <div class="detail">
      {@render art(detail, true)}
      <div class="detail-main">
        <div class="row wrap">
          {#if detail.official}<span class="pill official"><Icon name="check" size={12} /> Official</span>{/if}
          <span class="pill">v{detail.version}</span>
          <span class="pill">by {detail.author}</span>
          {#if on}<span class="pill live"><span class="dot"></span>Installed</span>{/if}
        </div>
        <p class="desc">{detail.description}</p>
        <h4>What it does</h4>
        <ul class="feat">
          {#each detail.features as f}<li><Icon name="check" size={14} /><span>{f}</span></li>{/each}
        </ul>
        <h4>What it can access</h4>
        <ul class="perms">
          {#each detail.permissions as p}<li><strong>{p.label}</strong><span>{p.detail}</span></li>{/each}
        </ul>
      </div>
    </div>
  {/if}
  {#snippet actions()}
    {#if detail && isInstalled(detail.id)}
      <button class="danger" onclick={() => { detailOpen = false; askRemove(detail!); }}>Remove</button>
      <button class="primary-sm" onclick={() => { detailOpen = false; openAddon(detail!); }}>{detail.load ? "Open" : "Settings"}</button>
    {:else if detail}
      <button onclick={() => (detailOpen = false)}>Not now</button>
      <button class="primary-sm" disabled={busy} onclick={() => install(detail!)}><Icon name="plus" size={15} /> Add to {brand}</button>
    {/if}
  {/snippet}
</Modal>

<Modal bind:open={removeOpen} title={`Remove ${removing?.name ?? ""}?`}>
  <p>It leaves the sidebar and stops running. You can add it again any time.</p>
  <label class="wipe">
    <input type="checkbox" bind:checked={wipeData} />
    <span>Also delete everything it saved{removing?.id === "discord-profile" ? " (your profile versions and bot token)" : ""}</span>
  </label>
  {#snippet actions()}
    <button onclick={() => (removeOpen = false)}>Keep it</button>
    <button class="danger" disabled={busy} onclick={remove}>Remove</button>
  {/snippet}
</Modal>

<style>
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 1rem; }
  .card {
    position: relative; overflow: hidden;
    background: var(--group); border: 1px solid var(--line-soft); border-radius: var(--r-lg);
    display: flex; flex-direction: column;
    transition: border-color 0.15s, transform 0.15s;
  }
  .card:hover { border-color: var(--line); transform: translateY(-1px); }
  .card-hit { position: absolute; inset: 0; z-index: 0; border: 0; background: transparent; border-radius: 0; padding: 0; }
  .card-hit:hover { background: transparent; }
  .card-body { position: relative; z-index: 1; pointer-events: none; padding: 0.9rem 1rem 1rem; display: grid; gap: 0.35rem; }
  .card-body button { pointer-events: auto; }
  .name-row { gap: 0.5rem; }
  .tagline { color: var(--muted); font-size: 0.9rem; }
  .meta { margin-top: 0.4rem; font-size: 0.82rem; }

  .art {
    position: relative; pointer-events: none;
    height: 110px;
    background:
      radial-gradient(120% 140% at 0% 0%, var(--b) 0%, transparent 55%),
      radial-gradient(120% 140% at 100% 100%, var(--c) 0%, transparent 60%),
      var(--a);
    display: grid; place-items: center;
  }
  .art::after {
    content: ""; position: absolute; inset: 0;
    background-image: radial-gradient(rgba(255, 255, 255, 0.16) 1px, transparent 1.2px);
    background-size: 14px 14px;
    mask-image: linear-gradient(180deg, transparent, black 70%);
  }
  .art.big { height: auto; min-height: 220px; width: 200px; border-radius: var(--r-md); flex-shrink: 0; }
  .art-icon {
    position: relative; z-index: 1;
    display: grid; place-items: center;
    width: 64px; height: 64px; border-radius: 18px;
    color: white; background: rgba(255, 255, 255, 0.14);
    border: 1px solid rgba(255, 255, 255, 0.25);
    backdrop-filter: blur(6px);
  }
  .art.big .art-icon { width: 84px; height: 84px; border-radius: 24px; }

  .pill.official { color: var(--lilac); border-color: color-mix(in srgb, var(--lilac) 35%, transparent); }
  .primary-sm {
    display: inline-flex; align-items: center; gap: 0.35rem;
    background: var(--peach); border-color: var(--peach); color: var(--peach-ink);
    padding: 0.35rem 0.9rem; border-radius: 999px; font-size: 0.86rem;
  }
  .primary-sm:hover { background: color-mix(in srgb, var(--peach) 85%, white); border-color: transparent; }

  .detail { display: flex; gap: 1.4rem; align-items: stretch; }
  .detail-main { display: grid; gap: 0.7rem; align-content: start; color: var(--text); }
  .desc { color: var(--muted); line-height: 1.55; }
  h4 { font-size: 0.78rem; letter-spacing: 0.1em; text-transform: uppercase; color: var(--faint); margin-top: 0.3rem; }
  .feat, .perms { list-style: none; margin: 0; padding: 0; display: grid; gap: 0.45rem; font-size: 0.9rem; }
  .feat li { display: flex; gap: 0.5rem; align-items: baseline; color: var(--muted); }
  .feat li :global(svg) { color: var(--mint); flex-shrink: 0; transform: translateY(2px); }
  .perms li { display: grid; gap: 0.1rem; padding: 0.55rem 0.7rem; border: 1px solid var(--line-soft); border-radius: var(--r-md); background: var(--sunk); }
  .perms strong { font-size: 0.86rem; }
  .perms span { color: var(--muted); font-size: 0.84rem; }

  .empty { display: grid; justify-items: center; gap: 0.6rem; padding: 3rem 1rem; color: var(--muted); }
  button.link { border: 0; background: none; padding: 0; color: var(--lilac); font-weight: 600; }
  .footnote { margin-top: 2rem; font-size: 0.84rem; max-width: 70ch; }
  .wipe { display: flex; gap: 0.6rem; align-items: flex-start; color: var(--text); cursor: pointer; }
  .wipe input { margin-top: 0.3rem; }

  @media (max-width: 760px) {
    .detail { flex-direction: column; }
    .art.big { width: 100%; min-height: 140px; }
  }
</style>
