<script lang="ts">
  // Discord status settings (Appearance → Addons): the Application ID, a
  // test, what to share, and a preview of how it looks on your profile.
  import { onMount } from "svelte";
  import Toggle from "../../components/Toggle.svelte";
  import Icon from "../../components/Icon.svelte";
  import GradientPicker from "../../components/GradientPicker.svelte";
  import { schemeFor } from "../../lib/gradient";
  import { api, openUrl, type PresenceStatus } from "../../lib/api";
  import { app, saveUi, saveSettingsNow, toast } from "../../lib/state.svelte";
  import type { PresencePrefs } from "../../lib/theme";

  const pr = $derived(app.ui.presence);
  function set<K extends keyof PresencePrefs>(k: K, v: PresencePrefs[K]) {
    app.ui.presence[k] = v;
    saveUi();
  }

  let idInput = $state(app.ui.presence.clientId);
  const idOk = $derived(/^\d{17,20}$/.test(idInput.trim()));
  let testing = $state(false);
  let tested = $state<string | null>(null);
  async function test() {
    testing = true;
    tested = null;
    try {
      const user = await api.presenceTest(idInput.trim());
      tested = user || "you";
      set("clientId", idInput.trim());
      await saveSettingsNow();
      toast(`Connected to Discord as ${tested}. Your profile shows a test status for a few seconds.`);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      testing = false;
    }
  }

  let status = $state<PresenceStatus | null>(null);
  onMount(() => {
    const poll = () => api.presenceStatus().then((s) => (status = s)).catch(() => {});
    poll();
    const t = setInterval(poll, 3000);
    return () => clearInterval(t);
  });

  // The preview: what Discord shows now, or an example.
  const sample = $derived(
    status?.showing ?? {
      kind: 0, details: pr.hideNames ? "Playing a Roblox game" : "Blox Fruits",
      state: pr.robloxServer && !pr.hideNames ? "Roblox · London, United Kingdom" : "Roblox",
      start: Date.now() - 23 * 60_000, end: null, largeImage: "game", largeText: "", smallImage: "arcadia", smallText: "via Arcadia",
      buttons: [...(pr.robloxJoin && !pr.hideNames ? [["Join my server", ""] as [string, string]] : []), ...(!pr.hideNames ? [["View game", ""] as [string, string]] : [])],
    },
  );
  let tick = $state(Date.now());
  onMount(() => {
    const t = setInterval(() => (tick = Date.now()), 1000);
    return () => clearInterval(t);
  });
  const elapsed = $derived.by(() => {
    if (!sample.start) return null;
    const s = Math.max(0, Math.floor((tick - sample.start) / 1000));
    const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), x = s % 60;
    return `${h ? `${h}:${String(m).padStart(2, "0")}` : m}:${String(x).padStart(2, "0")} elapsed`;
  });
  // The preview card: Discord's plain cards, or a Nitro profile in your two
  // colours (light or dark picked from the top colour, as Discord does).
  const cardLook = $derived(pr.card.mode === "gradient" ? schemeFor(pr.card.top) : pr.card.mode);
  function setCard(c: Partial<typeof pr.card>) {
    app.ui.presence.card = { ...pr.card, ...c };
    saveUi();
  }
  const BUILTIN: Record<string, string> = { arcadia: "#1c1d2b", game: "#5b5fd6", cube: "#4f9a45", music: "#d0558e", paused: "#4a4c5c" };
  const imgStyle = (k: string) => (k.startsWith("https://") ? `background-image:url("${k}")` : `background:${BUILTIN[k] ?? "#333"}`);
  const ICON: Record<string, string> = { arcadia: "", game: "gamepad", cube: "cube", music: "music", paused: "pause" };
</script>

