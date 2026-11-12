<script lang="ts">
  // Every optimisation session Arcadia has recorded, newest first, with the
  // value each change replaced and a way to put it back.
  import { onMount } from "svelte";
  import { ff } from "../../lib/api";
  import { toast } from "../../lib/state.svelte";
  import { LEVEL_NAME, type ChangeStatus, type OptSession, type SessionStatus } from "../../lib/fftypes";

  let { onchanged }: { onchanged?: () => void } = $props();
  let sessions: OptSession[] = $state([]);
  let loaded = $state(false);
  let busy = $state(false);
  let failedOnly = $state(false);

  const SESSION: Record<SessionStatus, string> = { inProgress: "Interrupted", applied: "Applied", failed: "Failed, put back", failedDirty: "Failed, needs attention", partlyReverted: "Partly reverted", reverted: "Reverted" };
  const CHANGE: Record<ChangeStatus, string> = { pending: "Interrupted before it finished", applied: "Applied", skipped: "Already set, not changed", failed: "Failed", rolledBack: "Put back automatically", reverted: "Reverted", revertFailed: "Could not be put back" };
  const bad = (s: OptSession) => s.status === "failed" || s.status === "failedDirty" || s.status === "inProgress";
  const shown = $derived(failedOnly ? sessions.filter(bad) : sessions);
  const when = (t: number) => new Date(t * 1000).toLocaleString();

  async function load() {
    try {
      sessions = await ff.optHistory();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      loaded = true;
    }
  }
  onMount(load);

  async function run(f: () => Promise<OptSession>, ok: string) {
    busy = true;
    try {
      const s = await f();
      if (s.error && s.status !== "reverted") toast(s.error, "error");
      else toast(ok);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = false;
      await load();
      onchanged?.();
    }
  }
</script>

<div class="bar">
  <span class="ff-note">Changes are kept on this PC in Arcadia's FrameForge history folder. Reverting writes the saved original value back and checks it.</span>
  <label class="check"><input type="checkbox" bind:checked={failedOnly} /> Failed only</label>
</div>

{#if loaded && !shown.length}
  <p class="ff-note">{failedOnly ? "No failed sessions." : "Nothing has been changed yet."}</p>
{/if}

{#each shown as s (s.id)}
  <section class="ff-card session" class:bad={bad(s)}>
    <header>
      <div>
        <div class="lead">{s.label || "Optimisation"}</div>
        <div class="ff-note">{when(s.startedAt)} · {LEVEL_NAME[s.level]} level</div>
      </div>
      <div class="row">
        <span class="pill" class:live={s.status === "applied"} class:warn={bad(s)}>{SESSION[s.status]}</span>
        {#if s.changes.some((c) => c.status === "applied" && c.reversible)}
          <button disabled={busy} onclick={() => run(() => ff.optRevertSession(s.id), "Session reverted")}>Revert all</button>
        {/if}
      </div>
    </header>
    {#if s.error}<p class="err">{s.error}</p>{/if}
    <table>
      <thead><tr><th>Change</th><th>Original value</th><th>Set to</th><th>State</th><th></th></tr></thead>
      <tbody>
        {#each s.changes as c (c.id)}
          <tr>
            <td>{c.name}<div class="mono faint">{c.key}</div></td>
            <td>{c.beforeText}</td>
            <td>{c.afterText ?? c.proposedText}</td>
            <td class:warn={c.status === "failed" || c.status === "revertFailed" || c.status === "pending"}>
              {CHANGE[c.status]}{#if c.revertedAt}<div class="faint">{when(c.revertedAt)}</div>{/if}
              {#if c.error}<div class="faint">{c.error}</div>{/if}
            </td>
            <td class="act">
              {#if c.status === "applied" && c.reversible}
                <button disabled={busy} onclick={() => run(() => ff.optRevertChange(s.id, c.id), `Reverted: ${c.name}`)}>Revert</button>
              {:else if c.status === "applied"}
                <span class="faint" title="This kind of change cannot be undone">One-way</span>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </section>
{/each}

<style>
  .bar { display: flex; justify-content: space-between; align-items: center; gap: 1rem; margin-bottom: 1rem; }
  .check { display: flex; gap: 0.4rem; align-items: center; font-size: 0.86rem; color: var(--muted); white-space: nowrap; }
  .check input { width: 15px; height: 15px; padding: 0; accent-color: var(--peach); }
  .session { margin-bottom: 1rem; }
  .session.bad { border-color: color-mix(in srgb, var(--rose) 35%, var(--line-soft)); }
  .session header { align-items: flex-start; }
  .row { display: flex; gap: 0.5rem; align-items: center; flex-shrink: 0; }
  .err { color: var(--rose); font-size: 0.88rem; }
  table { width: 100%; border-collapse: collapse; font-size: 0.86rem; }
  th { text-align: left; font-size: 0.7rem; letter-spacing: 0.08em; text-transform: uppercase; color: var(--faint); padding: 0 0.6rem 0.35rem 0; }
  td { padding: 0.45rem 0.6rem 0.45rem 0; border-top: 1px solid var(--line-soft); vertical-align: top; overflow-wrap: anywhere; }
  td.warn { color: var(--rose); }
  td.act { text-align: right; white-space: nowrap; padding-right: 0; }
  td.act button { padding: 0.25rem 0.6rem; font-size: 0.82rem; }
</style>
