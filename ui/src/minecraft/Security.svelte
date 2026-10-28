<script lang="ts">
  import { onMount } from "svelte";
  import { mc, openUrl } from "../lib/api";
  import { toast } from "../lib/state.svelte";
  import { mcs } from "../lib/mc.svelte";
  import type { Incident, SecurityReport } from "../lib/types";

  let incidents: Incident[] = $state([]);
  let report = $state<SecurityReport | null>(null);
  let scanning = $state(false);

  onMount(async () => {
    incidents = await mc.incidents();
    scan();
  });

  async function scan() {
    scanning = true;
    try {
      report = await mc.securityScan();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      scanning = false;
    }
  }

  const incidentName = (id: string) => incidents.find((i) => i.id === id)?.name ?? id;
  const clean = $derived(report && report.findings.length === 0 && report.leftovers.length === 0 && report.pipeblockerAdvised.length === 0);
  function host(u: string) {
    try {
      return new URL(u).hostname.replace(/^www\./, "");
    } catch {
      return u;
    }
  }
</script>

<div class="page">
  <div class="page-inner">
    <section class="section">
      <div class="row spread">
        <h3>Your instances</h3>
        <button onclick={scan} disabled={scanning}>{scanning ? "Checking…" : "Check again"}</button>
      </div>
      {#if !report}
        <p class="hint">Checking your mods against known incidents…</p>
      {:else if clean}
        <div class="verdict ok">
          <strong>Nothing found.</strong>
          <span>Checked {report.checkedInstances} instance{report.checkedInstances === 1 ? "" : "s"} against the incidents below, and looked for files fractureiser leaves behind.</span>
        </div>
      {:else}
        {#if report.leftovers.length}
          <div class="verdict bad">
            <strong>Files left by fractureiser were found on this PC.</strong>
            <span>Don't just delete them. Run a full antivirus scan, change your passwords from another device, and follow the fractureiser player guide.</span>
            <ul>{#each report.leftovers as l}<li><span class="mono">{l.path}</span> <span class="faint">{l.what}</span></li>{/each}</ul>
          </div>
        {/if}
        {#if report.findings.length}
          <p class="hint">These mods were named in a past incident. Most were fixed long ago; the note says which versions were affected.</p>
          <ul class="findings">
            {#each report.findings as f}
              <li>
                <span class="tag">{incidentName(f.incident)}</span>
                <div class="txt">
                  <span><strong>{f.modName}</strong> in <button class="link" onclick={() => { mcs.selected = f.instanceId; mcs.tab = "instances"; }}>{f.instanceName}</button></span>
                  <span class="muted note">{f.note}</span>
                  <span class="mono faint file">{f.fileName}</span>
                </div>
              </li>
            {/each}
          </ul>
        {/if}
        {#if report.pipeblockerAdvised.length}
          <div class="verdict warn">
            <strong>Add PipeBlocker to: {report.pipeblockerAdvised.join(", ")}</strong>
            <span>These run Forge 1.7.2 to 1.12.2, where BleedingPipe hit hardest. PipeBlocker, from the Minecraft Malware Prevention Alliance, blocks that whole class of exploit.</span>
          </div>
        {/if}
      {/if}
    </section>

    <section class="section">
      <h3>Known incidents</h3>
      <p class="hint">Past security problems in Minecraft and its mods that Arcadia checks for.</p>
      <div class="incidents">
        {#each incidents as i (i.id)}
          <article class="incident">
            <header>
              <h4>{i.name}</h4>
              <span class="faint">{i.date}</span>
            </header>
            <span class="kind">{i.kind}</span>
            <p>{i.summary}</p>
            <p class="todo"><strong>What to do:</strong> {i.whatToDo}</p>
            <div class="row wrap links">
              {#each i.links as l}<button class="ghost" onclick={() => openUrl(l)}>{host(l)}</button>{/each}
            </div>
          </article>
        {/each}
      </div>
    </section>
  </div>
</div>

<style>
  .verdict { display: grid; gap: 0.3rem; padding: 0.9rem 1.1rem; border-radius: var(--r-md); border: 1px solid; margin-bottom: 0.9rem; font-size: 0.92rem; }
  .verdict.ok { border-color: color-mix(in srgb, var(--mint) 40%, transparent); background: color-mix(in srgb, var(--mint) 8%, transparent); }
  .verdict.bad { border-color: color-mix(in srgb, var(--rose) 55%, transparent); background: color-mix(in srgb, var(--rose) 10%, transparent); }
  .verdict.warn { border-color: color-mix(in srgb, var(--peach) 45%, transparent); background: color-mix(in srgb, var(--peach) 8%, transparent); }
  .verdict ul { margin: 0.4rem 0 0; padding-left: 1.1rem; }

  .findings { list-style: none; margin: 0 0 1rem; padding: 0; }
  .findings li { display: flex; gap: 0.9rem; align-items: flex-start; padding: 0.6rem 0; border-top: 1px solid var(--line-soft); }
  .tag { font-size: 0.75rem; font-weight: 700; padding: 0.1rem 0.5rem; border-radius: 999px; background: var(--sunk); border: 1px solid var(--line); white-space: nowrap; margin-top: 0.1rem; }
  .txt { display: grid; gap: 0.1rem; min-width: 0; }
  .note { font-size: 0.86rem; }
  .file { font-size: 0.75rem; }
  .link { all: unset; cursor: pointer; color: var(--lilac); font-weight: 600; }
  .link:focus-visible { outline: 2px solid var(--lilac); }

  .incidents { display: grid; gap: 0.9rem; padding-bottom: 0.6rem; }
  .incident { display: grid; gap: 0.45rem; padding: 1rem 1.1rem; border: 1px solid var(--line-soft); border-radius: var(--r-md); background: var(--sunk); }
  .incident header { display: flex; align-items: baseline; justify-content: space-between; gap: 1rem; }
  h4 { font-size: 1.1rem; }
  .kind { color: var(--peach); font-weight: 600; font-size: 0.85rem; }
  .incident p { color: var(--muted); font-size: 0.9rem; max-width: 75ch; }
  .todo { color: var(--text) !important; }
  .links { gap: 0.2rem; }
  .links button { font-size: 0.82rem; padding: 0.3rem 0.6rem; }
</style>
