<script lang="ts">
  // Compare two .nip files. Works without an NVIDIA driver.
  import { nv, pickFile } from "../../lib/api";
  import { toast } from "../../lib/state.svelte";
  import type { NipDiff } from "../../lib/fftypes";
  import { hex } from "../../lib/nv";
  import Modal from "../../components/Modal.svelte";

  let { open = $bindable(false) }: { open?: boolean } = $props();
  let a: string | null = $state(null);
  let b: string | null = $state(null);
  let diff: NipDiff | null = $state(null);
  let busy = $state(false);
  let showSame = $state(false);

  const NIP = [{ name: "NVIDIA Profile Inspector profile", extensions: ["nip"] }];
  const base = (p: string | null) => p?.split(/[\\/]/).pop() ?? "Choose a file…";
  const STATUS = { onlyA: "Only in the first file", onlyB: "Only in the second file", changed: "Different", same: "Same" };

  async function pick(which: "a" | "b") {
    const p = await pickFile(NIP);
    if (!p) return;
    if (which === "a") a = p;
    else b = p;
    diff = null;
    if (a && b) run();
  }

  async function run() {
    if (!a || !b) return;
    busy = true;
    try {
      diff = await nv.compare(a, b);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = false;
    }
  }
</script>

<Modal bind:open title="Compare two profile files" wide>
  <div class="picks">
    <button onclick={() => pick("a")} title={a ?? ""}>1 · {base(a)}</button>
    <button onclick={() => pick("b")} title={b ?? ""}>2 · {base(b)}</button>
  </div>
  {#if busy}<p class="ff-note">Comparing…</p>{/if}
  {#if diff}
    {#if diff.identical}<p>The two files hold the same profiles, executables and values.</p>{/if}
    <label class="check"><input type="checkbox" bind:checked={showSame} /> Show profiles that are the same</label>
    {#each diff.profiles.filter((p) => showSame || p.status !== "same") as p (p.name)}
      <section class="p">
        <header><strong>{p.name}</strong><span class="pill" class:live={p.status === "changed"}>{STATUS[p.status]}</span></header>
        {#if p.executablesOnlyA.length}<p class="ff-note">Executables only in the first: <span class="mono">{p.executablesOnlyA.join(", ")}</span></p>{/if}
        {#if p.executablesOnlyB.length}<p class="ff-note">Executables only in the second: <span class="mono">{p.executablesOnlyB.join(", ")}</span></p>{/if}
        {#if p.settings.length}
          <table>
            <thead><tr><th>Setting</th><th>First file</th><th>Second file</th></tr></thead>
            <tbody>
              {#each p.settings as s (s.id)}
                <tr><td>{s.name || "Unnamed setting"}<div class="mono faint">{hex(s.id)}</div></td><td class="mono">{s.a ?? "—"}{#if s.aType && s.aType !== "Dword"}<span class="faint"> ({s.aType})</span>{/if}</td><td class="mono">{s.b ?? "—"}{#if s.bType && s.bType !== "Dword"}<span class="faint"> ({s.bType})</span>{/if}</td></tr>
              {/each}
            </tbody>
          </table>
        {/if}
        {#if p.status === "changed"}<p class="ff-note">{p.settingsSame} other setting{p.settingsSame === 1 ? " is" : "s are"} the same.</p>{/if}
      </section>
    {/each}
  {:else if !busy}
    <p class="ff-note">Choose two .nip files. Values are compared by setting ID, so files from different Profile Inspector versions compare correctly.</p>
  {/if}
  {#snippet actions()}<button onclick={() => (open = false)}>Close</button>{/snippet}
</Modal>

<style>
  .picks { display: grid; grid-template-columns: 1fr 1fr; gap: 0.6rem; }
  .picks button { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-align: left; }
  .check { display: flex; gap: 0.4rem; align-items: center; font-size: 0.86rem; }
  .check input { width: 15px; height: 15px; padding: 0; accent-color: var(--peach); }
  .p { border: 1px solid var(--line-soft); border-radius: var(--r-md); padding: 0.7rem 0.9rem; display: grid; gap: 0.4rem; color: var(--text); }
  .p header { display: flex; justify-content: space-between; align-items: center; gap: 1rem; }
  table { width: 100%; border-collapse: collapse; font-size: 0.84rem; }
  th { text-align: left; font-size: 0.66rem; letter-spacing: 0.08em; text-transform: uppercase; color: var(--faint); padding: 0 0.6rem 0.3rem 0; }
  td { padding: 0.3rem 0.6rem 0.3rem 0; border-top: 1px solid var(--line-soft); vertical-align: top; overflow-wrap: anywhere; }
</style>
