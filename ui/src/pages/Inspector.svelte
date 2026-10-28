<script lang="ts">
  import { onMount } from "svelte";
  import { api, on } from "../lib/api";
  import { bytes } from "../lib/state.svelte";
  import type { Entry } from "../lib/types";

  let entries: Entry[] = $state([]);
  let text = $state("");
  let kind: "all" | "changed" | "errors" = $state("all");
  let paused = $state(false);
  let selected: Entry | null = $state(null);

  onMount(() => {
    api.inspectorSnapshot().then((e) => (entries = e));
    const off = on<Entry>("proxy-entry", (e) => {
      if (paused) return;
      entries.push(e);
      if (entries.length > 2000) entries.splice(0, entries.length - 2000);
    });
    return () => off.then((f) => f());
  });

  const shown = $derived(
    entries
      .filter((e) => {
        if (kind === "changed" && !["modified", "served-file", "blocked"].includes(e.kind)) return false;
        if (kind === "errors" && e.kind !== "error" && e.status < 400) return false;
        const t = text.trim().toLowerCase();
        return !t || e.url.toLowerCase().includes(t) || e.note.toLowerCase().includes(t);
      })
      .slice(-500)
      .reverse(),
  );

  const kindLabel: Record<Entry["kind"], string> = {
    tunnel: "Passed through",
    passthrough: "Unchanged",
    modified: "Changed",
    "served-file": "Your file",
    blocked: "Removed",
    error: "Failed",
  };
  const time = (ms: number) => new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  function shortUrl(u: string) {
    return u.replace(/^https:\/\//, "");
  }
</script>

<div class="inspector">
  <header class="bar">
    <div>
      <h2>Inspector</h2>
      <p class="muted">Every request Roblox sends through the proxy. Logins, chat and purchases pass through encrypted and are never read.</p>
    </div>
    <div class="row">
      <button class="ghost" onclick={() => (paused = !paused)}>{paused ? "Resume" : "Pause"}</button>
      <button class="ghost" onclick={async () => { await api.inspectorClear(); entries = []; selected = null; }}>Clear</button>
    </div>
  </header>

  <div class="filters row">
    <input class="grow" placeholder="Filter by address or note" bind:value={text} aria-label="Filter requests" />
    <div class="seg" role="radiogroup" aria-label="Show">
      <button role="radio" aria-checked={kind === "all"} onclick={() => (kind = "all")}>Everything</button>
      <button role="radio" aria-checked={kind === "changed"} onclick={() => (kind = "changed")}>Changed by rules</button>
      <button role="radio" aria-checked={kind === "errors"} onclick={() => (kind = "errors")}>Problems</button>
    </div>
  </div>

  <div class="split" class:open={selected}>
    <div class="table-wrap">
      {#if shown.length === 0}
        <p class="empty">
          {entries.length === 0 ? "Nothing yet. Start Roblox with the asset proxy turned on and requests appear here as they happen." : "No requests match this filter."}
        </p>
      {:else}
        <table>
          <thead><tr><th>Time</th><th>Request</th><th>Result</th><th class="num">Size</th><th class="num">Took</th></tr></thead>
          <tbody>
            {#each shown as e (e.id)}
              <tr class:sel={selected?.id === e.id} onclick={() => (selected = e)} tabindex="0" onkeydown={(k) => k.key === "Enter" && (selected = e)}>
                <td class="mono faint nowrap">{time(e.timeMs)}</td>
                <td class="url"><span class="method">{e.method}</span> <span class="mono">{shortUrl(e.url)}</span></td>
                <td><span class="k k-{e.kind}">{kindLabel[e.kind]}</span>{#if e.note}<span class="note">{e.note}</span>{/if}</td>
                <td class="num mono">{bytes(e.respBytes)}</td>
                <td class="num mono">{e.durationMs} ms</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>

    {#if selected}
      <aside class="detail">
        <div class="row spread"><h3>{selected.method} {selected.status}</h3><button class="ghost" onclick={() => (selected = null)}>Close</button></div>
        <p class="mono url-full">{selected.url}</p>
        {#if selected.note}<p class="muted">{selected.note}</p>{/if}
        {#if selected.reqBody}<h4>Sent</h4><pre>{selected.reqBody}</pre>{/if}
        {#if selected.respBody}<h4>Received</h4><pre>{selected.respBody}</pre>{/if}
        {#if !selected.reqBody && !selected.respBody}<p class="faint">Bodies are only kept for requests a rule changed.</p>{/if}
      </aside>
    {/if}
  </div>
</div>

<style>
  .inspector { height: 100%; display: flex; flex-direction: column; padding: 2rem 2.5rem 0; }
  .bar { display: flex; justify-content: space-between; align-items: flex-end; gap: 1rem; }
  .bar p { margin-top: 0.3rem; max-width: 70ch; }
  .filters { margin: 1.2rem 0 0.8rem; }

  .split { flex: 1; min-height: 0; display: grid; grid-template-columns: 1fr; gap: 1rem; }
  .split.open { grid-template-columns: 1fr 380px; }
  .table-wrap { overflow: auto; padding-bottom: 2rem; }
  table { width: 100%; border-collapse: collapse; font-size: 0.86rem; }
  th { position: sticky; top: 0; background: var(--panel); backdrop-filter: blur(12px); text-align: left; color: var(--faint); font-weight: 600; padding: 0.4rem 0.6rem; border-bottom: 1px solid var(--line); }
  td { padding: 0.45rem 0.6rem; border-bottom: 1px solid var(--line-soft); vertical-align: top; }
  tr { cursor: pointer; }
  tbody tr:hover { background: var(--raised); }
  tr.sel { background: var(--raised); box-shadow: inset 3px 0 0 var(--lilac); }
  .num { text-align: right; white-space: nowrap; }
  .nowrap { white-space: nowrap; }
  .url { max-width: 0; width: 55%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .method { font-weight: 700; color: var(--muted); font-size: 0.78rem; }
  .k { font-weight: 600; white-space: nowrap; }
  .note { color: var(--muted); margin-left: 0.5rem; }
  .k-tunnel, .k-passthrough { color: var(--faint); }
  .k-modified { color: var(--lilac); }
  .k-served-file { color: var(--mint); }
  .k-blocked, .k-error { color: var(--rose); }
  .empty { color: var(--muted); padding: 2rem 0; max-width: 60ch; }

  .detail { overflow: auto; border-left: 1px solid var(--line-soft); padding: 0 0 2rem 1rem; user-select: text; }
  .url-full { word-break: break-all; font-size: 0.8rem; margin: 0.4rem 0 0.8rem; }
  h4 { margin: 1rem 0 0.3rem; font-size: 0.9rem; color: var(--muted); }
  pre { background: var(--sunk); border: 1px solid var(--line-soft); border-radius: var(--r-sm); padding: 0.7rem; font-size: 0.78rem; white-space: pre-wrap; word-break: break-all; max-height: 40vh; overflow: auto; margin: 0; }
</style>