<div class="wrap">
  <div class="left">
    <div class="steps">
      <div class="step" class:done={!!pr.clientId}>
        <span class="n">{pr.clientId ? "✓" : "1"}</span>
        <div>
          <strong>Make a Discord app called “Arcadia”</strong>
          <p class="muted">Its name is what friends see: “Playing Arcadia”. Then copy its Application ID from General Information.</p>
          <button class="ghost link" onclick={() => openUrl("https://discord.com/developers/applications")}>Open the Developer Portal <Icon name="link" size={13} /></button>
        </div>
      </div>
      <div class="step">
        <span class="n">2</span>
        <div class="grow">
          <strong>Paste the Application ID</strong>
          <div class="row id">
            <input class="mono grow" inputmode="numeric" placeholder="1234567890123456789" bind:value={idInput} aria-label="Discord Application ID" />
            <button disabled={!idOk || testing} onclick={test}>{testing ? "Testing…" : pr.clientId === idInput.trim() && status?.connected ? "Test again" : "Connect"}</button>
          </div>
          {#if idInput && !idOk}<p class="warn">It's a long number (17–20 digits).</p>{/if}
        </div>
      </div>
      <div class="step">
        <span class="n">3</span>
        <div>
          <strong>Turn on activity sharing in Discord</strong>
          <p class="muted">User Settings → Activity Privacy → “Share your detected activities with others”.</p>
        </div>
      </div>
    </div>

    <div class="state" class:ok={status?.connected} class:bad={!!status?.error}>
      <span class="dot"></span>
      {#if !pr.clientId}
        Not set up yet
      {:else if status?.connected}
        <span>{status.user ? "Connected as" : "Connected"} {#if status.user}<strong>{status.user}</strong>{/if}{status.showing ? ` · showing “${status.showing.details}”` : " · nothing to show right now"}</span>
      {:else if status?.error}
        {status.error.includes("isn't running") ? "Waiting for Discord to open" : status.error}
      {:else}
        Connecting…
      {/if}
    </div>

    <h4>What to share</h4>
    <div class="setting"><div><div class="label">Roblox</div><div class="desc">The game you're in, with its icon.</div></div><Toggle label="Share Roblox" checked={pr.roblox} onchange={(v) => set("roblox", v)} /></div>
    {#if pr.roblox}
      <div class="setting sub"><div class="label">Server region</div><Toggle label="Share server region" checked={pr.robloxServer} onchange={(v) => set("robloxServer", v)} /></div>
      <div class="setting sub"><div><div class="label">“Join my server” button</div><div class="desc">Lets anyone who sees your profile join the exact server you're in.</div></div><Toggle label="Join button" checked={pr.robloxJoin} onchange={(v) => set("robloxJoin", v)} /></div>
    {/if}
    <div class="setting"><div><div class="label">Minecraft</div><div class="desc">Instance name and version.</div></div><Toggle label="Share Minecraft" checked={pr.minecraft} onchange={(v) => set("minecraft", v)} /></div>
    {#if pr.minecraft}
      <div class="setting sub"><div class="label">Server address</div><Toggle label="Share Minecraft server" checked={pr.minecraftServer} onchange={(v) => set("minecraftServer", v)} /></div>
    {/if}
    <div class="setting"><div><div class="label">Library games</div><div class="desc">Games started from your Library, with their cover.</div></div><Toggle label="Share library games" checked={pr.games} onchange={(v) => set("games", v)} /></div>
    <div class="setting"><div><div class="label">Music</div><div class="desc">“Listening to” with the song, artist and a progress bar.</div></div><Toggle label="Share music" checked={pr.music} onchange={(v) => set("music", v)} /></div>
    {#if pr.music}<div class="setting sub"><div><div class="label">Album covers</div><div class="desc">Discord can't see covers inside your files, so Arcadia finds the same album's cover on Apple Music or Deezer (YouTube songs use their thumbnail). Sends the artist, album and title; each album is looked up once.</div></div><Toggle label="Album covers" checked={pr.covers} onchange={(v) => set("covers", v)} /></div>{/if}
    <div class="setting"><div><div class="label">“Browsing Arcadia”</div><div class="desc">When nothing else is going on.</div></div><Toggle label="Share browsing" checked={pr.idle} onchange={(v) => set("idle", v)} /></div>
    <div class="setting"><div class="label">Time elapsed</div><Toggle label="Show time elapsed" checked={pr.elapsed} onchange={(v) => set("elapsed", v)} /></div>
    <div class="setting"><div><div class="label">Hide game names</div><div class="desc">Shows “Playing a game” instead, with no buttons.</div></div><Toggle label="Hide game names" checked={pr.hideNames} onchange={(v) => set("hideNames", v)} /></div>
  </div>

  <div class="preview">
    <div class="label small">{status?.showing ? "On your profile now" : "Preview"}</div>
    <div class="dc {cardLook}" class:themed={pr.card.mode === "gradient"} style:--p={pr.card.top} style:--a={pr.card.bottom}>
      <div class="dc-banner"></div>
      <div class="dc-body">
        <div class="dc-av"><span>{(status?.user ?? "You").slice(0, 1).toUpperCase()}</span><i></i></div>
        <div class="dc-name">{status?.user ?? "You"}</div>
        <div class="dc-card">
          <div class="dc-head">{sample.kind === 2 ? "Listening to Arcadia" : "Playing Arcadia"}</div>
          <div class="dc-act">
            <div class="dc-img" style={imgStyle(sample.largeImage)}>
              {#if !sample.largeImage.startsWith("https://")}
                {#if sample.largeImage === "arcadia"}<span class="logo">a</span>{:else}<Icon name={ICON[sample.largeImage] ?? "gamepad"} size={30} />{/if}
              {/if}
              {#if sample.smallImage}<span class="dc-small" style={imgStyle(sample.smallImage)}>{#if sample.smallImage === "arcadia"}<span class="logo s">a</span>{:else}<Icon name={ICON[sample.smallImage] ?? "pause"} size={11} filled />{/if}</span>{/if}
            </div>
            <div class="dc-txt">
              <strong>{sample.details}</strong>
              {#if sample.state}<span>{sample.state}</span>{/if}
              {#if pr.elapsed && elapsed && sample.kind !== 2}<span>{elapsed}</span>{/if}
              {#if sample.kind === 2 && sample.start && sample.end}
                <span class="dc-bar"><i style:width="{Math.min(100, ((tick - sample.start) / (sample.end - sample.start)) * 100)}%"></i></span>
              {/if}
            </div>
          </div>
          {#each sample.buttons as [label]}<div class="dc-btn">{label}</div>{/each}
        </div>
      </div>
    </div>
    <div class="cardpick" role="radiogroup" aria-label="Preview card">
      {#each [["dark", "Dark"], ["light", "Light"], ["gradient", "Your colours"]] as [id, label]}
        <button role="radio" aria-checked={pr.card.mode === id} onclick={() => setCard({ mode: id as "dark" })}>{label}</button>
      {/each}
    </div>
    {#if pr.card.mode === "gradient"}
      <GradientPicker bind:top={app.ui.presence.card.top} bind:bottom={app.ui.presence.card.bottom} styles={false} onchange={saveUi} />
    {/if}
    <p class="faint small">Discord shows your status while its app is open on this PC.</p>
  </div>
</div>

<style>
  .wrap { display: grid; grid-template-columns: minmax(0, 1fr) 300px; gap: 1.4rem; align-items: start; }
  @media (max-width: 1050px) { .wrap { grid-template-columns: 1fr; } }
  .steps { display: flex; flex-direction: column; gap: 0.7rem; margin-bottom: 1rem; }
  .step { display: flex; gap: 0.8rem; align-items: flex-start; }
  .step .n { width: 26px; height: 26px; flex: none; border-radius: 50%; display: grid; place-items: center; font-weight: 700; font-size: 0.85rem; background: var(--raised); border: 1px solid var(--line); }
  .step.done .n { background: var(--mint); color: var(--ink); border-color: var(--mint); }
  .step p { font-size: 0.85rem; }
  .grow { flex: 1; min-width: 0; }
  .link { padding: 0.15rem 0.4rem; margin-left: -0.4rem; font-size: 0.85rem; display: inline-flex; gap: 0.3rem; align-items: center; color: var(--lilac); }
  .id { margin-top: 0.35rem; }
  .warn { color: var(--peach); font-size: 0.8rem; margin-top: 0.25rem; }
  .state { display: flex; align-items: center; gap: 0.5rem; padding: 0.6rem 0.8rem; border-radius: var(--r-md); background: var(--sunk); border: 1px solid var(--line-soft); font-size: 0.88rem; margin-bottom: 1rem; }
  .state .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--faint); flex: none; }
  .state.ok .dot { background: #3ba55d; box-shadow: 0 0 0 3px rgb(59 165 93 / 0.25); }
  .state.bad .dot { background: var(--peach); }
  h4 { margin: 0.4rem 0 0.2rem; font-size: 0.78rem; text-transform: uppercase; letter-spacing: 0.08em; color: var(--muted); }
  .sub { padding-left: 1.4rem; }
  .small { font-size: 0.78rem; }
  .preview { position: sticky; top: 0; display: flex; flex-direction: column; gap: 0.5rem; }
  /* A Discord-style profile card: plain dark, plain light, or a Nitro
     profile (the gradient behind a see-through layer, Discord's recipe). */
  .dc {
    --bg: #232428; --panel: #111214; --text: #dbdee1; --strong: #f2f3f5; --muted: #b5bac1; --btn: #4e5058; --btn-ink: #fff; --ring: #232428; --track: #4e5058;
    border-radius: 12px; overflow: hidden; background: var(--bg); color: var(--text); font-family: "gg sans", "Segoe UI", var(--body); box-shadow: 0 8px 28px rgb(0 0 0 / 0.35);
  }
  .dc.light { --bg: #ffffff; --panel: #f2f3f5; --text: #313338; --strong: #060607; --muted: #4e5058; --btn: #e3e5e8; --btn-ink: #060607; --ring: #ffffff; --track: #c4c9ce; }
  .dc.themed { background: linear-gradient(var(--p), var(--a)); padding: 4px; }
  .dc.themed > * { background: transparent; }
  .dc.themed.dark { --panel: rgba(0, 0, 0, 0.32); --text: #f2f3f5; --muted: rgba(255, 255, 255, 0.72); --btn: rgba(255, 255, 255, 0.16); --ring: color-mix(in srgb, var(--p) 45%, black); --track: rgba(255, 255, 255, 0.25); }
  .dc.themed.light { --panel: rgba(255, 255, 255, 0.5); --text: #313338; --strong: #060607; --muted: rgba(0, 0, 0, 0.62); --btn: rgba(0, 0, 0, 0.1); --btn-ink: #060607; --ring: color-mix(in srgb, var(--p) 40%, white); --track: rgba(0, 0, 0, 0.18); }
  .dc.themed .dc-body { background: rgba(0, 0, 0, 0.45); border-radius: 0 0 9px 9px; }
  .dc.themed.light .dc-body { background: rgba(255, 255, 255, 0.55); }
  .dc.themed .dc-banner { border-radius: 9px 9px 0 0; }
  .dc-banner { height: 60px; background: linear-gradient(120deg, #5865f2, #b8bbff); }
  .themed .dc-banner { background: color-mix(in srgb, var(--p) 80%, black); }
  .dc-body { padding: 0 14px 14px; position: relative; }
  .dc-av { width: 72px; height: 72px; border-radius: 50%; margin-top: -36px; border: 6px solid var(--ring); background: linear-gradient(145deg, #7d84ff, #3b3fa8); display: grid; place-items: center; position: relative; }
  .dc-av span { font-size: 1.6rem; font-weight: 700; color: #fff; }
  .dc-av i { position: absolute; right: -2px; bottom: -2px; width: 18px; height: 18px; border-radius: 50%; background: #23a55a; border: 4px solid var(--ring); }
  .dc-name { font-weight: 700; font-size: 1.1rem; color: var(--strong); margin: 6px 0 10px; }
  .dc-card { background: var(--panel); border-radius: 8px; padding: 10px 12px 12px; }
  .dc-head { font-size: 0.72rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.02em; color: var(--muted); margin-bottom: 8px; }
  .dc-act { display: flex; gap: 10px; align-items: center; }
  .dc-img { position: relative; width: 64px; height: 64px; flex: none; border-radius: 8px; background-size: cover; background-position: center; display: grid; place-items: center; color: #fff; }
  .dc-small { position: absolute; right: -5px; bottom: -5px; width: 22px; height: 22px; border-radius: 50%; border: 3px solid var(--panel); display: grid; place-items: center; background-size: cover; color: #fff; }
  .logo { font-family: var(--display); font-weight: 800; font-size: 2rem; color: #e9e9f2; line-height: 1; margin-top: -4px; }
  .logo.s { font-size: 0.8rem; margin-top: -2px; }
  .dc-txt { display: flex; flex-direction: column; min-width: 0; font-size: 0.82rem; line-height: 1.35; }
  .dc-txt strong { color: var(--strong); font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .dc-txt span { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .dc-bar { display: block; height: 4px; border-radius: 2px; background: var(--track); margin-top: 6px; overflow: hidden; }
  .dc-bar i { display: block; height: 100%; background: var(--strong); }
  .dc-btn { margin-top: 8px; padding: 6px; border-radius: 4px; background: var(--btn); color: var(--btn-ink); text-align: center; font-size: 0.82rem; font-weight: 500; }
  .cardpick { display: flex; gap: 0.3rem; }
  .cardpick button { flex: 1; font-size: 0.78rem; padding: 0.3rem 0.4rem; border-radius: 999px; background: transparent; }
  .cardpick button[aria-checked="true"] { border-color: var(--peach); color: var(--text); box-shadow: 0 0 0 1px var(--peach); }
  .preview :global(.gp .styles), .preview :global(.gp .top-row .pill) { display: none; }
</style>
