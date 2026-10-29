<script lang="ts" module>
  // The open tab, kept while switching pages.
  let lastTab = "play";
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import TreeTabs, { type TreeNode } from "../components/TreeTabs.svelte";
  import { api } from "../lib/api";
  import { app, saveSettings, toast } from "../lib/state.svelte";
  import type { FlagValue, Preset, RobloxStatus } from "../lib/types";
  import Toggle from "../components/Toggle.svelte";
  import Modal from "../components/Modal.svelte";
  import Icon from "../components/Icon.svelte";
  import PlayPanel from "./roblox/PlayPanel.svelte";
  import AccountsPanel from "./roblox/AccountsPanel.svelte";
  import GamesPanel from "./roblox/GamesPanel.svelte";
  import ServersPanel from "./roblox/ServersPanel.svelte";
  import ModStudio from "./roblox/ModStudio.svelte";
  import Thumb from "./roblox/Thumb.svelte";
  import { rbx, startRbx, effectiveAccount, accountById } from "./roblox/rbx.svelte";

  const s = app.settings!;
  let status = $state<RobloxStatus | null>(null);
  let latest: string | null = $state(null);
  let presets: Preset[] = $state([]);
  let launching = $state(false);

  let allow = $state<Set<string>>(new Set());
  onMount(async () => {
    startRbx();
    api.robloxFlagAllowlist().then((l) => (allow = new Set(l.map((x) => x.toLowerCase()))));
    [status, presets] = await Promise.all([api.robloxStatus(), api.fflagPresets()]);
    api.robloxLatest(s.roblox_channel).then((v) => (latest = v.clientVersionUpload)).catch(() => {});
  });

  const upToDate = $derived(latest && status?.installedVersion === latest);

  let tab = $state(lastTab);
  $effect(() => {
    lastTab = tab;
  });
  const flagCount = $derived(Object.keys(s.fast_flags).length);
  const profileCount = $derived(Object.keys(s.roblox_flag_profiles).length);
  const tree: TreeNode[] = $derived([
    { id: "play", label: "Play", icon: "play" },
    { id: "accounts", label: "Accounts", icon: "users", badge: rbx.accounts.list.length || "" },
    { id: "games", label: "Games", icon: "gamepad", children: [
      { id: "games-library", label: "Library", badge: rbx.history.games.length || "" },
      { id: "games-visits", label: "Recent visits" },
    ] },
    { id: "servers", label: "Servers", icon: "globe" },
    { id: "client", label: "Client & updates", icon: "roblox" },
    { id: "performance", label: "Performance", icon: "sparkle" },
    { id: "sessions", label: "Rejoin & windows", icon: "refresh", badge: s.roblox_auto_rejoin ? "on" : "" },
    { id: "fflags", label: "FastFlags", icon: "sliders", badge: flagCount || "", children: [
      { id: "fflags-presets", label: "Presets", badge: presets.filter(presetOn).length || "" },
      { id: "fflags-profiles", label: "Profiles", badge: s.roblox_flag_profile || profileCount || "" },
      { id: "fflags-mine", label: "Your flags", badge: flagCount || "" },
    ] },
    { id: "mods", label: "Mods", icon: "brush", children: [
      { id: "mods-studio", label: "Mod studio" },
      { id: "mods-files", label: "Files & cache" },
    ] },
  ]);
  const show = (id: string) => tab === id || id.startsWith(tab + "-");

  async function launch() {
    launching = true;
    try {
      await api.robloxPlay(rbx.picked);
      status = await api.robloxStatus();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      launching = false;
    }
  }

  // FPS cap: 0 means "leave Roblox's default".
  const fpsChoices = [0, 60, 144, 165, 240, 360, 9999];
  const fpsLabel = (n: number) => (n === 0 ? "Default" : n === 9999 ? "Unlimited" : `${n}`);

  // FastFlags
  function presetOn(p: Preset) {
    return Object.entries(p.flags).every(([k, v]) => String(s.fast_flags[k]) === String(v));
  }
  function togglePreset(p: Preset, on: boolean) {
    for (const [k, v] of Object.entries(p.flags)) {
      if (on) s.fast_flags[k] = v;
      else delete s.fast_flags[k];
    }
    // The three renderer presets are mutually exclusive.
    if (on && ["dx11", "vulkan", "opengl"].includes(p.id)) {
      for (const other of presets.filter((o) => o.id !== p.id && ["dx11", "vulkan", "opengl"].includes(o.id))) {
        for (const k of Object.keys(other.flags)) delete s.fast_flags[k];
      }
    }
    saveSettings();
  }
  let newFlag = $state("");
  let newValue = $state("");
  function addFlag() {
    if (!newFlag.trim()) return;
    s.fast_flags[newFlag.trim()] = newValue.trim() as FlagValue;
    newFlag = newValue = "";
    saveSettings();
  }
  function removeFlag(k: string) {
    delete s.fast_flags[k];
    saveSettings();
  }
  const ignored = (k: string) => allow.size > 0 && !allow.has(k.toLowerCase());
  const ignoredCount = $derived(Object.keys(s.fast_flags).filter(ignored).length);

  // Profiles: saved sets of flags. Using one copies it into "Your flags".
  let profileName = $state("");
  const sameFlags = (a: Record<string, unknown>, b: Record<string, unknown>) =>
    JSON.stringify(Object.entries(a).sort()) === JSON.stringify(Object.entries(b).sort());
  function saveProfile(name = profileName.trim()) {
    if (!name) return;
    s.roblox_flag_profiles[name] = { ...$state.snapshot(s.fast_flags) };
    s.roblox_flag_profile = name;
    profileName = "";
    saveSettings();
    toast(`Saved profile “${name}”`);
  }
  function useProfile(name: string) {
    s.fast_flags = { ...$state.snapshot(s.roblox_flag_profiles[name]) };
    s.roblox_flag_profile = name;
    saveSettings();
    toast(`Using “${name}”: ${Object.keys(s.fast_flags).length} flags`);
  }
  function deleteProfile(name: string) {
    delete s.roblox_flag_profiles[name];
    if (s.roblox_flag_profile === name) s.roblox_flag_profile = "";
    saveSettings();
  }
  async function copyProfile(name: string) {
    await navigator.clipboard.writeText(JSON.stringify(s.roblox_flag_profiles[name], null, 2));
    toast("Copied as JSON");
  }
  const profileChanged = $derived(!!s.roblox_flag_profile && !!s.roblox_flag_profiles[s.roblox_flag_profile] && !sameFlags(s.fast_flags, s.roblox_flag_profiles[s.roblox_flag_profile]));

  const rejoinDelays = [5, 10, 20, 30, 60];
  const acct = $derived(accountById(effectiveAccount()));

  let importing = $state(false);
  let importText = $state("");
  async function importFlags() {
    try {
      const m = await api.parseFflags(importText);
      Object.assign(s.fast_flags, m);
      saveSettings();
      toast(`Imported ${Object.keys(m).length} FastFlags`);
      importing = false;
      importText = "";
    } catch (e) {
      toast(`That isn't a JSON object of flags: ${e}`, "error");
    }
  }

  async function reinstall() {
    await api.robloxReinstall();
    status = await api.robloxStatus();
    toast("Roblox will be downloaded fresh on the next launch");
  }
  async function clearCache() {
    const r = await api.clearCache(false);
    toast(r.locked ? `Cleared ${r.removed}; ${r.locked} in use (close Roblox and try again)` : `Cleared ${r.removed} cache items`);
  }
  async function registerProtocol() {
    await api.registerProtocol();
    status = await api.robloxStatus();
    toast("Join links from roblox.com now open through Arcadia");
  }
