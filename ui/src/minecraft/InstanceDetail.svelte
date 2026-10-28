<script lang="ts">
  import { onMount } from "svelte";
  import { mc, openUrl } from "../lib/api";
  import { bytes, toast } from "../lib/state.svelte";
  import { mcs, ago, play, playtime, refreshInstances } from "../lib/mc.svelte";
  import { LOADER_LABEL, type Hit, type McInstance, type ModFile, type Verified } from "../lib/types";
  import Toggle from "../components/Toggle.svelte";
  import Modal from "../components/Modal.svelte";
  import Icon from "../components/Icon.svelte";

  let { inst }: { inst: McInstance } = $props();

  type Sub = "mods" | "add" | "settings";
  let sub = $state<Sub>("mods");
  let mods: ModFile[] = $state([]);
  let modQuery = $state("");
  let verified: Record<string, Verified> = $state({});
  let verifying = $state(false);

  const running = $derived(mcs.running.includes(inst.id));
  const shown = $derived(
    mods.filter((m) => {
      const q = modQuery.trim().toLowerCase();
      return !q || m.fileName.toLowerCase().includes(q) || (m.meta?.name ?? "").toLowerCase().includes(q);
    }),
  );

  async function loadMods() {
    try {
      mods = await mc.mods(inst.id);
    } catch (e) {
      toast(String(e), "error");
    }
  }
  onMount(loadMods);

  async function toggle(m: ModFile, on: boolean) {
    try {
      const newName = await mc.toggleMod(inst.id, m.fileName, on);
      m.fileName = newName;
      m.enabled = on;
    } catch (e) {
      toast(String(e), "error");
      await loadMods();
    }
  }
  async function removeMod(m: ModFile) {
    await mc.removeMod(inst.id, m.fileName);
    mods = mods.filter((x) => x !== m);
  }
  async function verify() {
    verifying = true;
    try {
      const r = await mc.verify(inst.id);
      verified = Object.fromEntries(r.map((v) => [v.fileName, v]));
      const unknown = r.filter((v) => v.source === "unknown").length;
      toast(unknown ? `${unknown} file${unknown === 1 ? "" : "s"} didn't match Modrinth or CurseForge` : "Every file matches a Modrinth or CurseForge upload");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      verifying = false;
    }
  }

  // ── Add content ──
  type Kind = "mod" | "resourcepack" | "shader";
  let source = $state<"modrinth" | "curseforge">("modrinth");
  let kind = $state<Kind>("mod");
  let q = $state("");
  let hits: Hit[] = $state([]);
  let searching = $state(false);
  let installing: string | null = $state(null);
  async function search() {
    searching = true;
    try {
      hits = await mc.search(source, q, kind, inst.id);
    } catch (e) {
      toast(String(e), "error");
      hits = [];
    } finally {
      searching = false;
    }
  }
  async function install(h: Hit) {
    installing = h.id;
    try {
      const r = await mc.install(h.source, h.id, kind, inst.id);
      toast(r.dependenciesAdded.length ? `Installed ${h.title} and ${r.dependenciesAdded.join(", ")}` : `Installed ${h.title}`);
      if (kind === "mod") await loadMods();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      installing = null;
    }
  }
  $effect(() => {
    if (sub === "add" && hits.length === 0 && !searching) search();
  });

  // ── Settings ──
  let name = $state("");
  let memory = $state(4096);
  let javaArgs = $state("");
  onMount(() => {
    name = inst.name;
    memory = inst.memoryMb;
    javaArgs = inst.javaArgs;
  });
  async function saveSettings() {
    try {
      await mc.update(inst.id, name, memory, javaArgs);
      await refreshInstances();
      toast("Instance saved");
    } catch (e) {
      toast(String(e), "error");
    }
  }
  let confirmDelete = $state(false);
  async function del() {
    try {
      await mc.remove(inst.id);
      confirmDelete = false;
      await refreshInstances();
    } catch (e) {
      toast(String(e), "error");
    }
  }
  async function duplicate() {
    const copy = await mc.duplicate(inst.id, `${inst.name} (copy)`);
    await refreshInstances();
    mcs.selected = copy.id;
  }
