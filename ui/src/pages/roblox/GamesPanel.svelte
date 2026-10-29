<script lang="ts">
  // Your Roblox games (total playtime, favourites, shortcuts) and every
  // visit (which server, which account, how long).
  import Icon from "../../components/Icon.svelte";
  import { api } from "../../lib/api";
  import { toast } from "../../lib/state.svelte";
  import type { RobloxPlayed } from "../../lib/types";
  import Thumb from "./Thumb.svelte";
  import { rbx, loadHistory, play, hours, ago, gameOf, accountById } from "./rbx.svelte";

  let { view, go }: { view: "library" | "visits"; go: (tab: string) => void } = $props();

  // ── library ──
  let sort = $state<"time" | "recent" | "name">("time");
  let onlyFav = $state(false);
  let q = $state("");
  const games = $derived.by(() => {
    const term = q.trim().toLowerCase();
    const list = rbx.history.games.filter((g) => (!onlyFav || g.favourite) && (!term || g.name.toLowerCase().includes(term) || g.creator.toLowerCase().includes(term)));
    const by = { time: (a: RobloxPlayed, b: RobloxPlayed) => b.seconds - a.seconds, recent: (a: RobloxPlayed, b: RobloxPlayed) => b.last - a.last, name: (a: RobloxPlayed, b: RobloxPlayed) => a.name.localeCompare(b.name) }[sort];
    return list.sort((a, b) => Number(b.favourite) - Number(a.favourite) || by(a, b));
  });
  const most = $derived(Math.max(1, ...rbx.history.games.map((g) => g.seconds)));
  const total = $derived(rbx.history.games.reduce((n, g) => n + g.seconds, 0));

  async function fav(g: RobloxPlayed) {
    g.favourite = !g.favourite;
    await api.robloxHistoryFavourite(g.placeId, g.favourite);
  }
  async function forget(g: RobloxPlayed) {
    await api.robloxHistoryForget(g.placeId);
    await loadHistory();
    toast(`Forgot ${g.name || g.placeId}`);
  }
  async function shortcut(g: RobloxPlayed) {
    try {
      await api.robloxShortcut(g.placeId, g.name || `Roblox ${g.placeId}`, rbx.accounts.default);
      toast(`Added a desktop shortcut for ${g.name}`);
    } catch (e) {
      toast(String(e), "error");
    }
  }
  let menu = $state<number | null>(null);

  // ── visits, grouped by day ──
  const days = $derived.by(() => {
    const out: { label: string; items: typeof rbx.history.visits }[] = [];
    const fmt = (t: number) => {
      const d = new Date(t * 1000);
      const today = new Date();
      const y = new Date(Date.now() - 86400000);
      if (d.toDateString() === today.toDateString()) return "Today";
      if (d.toDateString() === y.toDateString()) return "Yesterday";
      return d.toLocaleDateString(undefined, { weekday: "long", day: "numeric", month: "long" });
    };
    for (const v of [...rbx.history.visits].reverse()) {
      const l = fmt(v.started);
      if (out.at(-1)?.label !== l) out.push({ label: l, items: [] });
      out.at(-1)!.items.push(v);
    }
    return out;
  });
  const time = (t: number) => new Date(t * 1000).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  async function copyLink(place: number, job: string) {
    await navigator.clipboard.writeText(`roblox://experiences/start?placeId=${place}&gameInstanceId=${job}`);
    toast("Copied a join link for that server");
  }
  async function clearVisits() {
    await api.robloxHistoryClear();
    await loadHistory();
  }
</script>

