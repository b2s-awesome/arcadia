<script lang="ts">
  // Play: pick who to play as, paste a game link, or jump back into a game.
  import Icon from "../../components/Icon.svelte";
  import { api } from "../../lib/api";
  import { toast } from "../../lib/state.svelte";
  import Thumb from "./Thumb.svelte";
  import { rbx, play, signIn, hours, ago, gameName, effectiveAccount } from "./rbx.svelte";

  let { go }: { go: (tab: string) => void } = $props();

  const games = $derived(
    [...rbx.history.games].sort((a, b) => Number(b.favourite) - Number(a.favourite) || b.last - a.last).slice(0, 8),
  );

  let link = $state("");
  let busy = $state(false);
  async function joinLink(servers = false) {
    const place = await api.robloxPlaceFrom(link);
    if (!place) {
      toast("Paste a Roblox game link or a place ID", "error");
      return;
    }
    if (servers) {
      await api.robloxHistoryAdd(place).catch(() => {});
      rbx.serversFor = place;
      go("servers");
      return;
    }
    busy = true;
    await play(place);
    busy = false;
    link = "";
  }

  const who = $derived(effectiveAccount());
</script>

{#if rbx.rejoin && rbx.rejoin.state !== "limit"}
  <div class="rejoin">
    <span class="spin"><Icon name="refresh" size={18} /></span>
    <div class="grow">
      <strong>
        {#if rbx.rejoin.state === "waiting"}Rejoining {gameName(rbx.rejoin.placeId)} in {rbx.rejoin.seconds}s
        {:else if rbx.rejoin.state === "joining"}Rejoining {gameName(rbx.rejoin.placeId)}…
        {:else}Rejoin failed{/if}
      </strong>
      <div class="muted">You were disconnected. Arcadia closes that window and joins again.</div>
    </div>
    {#if rbx.rejoin.state === "waiting"}<button onclick={() => api.robloxRejoinCancel()}>Cancel</button>{/if}
  </div>
{/if}

<section class="section">
  <h3>Play as</h3>
  <div class="who" role="radiogroup" aria-label="Play as">
    <button class="acct" role="radio" aria-checked={rbx.picked === 0 || (rbx.picked === null && who === null)} onclick={() => (rbx.picked = 0)}>
      <span class="app-ico"><Icon name="roblox" size={20} /></span>
      <span class="txt"><strong>Roblox app</strong><span class="muted">Whoever it's signed in as</span></span>
    </button>
    {#each rbx.accounts.list as a (a.id)}
      <button class="acct" role="radio" aria-checked={who === a.id && rbx.picked !== 0} onclick={() => (rbx.picked = a.id)}>
        <Thumb src={a.avatar} seed={a.id} label={a.displayName} size={36} round />
        <span class="txt">
          <strong>{a.displayName}</strong>
          <span class="muted">@{a.name}{#if rbx.accounts.default === a.id}&nbsp;· default{/if}</span>
        </span>
      </button>
    {/each}
    <button class="acct add" onclick={signIn} disabled={rbx.signingIn}>
      <span class="app-ico"><Icon name="plus" size={18} /></span>
      <span class="txt"><strong>{rbx.signingIn ? "Waiting for sign-in…" : "Add account"}</strong><span class="muted">Opens Roblox's sign-in page</span></span>
    </button>
  </div>
</section>

<section class="section">
  <h3>Join a game</h3>
  <div class="join">
    <span class="lead"><Icon name="link" size={18} /></span>
    <input
      class="grow"
      placeholder="Paste a game link, share link or place ID"
      bind:value={link}
      onkeydown={(e) => e.key === "Enter" && link.trim() && joinLink()}
      aria-label="Game link or place ID"
    />
    <button class="ghost" disabled={!link.trim()} onclick={() => joinLink(true)}><Icon name="globe" size={16} /> Servers</button>
    <button disabled={!link.trim() || busy} onclick={() => joinLink()}><Icon name="play" size={16} filled /> Join</button>
  </div>
</section>

<section class="section">
  <div class="row spread">
    <h3>Jump back in</h3>
    {#if rbx.history.games.length}<button class="ghost" onclick={() => go("games-library")}>All games</button>{/if}
  </div>
  {#if games.length}
    <div class="tiles">
      {#each games as g (g.placeId)}
        <div class="tile">
          <div class="art">
            <Thumb src={g.icon} seed={g.placeId} label={g.name || String(g.placeId)} size={120} />
            {#if g.favourite}<span class="fav"><Icon name="star" size={13} filled /></span>{/if}
            <button class="go" aria-label="Play {g.name}" onclick={() => play(g.placeId)}><Icon name="play" size={20} filled /></button>
          </div>
          <div class="name" title={g.name}>{g.name || `Place ${g.placeId}`}</div>
          <div class="meta muted">{hours(g.seconds)} · {ago(g.last)}</div>
          <button class="ghost servers" onclick={() => { rbx.serversFor = g.placeId; go("servers"); }}><Icon name="globe" size={14} /> Servers</button>
        </div>
      {/each}
    </div>
  {:else}
    <p class="hint">Games you play show up here, with how long you've played them.</p>
  {/if}
</section>

<style>
  .rejoin {
    display: flex; align-items: center; gap: 0.9rem;
    padding: 0.9rem 1.1rem; margin-bottom: 1rem; border-radius: var(--r-lg);
    background: color-mix(in srgb, var(--lilac) 13%, var(--raised));
    border: 1px solid color-mix(in srgb, var(--lilac) 35%, transparent);
  }
  .rejoin .muted { font-size: 0.85rem; }
  .spin { display: inline-flex; color: var(--lilac); animation: spin 1.4s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }

  .who { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 0.6rem; }
  .acct {
    display: flex; align-items: center; gap: 0.7rem; text-align: left;
    padding: 0.6rem 0.75rem; border-radius: var(--r-lg); font-weight: 500;
    background: var(--sunk); border-color: var(--line-soft);
  }
  .acct[aria-checked="true"] {
    border-color: color-mix(in srgb, var(--peach) 70%, transparent);
    background: color-mix(in srgb, var(--peach) 9%, var(--raised));
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--peach) 14%, transparent);
  }
  .acct.add { border-style: dashed; }
  .txt { display: flex; flex-direction: column; min-width: 0; line-height: 1.25; }
  .txt strong, .txt .muted { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .txt .muted { font-size: 0.8rem; }
  .app-ico {
    width: 36px; height: 36px; flex: none; display: grid; place-items: center; border-radius: 50%;
    background: var(--raised); border: 1px solid var(--line); color: var(--muted);
  }

  .join { display: flex; gap: 0.5rem; align-items: center; position: relative; }
  .join .lead { position: absolute; left: 0.8rem; color: var(--faint); display: inline-flex; pointer-events: none; }
  .join input { padding-left: 2.4rem; height: 2.6rem; }
  .join button { display: inline-flex; align-items: center; gap: 0.4rem; white-space: nowrap; }

  .tiles { display: grid; grid-template-columns: repeat(auto-fill, minmax(132px, 1fr)); gap: 1.1rem 0.9rem; }
  .tile { min-width: 0; display: flex; flex-direction: column; align-items: flex-start; }
  .art { position: relative; width: 100%; aspect-ratio: 1; border-radius: var(--r-lg); overflow: hidden; }
  .art :global(.thumb) { width: 100% !important; height: 100% !important; border-radius: var(--r-lg); font-size: 2rem !important; }
  .fav {
    position: absolute; top: 0.45rem; left: 0.45rem; display: grid; place-items: center;
    width: 22px; height: 22px; border-radius: 50%; color: #ffd66b; background: rgb(0 0 0 / 0.45);
  }
  .go {
    position: absolute; right: 0.5rem; bottom: 0.5rem; width: 42px; height: 42px; padding: 0;
    display: grid; place-items: center; border-radius: 50%;
    background: var(--peach); color: var(--peach-ink); border: none;
    opacity: 0; transform: translateY(6px) scale(0.92); transition: opacity 0.15s, transform 0.15s;
    box-shadow: 0 6px 18px rgb(0 0 0 / 0.35);
  }
  .tile:hover .go, .go:focus-visible { opacity: 1; transform: none; }
  .name { margin-top: 0.5rem; font-weight: 600; max-width: 100%; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .meta { font-size: 0.8rem; }
  .servers { padding: 0.15rem 0.4rem; margin-left: -0.4rem; font-size: 0.8rem; display: inline-flex; align-items: center; gap: 0.3rem; }
</style>
