<script lang="ts">
  import { onMount } from "svelte";
  import { mc, pickFile, pickFolder } from "../lib/api";
  import { toast } from "../lib/state.svelte";
  import { mcs, refreshInstances } from "../lib/mc.svelte";
  import type { FoundInstance } from "../lib/types";

  let found: FoundInstance[] = $state([]);
  let busy: string | null = $state(null);
  let done = $state(new Set<string>());

  onMount(async () => {
    found = await mc.discover();
  });

  async function run(path: string) {
    busy = path;
    try {
      const r = await mc.importPath(path);
      await refreshInstances();
      done = new Set(done).add(path);
      toast(`Imported ${r.instance.name}`);
      r.warnings.forEach((w) => toast(w, "error"));
      mcs.selected = r.instance.id;
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = null;
    }
  }
  async function fromFile() {
    const p = await pickFile([{ name: "Modpacks", extensions: ["mrpack", "zip"] }]);
    if (p) run(p);
  }
  async function fromFolder() {
    const p = await pickFolder();
    if (p) run(p);
  }
</script>

<div class="page">
  <div class="page-inner">
    <section class="section">
      <h3>From a file</h3>
      <p class="hint">A Modrinth pack (<span class="mono">.mrpack</span>), a CurseForge modpack <span class="mono">.zip</span>, or an instance exported from Prism Launcher or MultiMC. No other launcher needs to be installed.</p>
      <div class="row">
        <button onclick={fromFile} disabled={busy !== null}>Choose a pack file</button>
        <button onclick={fromFolder} disabled={busy !== null}>Choose an instance folder</button>
      </div>
    </section>

    <section class="section">
      <h3>Found on this PC</h3>
      {#if found.length === 0}
        <p class="hint">No Prism Launcher or CurseForge app instances were found. If you keep them somewhere unusual, use “Choose an instance folder” above.</p>
      {:else}
        <p class="hint">Instances from other launchers. Importing copies them; the originals aren't changed.</p>
        <ul class="found">
          {#each found as f (f.path)}
            <li>
              <div class="txt">
                <span class="name">{f.name}</span>
                <span class="mono faint p">{f.path}</span>
              </div>
              <span class="faint from">{f.launcher}</span>
              {#if done.has(f.path)}
                <span class="ok">Imported</span>
              {:else}
                <button onclick={() => run(f.path)} disabled={busy !== null}>{busy === f.path ? "Importing…" : "Import"}</button>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  </div>
</div>

<style>
  .found { list-style: none; margin: 0; padding: 0; }
  .found li { display: flex; align-items: center; gap: 1rem; padding: 0.65rem 0; border-top: 1px solid var(--line-soft); }
  .found li:first-child { border-top: 0; }
  .txt { flex: 1; min-width: 0; display: grid; }
  .name { font-weight: 600; }
  .p { font-size: 0.75rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .from { font-size: 0.85rem; white-space: nowrap; }
  .ok { color: var(--mint); font-weight: 600; font-size: 0.88rem; }
</style>
