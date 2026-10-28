<script lang="ts">
  import { mc, openUrl } from "../lib/api";
  import { app, saveSettings, toast } from "../lib/state.svelte";
  import { mcs, refreshAccounts } from "../lib/mc.svelte";
  import type { DeviceCode } from "../lib/types";
  import Icon from "../components/Icon.svelte";

  const s = app.settings!;
  let code: DeviceCode | null = $state(null);
  let waiting = $state(false);
  let broken = $state(new Set<string>());
  let shortcutName = $state(`${(app.ui.brand.name || "Arcadia").trim()} Minecraft`);

  async function signIn() {
    try {
      code = await mc.loginStart();
      waiting = true;
      const acc = await mc.loginFinish(code);
      await refreshAccounts();
      toast(`Signed in as ${acc.name}`);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      waiting = false;
      code = null;
    }
  }
  async function copyCode() {
    if (code) await navigator.clipboard.writeText(code.userCode);
    toast("Code copied");
  }
  async function select(uuid: string) {
    await mc.selectAccount(uuid);
    await refreshAccounts();
  }
  async function remove(uuid: string) {
    await mc.removeAccount(uuid);
    await refreshAccounts();
  }
  async function shortcut() {
    try {
      const made = await mc.createShortcut(shortcutName);
      toast(`Shortcut added to ${made.length === 2 ? "the desktop and Start menu" : "the desktop"}`);
    } catch (e) {
      toast(String(e), "error");
    }
  }
</script>

<div class="page">
  <div class="page-inner">
    <section class="section">
      <div class="row spread">
        <h3>Microsoft accounts</h3>
        <button onclick={signIn} disabled={waiting || !s.minecraft_client_id}>Sign in with Microsoft</button>
      </div>
      {#if !s.minecraft_client_id}
        <p class="hint">Add your Azure client ID below first. Signing in is the only thing that needs it.</p>
      {/if}

      {#if code}
        <div class="device">
          <p>Go to <button class="link" onclick={() => openUrl(code!.verificationUri)}>{code.verificationUri.replace(/^https:\/\//, "")}</button> and enter this code:</p>
          <button class="code" onclick={copyCode} title="Copy">{code.userCode}</button>
          <p class="faint small">Waiting for you to finish signing in…</p>
        </div>
      {/if}

      {#if mcs.accounts.length}
        <ul class="accounts">
          {#each mcs.accounts as a (a.uuid)}
            <li class:active={a.active}>
              {#if !broken.has(a.uuid)}
                <img src="https://mc-heads.net/avatar/{a.uuid}/36" alt="" width="36" height="36" onerror={() => (broken = new Set(broken).add(a.uuid))} />
              {:else}
                <span class="initial">{a.name[0]}</span>
              {/if}
              <div class="txt"><strong>{a.name}</strong><span class="mono faint">{a.uuid}</span></div>
              {#if a.active}<span class="cur">Playing as</span>{:else}<button class="ghost" onclick={() => select(a.uuid)}>Use this account</button>{/if}
              <button class="ghost danger icon" onclick={() => remove(a.uuid)} aria-label="Sign out {a.name}"><Icon name="trash" size={16} /></button>
            </li>
          {/each}
        </ul>
      {:else if s.minecraft_client_id}
        <p class="hint">No accounts yet. You need a Microsoft account that owns Minecraft: Java Edition to play.</p>
      {/if}
    </section>

    <section class="section">
      <h3>Setup</h3>
      <div class="setting">
        <div>
          <div class="label">Azure client ID</div>
          <div class="desc">Microsoft sign-in needs an app registration of your own, approved for Minecraft through Mojang's request form. Prism and MultiMC ship with theirs.</div>
        </div>
        <input class="mono key" bind:value={s.minecraft_client_id} oninput={saveSettings} placeholder="00000000-0000-0000-0000-000000000000" aria-label="Azure client ID" />
      </div>
      <div class="setting">
        <div>
          <div class="label">CurseForge API key</div>
          <div class="desc">Lets Arcadia search and download from CurseForge. Free from CurseForge's developer console. Modrinth needs no key.</div>
        </div>
        <input class="mono key" type="password" bind:value={s.curseforge_api_key} oninput={saveSettings} placeholder="$2a$10$…" aria-label="CurseForge API key" />
      </div>
      <div class="setting">
        <div><div class="label">Default memory for new instances</div></div>
        <div class="row">
          <input type="range" min="1024" max="16384" step="512" bind:value={s.minecraft_default_memory_mb} oninput={saveSettings} aria-label="Default memory" />
          <span class="mono val">{(s.minecraft_default_memory_mb / 1024).toFixed(1)} GB</span>
        </div>
      </div>
    </section>

    <section class="section">
      <h3>Opening the launcher</h3>
      <div class="setting">
        <div><div class="label">When you open Minecraft from the library or tray</div></div>
        <div class="seg" role="radiogroup" aria-label="Open Minecraft">
          <button role="radio" aria-checked={s.minecraft_open_mode === "page"} onclick={() => { s.minecraft_open_mode = "page"; saveSettings(); }}>Inside Arcadia</button>
          <button role="radio" aria-checked={s.minecraft_open_mode === "window"} onclick={() => { s.minecraft_open_mode = "window"; saveSettings(); }}>In its own window</button>
        </div>
      </div>
      <div class="setting">
        <div>
          <div class="label">Standalone shortcut</div>
          <div class="desc">Adds a desktop and Start menu shortcut that opens only the Minecraft launcher, without the hub.</div>
        </div>
        <div class="row">
          <input bind:value={shortcutName} style="width: 13rem" aria-label="Shortcut name" />
          <button onclick={shortcut}>Create shortcut</button>
        </div>
      </div>
    </section>
  </div>
</div>

<style>
  .device { display: grid; justify-items: start; gap: 0.6rem; padding: 1rem 1.1rem; border: 1px solid var(--line); border-radius: var(--r-md); background: var(--sunk); margin-bottom: 0.9rem; }
  .code { font-family: "JetBrains Mono Variable", monospace; font-size: 1.6rem; font-weight: 700; letter-spacing: 0.12em; padding: 0.4rem 1rem; }
  .link { all: unset; cursor: pointer; color: var(--lilac); font-weight: 600; }
  .small { font-size: 0.82rem; }
  .accounts { list-style: none; margin: 0; padding: 0; }
  .accounts li { display: flex; align-items: center; gap: 0.9rem; padding: 0.6rem 0; border-top: 1px solid var(--line-soft); }
  .accounts li:first-child { border-top: 0; }
  .accounts img { border-radius: 6px; image-rendering: pixelated; }
  .initial { width: 36px; height: 36px; border-radius: 6px; display: grid; place-items: center; background: var(--peach); color: var(--peach-ink); font-weight: 800; }
  .txt { flex: 1; display: grid; min-width: 0; }
  .txt .mono { font-size: 0.72rem; }
  .cur { color: var(--mint); font-weight: 600; font-size: 0.85rem; }
  .icon { padding: 0.35rem; }
  .key { width: 21rem; }
  .val { width: 4.5rem; text-align: right; color: var(--muted); }
</style>
