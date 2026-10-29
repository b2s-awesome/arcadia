<script lang="ts">
  import { onMount } from "svelte";
  import { mc, openUrl } from "../lib/api";
  import { bytes, toast } from "../lib/state.svelte";
  import { mcs, ago, play, playtime, refreshInstances } from "../lib/mc.svelte";
  import { LOADER_LABEL, type Hit, type McInstance, type ModFile, type Verified } from "../lib/types";
  import Toggle from "../components/Toggle.svelte";
  import Modal from "../components/Modal.svelte";
  import Icon from "../components/Icon.svelte";
  import TreeTabs, { type TreeNode } from "../components/TreeTabs.svelte";
  import Packs from "./Packs.svelte";
  import Worlds from "./Worlds.svelte";
  import Screenshots from "./Screenshots.svelte";
  import Servers from "./Servers.svelte";
  import Logs from "./Logs.svelte";
  import Updates from "./Updates.svelte";
  import { pickFile, pickSave } from "../lib/api";
  import type { McExportOptions } from "../lib/types";

  let { inst }: { inst: McInstance } = $props();

  let mods: ModFile[] = $state([]);
  let sub = $state("mods");
  $effect(() => {
    if (mcs.openSection) {
      sub = mcs.openSection;
      mcs.openSection = null;
    }
  });
  let updateCount = $state(0);
  const tree: TreeNode[] = $derived([
    { id: "content", label: "Content", icon: "cube", children: [
      { id: "mods", label: "Mods", badge: inst.loader === "vanilla" ? "" : mods.length || "" },
      { id: "updates", label: "Updates", badge: updateCount || "" },
      { id: "resourcepacks", label: "Resource packs" },
      { id: "shaderpacks", label: "Shaders" },
      { id: "add", label: "Add content" },
    ] },
    { id: "play", label: "Play", icon: "gamepad", children: [
      { id: "worlds", label: "Worlds" },
      { id: "servers", label: "Servers" },
      { id: "screenshots", label: "Screenshots" },
    ] },
    { id: "logs", label: "Logs & crashes", icon: "code" },
    { id: "settings", label: "Settings", icon: "settings", children: [
      { id: "settings-general", label: "General" },
      { id: "settings-java", label: "Java & launch" },
      { id: "settings-export", label: "Export & share" },
    ] },
  ]);
  const at = (id: string) => sub === id || (id.startsWith(sub + "-") && sub !== "") || (sub === "content" && id === "mods") || (sub === "play" && id === "worlds");
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
  function findContent(k: Kind) {
    kind = k;
    hits = [];
    sub = "add";
  }

  // ── Java & launch ──
  let runtimes = $state<{ id: string; label: string }[]>([]);
  let java = $state("");
  let javaPath = $state<string | null>(null);
  let autoJoin = $state("");
  onMount(() => {
    mc.javaRuntimes().then((r) => (runtimes = r)).catch(() => {});
    java = inst.java ?? "";
    javaPath = inst.javaPath ?? null;
    autoJoin = inst.autoJoin ?? "";
  });
  async function saveLaunch() {
    try {
      await mc.setLaunch(inst.id, java, javaPath, autoJoin);
      await refreshInstances();
      toast("Saved");
    } catch (e) {
      toast(String(e), "error");
    }
  }
  async function chooseJava() {
    const f = await pickFile([{ name: "Java", extensions: ["exe"] }]);
    if (f) {
      javaPath = f;
      java = "custom";
    }
  }

  // ── Export ──
  let ex = $state<McExportOptions>({ linkDownloads: true, includeConfig: true, includeWorlds: false, includeOptions: false, includeResourcepacks: true, includeShaderpacks: true, version: "1.0.0" });
  let exporting = $state(false);
  async function doExport() {
    const safe = inst.name.replace(/[\\/:*?"<>|]/g, "").trim() || "pack";
    const dest = await pickSave(`${safe} ${ex.version}.mrpack`, [{ name: "Modrinth pack", extensions: ["mrpack"] }, { name: "Zip", extensions: ["zip"] }]);
    if (!dest) return;
    exporting = true;
    try {
      const r = await mc.exportPack(inst.id, dest, $state.snapshot(ex));
      toast(`Exported: ${r.linked} linked to Modrinth, ${r.packed} packed in`);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      exporting = false;
    }
  }

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

  <div class="split">
  <TreeTabs key="mc-instance" label="{inst.name} sections" nodes={tree} bind:selected={sub} />
  <div class="pane">
  {#if at("mods")}
    <div class="row spread head2">
      <h3>Mods <span class="n">{mods.length}</span></h3>
      <div class="row">
        <button class="ghost" onclick={() => mc.openFolder(inst.id, "mods")}><Icon name="folder" size={15} /> Folder</button>
        {#if inst.loader !== "vanilla"}<button class="ghost" onclick={() => (sub = "updates")}><Icon name="refresh" size={15} /> Updates{#if updateCount} ({updateCount}){/if}</button>{/if}
        {#if inst.loader !== "vanilla"}<button onclick={() => findContent("mod")}><Icon name="search" size={15} /> Find mods</button>{/if}
      </div>
    </div>
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
  {:else if sub === "updates"}
    <Updates {inst} {running} bind:found={updateCount} onchanged={loadMods} />
  {:else if sub === "resourcepacks"}
    <Packs {inst} kind="resourcepacks" onadd={() => findContent("resourcepack")} />
  {:else if sub === "shaderpacks"}
    <Packs {inst} kind="shaderpacks" onadd={() => findContent("shader")} />
  {:else if at("worlds")}
    <Worlds {inst} {running} />
  {:else if sub === "servers"}
    <Servers {inst} {running} />
  {:else if sub === "screenshots"}
    <Screenshots {inst} />
  {:else if sub === "logs"}
    <Logs {inst} {running} />
  {:else if sub === "settings-java"}
    <div class="settings">
      <h3>Java & launch</h3>
      <div class="setting">
        <div><div class="label">Java</div><div class="desc">Arcadia downloads the Java each Minecraft version asks for. Pick another if a modpack says it needs one.</div></div>
        <select bind:value={java} aria-label="Java">
          <option value="">Automatic (recommended)</option>
          {#each runtimes as r}<option value={r.id}>{r.label} (Mojang)</option>{/each}
          <option value="custom">Your own Java…</option>
        </select>
      </div>
      {#if java === "custom"}
        <div class="setting">
          <div><div class="label">Java program</div><div class="desc mono">{javaPath ?? "None chosen"}</div></div>
          <button onclick={chooseJava}>Choose javaw.exe</button>
        </div>
      {/if}
      <div class="setting">
        <div><div class="label">Join a server on start</div><div class="desc">Connect straight to this server every time you press Play. Leave empty to open the title screen.</div></div>
        <input class="mono" bind:value={autoJoin} placeholder="play.example.net" style="width: 18rem" aria-label="Server to join on start" />
      </div>
      <div class="row save"><button onclick={saveLaunch}>Save changes</button></div>
    </div>
  {:else if sub === "settings-export"}
    <div class="settings">
      <h3>Export & share</h3>
      <p class="hint">Makes a <span class="mono">.mrpack</span> that Arcadia, Prism Launcher and the Modrinth app can import.</p>
      <div class="setting">
        <div><div class="label">Link Modrinth files</div><div class="desc">Files from Modrinth are listed by link instead of packed in, so the pack stays small. Turn off for a full copy that works offline.</div></div>
        <Toggle label="Link Modrinth files" bind:checked={ex.linkDownloads} />
      </div>
      <div class="setting"><div class="label">Mod settings (config)</div><Toggle label="Include config" bind:checked={ex.includeConfig} /></div>
      <div class="setting"><div class="label">Resource packs</div><Toggle label="Include resource packs" bind:checked={ex.includeResourcepacks} /></div>
      <div class="setting"><div class="label">Shader packs</div><Toggle label="Include shader packs" bind:checked={ex.includeShaderpacks} /></div>
      <div class="setting"><div><div class="label">Game options</div><div class="desc">Keybinds and video settings (options.txt).</div></div><Toggle label="Include options" bind:checked={ex.includeOptions} /></div>
      <div class="setting"><div><div class="label">Worlds</div><div class="desc">Can make the pack very large.</div></div><Toggle label="Include worlds" bind:checked={ex.includeWorlds} /></div>
      <div class="setting">
        <div class="label">Pack version</div>
        <input class="mono" bind:value={ex.version} style="width: 8rem" aria-label="Pack version" />
      </div>
      <div class="row save"><button class="play" disabled={exporting} onclick={doExport}><Icon name="download" size={16} /> {exporting ? "Exporting…" : "Export pack"}</button></div>
    </div>
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
      <h3>General</h3>
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
  </div>
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

  .split { display: grid; grid-template-columns: 190px minmax(0, 1fr); gap: 1.4rem; align-items: start; }
  @media (max-width: 860px) { .split { grid-template-columns: 1fr; } }
  .pane { min-width: 0; }
  .pane h3 { margin-bottom: 0.4rem; }
  .head2 { margin-bottom: 0.6rem; }
  .head2 button { display: inline-flex; align-items: center; gap: 0.35rem; }
  .n { color: var(--faint); font-weight: 500; font-variant-numeric: tabular-nums; }
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
