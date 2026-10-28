<script lang="ts">
  import { onMount } from "svelte";
  import { api, pickFile } from "../lib/api";
  import { app, saveSettings, toast } from "../lib/state.svelte";
  import type { Action, ProxyStatus, Rule, RuleConfig } from "../lib/types";
  import Toggle from "../components/Toggle.svelte";
  import Modal from "../components/Modal.svelte";
  import Icon from "../components/Icon.svelte";

  const s = app.settings!;
  let status = $state<ProxyStatus | null>(null);
  let config: RuleConfig = $state({ profiles: [] });

  async function refreshStatus() {
    status = await api.proxyStatus();
  }
  onMount(() => {
    refreshStatus();
    api.rulesGet().then((c) => (config = c));
    const t = setInterval(refreshStatus, 2000);
    return () => clearInterval(t);
  });

  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  function persist() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => api.rulesSave($state.snapshot(config)).catch((e) => toast(String(e), "error")), 250);
  }

  // ── Rule editor ──
  type Kind = Action["type"];
  let editing: { profile: number; index: number | null } | null = $state(null);
  let editorOpen = $state(false);
  let fName = $state("");
  let fIds = $state("");
  let fKind = $state<Kind>("replace");
  let fWith = $state("");
  let fPath = $state("");
  let parsedIds: number[] = $state([]);
  $effect(() => {
    const text = fIds;
    api.parseIds(text).then((ids) => (parsedIds = ids));
  });

  function openEditor(profile: number, index: number | null) {
    const r = index === null ? null : config.profiles[profile].rules[index];
    fName = r?.name ?? "";
    fIds = r?.assetIds.join(", ") ?? "";
    fKind = r?.action.type ?? "replace";
    fWith = r?.action.type === "replace" ? String(r.action.withId) : "";
    fPath = r?.action.type === "file" ? r.action.path : "";
    editing = { profile, index };
    editorOpen = true;
  }
  async function chooseFile() {
    const p = await pickFile();
    if (p) fPath = await api.importFile(p);
  }
  const canSave = $derived(
    parsedIds.length > 0 && (fKind === "remove" || (fKind === "replace" && /^\d+$/.test(fWith.trim())) || (fKind === "file" && fPath)),
  );
  function saveRule() {
    if (!editing) return;
    const action: Action =
      fKind === "replace" ? { type: "replace", withId: Number(fWith.trim()) } : fKind === "file" ? { type: "file", path: fPath } : { type: "remove" };
    const list = config.profiles[editing.profile].rules;
    const name = fName.trim() || `${parsedIds.length} asset${parsedIds.length === 1 ? "" : "s"}`;
    if (editing.index === null) list.push({ id: crypto.randomUUID(), name, enabled: true, assetIds: parsedIds, action });
    else Object.assign(list[editing.index], { name, assetIds: parsedIds, action });
    editorOpen = false;
    persist();
  }
  function deleteRule(p: number, i: number) {
    config.profiles[p].rules.splice(i, 1);
    persist();
  }

  function describe(a: Action) {
    if (a.type === "replace") return `Swap for asset ${a.withId}`;
    if (a.type === "remove") return "Remove";
    return `Use ${a.path.split(/[\\/]/).pop()}`;
  }

  // ── Profiles ──
  let newProfile = $state("");
  function addProfile() {
    const name = newProfile.trim();
    if (!name || config.profiles.some((p) => p.name === name)) return;
    config.profiles.push({ name, enabled: true, rules: [] });
    newProfile = "";
    persist();
  }
  function deleteProfile(i: number) {
    config.profiles.splice(i, 1);
    persist();
  }
  let importOpen = $state(false);
  let importText = $state("");
  async function doImport() {
    try {
      config = await api.rulesImport(importText);
      importOpen = false;
      importText = "";
      toast("Profiles imported");
    } catch (e) {
      toast(`That file isn't an Arcadia profile: ${e}`, "error");
    }
  }
  async function exportProfile(name: string | null) {
    const json = await api.rulesExport(name);
    await navigator.clipboard.writeText(json);
    toast(name ? `Copied “${name}” to the clipboard` : "Copied all profiles to the clipboard");
  }

  // ── Live FastFlags ──
  let riskOpen = $state(false);
  let lfName = $state("");
  let lfValue = $state("");
  function addLiveFlag() {
    if (!lfName.trim()) return;
    s.proxy_fflag_overrides[lfName.trim()] = lfValue.trim();
    lfName = lfValue = "";
    saveSettings();
  }
  function enableLiveFlags(v: boolean) {
    if (v && !s.proxy_fflags_risk_accepted) {
      riskOpen = true;
      return;
    }
    s.proxy_fflags_risk_accepted = v;
    saveSettings();
  }
</script>