{#if view === "library"}
  <section class="section">
    <h3>Your games</h3>
    <p class="hint sub">{rbx.history.games.length} games · {hours(total)} played through Arcadia</p>
    <div class="row tools">
      <input class="find grow" placeholder="Find a game" bind:value={q} aria-label="Find a game" />
      <button class="ghost" class:on={onlyFav} aria-pressed={onlyFav} onclick={() => (onlyFav = !onlyFav)}><Icon name="star" size={15} filled={onlyFav} /> Favourites</button>
      <div class="seg" role="radiogroup" aria-label="Sort by">
        <button role="radio" aria-checked={sort === "time"} onclick={() => (sort = "time")}>Most played</button>
        <button role="radio" aria-checked={sort === "recent"} onclick={() => (sort = "recent")}>Recent</button>
        <button role="radio" aria-checked={sort === "name"} onclick={() => (sort = "name")}>A–Z</button>
      </div>
    </div>

    {#if games.length === 0}
      <p class="hint">{rbx.history.games.length ? "No games match." : "Nothing yet. Play something and it'll appear here with its playtime."}</p>
    {:else}
      <ul class="games">
        {#each games as g (g.placeId)}
          <li>
            <Thumb src={g.icon} seed={g.placeId} label={g.name || String(g.placeId)} size={54} />
            <div class="info">
              <div class="title">
                <strong>{g.name || `Place ${g.placeId}`}</strong>
                {#if g.creator}<span class="muted">by {g.creator}</span>{/if}
              </div>
              <div class="bar" title="{hours(g.seconds)} played"><span style:width="{(g.seconds / most) * 100}%"></span></div>
              <div class="faint small">{hours(g.seconds)} · {g.sessions} session{g.sessions === 1 ? "" : "s"} · last played {ago(g.last)}</div>
            </div>
            <div class="acts">
              <button class="ghost star" class:on={g.favourite} aria-pressed={g.favourite} aria-label="Favourite" onclick={() => fav(g)}><Icon name="star" size={17} filled={g.favourite} /></button>
              <button class="ghost" onclick={() => { rbx.serversFor = g.placeId; go("servers"); }}><Icon name="globe" size={15} /> Servers</button>
              <button onclick={() => play(g.placeId)}><Icon name="play" size={14} filled /> Play</button>
              <div class="more">
                <button class="ghost" aria-label="More" aria-expanded={menu === g.placeId} onclick={() => (menu = menu === g.placeId ? null : g.placeId)}><Icon name="more" size={18} /></button>
                {#if menu === g.placeId}
                  <div class="pop" role="menu">
                    <button role="menuitem" onclick={() => { menu = null; shortcut(g); }}><Icon name="link" size={15} /> Desktop shortcut</button>
                    <button role="menuitem" onclick={() => { menu = null; navigator.clipboard.writeText(`https://www.roblox.com/games/${g.placeId}`); toast("Copied the game link"); }}><Icon name="copy" size={15} /> Copy game link</button>
                    <button role="menuitem" class="danger" onclick={() => { menu = null; forget(g); }}><Icon name="trash" size={15} /> Forget this game</button>
                  </div>
                {/if}
              </div>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
{:else}
  <section class="section">
    <div class="row spread">
      <h3>Recent visits</h3>
      {#if rbx.history.visits.length}<button class="ghost" onclick={clearVisits}>Clear list</button>{/if}
    </div>
    <p class="hint">Every server you've been in. Rejoin the exact server while it's still running, or copy a link to send a friend.</p>
    {#if days.length === 0}
      <p class="hint">No visits yet.</p>
    {/if}
    {#each days as d (d.label)}
      <div class="day">{d.label}</div>
      <ul class="visits">
        {#each d.items as v (v.session + v.jobId)}
          {@const g = gameOf(v.placeId)}
          {@const acc = accountById(v.account)}
          <li>
            <span class="when mono">{time(v.started)}</span>
            <Thumb src={g?.icon} seed={v.placeId} label={g?.name || String(v.placeId)} size={34} />
            <div class="info">
              <strong>{g?.name || `Place ${v.placeId}`}</strong>
              <div class="faint small">
                {v.ended ? hours(v.ended - v.started) : "playing now"}
                {#if v.region} · <Icon name="globe" size={11} /> {v.region}{/if}
                {#if acc} · as {acc.displayName}{/if}
              </div>
            </div>
            <div class="acts">
              <button class="ghost" title="Copy a join link for this server" aria-label="Copy server link" onclick={() => copyLink(v.placeId, v.jobId)}><Icon name="copy" size={15} /></button>
              <button onclick={() => play(v.placeId, v.jobId, v.account ?? 0)}>Rejoin server</button>
            </div>
          </li>
        {/each}
      </ul>
    {/each}
  </section>
{/if}

<style>
  .sub { margin-top: 0.15rem; }
  .row button { display: inline-flex; align-items: center; gap: 0.35rem; }
  .tools { margin-top: 0.9rem; gap: 0.5rem; }
  .find { max-width: 18rem; }
  .seg button { white-space: nowrap; }
  .ghost.on { color: #ffd66b; }
  .games, .visits { list-style: none; margin: 0.6rem 0 0; padding: 0; display: flex; flex-direction: column; }
  .games li, .visits li {
    display: flex; align-items: center; gap: 0.9rem; padding: 0.65rem 0.4rem;
    border-top: 1px solid var(--line-soft); border-radius: var(--r-md);
  }
  .games li:first-child, .visits li:first-child { border-top: none; }
  .games li:hover, .visits li:hover { background: color-mix(in srgb, var(--raised) 60%, transparent); }
  .info { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 0.2rem; }
  .title { display: flex; gap: 0.5rem; align-items: baseline; min-width: 0; }
  .title strong, .info > strong { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .title .muted { font-size: 0.85rem; white-space: nowrap; }
  .bar { height: 5px; border-radius: 99px; background: var(--line-soft); overflow: hidden; max-width: 22rem; }
  .bar span { display: block; height: 100%; border-radius: inherit; background: linear-gradient(90deg, var(--lilac), var(--peach)); }
  .small { font-size: 0.78rem; }
  .small :global(svg) { vertical-align: -1px; }
  .acts { display: flex; align-items: center; gap: 0.3rem; }
  .acts button { display: inline-flex; align-items: center; gap: 0.35rem; }
  .star { color: var(--faint); }
  .star.on { color: #ffd66b; }
  .more { position: relative; }
  .pop {
    position: absolute; right: 0; top: calc(100% + 4px); z-index: 20; min-width: 13rem;
    display: flex; flex-direction: column; padding: 0.3rem; border-radius: var(--r-md);
    background: var(--raised); border: 1px solid var(--line); box-shadow: 0 12px 32px rgb(0 0 0 / 0.35);
  }
  .pop button { justify-content: flex-start; background: transparent; border: none; font-weight: 500; padding: 0.45rem 0.6rem; }
  .pop button:hover { background: var(--sunk); }
  .day { margin: 1.1rem 0 0.1rem; font-size: 0.78rem; font-weight: 700; letter-spacing: 0.06em; text-transform: uppercase; color: var(--faint); }
  .when { width: 4.6rem; flex: none; white-space: nowrap; color: var(--muted); font-size: 0.85rem; }
</style>
