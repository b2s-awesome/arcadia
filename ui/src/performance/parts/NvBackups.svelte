<script lang="ts">
  // Snapshots Arcadia took before each driver change, plus manual ones.
  // Each is an ordinary .nip file; restoring one is itself snapshotted.
  import { nv } from "../../lib/api";
  import { toast } from "../../lib/state.svelte";
  import type { NvBackup } from "../../lib/fftypes";
  import { hex } from "../../lib/nv";
  import Modal from "../../components/Modal.svelte";

  let { open = $bindable(false), selected, available, ondone }: { open?: boolean; selected: string | null; available: boolean; ondone?: () => void } = $props();

  let list: NvBackup[] = $state([]);
  let label = $state("");
  let busy = $state(false);
  let shown: Record<string, boolean> = $state({});
  let restoring: NvBackup | null = $state(null);
  let restoreOpen = $state(false);

  const REASON = { edit: "Before an edit", import: "Before an import", manual: "Manual", restore: "Before a restore", delete: "Before a delete" };
  const when = (t: number) => new Date(t * 1000).toLocaleString();

  async function load() {
    try {
      list = await nv.backups();
    } catch (e) {
      toast(String(e), "error");
    }
  }
  $effect(() => {
    if (open) load();
  });

  async function backup(names: string[]) {
    busy = true;
    try {
      const b = await nv.backupNow(names, label);
      label = "";
      toast(`Saved a snapshot of ${b.profiles.length} profile${b.profiles.length === 1 ? "" : "s"}`);
      await load();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function restore() {
    const b = restoring;
    restoreOpen = false;
    if (!b) return;
    busy = true;
    try {
      const out = await nv.restoreBackup(b.id);
      if (out.verified) toast("Restored and verified");
      else toast(`Restored with problems: ${[...out.profiles.filter((p) => !p.ok).map((p) => `${p.profile}: ${p.error}`), ...out.mismatches].join("; ")}`, "error");
      ondone?.();
      await load();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function remove(b: NvBackup) {
    try {
      await nv.deleteBackup(b.id);
      await load();
    } catch (e) {
      toast(String(e), "error");
    }
  }
</script>

<Modal bind:open title="Profile backups" wide>
  {#if available}
    <div class="new">
      <input placeholder="Name for a new snapshot (optional)" bind:value={label} aria-label="Snapshot name" />
      <button disabled={busy || !selected} onclick={() => selected && backup([selected])} title={selected ?? ""}>Back up this profile</button>
      <button class="ghost" disabled={busy} onclick={() => backup([])}>Back up all changed</button>
    </div>
  {/if}
  <p class="ff-note">Arcadia saves one of these before every change it makes to the driver. They are .nip files in Arcadia's FrameForge data folder and are never deleted automatically.</p>
  {#if !list.length}<p class="ff-note">No snapshots yet.</p>{/if}
  {#each list as b (b.id)}
    <section class="b">
      <header>
        <div>
          <strong>{b.label}</strong>
          <div class="ff-note">{when(b.createdAt)} · {REASON[b.reason]}{b.driverVersion ? ` · driver ${b.driverVersion}` : ""}</div>
          <div class="ff-note">{[...b.profiles, ...b.createdProfiles.map((p) => `${p} (did not exist yet)`)].join(", ") || "No profiles"}</div>
        </div>
        <div class="row">
          {#if b.verified === true}<span class="pill live" title="The change this snapshot preceded was read back and matched">Change verified</span>
          {:else if b.verified === false}<span class="pill warn" title={b.note ?? ""}>Change not verified</span>
          {:else if b.reason !== "manual"}<span class="pill" title={b.note ?? ""}>Nothing applied</span>{/if}
          {#if b.changes.length}<button class="ghost small" aria-expanded={!!shown[b.id]} onclick={() => (shown[b.id] = !shown[b.id])}>{shown[b.id] ? "Hide" : "Changes"}</button>{/if}
          <button class="small" disabled={busy || !available} onclick={() => { restoring = b; restoreOpen = true; }}>Restore</button>
          <button class="ghost small" disabled={busy} onclick={() => remove(b)}>Delete</button>
        </div>
      </header>
      {#if b.note}<p class="note">{b.note}</p>{/if}
      {#if shown[b.id]}
        <table>
          <thead><tr><th>Profile</th><th>Setting</th><th>Before</th><th>After</th></tr></thead>
          <tbody>
            {#each b.changes as c}
              <tr><td>{c.profile}</td><td>{c.what}{#if c.settingId !== null}<div class="mono faint">{hex(c.settingId)}</div>{/if}</td><td class="mono">{c.before}</td><td class="mono">{c.after}</td></tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </section>
  {/each}
  {#snippet actions()}<button onclick={() => (open = false)}>Close</button>{/snippet}
</Modal>

<Modal bind:open={restoreOpen} title="Restore this snapshot?">
  <p><strong>{restoring?.label}</strong></p>
  <p>The profiles in it are put back exactly as they were{restoring?.createdProfiles.length ? `, and ${restoring.createdProfiles.join(", ")} (created afterwards) is removed` : ""}. The current state is snapshotted first, so this can be undone too.</p>
  {#snippet actions()}
    <button class="ghost" onclick={() => (restoreOpen = false)}>Cancel</button>
    <button class="play" onclick={restore}>Restore</button>
  {/snippet}
</Modal>

<style>
  .new { display: flex; gap: 0.5rem; }
  .new input { flex: 1; }
  .b { border: 1px solid var(--line-soft); border-radius: var(--r-md); padding: 0.7rem 0.9rem; display: grid; gap: 0.4rem; color: var(--text); }
  .b header { display: flex; justify-content: space-between; gap: 1rem; align-items: flex-start; }
  .row { display: flex; gap: 0.4rem; align-items: center; flex-shrink: 0; flex-wrap: wrap; justify-content: flex-end; }
  .small { padding: 0.25rem 0.6rem; font-size: 0.8rem; }
  .note { color: var(--rose); font-size: 0.82rem; }
  table { width: 100%; border-collapse: collapse; font-size: 0.82rem; }
  th { text-align: left; font-size: 0.66rem; letter-spacing: 0.08em; text-transform: uppercase; color: var(--faint); padding: 0 0.6rem 0.3rem 0; }
  td { padding: 0.3rem 0.6rem 0.3rem 0; border-top: 1px solid var(--line-soft); vertical-align: top; overflow-wrap: anywhere; }
</style>
