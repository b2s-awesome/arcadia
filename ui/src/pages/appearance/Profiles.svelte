<script lang="ts">
  // Profiles (whole setups you can switch between), backups to a file, and
  // a sync folder that keeps your setup the same on every PC.
  import { onMount } from "svelte";
  import { api, pickFile, pickFolder, pickSave, type BackupManifest } from "../../lib/api";
  import { app, saveUi, saveSettings, toast, reloadSettings } from "../../lib/state.svelte";
  import { normalizeUi, PRESETS, type Profile } from "../../lib/theme";
  import Icon from "../../components/Icon.svelte";
  import Modal from "../../components/Modal.svelte";
  import Toggle from "../../components/Toggle.svelte";

  let { view }: { view: "profiles" | "backup" } = $props();

  // ── profiles ──
  const snapshot = () => {
    const u = $state.snapshot(app.ui) as unknown as Record<string, unknown>;
    delete u.profiles;
    delete u.bundles;
    return u;
  };
  let newName = $state("");
  function saveNew() {
    const name = newName.trim();
    if (!name) return;
    const p: Profile = { id: `p${Date.now().toString(36)}`, name, created: Math.floor(Date.now() / 1000), ui: snapshot() };
    app.ui.profiles.list = [...app.ui.profiles.list, p];
    app.ui.profiles.active = p.id;
    newName = "";
    saveUi();
    toast(`Saved profile “${name}”`);
  }
  function update(p: Profile) {
    p.ui = snapshot();
    saveUi();
    toast(`Updated “${p.name}”`);
  }
  function use(p: Profile) {
    const keep = { profiles: { ...app.ui.profiles, active: p.id }, bundles: app.ui.bundles };
    Object.assign(app.ui, normalizeUi({ ...structuredClone(p.ui), ...keep }));
    saveUi();
    toast(`Switched to “${p.name}”`);
  }
  function drop(p: Profile) {
    app.ui.profiles.list = app.ui.profiles.list.filter((x) => x.id !== p.id);
    if (app.ui.profiles.active === p.id) app.ui.profiles.active = null;
    saveUi();
  }
  function rename(p: Profile, name: string) {
    if (!name.trim()) return;
    p.name = name.trim();
    saveUi();
  }
  const lookOf = (p: Profile) => {
    const l = p.ui.look as { preset?: keyof typeof PRESETS; accent?: string | null } | undefined;
    const pre = l?.preset && PRESETS[l.preset] ? PRESETS[l.preset] : null;
    return { name: pre?.name ?? "Custom", accent: l?.accent ?? pre?.colors.accent ?? "#888", panel: pre?.colors.panel ?? "#111" };
  };
  const date = (s: number) => new Date(s * 1000).toLocaleString(undefined, { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

  // ── backup ──
  let parts = $state<{ id: string; label: string }[]>([]);
  let chosen = $state<string[]>([]);
  let keys = $state(false);
  let busy = $state(false);
  onMount(async () => {
    parts = await api.backupParts();
    chosen = parts.map((p) => p.id);
    check();
  });
  async function backup() {
    const d = new Date();
    const dest = await pickSave(`Arcadia backup ${d.toISOString().slice(0, 10)}.arcadia`, [{ name: "Arcadia backup", extensions: ["arcadia"] }]);
    if (!dest) return;
    busy = true;
    try {
      const m = await api.backupExport(dest, chosen, keys);
      toast(`Backed up ${m.files} files`);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = false;
    }
  }
  let restoring = $state<{ path: string; m: BackupManifest; parts: string[] } | null>(null);
  let restoreOpen = $state(false);
  async function pickRestore() {
    const f = await pickFile([{ name: "Arcadia backup", extensions: ["arcadia", "zip"] }]);
    if (!f) return;
    try {
      const m = await api.backupInspect(f);
      restoring = { path: f, m, parts: [...m.parts] };
      restoreOpen = true;
    } catch (e) {
      toast(String(e), "error");
    }
  }
  async function restore() {
    if (!restoring) return;
    busy = true;
    try {
      const n = await api.backupRestore(restoring.path, restoring.parts);
      restoreOpen = false;
      await reloadSettings();
      toast(`Restored ${n} files. What you had before is kept in Arcadia's backups folder.`);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = false;
    }
  }
  const label = (id: string) => parts.find((p) => p.id === id)?.label ?? id;

  // ── sync ──
  let newer = $state<BackupManifest | null>(null);
  async function check() {
    newer = app.settings?.sync_folder ? await api.syncCheck().catch(() => null) : null;
  }
  async function chooseFolder() {
    const f = await pickFolder();
    if (!f || !app.settings) return;
    app.settings.sync_folder = f;
    saveSettings();
    setTimeout(check, 500);
  }
  async function syncNow() {
    busy = true;
    try {
      const m = await api.syncNow();
      if (app.settings) app.settings.sync_last = m.created;
      toast("Synced");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = false;
    }
  }
  async function takeNewer() {
    busy = true;
    try {
      await api.syncRestore();
      await reloadSettings();
      newer = null;
      toast("Your setup now matches the sync folder");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = false;
    }
  }
</script>

{#if view === "profiles"}
  <section class="section">
    <h3>Profiles</h3>
    <p class="hint">A profile keeps your whole setup: look, wallpaper, sidebar, top bar, Home, keys, motion and sound, and addon settings. Make one for gaming, one for work, one for showing friends, and switch in a click.</p>
    {#if app.ui.profiles.list.length}
      <ul class="profiles">
        {#each app.ui.profiles.list as p (p.id)}
          {@const l = lookOf(p)}
          <li class:on={app.ui.profiles.active === p.id}>
            <span class="swatch" style:background={l.panel}><i style:background={l.accent}></i></span>
            <div class="txt">
              <input class="nm" value={p.name} onchange={(e) => rename(p, e.currentTarget.value)} aria-label="Profile name" />
              <span class="faint small">{l.name} look · saved {date(p.created)}</span>
            </div>
            {#if app.ui.profiles.active === p.id}
              <span class="pill live"><span class="dot"></span>In use</span>
              <button class="ghost" onclick={() => update(p)} title="Save your current setup into this profile">Update</button>
            {:else}
              <button onclick={() => use(p)}>Switch</button>
            {/if}
            <button class="ghost danger icon" aria-label="Delete {p.name}" onclick={() => drop(p)}><Icon name="trash" size={15} /></button>
          </li>
        {/each}
      </ul>
    {/if}
    <div class="row add">
      <input class="grow" placeholder="Name this setup, e.g. Gaming" bind:value={newName} onkeydown={(e) => e.key === "Enter" && saveNew()} aria-label="New profile name" />
      <button disabled={!newName.trim()} onclick={saveNew}><Icon name="save" size={15} /> Save current setup</button>
    </div>
  </section>
{:else}
  {#if newer}
    <div class="newer">
      <Icon name="refresh" size={18} />
      <div class="grow">
        <strong>A newer setup is in your sync folder</strong>
        <p class="muted">From {newer.computer}, {date(newer.created)}. Restoring keeps a backup of what you have now.</p>
      </div>
      <button class="ghost" onclick={() => (newer = null)}>Not now</button>
      <button disabled={busy} onclick={takeNewer}>Use it</button>
    </div>
  {/if}

  <section class="section">
    <h3>Sync folder</h3>
    <p class="hint">Pick a folder that another app already syncs, like OneDrive, Dropbox or Google Drive. Arcadia keeps the latest copy of your setup there and offers it on your other PCs. Logins and API keys stay on each PC.</p>
    <div class="setting">
      <div>
        <div class="label">Folder</div>
        <div class="desc mono">{app.settings?.sync_folder || "Not set up"}</div>
      </div>
      <div class="row">
        {#if app.settings?.sync_folder}<button class="ghost" onclick={() => { if (app.settings) { app.settings.sync_folder = ""; saveSettings(); newer = null; } }}>Stop syncing</button>{/if}
        <button onclick={chooseFolder}><Icon name="folder" size={15} /> {app.settings?.sync_folder ? "Change" : "Choose folder"}</button>
      </div>
    </div>
    {#if app.settings?.sync_folder}
      <div class="setting">
        <div><div class="label">Last synced</div><div class="desc">{app.settings.sync_last ? date(app.settings.sync_last) : "Not yet"}. Changes sync on their own a few seconds after you make them.</div></div>
        <button disabled={busy} onclick={syncNow}><Icon name="refresh" size={15} /> Sync now</button>
      </div>
    {/if}
  </section>

  <section class="section">
    <h3>Backup</h3>
    <p class="hint">Save your setup to a single <span class="mono">.arcadia</span> file, to keep or to move to another PC.</p>
    <div class="parts">
      {#each parts as p (p.id)}
        <label class="part"><input type="checkbox" checked={chosen.includes(p.id)} onchange={(e) => (chosen = e.currentTarget.checked ? [...chosen, p.id] : chosen.filter((x) => x !== p.id))} /> {p.label}</label>
      {/each}
    </div>
    <div class="setting">
      <div><div class="label">Include API keys</div><div class="desc">CurseForge and SteamGridDB keys. Leave off if you'll share the file.</div></div>
      <Toggle label="Include API keys" bind:checked={keys} />
    </div>
    <div class="row actions">
      <button class="play" disabled={busy || !chosen.length} onclick={backup}><Icon name="download" size={16} /> Back up…</button>
      <button disabled={busy} onclick={pickRestore}><Icon name="upload" size={16} /> Restore from a file…</button>
    </div>
    <p class="faint small">Never included: Roblox and Minecraft logins (they stay in Windows Credential Manager), addon secrets, and downloaded games.</p>
  </section>

  <Modal bind:open={restoreOpen} title="Restore backup">
    {#if restoring}
      <p>From <strong>{restoring.m.computer}</strong>, {date(restoring.m.created)} · {restoring.m.files} files.</p>
      <div class="parts">
        {#each restoring.m.parts as id (id)}
          <label class="part"><input type="checkbox" checked={restoring.parts.includes(id)} onchange={(e) => (restoring!.parts = e.currentTarget.checked ? [...restoring!.parts, id] : restoring!.parts.filter((x) => x !== id))} /> {label(id)}</label>
        {/each}
      </div>
      <p class="faint small">What you have now is backed up first, so this can be undone.</p>
    {/if}
    {#snippet actions()}
      <button class="ghost" onclick={() => (restoreOpen = false)}>Cancel</button>
      <button disabled={busy || !restoring?.parts.length} onclick={restore}>Restore</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .profiles { list-style: none; margin: 0 0 0.8rem; padding: 0; display: flex; flex-direction: column; gap: 0.45rem; }
  .profiles li { display: flex; align-items: center; gap: 0.7rem; padding: 0.6rem 0.8rem; border-radius: var(--r-lg); background: var(--sunk); border: 1px solid var(--line-soft); }
  .profiles li.on { border-color: color-mix(in srgb, var(--peach) 55%, transparent); }
  .swatch { width: 38px; height: 38px; border-radius: 10px; border: 1px solid var(--line); display: grid; place-items: center; flex: none; }
  .swatch i { width: 14px; height: 14px; border-radius: 50%; }
  .txt { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .nm { background: transparent; border-color: transparent; padding: 0.1rem 0.3rem; margin-left: -0.3rem; font-weight: 600; }
  .nm:hover { border-color: var(--line-soft); }
  .small { font-size: 0.78rem; }
  .icon { padding: 0.35rem; }
  .add button, .row button, .actions button { display: inline-flex; align-items: center; gap: 0.35rem; }
  .newer { display: flex; align-items: center; gap: 0.9rem; padding: 0.9rem 1.1rem; margin-bottom: 1rem; border-radius: var(--r-lg); background: color-mix(in srgb, var(--lilac) 12%, var(--raised)); border: 1px solid color-mix(in srgb, var(--lilac) 35%, transparent); }
  .newer .muted { font-size: 0.85rem; }
  .grow { flex: 1; }
  .parts { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 0.4rem 1rem; margin: 0.4rem 0 0.8rem; }
  .part { display: inline-flex; gap: 0.5rem; align-items: center; cursor: pointer; color: var(--text); font-size: 0.92rem; }
  .actions { margin: 0.8rem 0 0.6rem; }
</style>