<div class="page">
  <div class="page-inner">
    <header class="page-head">
      <div>
        <h2>Asset proxy</h2>
        <p>Swap or remove textures, sounds, meshes and other assets before they reach Roblox. Only you see the changes.</p>
      </div>
      <div class="row">
        {#if status?.running}
          <span class="pill live"><span class="dot"></span>Running on port {status.port}</span>
          <button class="ghost" onclick={async () => { await api.proxyStop(); refreshStatus(); }}>Stop</button>
        {:else}
          <span class="pill">Stopped</span>
          <button onclick={async () => { try { await api.proxyStart(); refreshStatus(); } catch (e) { toast(String(e), "error"); } }}>Start now</button>
        {/if}
      </div>
    </header>

    {#if status?.stats}
      <dl class="stats">
        <div><dt>Assets swapped</dt><dd>{status.stats.replaced}</dd></div>
        <div><dt>Assets removed</dt><dd>{status.stats.removed}</dd></div>
        <div><dt>Files served</dt><dd>{status.stats.servedFiles}</dd></div>
        <div><dt>Requests seen</dt><dd>{status.stats.requests}</dd></div>
      </dl>
    {/if}

    <section class="section">
      <div class="setting">
        <div><div class="label">Use the proxy when Roblox starts</div><div class="desc">Roblox launched from Arcadia routes asset requests through it. Nothing else on your PC is affected.</div></div>
        <Toggle label="Use the proxy when Roblox starts" bind:checked={s.proxy_enabled} onchange={saveSettings} />
      </div>
      <div class="setting">
        <div><div class="label">Clear cache on launch and exit</div><div class="desc">Needed for changed rules to take effect; Roblox otherwise reuses assets it already downloaded.</div></div>
        <Toggle
          label="Clear cache on launch and exit"
          checked={s.proxy_clear_cache_on_launch && s.proxy_clear_cache_on_exit}
          onchange={(v) => { s.proxy_clear_cache_on_launch = s.proxy_clear_cache_on_exit = v; saveSettings(); }}
        />
      </div>
    </section>

    <section class="section">
      <div class="row spread">
        <h3>Replacement profiles</h3>
        <div class="row">
          <button class="ghost" onclick={() => (importOpen = true)}><Icon name="download" size={16} /> Import</button>
          <button class="ghost" onclick={() => exportProfile(null)}><Icon name="upload" size={16} /> Export all</button>
        </div>
      </div>
      <p class="hint">Turn whole profiles on and off, or single rules within them. When two rules target the same asset, the lower profile wins.</p>

      {#each config.profiles as profile, pi (profile.name)}
        <div class="profile" class:off={!profile.enabled}>
          <div class="profile-head">
            <Toggle label="Enable {profile.name}" bind:checked={profile.enabled} onchange={persist} />
            <h4>{profile.name}</h4>
            <span class="faint">{profile.rules.length} rule{profile.rules.length === 1 ? "" : "s"}</span>
            <div class="grow"></div>
            <button class="ghost" onclick={() => exportProfile(profile.name)}>Share</button>
            <button class="ghost danger" onclick={() => deleteProfile(pi)} aria-label="Delete profile {profile.name}"><Icon name="trash" size={16} /></button>
            <button onclick={() => openEditor(pi, null)}><Icon name="plus" size={16} /> Add rule</button>
          </div>
          {#if profile.rules.length}
            <ul class="rules">
              {#each profile.rules as rule, ri (rule.id)}
                <li class:off={!rule.enabled}>
                  <Toggle label="Enable {rule.name}" bind:checked={rule.enabled} onchange={persist} />
                  <button class="rule-body" onclick={() => openEditor(pi, ri)}>
                    <span class="rule-name">{rule.name}</span>
                    <span class="mono faint ids">{rule.assetIds.join(", ")}</span>
                  </button>
                  <span class="act act-{rule.action.type}">{describe(rule.action)}</span>
                  <button class="ghost danger" onclick={() => deleteRule(pi, ri)} aria-label="Delete rule {rule.name}"><Icon name="trash" size={16} /></button>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="none">No rules yet. Add one with the asset IDs you want to change.</p>
          {/if}
        </div>
      {/each}
      <div class="row new-profile">
        <input placeholder="New profile name" bind:value={newProfile} onkeydown={(e) => e.key === "Enter" && addProfile()} aria-label="New profile name" />
        <button onclick={addProfile} disabled={!newProfile.trim()}>Create profile</button>
      </div>
    </section>

    <section class="section">
      <div class="row spread">
        <h3>Live FastFlags</h3>
        <Toggle label="Enable live FastFlags" checked={s.proxy_fflags_risk_accepted} onchange={enableLiveFlags} />
      </div>
      <p class="hint">Delivered through the proxy while Roblox runs, so they skip Roblox's local allowlist. Roblox can ban accounts that use them.</p>
      {#if s.proxy_fflags_risk_accepted}
        <table class="flags">
          <tbody>
            {#each Object.entries(s.proxy_fflag_overrides) as [k, v] (k)}
              <tr>
                <td class="mono">{k}</td>
                <td class="mono">{String(v)}</td>
                <td><button class="ghost danger" onclick={() => { delete s.proxy_fflag_overrides[k]; saveSettings(); }}>Remove</button></td>
              </tr>
            {/each}
            <tr>
              <td><input class="mono" placeholder="DFIntSomething" bind:value={lfName} aria-label="Flag name" /></td>
              <td><input class="mono" placeholder="Value" bind:value={lfValue} onkeydown={(e) => e.key === "Enter" && addLiveFlag()} aria-label="Flag value" /></td>
              <td><button onclick={addLiveFlag} disabled={!lfName.trim()}>Add flag</button></td>
            </tr>
          </tbody>
        </table>
      {/if}
    </section>
  </div>
</div>

<Modal bind:open={editorOpen} title={editing?.index === null ? "Add rule" : "Edit rule"}>
  <label class="field">Name <input bind:value={fName} placeholder="Night sky" /></label>
  <label class="field">
    Asset IDs
    <textarea rows="3" bind:value={fIds} placeholder="Paste IDs or catalog links, separated however you like"></textarea>
    <span class="faint small">{parsedIds.length} ID{parsedIds.length === 1 ? "" : "s"} found</span>
  </label>
  <div class="seg full" role="radiogroup" aria-label="What to do">
    <button role="radio" aria-checked={fKind === "replace"} onclick={() => (fKind = "replace")}>Swap for another asset</button>
    <button role="radio" aria-checked={fKind === "file"} onclick={() => (fKind = "file")}>Use a file</button>
    <button role="radio" aria-checked={fKind === "remove"} onclick={() => (fKind = "remove")}>Remove</button>
  </div>
  {#if fKind === "replace"}
    <label class="field">Replacement asset ID <input class="mono" bind:value={fWith} placeholder="13541203" /></label>
  {:else if fKind === "file"}
    <div class="row">
      <span class="mono grow small">{fPath || "No file chosen"}</span>
      <button onclick={chooseFile}>Choose file</button>
    </div>
  {:else}
    <p>These assets are stripped from Roblox's requests and never load.</p>
  {/if}
  {#snippet actions()}
    <button class="ghost" onclick={() => (editorOpen = false)}>Cancel</button>
    <button disabled={!canSave} onclick={saveRule}>Save rule</button>
  {/snippet}
</Modal>

<Modal bind:open={importOpen} title="Import profiles">
  <p>Paste a profile someone shared with you.</p>
  <textarea rows="9" bind:value={importText}></textarea>
  {#snippet actions()}
    <button class="ghost" onclick={() => (importOpen = false)}>Cancel</button>
    <button disabled={!importText.trim()} onclick={doImport}>Import</button>
  {/snippet}
</Modal>

<Modal bind:open={riskOpen} title="Turn on live FastFlags?">
  <p>These flags skip the allowlist Roblox enforces for local FastFlags. Roblox has said accounts using them can be banned.</p>
  <p>Only turn this on for accounts you're willing to risk.</p>
  {#snippet actions()}
    <button class="ghost" onclick={() => (riskOpen = false)}>Keep it off</button>
    <button class="danger" onclick={() => { s.proxy_fflags_risk_accepted = true; saveSettings(); riskOpen = false; }}>Turn on anyway</button>
  {/snippet}
</Modal>

<style>
  .stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 1rem; margin: 0 0 0.5rem; }
  .stats div { padding: 0.2rem 0; }
  .stats dt { color: var(--muted); font-size: 0.85rem; }
  .stats dd { margin: 0; font-family: var(--display); font-size: 2rem; font-weight: 700; font-variant-numeric: tabular-nums; }

  .profile { border: 1px solid var(--line-soft); border-radius: var(--r-lg); padding: 0.9rem 1rem; margin-bottom: 0.9rem; background: color-mix(in srgb, var(--raised) 55%, transparent); }
  .profile.off { opacity: 0.6; }
  .profile-head { display: flex; align-items: center; gap: 0.7rem; }
  h4 { margin: 0; font-family: var(--display); font-size: 1.05rem; }
  .rules { list-style: none; margin: 0.8rem 0 0; padding: 0; }
  .rules li { display: flex; align-items: center; gap: 0.8rem; padding: 0.5rem 0; border-top: 1px solid var(--line-soft); }
  .rules li.off .rule-body, .rules li.off .act { opacity: 0.45; }
  .rule-body { all: unset; cursor: pointer; flex: 1; min-width: 0; display: grid; }
  .rule-body:focus-visible { outline: 2px solid var(--lilac); border-radius: 4px; }
  .rule-name { font-weight: 600; }
  .ids { font-size: 0.78rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .act { font-size: 0.85rem; font-weight: 600; white-space: nowrap; }
  .act-replace { color: var(--lilac); }
  .act-file { color: var(--mint); }
  .act-remove { color: var(--rose); }
  .none { color: var(--faint); font-size: 0.9rem; margin-top: 0.6rem; }
  .new-profile input { width: 16rem; }

  .flags { width: 100%; border-collapse: collapse; }
  .flags td { padding: 0.3rem 0.5rem 0.3rem 0; border-top: 1px solid var(--line-soft); }
  .flags input { width: 100%; }
  .flags td:last-child { width: 1%; white-space: nowrap; }

  .seg.full { display: flex; }
  .seg.full button { flex: 1; }
  .field { display: grid; gap: 0.35rem; color: var(--text); font-weight: 600; font-size: 0.9rem; }
  .small { font-size: 0.8rem; font-weight: 400; }
</style>
