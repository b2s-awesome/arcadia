<script lang="ts">
  // Background apps: what is running, what it costs, and the few things
  // Arcadia will do about it. Protected processes are listed but have no controls.
  import { onMount } from "svelte";
  import { ff } from "../../lib/api";
  import { toast } from "../../lib/state.svelte";
  import { num, size } from "../../lib/ff.svelte";
  import type { ProcessInfo } from "../../lib/fftypes";
  import Icon from "../../components/Icon.svelte";
  import Modal from "../../components/Modal.svelte";

  let { onchanged }: { onchanged?: () => void } = $props();

  let list: ProcessInfo[] = $state([]);
  let loading = $state(false);
  let query = $state("");
  let filter: "relevant" | "all" | "games" = $state("relevant");
  let sort: "cpu" | "memory" | "gpu" | "name" = $state("memory");
  let picked: Record<number, boolean> = $state({});
  let closing = $state(false);
  let ending: ProcessInfo | null = $state(null);
  let endOpen = $state(false);
  let prio: { p: ProcessInfo; cls: string } | null = $state(null);
  let prioOpen = $state(false);
  let busy = $state(false);

  const PRIORITIES = [["High", "High"], ["AboveNormal", "Above normal"], ["Normal", "Normal"], ["BelowNormal", "Below normal"], ["Idle", "Idle"]];
  const CAT: Record<string, string> = { game: "Game", launcher: "Launcher", browser: "Browser", overlay: "Overlay or recorder", chat: "Chat or media", protected: "Protected", app: "App" };

  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return list
      .filter((p) => (filter === "all" ? true : filter === "games" ? p.category === "game" : p.category !== "protected" && (p.relevance !== "" || p.category === "game")))
      .filter((p) => !q || p.name.toLowerCase().includes(q) || String(p.pid).includes(q) || (p.exe ?? "").toLowerCase().includes(q))
      .sort((a, b) => (sort === "name" ? a.name.localeCompare(b.name) : sort === "cpu" ? b.cpuPct - a.cpuPct : sort === "gpu" ? (b.gpuPct ?? -1) - (a.gpuPct ?? -1) : b.memoryBytes - a.memoryBytes));
  });
  const chosen = $derived(list.filter((p) => picked[p.pid] && p.closable));

  async function load() {
    loading = true;
    try {
      list = await ff.processes();
      for (const k of Object.keys(picked)) if (!list.some((p) => p.pid === Number(k))) delete picked[Number(k)];
    } catch (e) {
      toast(String(e), "error");
    } finally {
      loading = false;
    }
  }
  onMount(load);

  async function closeChosen() {
    closing = false;
    busy = true;
    let asked = 0;
    const failed: string[] = [];
    // One recorded session per app, so one app without a window does not stop the others.
    for (const p of chosen) {
      try {
        const s = await ff.optApply([{ id: "process.close", params: { pid: p.pid } }], 1, `Asked ${p.name} to close`);
        if (s.status === "applied") asked += 1;
        else failed.push(`${p.name}: ${s.changes[0]?.error ?? s.error ?? "failed"}`);
      } catch (e) {
        failed.push(`${p.name}: ${e}`);
      }
    }
    busy = false;
    picked = {};
    if (asked) toast(`Asked ${asked} app${asked === 1 ? "" : "s"} to close`);
    if (failed.length) toast(failed.join(" · "), "error");
    onchanged?.();
    setTimeout(load, 1200);
  }

  async function end() {
    const p = ending;
    endOpen = false;
    if (!p) return;
    busy = true;
    try {
      const s = await ff.optApply([{ id: "process.end", params: { pid: p.pid } }], 2, `Ended ${p.name}`);
      if (s.status === "applied") toast(`${p.name} was ended`);
      else toast(s.error ?? "The process could not be ended", "error");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = false;
      onchanged?.();
      load();
    }
  }

  async function setPriority() {
    const x = prio;
    prioOpen = false;
    if (!x) return;
    busy = true;
    try {
      const s = await ff.optApply([{ id: "process.priority", params: { pid: x.p.pid, class: x.cls } }], 2, `Priority of ${x.p.name}`);
      if (s.status === "applied") toast(s.changes[0]?.status === "skipped" ? `${x.p.name} already has that priority` : `${x.p.name} priority changed until it exits or you revert it in History`);
      else toast(s.error ?? "The priority could not be changed", "error");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = false;
      onchanged?.();
    }
  }
</script>