</script>

<div class="page">
  <div class="page-inner">
    <header class="page-head">
      <div>
        <h2>Roblox</h2>
        <p>
          {#if status?.installedVersion}
            <span class="mono">{status.installedVersion}</span> on {status.channel ?? "production"}
            {#if s.roblox_pinned_version}<span class="pill">Updates paused</span>
            {:else if upToDate}<span class="pill live"><span class="dot"></span>Up to date</span>
            {:else if latest}<span class="pill">Update on next launch</span>{/if}
          {:else}
            Not installed yet. Arcadia downloads Roblox the first time you play.
          {/if}
        </p>
      </div>
      <div class="cta">
        {#if acct}
          <button class="as" title="Change on the Play tab" onclick={() => (tab = "play")}>
            <Thumb src={acct.avatar} seed={acct.id} label={acct.displayName} size={26} round />
            <span><span class="faint">Playing as</span> {acct.displayName}</span>
          </button>
        {/if}
        <button class="play" disabled={launching} onclick={launch}>{launching ? "Starting…" : "Play Roblox"}</button>
      </div>
    </header>

    {#if status && !status.protocolRegistered}
      <div class="notice">
        <p>Join buttons on roblox.com still open the official launcher.</p>
        <button onclick={registerProtocol}>Use Arcadia for join links</button>
      </div>
    {/if}

    <div class="split">
    <TreeTabs key="roblox" label="Roblox sections" nodes={tree} bind:selected={tab} />
    <div class="panes">
    {#if tab === "play"}<PlayPanel go={(t) => (tab = t)} />{/if}
    {#if tab === "accounts"}<AccountsPanel />{/if}
    {#if show("games-library")}<GamesPanel view="library" go={(t) => (tab = t)} />{/if}
    {#if tab === "games-visits"}<GamesPanel view="visits" go={(t) => (tab = t)} />{/if}
    {#if tab === "servers"}<ServersPanel />{/if}

    {#if show("client")}
    <section class="section">
      <h3>Client & updates</h3>
      <div class="setting">
        <div><div class="label">Update automatically</div><div class="desc">Install new Roblox versions as soon as they ship.</div></div>
        <Toggle label="Update automatically" bind:checked={s.roblox_auto_update} onchange={saveSettings} />
      </div>
      <div class="setting">
        <div>
          <div class="label">Pause updates</div>
          <div class="desc">Stay on the version you have now, for example while a mod or FastFlag is broken on the newest one.</div>
        </div>
        {#if s.roblox_pinned_version}
          <button onclick={() => { s.roblox_pinned_version = null; saveSettings(); }}>Resume updates</button>
        {:else}
          <button disabled={!status?.installedVersion} onclick={() => { s.roblox_pinned_version = status?.installedVersion ?? null; saveSettings(); toast("Roblox will stay on this version"); }}>Stay on this version</button>
        {/if}
      </div>
      <div class="setting">
        <div><div class="label">Stay on a version</div><div class="desc">Or paste any <span class="mono">version-…</span> hash. Empty means latest.</div></div>
        <input
          class="mono"
          style="width: 14rem"
          placeholder="latest"
          value={s.roblox_pinned_version ?? ""}
          oninput={(e) => { s.roblox_pinned_version = e.currentTarget.value.trim() || null; saveSettings(); }}
          aria-label="Pinned version"
        />
      </div>
      <div class="setting">
        <div><div class="label">Channel</div><div class="desc">Leave on <span class="mono">production</span> unless you've been invited to a Roblox test channel.</div></div>
        <div class="row">
          <select value={["production", "zflag", "zcanary", "zintegration"].includes(s.roblox_channel.toLowerCase()) ? s.roblox_channel.toLowerCase() : "other"}
            onchange={(e) => { const v = e.currentTarget.value; if (v !== "other") { s.roblox_channel = v; saveSettings(); api.robloxLatest(v).then((x) => (latest = x.clientVersionUpload)).catch(() => (latest = null)); } }} aria-label="Channel">
            <option value="production">production (live)</option>
            <option value="zflag">zflag</option>
            <option value="zcanary">zcanary</option>
            <option value="zintegration">zintegration</option>
            <option value="other">Other…</option>
          </select>
          <input class="mono" style="width: 10rem" bind:value={s.roblox_channel} oninput={saveSettings} aria-label="Channel name" />
        </div>
      </div>
      <div class="setting">
        <div><div class="label">Reinstall Roblox</div><div class="desc">Fixes a broken install. Your settings, FastFlags and mods are kept.</div></div>
        <button onclick={reinstall}>Reinstall</button>
      </div>
    </section>
    {/if}

    {#if show("performance")}
    <section class="section">
      <h3>Performance</h3>
      <div class="setting">
        <div>
          <div class="label">Frame rate cap</div>
          <div class="desc">
            {#if s.proxy_fflags_risk_accepted}
              Sent to Roblox as a server flag at launch. Roblox caps at 60 by default.
            {:else}
              Roblox no longer accepts this flag locally, so it applies only with live FastFlags turned on (Asset proxy page). Roblox's own in-game “Maximum Frame Rate” setting needs nothing.
            {/if}
          </div>
        </div>
        <div class="seg" role="radiogroup" aria-label="Frame rate cap">
          {#each fpsChoices as n}
            <button role="radio" aria-checked={s.roblox_fps_cap === n} onclick={() => { s.roblox_fps_cap = n; saveSettings(); }}>{fpsLabel(n)}</button>
          {/each}
        </div>
      </div>
      <div class="setting">
        <div><div class="label">Process priority</div><div class="desc">Higher priority can smooth out stutters when other apps are busy.</div></div>
        <select bind:value={s.roblox_priority} onchange={saveSettings} aria-label="Process priority">
          <option value="BelowNormal">Below normal</option>
          <option value="Normal">Normal</option>
          <option value="AboveNormal">Above normal</option>
          <option value="High">High</option>
        </select>
      </div>
    </section>
    {/if}

    {#if show("sessions")}
    <section class="section">
      <h3>Rejoin & windows</h3>
      <div class="setting">
        <div>
          <div class="label">Rejoin when disconnected</div>
          <div class="desc">After an idle kick or a lost connection, Arcadia closes that Roblox window and joins again. You get a countdown to cancel, and it stops after five rejoins in half an hour.</div>
        </div>
        <Toggle label="Rejoin when disconnected" bind:checked={s.roblox_auto_rejoin} onchange={saveSettings} />
      </div>
      {#if s.roblox_auto_rejoin}
        <div class="setting">
          <div><div class="label">Wait before rejoining</div><div class="desc">Time to cancel, or to read why you were kicked.</div></div>
          <div class="seg" role="radiogroup" aria-label="Wait before rejoining">
            {#each rejoinDelays as d}
              <button role="radio" aria-checked={s.roblox_rejoin_delay === d} onclick={() => { s.roblox_rejoin_delay = d; saveSettings(); }}>{d}s</button>
            {/each}
          </div>
        </div>
        <div class="setting">
          <div><div class="label">Same server</div><div class="desc">Go back to the server you were in. Off joins any server of that game.</div></div>
          <Toggle label="Same server" bind:checked={s.roblox_rejoin_same_server} onchange={saveSettings} />
        </div>
      {/if}
      <div class="setting">
        <div><div class="label">Multiple Roblox windows</div><div class="desc">Run more than one client at the same time, for alt accounts.</div></div>
        <Toggle label="Multiple Roblox windows" bind:checked={s.roblox_multi_instance} onchange={saveSettings} />
      </div>
    </section>
    {/if}

    {#if tab === "fflags" || tab === "fflags-presets"}
    <section class="section">
      <div class="row spread">
        <h3>FastFlag presets</h3>
        <button class="ghost" onclick={() => (importing = true)}>Import JSON</button>
      </div>
      <p class="hint">Every preset uses flags on Roblox's allowlist, so they work without live FastFlags.</p>
      <div class="presets">
        {#each presets as p}
          <label class="preset">
            <Toggle label={p.label} checked={presetOn(p)} onchange={(v) => togglePreset(p, v)} />
            <span><strong>{p.label}</strong><br /><span class="muted">{p.description}</span></span>
          </label>
        {/each}
      </div>
    </section>
    {/if}

    {#if tab === "fflags-profiles"}
    <section class="section">
      <h3>FastFlag profiles</h3>
      <p class="hint">Save your current flags as a profile, then swap between them, for example a light one for big games and a pretty one for showcases.</p>
      {#if Object.keys(s.roblox_flag_profiles).length}
        <ul class="profiles">
          {#each Object.entries(s.roblox_flag_profiles) as [name, flags] (name)}
            <li class:on={s.roblox_flag_profile === name}>
              <span class="radio" aria-hidden="true"></span>
              <div class="grow">
                <strong>{name}</strong>
                <div class="faint small">{Object.keys(flags).length} flag{Object.keys(flags).length === 1 ? "" : "s"}{#if s.roblox_flag_profile === name} · in use{#if profileChanged}, with unsaved changes{/if}{/if}</div>
              </div>
              {#if s.roblox_flag_profile === name && profileChanged}<button onclick={() => saveProfile(name)}>Update</button>{/if}
              {#if s.roblox_flag_profile !== name}<button onclick={() => useProfile(name)}>Use</button>{/if}
              <button class="ghost" aria-label="Copy {name} as JSON" title="Copy as JSON" onclick={() => copyProfile(name)}><Icon name="copy" size={15} /></button>
              <button class="ghost danger" aria-label="Delete {name}" onclick={() => deleteProfile(name)}><Icon name="trash" size={15} /></button>
            </li>
          {/each}
        </ul>
      {/if}
      <div class="row new-profile">
        <input placeholder="Profile name" bind:value={profileName} onkeydown={(e) => e.key === "Enter" && saveProfile()} aria-label="New profile name" />
        <button disabled={!profileName.trim()} onclick={() => saveProfile()}>Save current flags ({flagCount})</button>
      </div>
    </section>
    {/if}

    {#if tab === "fflags-mine"}
    <section class="section">
      <div class="row spread">
        <h3>Your FastFlags</h3>
        <button class="ghost" onclick={() => (importing = true)}>Import JSON</button>
      </div>
      <p class="hint">Written to <span class="mono">ClientAppSettings.json</span> each launch.</p>
      {#if ignoredCount}
        <div class="warnbox">
          <strong>{ignoredCount} flag{ignoredCount === 1 ? " isn't" : "s aren't"} on Roblox's allowlist.</strong>
          Roblox ignores {ignoredCount === 1 ? "it" : "them"} in this file. Use live FastFlags on the Asset proxy page instead, which Roblox warns can lead to a ban.
        </div>
      {/if}
      <table class="flags">
        <tbody>
          {#each Object.entries(s.fast_flags) as [k, v] (k)}
            <tr>
              <td class="mono">{k}{#if ignored(k)} <span class="pill warn" title="Not on Roblox's allowlist">ignored</span>{/if}</td>
              <td><input class="mono" value={String(v)} oninput={(e) => { s.fast_flags[k] = e.currentTarget.value; saveSettings(); }} aria-label="Value for {k}" /></td>
              <td><button class="ghost danger" onclick={() => removeFlag(k)} aria-label="Remove {k}">Remove</button></td>
            </tr>
          {/each}
          <tr>
            <td><input class="mono" placeholder="FFlagName" bind:value={newFlag} onkeydown={(e) => e.key === "Enter" && addFlag()} aria-label="New flag name" list="allowlist" /></td>
            <td><input class="mono" placeholder="True" bind:value={newValue} onkeydown={(e) => e.key === "Enter" && addFlag()} aria-label="New flag value" /></td>
            <td><button onclick={addFlag} disabled={!newFlag.trim()}>Add flag</button></td>
          </tr>
        </tbody>
      </table>
      <datalist id="allowlist">{#each [...allow] as a}<option value={a}></option>{/each}</datalist>
    </section>
    {/if}

    {#if tab === "mods" || tab === "mods-studio"}<ModStudio />{/if}

    {#if tab === "mods-files"}
    <section class="section">
      <div class="row spread">
        <h3>Files & cache</h3>
        <button onclick={() => api.openFolder("mods")}>Open mods folder</button>
      </div>
      <p class="hint">Files in the mods folder replace Roblox's own when you launch, using the same layout. For example, <span class="mono">content\sounds\ouch.ogg</span> changes the death sound. Remove a file to undo it. Mod studio choices are applied on top.</p>
      <div class="setting">
        <div class="label">Apply the mods folder</div>
        <Toggle label="Apply mods" bind:checked={s.roblox_apply_mods} onchange={saveSettings} />
      </div>
      <div class="setting">
        <div><div class="label">Clear asset cache</div><div class="desc">Makes Roblox download textures and sounds again.</div></div>
        <button onclick={clearCache}>Clear cache</button>
      </div>
    </section>
    {/if}
    </div>
    </div>
  </div>
</div>

<Modal bind:open={importing} title="Import FastFlags">
  <p>Paste a JSON object from another bootstrapper. Flags with the same name are overwritten.</p>
  <textarea rows="9" bind:value={importText} placeholder={'{\n  "DFIntTaskSchedulerTargetFps": "240"\n}'}></textarea>
  {#snippet actions()}
    <button class="ghost" onclick={() => (importing = false)}>Cancel</button>
    <button disabled={!importText.trim()} onclick={importFlags}>Import flags</button>
  {/snippet}
</Modal>

<style>
  .split { display: grid; grid-template-columns: 210px minmax(0, 1fr); gap: 1.6rem; align-items: start; }
  .panes { min-width: 0; display: flex; flex-direction: column; gap: 1rem; }
  .panes .section { margin-top: 0; }
  @media (max-width: 860px) { .split { grid-template-columns: 1fr; } }
  .notice {
    display: flex; align-items: center; justify-content: space-between; gap: 1rem;
    padding: 0.9rem 1.1rem; border-radius: var(--r-md);
    background: color-mix(in srgb, var(--lilac) 12%, transparent); border: 1px solid color-mix(in srgb, var(--lilac) 30%, transparent);
    margin-bottom: 0.5rem;
  }
  .page-head p { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; }
  .cta { display: flex; align-items: center; gap: 0.8rem; }
  .as { display: inline-flex; align-items: center; gap: 0.5rem; padding: 0.3rem 0.8rem 0.3rem 0.3rem; border-radius: 999px; font-weight: 500; background: var(--sunk); }
  .profiles { list-style: none; margin: 0 0 1rem; padding: 0; display: flex; flex-direction: column; gap: 0.45rem; }
  .profiles li { display: flex; align-items: center; gap: 0.6rem; padding: 0.6rem 0.8rem; border-radius: var(--r-md); background: var(--sunk); border: 1px solid var(--line-soft); }
  .profiles li.on { border-color: color-mix(in srgb, var(--peach) 55%, transparent); }
  .profiles button { display: inline-flex; align-items: center; }
  .radio { width: 14px; height: 14px; border-radius: 50%; border: 2px solid var(--line); flex: none; }
  .on .radio { border-color: var(--peach); background: radial-gradient(circle, var(--peach) 40%, transparent 45%); }
  .small { font-size: 0.8rem; }
  .new-profile input { width: 14rem; }
  .warnbox { padding: 0.7rem 0.9rem; margin-bottom: 0.8rem; border-radius: var(--r-md); font-size: 0.9rem; color: var(--muted);
    background: color-mix(in srgb, var(--rose) 9%, transparent); border: 1px solid color-mix(in srgb, var(--rose) 28%, transparent); }
  .warnbox strong { color: var(--text); }
  .flags .pill { font-size: 0.7rem; vertical-align: 1px; margin-left: 0.4rem; }
  .presets { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 0.9rem 1.5rem; margin-bottom: 1.2rem; }
  .preset { display: flex; gap: 0.8rem; align-items: flex-start; font-size: 0.9rem; line-height: 1.35; }
  .flags { width: 100%; border-collapse: collapse; }
  .flags td { padding: 0.3rem 0.5rem 0.3rem 0; border-top: 1px solid var(--line-soft); }
  .flags td:first-child { width: 55%; word-break: break-all; }
  .flags input { width: 100%; }
  .flags td:last-child { width: 1%; white-space: nowrap; }
</style>
