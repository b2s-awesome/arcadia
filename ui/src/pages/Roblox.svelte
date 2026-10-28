<script lang="ts">
  import { onMount } from "svelte";
  import { api, pickFile } from "../lib/api";
  import { app, saveSettings, toast } from "../lib/state.svelte";
  import type { FlagValue, Preset, RobloxStatus } from "../lib/types";
  import Toggle from "../components/Toggle.svelte";
  import Modal from "../components/Modal.svelte";

  const s = app.settings!;
  let status = $state<RobloxStatus | null>(null);
  let latest: string | null = $state(null);
  let presets: Preset[] = $state([]);
  let launching = $state(false);

  onMount(async () => {
    [status, presets] = await Promise.all([api.robloxStatus(), api.fflagPresets()]);
    api.robloxLatest(s.roblox_channel).then((v) => (latest = v.clientVersionUpload)).catch(() => {});
  });

  const upToDate = $derived(latest && status?.installedVersion === latest);

  async function launch() {
    launching = true;
    try {
      await api.robloxLaunch(null);
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

  async function chooseFont() {
    const f = await pickFile([{ name: "Fonts", extensions: ["ttf", "otf"] }]);
    if (f) {
      s.roblox_custom_font = f;
      saveSettings();
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
            {#if upToDate}<span class="pill live"><span class="dot"></span>Up to date</span>
            {:else if latest}<span class="pill">Update on next launch</span>{/if}
          {:else}
            Not installed yet. Arcadia downloads Roblox the first time you play.
          {/if}
        </p>
      </div>
      <button class="play" disabled={launching} onclick={launch}>{launching ? "Starting…" : "Play Roblox"}</button>
    </header>

    {#if status && !status.protocolRegistered}
      <div class="notice">
        <p>Join buttons on roblox.com still open the official launcher.</p>
        <button onclick={registerProtocol}>Use Arcadia for join links</button>
      </div>
    {/if}

    <section class="section">
      <h3>Client</h3>
      <div class="setting">
        <div><div class="label">Update automatically</div><div class="desc">Install new Roblox versions as soon as they ship.</div></div>
        <Toggle label="Update automatically" bind:checked={s.roblox_auto_update} onchange={saveSettings} />
      </div>
      <div class="setting">
        <div><div class="label">Channel</div><div class="desc">Leave on <span class="mono">production</span> unless you're testing a Roblox beta channel.</div></div>
        <input class="mono" style="width: 14rem" bind:value={s.roblox_channel} oninput={saveSettings} aria-label="Channel" />
      </div>
      <div class="setting">
        <div><div class="label">Stay on a version</div><div class="desc">Paste a <span class="mono">version-…</span> hash to keep using it. Empty means latest.</div></div>
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
        <div><div class="label">Reinstall Roblox</div><div class="desc">Fixes a broken install. Your settings, FastFlags and mods are kept.</div></div>
        <button onclick={reinstall}>Reinstall</button>
      </div>
    </section>

    <section class="section">
      <h3>Performance</h3>
      <div class="setting">
        <div><div class="label">Frame rate cap</div><div class="desc">Roblox caps at 60 by default.</div></div>
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
      <div class="setting">
        <div><div class="label">Multiple Roblox windows</div><div class="desc">Run more than one client at the same time, for alt accounts.</div></div>
        <Toggle label="Multiple Roblox windows" bind:checked={s.roblox_multi_instance} onchange={saveSettings} />
      </div>
    </section>

    <section class="section">
      <div class="row spread">
        <h3>FastFlags</h3>
        <button class="ghost" onclick={() => (importing = true)}>Import JSON</button>
      </div>
      <p class="hint">Written to <span class="mono">ClientAppSettings.json</span> each launch. Roblox only honours flags on its allowlist this way; for anything else use live FastFlags on the Asset proxy page.</p>
      <div class="presets">
        {#each presets as p}
          <label class="preset">
            <Toggle label={p.label} checked={presetOn(p)} onchange={(v) => togglePreset(p, v)} />
            <span><strong>{p.label}</strong><br /><span class="muted">{p.description}</span></span>
          </label>
        {/each}
      </div>
      <table class="flags">
        <tbody>
          {#each Object.entries(s.fast_flags) as [k, v] (k)}
            <tr>
              <td class="mono">{k}</td>
              <td><input class="mono" value={String(v)} oninput={(e) => { s.fast_flags[k] = e.currentTarget.value; saveSettings(); }} aria-label="Value for {k}" /></td>
              <td><button class="ghost danger" onclick={() => removeFlag(k)} aria-label="Remove {k}">Remove</button></td>
            </tr>
          {/each}
          <tr>
            <td><input class="mono" placeholder="FFlagName" bind:value={newFlag} onkeydown={(e) => e.key === "Enter" && addFlag()} aria-label="New flag name" /></td>
            <td><input class="mono" placeholder="True" bind:value={newValue} onkeydown={(e) => e.key === "Enter" && addFlag()} aria-label="New flag value" /></td>
            <td><button onclick={addFlag} disabled={!newFlag.trim()}>Add flag</button></td>
          </tr>
        </tbody>
      </table>
    </section>

    <section class="section">
      <div class="row spread">
        <h3>Mods</h3>
        <button onclick={() => api.openFolder("mods")}>Open mods folder</button>
      </div>
      <p class="hint">Files in the mods folder replace Roblox's own when you launch, using the same layout. For example, <span class="mono">content\sounds\ouch.ogg</span> changes the death sound. Remove a file to undo it.</p>
      <div class="setting">
        <div class="label">Apply mods</div>
        <Toggle label="Apply mods" bind:checked={s.roblox_apply_mods} onchange={saveSettings} />
      </div>
      <div class="setting">
        <div><div class="label">Custom font</div><div class="desc mono">{s.roblox_custom_font ?? "Roblox default"}</div></div>
        <div class="row">
          {#if s.roblox_custom_font}<button class="ghost" onclick={() => { s.roblox_custom_font = null; saveSettings(); }}>Reset</button>{/if}
          <button onclick={chooseFont}>Choose font</button>
        </div>
      </div>
      <div class="setting">
        <div><div class="label">Clear asset cache</div><div class="desc">Makes Roblox download textures and sounds again.</div></div>
        <button onclick={clearCache}>Clear cache</button>
      </div>
    </section>
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
  .notice {
    display: flex; align-items: center; justify-content: space-between; gap: 1rem;
    padding: 0.9rem 1.1rem; border-radius: var(--r-md);
    background: color-mix(in srgb, var(--lilac) 12%, transparent); border: 1px solid color-mix(in srgb, var(--lilac) 30%, transparent);
    margin-bottom: 0.5rem;
  }
  .page-head p { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; }
  .presets { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 0.9rem 1.5rem; margin-bottom: 1.2rem; }
  .preset { display: flex; gap: 0.8rem; align-items: flex-start; font-size: 0.9rem; line-height: 1.35; }
  .flags { width: 100%; border-collapse: collapse; }
  .flags td { padding: 0.3rem 0.5rem 0.3rem 0; border-top: 1px solid var(--line-soft); }
  .flags td:first-child { width: 55%; word-break: break-all; }
  .flags input { width: 100%; }
  .flags td:last-child { width: 1%; white-space: nowrap; }
</style>
