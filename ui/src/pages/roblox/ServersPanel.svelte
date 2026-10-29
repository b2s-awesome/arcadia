<script lang="ts">
  // Server browser: a game's public servers, where each one is, and a
  // one-click join. Finding the region asks Roblox for the server's address
  // (needs a signed-in account), then looks the address up.
  import { untrack } from "svelte";
  import Icon from "../../components/Icon.svelte";
  import { api } from "../../lib/api";
  import { toast } from "../../lib/state.svelte";
  import type { RobloxServer } from "../../lib/types";
  import Thumb from "./Thumb.svelte";
  import { rbx, play, gameOf } from "./rbx.svelte";

  let servers = $state<RobloxServer[]>([]);
  let next = $state<string | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let regions = $state<Record<string, string | null | undefined>>({});
  let finding = $state(false);
  let region = $state<string | null>(null);
  let sort = $state<"full" | "empty" | "ping">("full");

  const known = $derived([...rbx.history.games].sort((a, b) => b.last - a.last));
  let place = $state<number | null>(untrack(() => rbx.serversFor ?? known[0]?.placeId ?? null));
  $effect(() => {
    if (rbx.serversFor && rbx.serversFor !== place) place = rbx.serversFor;
    else if (!place && known.length) place = known[0].placeId;
  });

  async function load(more = false) {
    if (!place) return;
    loading = true;
    error = null;
    try {
      const r = await api.robloxServers(place, more ? next : null);
      servers = more ? [...servers, ...r.servers] : r.servers;
      next = r.next;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }
  $effect(() => {
    if (place) {
      rbx.serversFor = place;
      servers = [];
      region = null;
      void load();
    }
  });

  async function findRegions() {
    if (!place) return;
    if (!rbx.accounts.list.length) {
      toast("Add a Roblox account first: Roblox only shares server addresses with signed-in players", "error");
      return;
    }
    finding = true;
    const todo = servers.filter((s) => regions[s.id] === undefined);
    const p = place;
    let i = 0;
    const worker = async () => {
      while (i < todo.length && finding && p === place) {
        const s = todo[i++];
        try {
          regions[s.id] = await api.robloxServerRegion(p, s.id);
        } catch (e) {
          regions[s.id] = null;
          if (String(e).includes("account")) {
            toast(String(e), "error");
            finding = false;
          }
        }
      }
    };
    await Promise.all([worker(), worker(), worker()]);
    finding = false;
  }

  const found = $derived.by(() => {
    const m = new Map<string, number>();
    for (const s of servers) {
      const r = regions[s.id];
      if (r) m.set(r, (m.get(r) ?? 0) + 1);
    }
    return [...m.entries()].sort((a, b) => b[1] - a[1]);
  });
  const shown = $derived.by(() => {
    const list = servers.filter((s) => !region || regions[s.id] === region);
    const by = {
      full: (a: RobloxServer, b: RobloxServer) => b.playing - a.playing,
      empty: (a: RobloxServer, b: RobloxServer) => a.playing - b.playing,
      ping: (a: RobloxServer, b: RobloxServer) => (a.ping ?? 9999) - (b.ping ?? 9999),
    }[sort];
    return [...list].sort(by);
  });
  const resolved = $derived(servers.filter((s) => regions[s.id] !== undefined).length);

  async function joinRegion() {
    const s = shown.find((x) => x.playing < x.maxPlayers);
    if (s && place) play(place, s.id);
  }
  const g = $derived(place ? gameOf(place) : null);
  const pingClass = (p: number | null) => (p == null ? "" : p < 80 ? "good" : p < 160 ? "ok" : "bad");
</script>

<section class="section">
  <div class="head">
    {#if place}<Thumb src={g?.icon} seed={place} label={g?.name || String(place)} size={46} />{/if}
    <div class="grow">
      <h3>Servers</h3>
      <select bind:value={place} aria-label="Game">
        {#if place && !known.some((k) => k.placeId === place)}<option value={place}>Place {place}</option>{/if}
        {#each known as k (k.placeId)}<option value={k.placeId}>{k.name || `Place ${k.placeId}`}</option>{/each}
      </select>
    </div>
    <div class="row">
      <button class="ghost" onclick={() => load()} disabled={loading}><Icon name="refresh" size={15} /> Refresh</button>
      {#if finding}
        <button onclick={() => (finding = false)}>Stop ({resolved}/{servers.length})</button>
      {:else}
        <button onclick={findRegions} disabled={!servers.length}><Icon name="globe" size={15} /> Find regions</button>
      {/if}
    </div>
  </div>

  {#if !place}
    <p class="hint">Pick a game, or paste its link on the Play tab and choose <em>Servers</em>.</p>
  {:else}
    {#if found.length}
      <div class="regions" role="radiogroup" aria-label="Region">
        <button class="chip" role="radio" aria-checked={region === null} onclick={() => (region = null)}>Everywhere <span>{servers.length}</span></button>
        {#each found as [r, n] (r)}
          <button class="chip" role="radio" aria-checked={region === r} onclick={() => (region = r)}>{r} <span>{n}</span></button>
        {/each}
        {#if region}<button class="chip go" onclick={joinRegion}><Icon name="play" size={12} filled /> Join best in {region.split(",")[0]}</button>{/if}
      </div>
    {/if}

    <div class="row spread bar">
      <span class="faint small">{shown.length} server{shown.length === 1 ? "" : "s"}{#if region} in {region}{/if}</span>
      <div class="seg" role="radiogroup" aria-label="Sort servers">
        <button role="radio" aria-checked={sort === "full"} onclick={() => (sort = "full")}>Fullest</button>
        <button role="radio" aria-checked={sort === "empty"} onclick={() => (sort = "empty")}>Emptiest</button>
        <button role="radio" aria-checked={sort === "ping"} onclick={() => (sort = "ping")}>Lowest ping</button>
      </div>
    </div>

    {#if error}<p class="err">{error}</p>{/if}
    <ul class="servers">
      {#each shown as s (s.id)}
        <li>
          <div class="fill" title="{s.playing} of {s.maxPlayers} players">
            <div class="track"><span style:width="{(s.playing / Math.max(1, s.maxPlayers)) * 100}%"></span></div>
            <span class="mono small">{s.playing}/{s.maxPlayers}</span>
          </div>
          <span class="ping mono small {pingClass(s.ping)}" title="Ping Roblox reported"><Icon name="signal" size={13} /> {s.ping ?? "–"} ms</span>
          <span class="where small">
            {#if regions[s.id]}<Icon name="globe" size={13} /> {regions[s.id]}
            {:else if regions[s.id] === null}<span class="faint">Unknown</span>
            {:else}<span class="faint">—</span>{/if}
          </span>
          <span class="id mono faint small" title={s.id}>{s.id.slice(0, 8)}</span>
          <button class="ghost" aria-label="Copy join link" title="Copy a join link" onclick={() => { navigator.clipboard.writeText(`roblox://experiences/start?placeId=${place}&gameInstanceId=${s.id}`); toast("Copied a join link"); }}><Icon name="copy" size={14} /></button>
          <button disabled={s.playing >= s.maxPlayers} onclick={() => place && play(place, s.id)}>Join</button>
        </li>
      {/each}
    </ul>
    {#if loading}<p class="hint">Loading servers…</p>{/if}
    {#if next && !loading}<button class="more ghost" onclick={() => load(true)}>Load more servers</button>{/if}
  {/if}
</section>

<style>
  .head { display: flex; align-items: center; gap: 0.9rem; }
  .head select { margin-top: 0.25rem; max-width: 22rem; }
  .row button, .chip, .more { display: inline-flex; align-items: center; gap: 0.35rem; }
  .regions { display: flex; flex-wrap: wrap; gap: 0.4rem; margin-top: 1rem; }
  .chip { padding: 0.3rem 0.7rem; border-radius: 999px; font-weight: 500; font-size: 0.85rem; background: var(--sunk); }
  .chip span { color: var(--faint); font-variant-numeric: tabular-nums; }
  .chip[aria-checked="true"] { background: color-mix(in srgb, var(--lilac) 20%, var(--raised)); border-color: color-mix(in srgb, var(--lilac) 50%, transparent); }
  .chip.go { background: var(--peach); color: var(--peach-ink); border-color: var(--peach); font-weight: 600; }
  .bar { margin: 1rem 0 0.3rem; }
  .small { font-size: 0.8rem; }
  .servers { list-style: none; margin: 0; padding: 0; }
  .servers li {
    display: grid; grid-template-columns: minmax(130px, 1.2fr) 80px minmax(140px, 1.6fr) 80px auto auto;
    align-items: center; gap: 0.8rem; padding: 0.5rem 0.4rem; border-top: 1px solid var(--line-soft);
  }
  .servers li:hover { background: color-mix(in srgb, var(--raised) 60%, transparent); }
  .fill { display: flex; align-items: center; gap: 0.6rem; }
  .track { flex: 1; height: 6px; border-radius: 99px; background: var(--line-soft); overflow: hidden; }
  .track span { display: block; height: 100%; background: var(--mint); border-radius: inherit; }
  .ping { display: inline-flex; align-items: center; gap: 0.3rem; color: var(--muted); }
  .ping.good { color: #7fdca0; }
  .ping.ok { color: #f2cf6b; }
  .ping.bad { color: var(--rose); }
  .where { display: inline-flex; align-items: center; gap: 0.35rem; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .id { overflow: hidden; }
  .err { color: var(--rose); margin-top: 0.6rem; }
  .more { margin-top: 0.6rem; }
</style>
