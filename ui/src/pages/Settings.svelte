<script lang="ts">
  import { api } from "../lib/api";
  import { app, saveSettings } from "../lib/state.svelte";
  import Toggle from "../components/Toggle.svelte";

  const s = app.settings!;
</script>

<div class="page">
  <div class="page-inner">
    <header class="page-head">
      <div>
        <h2>Settings</h2>
      </div>
    </header>

    <section class="section">
      <h3>App</h3>
      <div class="setting">
        <div><div class="label">Start with Windows</div><div class="desc">Opens quietly in the tray when you sign in.</div></div>
        <Toggle label="Start with Windows" bind:checked={s.run_on_boot} onchange={saveSettings} />
      </div>
      <div class="setting">
        <div><div class="label">Keep running in the tray</div><div class="desc">Closing the window hides Arcadia instead of quitting, and it tucks itself away while you play.</div></div>
        <Toggle label="Keep running in the tray" bind:checked={s.minimize_to_tray} onchange={saveSettings} />
      </div>
    </section>

    <section class="section">
      <h3>Playing</h3>
      <div class="setting">
        <div><div class="label">Hide while a game is running</div><div class="desc">Tuck Arcadia away when you start a game from it.</div></div>
        <Toggle label="Hide while a game is running" bind:checked={s.hide_on_game_launch} onchange={saveSettings} />
      </div>
    </section>

    <section class="section">
      <h3>Asset proxy</h3>
      <div class="setting">
        <div><div class="label">Port</div><div class="desc">If it's taken, Arcadia picks a free one automatically.</div></div>
        <input type="number" min="1024" max="65535" style="width: 7rem" bind:value={s.proxy_port} oninput={saveSettings} aria-label="Proxy port" />
      </div>
      <div class="setting">
        <div><div class="label">Keep request contents in the Inspector</div><div class="desc">Turn off to log only addresses and sizes.</div></div>
        <Toggle label="Keep request contents" bind:checked={s.proxy_inspector_enabled} onchange={saveSettings} />
      </div>
    </section>

    <section class="section">
      <h3>Folders</h3>
      <div class="row wrap folders">
        <button onclick={() => api.openFolder("data")}>Arcadia data</button>
        <button onclick={() => api.openFolder("roblox")}>Roblox versions</button>
        <button onclick={() => api.openFolder("mods")}>Mods</button>
        <button onclick={() => api.openFolder("proxy")}>Proxy files and certificate</button>
      </div>
    </section>

    <section class="section about">
      <h3>How Arcadia changes Roblox</h3>
      <p>Arcadia installs its own copy of Roblox and only ever edits that copy. The asset proxy runs on your computer, is used only by Roblox started from Arcadia, and decrypts just three Roblox addresses: asset downloads and client settings. Its certificate is added to that Roblox copy alone, never to Windows.</p>
      <p>Asset swaps are client-side: other players don't see them. Live FastFlags are the one feature Roblox may ban for, which is why they're off until you accept that risk.</p>
      <p class="faint">Arcadia 0.1.0</p>
    </section>
  </div>
</div>

<style>
  .folders { margin-top: 0.6rem; }
  .about p { color: var(--muted); max-width: 68ch; margin-top: 0.6rem; }
</style>