</script>

<div class="detail">
  <header class="top">
    <div class="who">
      <h1>{inst.name}</h1>
      <div class="facts">
        <span class="pill">Minecraft {inst.mcVersion}</span>
        <span class="pill">{LOADER_LABEL[inst.loader]}{inst.loaderVersion ? ` ${inst.loaderVersion}` : ""}</span>
        {#if inst.source?.name || inst.source?.platform}
          <span class="pill">From {inst.source.platform === "modrinth" ? "Modrinth" : inst.source.platform === "curseforge" ? "CurseForge" : inst.source.platform}</span>
        {/if}
        <span class="faint">Played {playtime(inst.playtimeSecs)}, last {ago(inst.lastPlayed)}</span>
      </div>
    </div>
    <div class="row actions">
      {#if running}
        <button onclick={() => (mcs.consoleFor = inst.id)}>Console</button>
        <button class="danger" onclick={() => mc.kill(inst.id)}>Stop</button>
      {:else}
        <button class="play" onclick={() => play(inst.id)}><Icon name="play" size={16} /> Play</button>
      {/if}
    </div>
  </header>

  {#if inst.manualDownloads.length}
    <div class="notice">
      <p><strong>{inst.manualDownloads.length} file{inst.manualDownloads.length === 1 ? "" : "s"} need downloading by hand.</strong> Their authors don't allow other launchers to download them. Open each page, download the file, then put it in the folder shown.</p>
      <ul>
        {#each inst.manualDownloads as d}
          <li>
            <span class="grow">{d.name} <span class="mono faint">{d.fileName}</span></span>
            <button class="ghost" onclick={() => openUrl(d.pageUrl)}>Open page</button>
            <button class="ghost" onclick={() => mc.openFolder(inst.id, d.folder)}>Open {d.folder}</button>
          </li>
        {/each}
      </ul>
    </div>
  {/if}

  <div class="subtabs" role="tablist" tabindex="-1">
    <button role="tab" aria-selected={sub === "mods"} class:on={sub === "mods"} onclick={() => (sub = "mods")}>Mods <span class="n">{mods.length}</span></button>
    <button role="tab" aria-selected={sub === "add"} class:on={sub === "add"} onclick={() => (sub = "add")}>Add content</button>
    <button role="tab" aria-selected={sub === "settings"} class:on={sub === "settings"} onclick={() => (sub = "settings")}>Settings</button>
    <div class="grow"></div>
    <button class="ghost" onclick={() => mc.openFolder(inst.id)}><Icon name="folder" size={16} /> Folder</button>
    {#if !running}<button class="ghost" onclick={() => (mcs.consoleFor = inst.id)}>Console</button>{/if}
  </div>

  {#if sub === "mods"}
    {#if inst.loader === "vanilla"}
      <p class="none">Vanilla instances can't load mods. Make a Fabric, Forge or NeoForge instance to use mods. Resource packs and shaders still work from Add content.</p>
    {:else}
      <div class="row tools">
        <input class="grow" placeholder="Filter mods" bind:value={modQuery} aria-label="Filter mods" />
        <button onclick={verify} disabled={verifying || mods.length === 0} title="Check each file against Modrinth and CurseForge">
          {verifying ? "Checking…" : "Check sources"}
        </button>
      </div>
      {#if mods.length === 0}
        <p class="none">No mods yet. Find some in Add content, or drop .jar files in the mods folder.</p>
      {:else}
        <ul class="mods">
          {#each shown as m (m.fileName)}
            {@const v = verified[m.fileName]}
            <li class:off={!m.enabled}>
              <Toggle label="Enable {m.meta?.name ?? m.fileName}" checked={m.enabled} onchange={(on) => toggle(m, on)} />
              <div class="mtxt">
                <span class="mname">{m.meta?.name ?? m.fileName.replace(/\.jar(\.disabled)?$/, "")}{#if m.meta?.version}<span class="faint ver">{m.meta.version}</span>{/if}</span>
                <span class="mono faint file">{m.fileName}</span>
              </div>
              {#if v}
                <span class="src src-{v.source}" title={v.project ?? ""}>{v.source === "unknown" ? "Unknown source" : v.source === "modrinth" ? "Modrinth" : "CurseForge"}</span>
              {/if}
              <span class="faint size">{bytes(m.size)}</span>
              <button class="ghost danger icon" onclick={() => removeMod(m)} aria-label="Remove {m.fileName}"><Icon name="trash" size={16} /></button>
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
  {:else if sub === "add"}
    <div class="row tools wrap">
      <div class="seg" role="radiogroup" aria-label="Source">
        <button role="radio" aria-checked={source === "modrinth"} onclick={() => { source = "modrinth"; search(); }}>Modrinth</button>
        <button role="radio" aria-checked={source === "curseforge"} onclick={() => { source = "curseforge"; search(); }}>CurseForge</button>
      </div>
      <div class="seg" role="radiogroup" aria-label="Type">
        <button role="radio" aria-checked={kind === "mod"} disabled={inst.loader === "vanilla"} onclick={() => { kind = "mod"; search(); }}>Mods</button>
        <button role="radio" aria-checked={kind === "resourcepack"} onclick={() => { kind = "resourcepack"; search(); }}>Resource packs</button>
        <button role="radio" aria-checked={kind === "shader"} onclick={() => { kind = "shader"; search(); }}>Shaders</button>
      </div>
      <input class="grow" placeholder="Search" bind:value={q} onkeydown={(e) => e.key === "Enter" && search()} aria-label="Search" />
      <button onclick={search} disabled={searching}>Search</button>
    </div>
    <p class="hint faint">Results are filtered to Minecraft {inst.mcVersion}{inst.loader !== "vanilla" ? ` and ${LOADER_LABEL[inst.loader]}` : ""}. Required dependencies are added for you.</p>
    {#if searching}
      <p class="none">Searching…</p>
    {:else if hits.length === 0}
      <p class="none">Nothing found for this version. Try another search or source.</p>
    {:else}
      <ul class="hits">
        {#each hits as h (h.id)}
          <li>
            {#if h.iconUrl}<img src={h.iconUrl} alt="" width="40" height="40" loading="lazy" />{:else}<span class="noicon"></span>{/if}
            <div class="htxt">
              <span class="htitle">{h.title} <span class="faint by">by {h.author}</span></span>
              <span class="muted hdesc">{h.description}</span>
            </div>
            <span class="faint dl">{Intl.NumberFormat(undefined, { notation: "compact" }).format(h.downloads)}</span>
            <button onclick={() => install(h)} disabled={installing !== null}>{installing === h.id ? "Installing…" : "Install"}</button>
          </li>
        {/each}
      </ul>
    {/if}
  {:else}
    <div class="settings">
      <div class="setting">
        <div><div class="label">Name</div></div>
        <input bind:value={name} style="width: 18rem" aria-label="Instance name" />
      </div>
      <div class="setting">
        <div><div class="label">Memory</div><div class="desc">How much RAM Minecraft may use. Big modpacks want 6–8 GB; vanilla is happy with 2–4 GB.</div></div>
        <div class="row">
          <input type="range" min="1024" max="16384" step="512" bind:value={memory} aria-label="Memory" />
          <span class="mono val">{(memory / 1024).toFixed(1)} GB</span>
        </div>
      </div>
      <div class="setting">
        <div><div class="label">Java arguments</div><div class="desc">Extra JVM flags. Leave empty unless a modpack asks for them.</div></div>
        <input class="mono" bind:value={javaArgs} placeholder="-XX:+UseZGC" style="width: 18rem" aria-label="Java arguments" />
      </div>
      <div class="row save">
        <button onclick={saveSettings}>Save changes</button>
        <div class="grow"></div>
        <button class="ghost" onclick={duplicate}>Duplicate</button>
        <button class="ghost danger" onclick={() => (confirmDelete = true)}>Delete instance</button>
      </div>
    </div>
  {/if}
</div>

<Modal bind:open={confirmDelete} title="Delete {inst.name}?">
  <p>This deletes the instance's worlds, mods and settings from this computer. It can't be undone.</p>
  {#snippet actions()}
    <button class="ghost" onclick={() => (confirmDelete = false)}>Keep it</button>
    <button class="danger" onclick={del}>Delete instance</button>
  {/snippet}
</Modal>

<style>
  .detail { padding: 1.4rem 2rem 3rem; }
  .top { display: flex; align-items: flex-end; justify-content: space-between; gap: 1rem; margin-bottom: 1rem; }
  h1 { font-size: clamp(1.8rem, 3.4vw, 2.6rem); line-height: 1; letter-spacing: -0.03em; font-weight: 800; margin-bottom: 0.6rem; overflow-wrap: anywhere; }
  .facts { display: flex; flex-wrap: wrap; align-items: center; gap: 0.4rem; font-size: 0.85rem; }
  .actions { flex-shrink: 0; }

  .notice {
    border: 1px solid color-mix(in srgb, var(--peach) 40%, transparent);
    background: color-mix(in srgb, var(--peach) 9%, transparent);
    border-radius: var(--r-md); padding: 0.8rem 1rem; margin-bottom: 1rem; font-size: 0.9rem;
  }
  .notice ul { list-style: none; margin: 0.6rem 0 0; padding: 0; }
  .notice li { display: flex; align-items: center; gap: 0.4rem; padding: 0.2rem 0; }

  .subtabs { display: flex; align-items: center; gap: 0.2rem; border-bottom: 1px solid var(--line-soft); margin-bottom: 0.9rem; }
  .subtabs button[role="tab"] { border: 0; background: transparent; border-radius: 0; padding: 0.55rem 0.7rem; color: var(--muted); font-weight: 600; font-size: 0.88rem; box-shadow: inset 0 -2px 0 transparent; }
  .subtabs button.on { color: var(--text); box-shadow: inset 0 -2px 0 var(--peach); }
  .n { color: var(--faint); font-variant-numeric: tabular-nums; }
  .grow { flex: 1; }
  .tools { margin-bottom: 0.7rem; }
  .hint { font-size: 0.82rem; margin: -0.2rem 0 0.7rem; }
  .none { color: var(--muted); padding: 1.2rem 0; max-width: 62ch; }

  .mods, .hits { list-style: none; margin: 0; padding: 0; }
  .mods li { display: flex; align-items: center; gap: 0.8rem; padding: 0.5rem 0; border-bottom: 1px solid var(--line-soft); }
  .mods li.off .mtxt { opacity: 0.5; }
  .mtxt { flex: 1; min-width: 0; display: grid; }
  .mname { font-weight: 600; }
  .ver { margin-left: 0.5rem; font-weight: 500; font-size: 0.82rem; }
  .file { font-size: 0.75rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .size { font-size: 0.8rem; white-space: nowrap; width: 5rem; text-align: right; }
  .icon { padding: 0.35rem; }
  .src { font-size: 0.78rem; font-weight: 600; white-space: nowrap; }
  .src-modrinth { color: var(--mint); }
  .src-curseforge { color: var(--peach); }
  .src-unknown { color: var(--rose); }

  .hits li { display: flex; align-items: center; gap: 0.9rem; padding: 0.6rem 0; border-bottom: 1px solid var(--line-soft); }
  .hits img, .noicon { width: 40px; height: 40px; border-radius: var(--r-sm); flex-shrink: 0; background: var(--sunk); object-fit: cover; }
  .htxt { flex: 1; min-width: 0; display: grid; gap: 0.1rem; }
  .htitle { font-weight: 600; }
  .by { font-weight: 500; font-size: 0.82rem; }
  .hdesc { font-size: 0.85rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dl { font-size: 0.8rem; white-space: nowrap; }

  .settings { max-width: 760px; }
  .val { width: 4.5rem; text-align: right; color: var(--muted); }
  .save { padding-top: 1rem; }
</style>
