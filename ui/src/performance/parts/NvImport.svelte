<script lang="ts">
  // A .nip file opened for inspection: what it holds, next to what the driver
  // has now. Values can be edited before importing; unknown entries are kept
  // in the file and shown read-only.
  import { nv, pickSavePath } from "../../lib/api";
  import { toast } from "../../lib/state.svelte";
  import type { NipFile, NvImportOutcome, NvImportPreview, NvPreviewSetting } from "../../lib/fftypes";
  import { hex } from "../../lib/nv";
  import Modal from "../../components/Modal.svelte";

  let { open = $bindable(false), preview, path, npiReady, ondone }: { open?: boolean; preview: NvImportPreview; path: string | null; npiReady: boolean; ondone?: () => void } = $props();

  // The effect below replaces this whenever another file is opened.
  // svelte-ignore state_referenced_locally
  let current: NvImportPreview = $state(structuredClone($state.snapshot(preview)));
  let chosen: Record<string, boolean> = $state({});
  let mode: "replace" | "merge" = $state("merge");
  let busy = $state(false);
  let edited = $state(false);
  let outcome: NvImportOutcome | null = $state(null);
  let confirmOpen = $state(false);

  // A new file was opened.
  $effect(() => {
    current = structuredClone($state.snapshot(preview));
    chosen = Object.fromEntries(preview.profiles.map((p) => [p.name, true]));
    edited = false;
    outcome = null;
  });

  const names = $derived(current.profiles.filter((p) => chosen[p.name]).map((p) => p.name));
  const fileName = $derived(path?.split(/[\\/]/).pop() ?? "profile file");
  const KIND = { global: "Global profile", predefined: "NVIDIA profile", user: "Your profile" };
  const editableType = (t: string) => t === "Dword" || t === "Qword" || t === "String" || t === "AnsiString";

  /** Change one value in the file, then let the backend re-check the whole file. */
  async function edit(profile: string, s: NvPreviewSetting, raw: string) {
    const file: NipFile = structuredClone($state.snapshot(current.file));
    const text = /^0x[0-9a-fA-F]+$/.test(raw.trim()) && (s.valueType === "Dword" || s.valueType === "Qword") ? BigInt(raw.trim()).toString() : raw;
    for (const p of file.profiles) if (p.name === profile) for (const x of p.settings) if (x.id === s.id) x.rawValue = text;
    try {
      current = await nv.preview(file);
      edited = true;
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function run() {
    confirmOpen = false;
    busy = true;
    try {
      outcome = await nv.importFile($state.snapshot(current.file), names, mode, `Before importing ${fileName}`);
      const ok = outcome.profiles.filter((p) => p.ok).length;
      if (outcome.verified) toast(`Imported ${ok} profile${ok === 1 ? "" : "s"} and verified`);
      else toast("The import finished with problems. See the result for details.", "error");
      ondone?.();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function saveCopy() {
    const target = await pickSavePath(fileName.replace(/\.nip$/i, "") + "-edited.nip", [{ name: "NVIDIA Profile Inspector profile", extensions: ["nip"] }]);
    if (!target) return;
    try {
      await nv.saveFile(target, $state.snapshot(current.file));
      toast("Saved");
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function withNpi() {
    if (!path) return;
    busy = true;
    try {
      toast((await nv.npiImport(path)).message);
      ondone?.();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = false;
    }
  }
</script>

<Modal bind:open title={fileName} wide>
  {#if outcome}
    <p>{outcome.verified ? "Every imported value was read back from the driver and matches." : "Not everything could be imported or verified."} A snapshot of the previous state is under Backups.</p>
    {#each outcome.profiles as r (r.profile)}
      <div class="res" class:bad={!r.ok}>
        <strong>{r.profile}</strong>
        <span>{r.ok ? `${r.created ? "created, " : ""}${r.applied} setting(s) applied` : r.error}</span>
        {#if r.skipped.length}
          <ul>{#each r.skipped as s}<li><span class="mono">{hex(s.id)}</span> {s.name}: {s.reason}</li>{/each}</ul>
        {/if}
      </div>
    {/each}
    {#each outcome.mismatches as m}<p class="err">{m}</p>{/each}
  {:else}
    {#if !current.driverAvailable}
      <div class="ff-banner"><span>The NVIDIA driver is not available ({current.driverReason}), so the file can be inspected, edited and saved, but not imported.</span></div>
    {/if}
    {#each current.warnings as w}<p class="warn">{w}</p>{/each}
    {#each current.profiles as p (p.name)}
      <section class="prof">
        <header>
          <label class="check"><input type="checkbox" bind:checked={chosen[p.name]} disabled={!current.driverAvailable} /> <strong>{p.name}</strong></label>
          <span class="pill">{p.exists && p.kind ? `Exists: ${KIND[p.kind]}` : current.driverAvailable ? "New profile" : "Not checked"}</span>
        </header>
        {#if p.executables.length}<p class="ff-note mono">{p.executables.join(", ")}</p>{/if}
        {#each p.conflicts as [exe, owner]}<p class="err">{exe} already belongs to the driver profile "{owner}". This profile cannot be imported until that is removed there.</p>{/each}
        {#if p.unknownElements}<p class="ff-note">{p.unknownElements} entr{p.unknownElements === 1 ? "y" : "ies"} from a newer file format are kept as they are.</p>{/if}
        <table>
          <thead><tr><th>Setting</th><th>In the file</th><th>In the driver now</th><th></th></tr></thead>
          <tbody>
            {#each p.settings as s (s.id)}
              <tr class:bad={s.problem}>
                <td>{s.name || "Unnamed setting"}<div class="mono faint">{s.idHex}{#if !s.documented} · undocumented{/if}</div></td>
                <td>
                  {#if editableType(s.valueType)}
                    <input class="mono" aria-label="Value of {s.name || s.idHex}" value={s.rawValue} onchange={(e) => edit(p.name, s, e.currentTarget.value)} />
                  {:else}
                    <span class="mono ro" title="Shown as stored; not editable here">{s.rawValue.length > 40 ? s.rawValue.slice(0, 40) + "…" : s.rawValue}</span>
                  {/if}
                  <div class="faint small">{s.label ?? ""}{s.label ? " · " : ""}{s.valueType}</div>
                </td>
                <td class="mono">{s.live ?? (p.exists ? "not set" : "—")}</td>
                <td class="st">{#if s.problem}<span class="err" title={s.problem}>Skipped: {s.problem}</span>{:else if s.sameAsLive}<span class="faint">Same</span>{:else}<span class="pill live">Changes</span>{/if}</td>
              </tr>
            {/each}
            {#if mode === "replace"}
              {#each p.liveOnly as s (s.id)}
                <tr class="gone"><td>{s.name || "Unnamed setting"}<div class="mono faint">{s.idHex}</div></td><td class="faint">not in the file</td><td class="mono">{s.live}</td><td class="st"><span class="pill">Restored</span></td></tr>
              {/each}
            {/if}
          </tbody>
        </table>
        {#if !p.settings.length}<p class="ff-note">This profile has no settings in the file.</p>{/if}
      </section>
    {/each}
    {#if current.driverAvailable}
      <div class="mode">
        <div class="seg" role="radiogroup" aria-label="Import mode">
          <button role="radio" aria-checked={mode === "merge"} onclick={() => (mode = "merge")}>Merge</button>
          <button role="radio" aria-checked={mode === "replace"} onclick={() => (mode = "replace")}>Replace</button>
        </div>
        <span class="ff-note">{mode === "merge" ? "Adds the file's values on top of what the profile has." : "Makes each profile match the file: your other changes to it are restored to NVIDIA's values."}</span>
      </div>
    {/if}
  {/if}
  {#snippet actions()}
    {#if outcome}
      <button onclick={() => (open = false)}>Close</button>
    {:else}
      {#if npiReady && path}<button class="ghost" disabled={busy} onclick={withNpi} title="Fallback: hands the original file to your NVIDIA Profile Inspector, which replaces the profiles named in it">Import with Profile Inspector</button>{/if}
      <button class="ghost" onclick={saveCopy}>{edited ? "Save edited copy…" : "Save a copy…"}</button>
      <button class="ghost" onclick={() => (open = false)}>Close</button>
      <button class="play" disabled={!current.driverAvailable || !names.length || busy} onclick={() => (confirmOpen = true)}>{busy ? "Importing…" : `Import ${names.length} profile${names.length === 1 ? "" : "s"}`}</button>
    {/if}
  {/snippet}
</Modal>

<Modal bind:open={confirmOpen} title="Import into the NVIDIA driver?">
  <p>{names.join(", ")}</p>
  <p>Arcadia snapshots these profiles first, then {mode === "merge" ? "adds the file's values" : "replaces them with the file's contents"}, saves, and reads every value back. Entries marked "Skipped" are left out. You can undo the whole import from Backups.</p>
  {#snippet actions()}
    <button class="ghost" onclick={() => (confirmOpen = false)}>Cancel</button>
    <button class="play" onclick={run}>Import</button>
  {/snippet}
</Modal>

<style>
  .prof { border: 1px solid var(--line-soft); border-radius: var(--r-md); padding: 0.8rem 1rem; display: grid; gap: 0.5rem; }
  .prof header { display: flex; justify-content: space-between; align-items: center; gap: 1rem; }
  .check { display: flex; gap: 0.5rem; align-items: center; color: var(--text); }
  .check input { width: 15px; height: 15px; padding: 0; accent-color: var(--peach); }
  table { width: 100%; border-collapse: collapse; font-size: 0.84rem; }
  th { text-align: left; font-size: 0.68rem; letter-spacing: 0.08em; text-transform: uppercase; color: var(--faint); padding: 0 0.6rem 0.3rem 0; }
  td { padding: 0.35rem 0.6rem 0.35rem 0; border-top: 1px solid var(--line-soft); vertical-align: top; color: var(--text); overflow-wrap: anywhere; }
  td input { width: 100%; padding: 0.25rem 0.45rem; font-size: 0.82rem; }
  tr.bad td { background: color-mix(in srgb, var(--rose) 7%, transparent); }
  tr.gone td { opacity: 0.75; }
  .st { width: 150px; }
  .ro { color: var(--muted); }
  .small { font-size: 0.74rem; }
  .err { color: var(--rose); font-size: 0.84rem; }
  .warn { color: var(--muted); font-size: 0.86rem; }
  .mode { display: flex; gap: 0.8rem; align-items: center; }
  .res { border: 1px solid var(--line-soft); border-radius: var(--r-md); padding: 0.6rem 0.8rem; display: grid; gap: 0.25rem; color: var(--text); }
  .res.bad { border-color: var(--rose); }
  .res ul { margin: 0; padding-left: 1.1rem; font-size: 0.82rem; color: var(--muted); }
</style>
