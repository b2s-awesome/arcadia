<script lang="ts">
  // How the profile looks inside Discord: the popout you get by clicking a
  // name, the full profile, and a chat message beside the member list.
  // Measurements follow Discord's layout; colours follow its dark and light
  // themes. Nothing here is Discord's artwork.
  import { renderBio } from "./markdown";
  import { inkOn } from "./palette";
  import { STATUS_LABEL, type DProfile, type PreviewView, type Status } from "./model";

  let {
    profile,
    view = "popout",
    scheme = "dark",
    onstatus,
  }: { profile: DProfile; view?: PreviewView; scheme?: "dark" | "light"; onstatus?: (s: Status) => void } = $props();

  const name = $derived(profile.displayName.trim() || profile.username);
  const since = $derived(
    profile.createdAt ? new Date(profile.createdAt).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" }) : null,
  );
  const bio = $derived(renderBio(profile.bio));
  const themed = $derived(!!profile.theme);
  const ink = $derived(themed ? inkOn(scheme === "dark" ? "#000000" : "#ffffff") : null);
  const initial = $derived((name.trim()[0] ?? "?").toUpperCase());

  const order: Status[] = ["online", "idle", "dnd", "offline"];
  function cycle() {
    onstatus?.(order[(order.indexOf(profile.status) + 1) % order.length]);
  }
  // Spoilers reveal on click, as in Discord.
  function reveal(e: MouseEvent | KeyboardEvent) {
    const t = (e.target as HTMLElement).closest(".md-spoiler");
    if (t && (!("key" in e) || e.key === "Enter" || e.key === " ")) t.classList.add("shown");
  }
  const now = new Date();
  const clock = now.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
</script>