<div class="bar">
  <div class="row wrap">
    <input type="search" placeholder="Search name, PID or path" bind:value={query} aria-label="Search processes" />
    <div class="seg" role="radiogroup" aria-label="Which processes">
      {#each [["relevant", "Worth a look"], ["games", "Games"], ["all", "All"]] as [id, label]}
        <button role="radio" aria-checked={filter === id} onclick={() => (filter = id as typeof filter)}>{label}</button>
      {/each}
    </div>
    <select bind:value={sort} aria-label="Sort by">
      <option value="memory">Sort: memory</option>
      <option value="cpu">Sort: CPU</option>
      <option value="gpu">Sort: GPU</option>
      <option value="name">Sort: name</option>
    </select>
  </div>
  <div class="row">
    <button class="ghost" disabled={loading} onclick={load}><Icon name="refresh" size={15} /> Refresh</button>
    <button disabled={!chosen.length || busy} onclick={() => (closing = true)}>Ask {chosen.length || ""} selected to close</button>
  </div>
</div>

<p class="ff-note">Arcadia never closes anything by itself. Windows components, drivers, security software and anti-cheat are protected and have no controls.</p>

<div class="ff-card wrap-table">
  <table>
    <thead>
      <tr><th></th><th>Process</th><th class="n">PID</th><th class="n">CPU</th><th class="n">Memory</th><th class="n">GPU</th><th>Owner</th><th>Why it may matter</th><th></th></tr>
    </thead>
    <tbody>
      {#each shown as p (p.pid)}
        <tr class:prot={p.category === "protected"}>
          <td class="pick">{#if p.closable}<input type="checkbox" aria-label="Select {p.name}" bind:checked={picked[p.pid]} />{/if}</td>
          <td title={p.exe ?? "Path not accessible"}>
            <div class="pname">{p.gameName ?? p.name}{#if p.gameName} <span class="pill live">Library game</span>{/if}</div>
            <div class="mono faint path">{p.exe ?? p.name}</div>
          </td>
          <td class="n">{p.pid}</td>
          <td class="n">{num(p.cpuPct, 1)}%</td>
          <td class="n">{size(p.memoryBytes)}</td>
          <td class="n" title={p.gpuPct === null ? "Windows did not report GPU use for this process" : undefined}>{p.gpuPct === null ? "—" : `${num(p.gpuPct)}%`}</td>
          <td>{p.owner ?? "—"}</td>
          <td class="rel">{#if p.category === "protected"}<span class="faint">{p.protectedReason}</span>{:else}<span class="pill">{CAT[p.category]}</span> {p.relevance}{/if}</td>
          <td class="act">
            {#if p.category !== "protected"}
              <select aria-label="Priority of {p.name}" disabled={busy} value="" onchange={(e) => { const v = e.currentTarget.value; e.currentTarget.value = ""; if (v) { prio = { p, cls: v }; prioOpen = true; } }}>
                <option value="">Priority…</option>
                {#each PRIORITIES as [v, l]}<option value={v}>{l}</option>{/each}
              </select>
            {/if}
            {#if p.closable}<button class="danger" disabled={busy} onclick={() => { ending = p; endOpen = true; }}>End</button>{/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
  {#if !shown.length}<p class="ff-note empty">{loading ? "Reading processes…" : list.length ? "Nothing matches." : "The process list is not available on this system."}</p>{/if}
</div>

<Modal bind:open={closing} title="Ask these apps to close?">
  <p>Each app gets the same request as clicking its close button, so it can still ask you to save your work. Arcadia does not reopen them.</p>
  <ul>{#each chosen as p (p.pid)}<li>{p.name} <span class="faint">({size(p.memoryBytes)})</span></li>{/each}</ul>
  {#snippet actions()}
    <button class="ghost" onclick={() => (closing = false)}>Cancel</button>
    <button onclick={closeChosen}>Ask to close</button>
  {/snippet}
</Modal>

<Modal bind:open={endOpen} title="End {ending?.name ?? 'process'}?">
  <p>This stops the process immediately. It does not get the chance to save, so unsaved work in it is lost. This cannot be undone.</p>
  <p>Try "Ask to close" first where you can.</p>
  {#snippet actions()}
    <button class="ghost" onclick={() => (endOpen = false)}>Cancel</button>
    <button class="danger" onclick={end}>End process</button>
  {/snippet}
</Modal>

<Modal bind:open={prioOpen} title="Change priority of {prio?.p.name ?? ''}?">
  <p>Sets the priority class to <strong>{PRIORITIES.find((x) => x[0] === prio?.cls)?.[1]}</strong> until the process exits or you revert it in History. Realtime is never used.</p>
  <p>A higher priority only helps when the processor is the limit, and can starve voice chat or streaming apps.</p>
  {#snippet actions()}
    <button class="ghost" onclick={() => (prioOpen = false)}>Cancel</button>
    <button onclick={setPriority}>Change priority</button>
  {/snippet}
</Modal>

<style>
  .bar { display: flex; justify-content: space-between; align-items: center; gap: 1rem; margin-bottom: 0.75rem; flex-wrap: wrap; }
  .row { display: flex; gap: 0.5rem; align-items: center; }
  .row.wrap { flex-wrap: wrap; }
  input[type="search"] { width: 240px; }
  .wrap-table { margin-top: 0.75rem; padding: 0.4rem 0.9rem 0.6rem; overflow-x: auto; display: block; }
  table { width: 100%; border-collapse: collapse; font-size: 0.85rem; font-variant-numeric: tabular-nums; }
  th { text-align: left; font-size: 0.68rem; letter-spacing: 0.08em; text-transform: uppercase; color: var(--faint); padding: 0.5rem 0.6rem 0.4rem 0; white-space: nowrap; }
  td { padding: 0.45rem 0.6rem 0.45rem 0; border-top: 1px solid var(--line-soft); vertical-align: middle; }
  .n { text-align: right; white-space: nowrap; }
  .pick { width: 22px; }
  .pick input { width: 15px; height: 15px; padding: 0; accent-color: var(--peach); }
  .pname { font-weight: 600; }
  .path { max-width: 320px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rel { color: var(--muted); max-width: 300px; }
  .prot td { color: var(--faint); }
  .act { text-align: right; white-space: nowrap; padding-right: 0; }
  .act select, .act button { padding: 0.25rem 0.5rem; font-size: 0.8rem; }
  .act select { width: auto; }
  .empty { padding: 1rem 0; }
  ul { margin: 0; padding-left: 1.1rem; color: var(--text); }
</style>