{#snippet avatar(size: number, ring: boolean, statusSize: number)}
  <div class="av" style:--s="{size}px" style:--st="{statusSize}px">
    {#if profile.avatar}
      <img class="av-img" src={profile.avatar} alt="" draggable="false" />
    {:else}
      <div class="av-img av-fallback" style:background={profile.bannerColor} style:color={inkOn(profile.bannerColor)}>{initial}</div>
    {/if}
    {#if profile.decoration}<img class="deco" src={profile.decoration} alt="" draggable="false" />{/if}
    {#if statusSize}
      <button
        class="status st-{profile.status}"
        class:ring
        title="{STATUS_LABEL[profile.status]}{onstatus ? ' · click to change' : ''}"
        aria-label="Status: {STATUS_LABEL[profile.status]}"
        onclick={cycle}
        disabled={!onstatus}
      ></button>
    {/if}
  </div>
{/snippet}

{#snippet banner(ratio: number)}
  <div class="banner" style:aspect-ratio={ratio} style:background={profile.banner ? undefined : profile.bannerColor}>
    {#if profile.banner}<img src={profile.banner} alt="" draggable="false" />{/if}
  </div>
{/snippet}

{#snippet bubble()}
  {#if profile.customStatus.trim()}
    <div class="bubble"><span>{profile.customStatus}</span></div>
  {/if}
{/snippet}

<div
  class="dc {scheme}"
  class:themed
  style:--p={profile.theme?.primary}
  style:--a={profile.theme?.accent}
  style:--ink={ink}
  role="presentation"
  onclick={reveal}
  onkeydown={reveal}
>
  {#if view === "popout"}
    <div class="card popout">
      <div class="inner">
        {@render banner(profile.banner ? 300 / 120 : 300 / 105)}
        <div class="head">
          {@render avatar(80, true, 16)}
          {@render bubble()}
        </div>
        <div class="body">
          <div class="names">
            <div class="display">{name}</div>
            <div class="sub">
              <span>{profile.username}</span>
              {#if profile.pronouns.trim()}<span class="dotsep">•</span><span>{profile.pronouns}</span>{/if}
            </div>
          </div>
          {#if profile.bio.trim()}<div class="bio">{@html bio}</div>{/if}
          {#if since}
            <div class="since">
              <div class="label">Member Since</div>
              <div>{since}</div>
            </div>
          {/if}
          <div class="fake-input">Message @{name}</div>
        </div>
      </div>
    </div>
  {:else if view === "profile"}
    <div class="card full">
      <div class="inner">
        {@render banner(profile.banner ? 600 / 212 : 600 / 106)}
        <div class="head big">
          {@render avatar(120, true, 24)}
          {@render bubble()}
          <div class="actions">
            <span class="btn primary">Message</span>
            <span class="btn">•••</span>
          </div>
        </div>
        <div class="body">
          <div class="names">
            <div class="display xl">{name}</div>
            <div class="sub">
              <span>{profile.username}</span>
              {#if profile.pronouns.trim()}<span class="dotsep">•</span><span>{profile.pronouns}</span>{/if}
            </div>
          </div>
          <div class="tabs" aria-hidden="true">
            <span class="on">About Me</span><span>Mutual Friends</span><span>Mutual Servers</span>
          </div>
          {#if profile.bio.trim()}<div class="bio">{@html bio}</div>{:else}<div class="bio faint-dc">No About Me yet.</div>{/if}
          {#if since}
            <div class="since">
              <div class="label">Member Since</div>
              <div>{since}</div>
            </div>
          {/if}
        </div>
      </div>
    </div>
  {:else}
    <div class="chat">
      <div class="channel">
        <div class="chan-head"><span class="hash">#</span> general</div>
        <div class="msgs">
          <div class="msg">
            {@render avatar(40, false, 0)}
            <div class="msg-body">
              <div class="msg-head"><span class="author">{name}</span><span class="time">Today at {clock}</span></div>
              <p>new profile, who dis</p>
              <p>how do the colours look?</p>
            </div>
          </div>
        </div>
        <div class="composer">Message #general</div>
      </div>
      <aside class="members">
        <div class="group">Online — 1</div>
        <div class="member" class:off={profile.status === "offline"}>
          {@render avatar(32, true, 10)}
          <div class="m-text">
            <div class="m-name">{name}</div>
            {#if profile.customStatus.trim()}<div class="m-status">{profile.customStatus}</div>{/if}
          </div>
        </div>
      </aside>
    </div>
  {/if}
</div>

<style>
  .dc {
    --bg: #111214;
    --bg2: #232428;
    --overlay: rgba(0, 0, 0, 0.62);
    --text: #f2f3f5;
    --muted: #b5bac1;
    --line: rgba(255, 255, 255, 0.08);
    --chip: #2b2d31;
    --blurple: #5865f2;
    font-family: "gg sans", "Noto Sans", "Figtree Variable", "Segoe UI", sans-serif;
    color: var(--text);
    font-size: 14px;
    line-height: 1.375;
    -webkit-font-smoothing: antialiased;
    user-select: text;
  }
  .dc.light {
    --bg: #ffffff;
    --bg2: #f2f3f5;
    --overlay: rgba(255, 255, 255, 0.62);
    --text: #060607;
    --muted: #4e5058;
    --line: rgba(6, 6, 7, 0.08);
    --chip: #e3e5e8;
  }
  .dc.themed { --text: var(--ink); }
  .dc.themed.dark { --muted: rgba(255, 255, 255, 0.72); --line: rgba(255, 255, 255, 0.14); --chip: rgba(0, 0, 0, 0.3); }
  .dc.themed.light { --muted: rgba(0, 0, 0, 0.62); --line: rgba(0, 0, 0, 0.12); --chip: rgba(255, 255, 255, 0.5); }

  /* ── card shell ── */
  .card { border-radius: 8px; overflow: hidden; background: var(--bg); box-shadow: 0 8px 24px rgba(0, 0, 0, 0.32); }
  .themed .card { background: linear-gradient(var(--p), var(--a)); padding: 4px; }
  .inner { position: relative; border-radius: 4px; overflow: hidden; }
  .themed .inner { background: var(--overlay); }
  .popout { width: 300px; }
  .full { width: 100%; max-width: 600px; }

  .banner { position: relative; overflow: hidden; }
  .banner img { width: 100%; height: 100%; object-fit: cover; display: block; }

  /* The avatar sits half over the banner with a ring cut out of it. */
  .head { position: relative; height: 40px; padding-left: 16px; }
  .head.big { height: 72px; padding-left: 22px; }
  .head .av { position: absolute; top: -40px; }
  .head.big .av { top: -60px; }
  .av { position: relative; width: var(--s); height: var(--s); flex-shrink: 0; }
  .head .av .av-img { box-shadow: 0 0 0 6px var(--ring, var(--bg)); }
  .head.big .av .av-img { box-shadow: 0 0 0 7px var(--ring, var(--bg)); }
  .themed .head .av .av-img { box-shadow: none; outline: 6px solid color-mix(in srgb, var(--p) 45%, black); }
  .themed.light .head .av .av-img { outline-color: color-mix(in srgb, var(--p) 40%, white); }
  .av-img { width: 100%; height: 100%; border-radius: 50%; object-fit: cover; display: block; }
  .av-fallback { display: grid; place-items: center; font-weight: 700; font-size: calc(var(--s) * 0.42); }
  .deco { position: absolute; left: -10%; top: -10%; width: 120%; height: 120%; pointer-events: none; }

  /* Status: a dot in the bottom-right of the avatar, ringed like Discord's. */
  .status {
    position: absolute; right: calc(var(--st) * -0.05); bottom: calc(var(--st) * -0.05);
    width: var(--st); height: var(--st); padding: 0; border: 0; border-radius: 50%;
    box-shadow: 0 0 0 calc(var(--st) * 0.25) var(--bg); cursor: pointer;
  }
  .head .status { right: 0; bottom: 0; }
  .themed .status { box-shadow: 0 0 0 calc(var(--st) * 0.25) color-mix(in srgb, var(--p) 45%, black); }
  .status:disabled { cursor: default; opacity: 1; }
  .st-online { background: #23a55a; }
  .st-idle { background: radial-gradient(circle at 22% 22%, var(--bg) 0 34%, #f0b232 35%); }
  .st-dnd { background: #f23f43; }
  .st-dnd::after { content: ""; position: absolute; left: 22%; right: 22%; top: 40%; height: 20%; border-radius: 4px; background: var(--bg); }
  .st-offline { background: #80848e; }
  .st-offline::after { content: ""; position: absolute; inset: 28%; border-radius: 50%; background: var(--bg); }
  .themed .st-idle { background: radial-gradient(circle at 22% 22%, color-mix(in srgb, var(--p) 45%, black) 0 34%, #f0b232 35%); }

  /* Custom status: Discord's thought bubble beside the avatar. */
  .bubble {
    position: absolute; left: 108px; top: -28px; max-width: 170px;
    background: var(--bg2); color: var(--text); border-radius: 12px;
    padding: 6px 10px; font-size: 13px; line-height: 1.3;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
  }
  .bubble span { display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; overflow-wrap: anywhere; }
  .bubble::before, .bubble::after { content: ""; position: absolute; border-radius: 50%; background: var(--bg2); }
  .bubble::before { width: 10px; height: 10px; left: -6px; bottom: -2px; }
  .bubble::after { width: 5px; height: 5px; left: -13px; bottom: 4px; }
  .head.big .bubble { left: 160px; top: -12px; max-width: 220px; }
  .themed .bubble { background: var(--chip); backdrop-filter: blur(8px); }

  .body { padding: 12px 16px 16px; display: grid; gap: 12px; }
  .full .body { padding: 8px 22px 22px; }
  .display { font-size: 20px; font-weight: 700; line-height: 24px; overflow-wrap: anywhere; }
  .display.xl { font-size: 24px; line-height: 30px; }
  .sub { color: var(--text); font-size: 14px; display: flex; gap: 6px; flex-wrap: wrap; align-items: baseline; }
  .sub span:first-child { font-weight: 500; }
  .dotsep { color: var(--muted); }

  .bio { white-space: normal; overflow-wrap: anywhere; font-size: 14px; }
  .bio :global(strong) { font-weight: 700; }
  .bio :global(code) { font-family: Consolas, "JetBrains Mono Variable", monospace; font-size: 85%; background: var(--chip); padding: 0 3px; border-radius: 3px; }
  .bio :global(.md-link) { color: #00a8fc; }
  .light .bio :global(.md-link) { color: #006ce7; }
  .bio :global(.md-spoiler) { background: #1e1f22; color: transparent; border-radius: 3px; cursor: pointer; }
  .light .bio :global(.md-spoiler) { background: #c4c9ce; }
  .bio :global(.md-spoiler.shown) { color: inherit; background: var(--chip); }
  .bio :global(.md-h1) { font-size: 1.5em; font-weight: 700; }
  .bio :global(.md-h2) { font-size: 1.25em; font-weight: 700; }
  .bio :global(.md-h3) { font-size: 1.1em; font-weight: 700; }
  .bio :global(.md-sub) { font-size: 12px; color: var(--muted); }
  .bio :global(.md-quote) { display: inline-block; border-left: 4px solid var(--muted); padding-left: 8px; opacity: 0.95; }
  .bio :global(.md-li)::before { content: "• "; }
  .faint-dc { color: var(--muted); }

  .since { display: grid; gap: 2px; font-size: 14px; }
  .label { font-size: 12px; font-weight: 700; color: var(--text); }

  .fake-input { border-radius: 8px; padding: 9px 12px; background: var(--chip); color: var(--muted); font-size: 14px; }

  .actions { position: absolute; right: 16px; top: 12px; display: flex; gap: 8px; }
  .btn { padding: 4px 14px; border-radius: 4px; background: var(--chip); font-size: 14px; font-weight: 500; }
  .btn.primary { background: #248046; color: white; }
  .tabs { display: flex; gap: 22px; border-bottom: 1px solid var(--line); font-size: 14px; color: var(--muted); }
  .tabs span { padding-bottom: 8px; }
  .tabs .on { color: var(--text); box-shadow: inset 0 -2px 0 var(--text); }

  /* ── chat + member list ── */
  .chat { display: grid; grid-template-columns: 1fr 172px; height: 330px; border-radius: 8px; overflow: hidden; background: #313338; box-shadow: 0 8px 24px rgba(0, 0, 0, 0.32); }
  .light .chat { background: #ffffff; }
  .themed .chat { background: #313338; }
  .themed.light .chat { background: #ffffff; }
  .dc.themed .chat { --text: #f2f3f5; --muted: #b5bac1; --bg: #313338; --chip: #383a40; }
  .dc.themed.light .chat { --text: #060607; --muted: #4e5058; --bg: #ffffff; --chip: #ebedef; }
  .channel { display: flex; flex-direction: column; min-width: 0; --bg: #313338; }
  .light .channel { --bg: #ffffff; }
  .chan-head { padding: 12px 16px; font-weight: 600; border-bottom: 1px solid rgba(0, 0, 0, 0.24); display: flex; gap: 6px; align-items: center; }
  .hash { color: var(--muted); font-size: 20px; line-height: 1; }
  .msgs { flex: 1; padding: 16px; display: flex; flex-direction: column; justify-content: flex-end; }
  .msg { display: flex; gap: 16px; }
  .msg-head { display: flex; gap: 8px; align-items: baseline; min-width: 0; }
  .msg-body { min-width: 0; }
  .author { font-weight: 500; font-size: 16px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .time { font-size: 12px; color: var(--muted); white-space: nowrap; }
  .msg-body p { margin: 2px 0 0; overflow-wrap: anywhere; }
  .composer { margin: 0 16px 16px; padding: 11px 16px; border-radius: 8px; background: #383a40; color: var(--muted); }
  .light .composer { background: #ebedef; }
  .members { background: #2b2d31; padding: 16px 8px; --bg: #2b2d31; }
  .light .members { background: #f2f3f5; --bg: #f2f3f5; }
  .group { font-size: 12px; font-weight: 600; letter-spacing: 0.02em; text-transform: uppercase; color: var(--muted); padding: 0 8px 4px; }
  .member { display: flex; gap: 12px; align-items: center; padding: 6px 8px; border-radius: 4px; }
  .member:hover { background: rgba(78, 80, 88, 0.3); }
  .member.off { opacity: 0.4; }
  .m-text { min-width: 0; }
  .m-name { font-weight: 500; font-size: 15px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .m-status { font-size: 12px; color: var(--muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .member .status { box-shadow: 0 0 0 3px var(--bg); }
</style>
