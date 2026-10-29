<script lang="ts">
  // Worlds in an instance, with one-click backups and restores.
  import { mc } from "../lib/api";
  import { bytes, toast } from "../lib/state.svelte";
  import { ago } from "../lib/mc.svelte";
  import type { McInstance, McWorld } from "../lib/types";
  import Icon from "../components/Icon.svelte";
  import Modal from "../components/Modal.svelte";

  let { inst, running }: { inst: McInstance; running: boolean } = $props();

  let worlds = $state<McWorld[]>([]);
  let loading = $state(true);
  let busy = $state<string | null>(null);
  let open = $state<string | null>(null);
  async function load() {
    loading = true;
    worlds = await mc.worlds(inst.id).catch(() => []);
    loading = false;
  }
  $effect(() => {
    void inst.id;
    load();
  });

  const MODES = ["Survival", "Creative", "Adventure", "Spectator"];
  const mode = (w: McWorld) => (w.hardcore ? "Hardcore" : MODES[w.gameMode ?? 0] ?? "Survival");
  const when = (ms: number | null) => (ms ? ago(Math.round(ms / 1000)) : "never");
  const date = (s: number) => new Date(s * 1000).toLocaleString(undefined, { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

  async function backup(w: McWorld) {
    busy = w.folder;
    try {
      const b = await mc.worldBackup(inst.id, w.folder);
      w.backups = [b, ...w.backups];
      toast(`Backed up ${w.name} (${bytes(b.size)})`);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = null;
    }
  }
  let restoring = $state<{ w: McWorld; file: string } | null>(null);
  let confirmRestore = $state(false);
  async function restore() {
    if (!restoring) return;
    const { w, file } = restoring;
    confirmRestore = false;
    busy = w.folder;
    try {
      await mc.worldRestore(inst.id, w.folder, file);
      toast(`Restored ${w.name}. The world as it was is backed up too.`);
      await load();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = null;
    }
  }
  async function dropBackup(w: McWorld, file: string) {
    await mc.worldBackupDelete(inst.id, w.folder, file);
    w.backups = w.backups.filter((b) => b.file !== file);
  }
  let deleting = $state<McWorld | null>(null);
  let confirmDelete = $state(false);
  let keep = $state(true);
  async function del() {
    if (!deleting) return;
    confirmDelete = false;
    busy = deleting.folder;
    try {
      await mc.worldDelete(inst.id, deleting.folder, keep);
      toast(keep ? `Deleted ${deleting.name}; a backup was kept` : `Deleted ${deleting.name}`);
      await load();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = null;
    }
  }
  async function copySeed(w: McWorld) {
    await navigator.clipboard.writeText(String(w.seed));
    toast("Seed copied");
  }
</script>

<div class="row spread bar">
  <h3>Worlds <span class="n">{worlds.length}</span></h3>
  <button class="ghost" onclick={() => mc.openFolder(inst.id, "saves")}><Icon name="folder" size={15} /> Saves folder</button>
</div>
{#if running}<p class="hint warn">The game is running. Back up from the pause menu's “Save and Quit to Title” first, or close the game, so the backup isn't mid-save.</p>{/if}

{#if loading}
  <p class="none">Loading…</p>
{:else if worlds.length === 0}
  <p class="none">No worlds yet. Make one in game and it shows up here.</p>
{:else}
  <ul class="worlds">
    {#each worlds as w (w.folder)}
      <li class:open={open === w.folder}>
        <div class="main">
          {#if w.icon}<img src={w.icon} alt="" width="56" height="56" />{:else}<span class="noicon"><Icon name="cube" size={22} /></span>{/if}
          <div class="txt">
            <span class="nm">{w.name}</span>
            <span class="facts">
              <span class="tag" class:hard={w.hardcore}>{mode(w)}</span>
              {#if w.cheats}<span class="tag">Cheats</span>{/if}
            </span>
            <span class="faint small">{#if w.version}{w.version} · {/if}{bytes(w.size)} · played {when(w.lastPlayed)}</span>
          </div>
          <div class="acts">
            <button disabled={busy !== null} onclick={() => backup(w)}><Icon name="save" size={15} /> {busy === w.folder ? "Working…" : "Back up"}</button>
            <button class="ghost" aria-expanded={open === w.folder} onclick={() => (open = open === w.folder ? null : w.folder)}>
              {w.backups.length} backup{w.backups.length === 1 ? "" : "s"} <Icon name={open === w.folder ? "up" : "down"} size={14} />
            </button>
            <button class="ghost danger icon" aria-label="Delete {w.name}" onclick={() => { deleting = w; keep = true; confirmDelete = true; }}><Icon name="trash" size={16} /></button>
          </div>
        </div>
        {#if open === w.folder}
          <div class="more">
            {#if w.seed !== null}<button class="ghost seed" onclick={() => copySeed(w)}>Seed <span class="mono">{w.seed}</span> <Icon name="copy" size={13} /></button>{/if}
            <button class="ghost" onclick={() => mc.openFolder(inst.id, `saves/${w.folder}`)}><Icon name="folder" size={14} /> Open world folder</button>
            {#if w.backups.length === 0}
              <p class="faint small">No backups yet.</p>
            {:else}
              <ul class="backups">
                {#each w.backups as b (b.file)}
                  <li>
                    <Icon name="clock" size={14} />
                    <span class="grow">{date(b.created)}</span>
                    <span class="faint small">{bytes(b.size)}</span>
                    <button class="ghost" disabled={busy !== null} onclick={() => { restoring = { w, file: b.file }; confirmRestore = true; }}>Restore</button>
                    <button class="ghost danger icon" aria-label="Delete backup" onclick={() => dropBackup(w, b.file)}><Icon name="trash" size={14} /></button>
                  </li>
                {/each}
              </ul>
            {/if}
          </div>
        {/if}
      </li>
    {/each}
  </ul>
{/if}

<Modal bind:open={confirmRestore} title="Restore {restoring?.w.name ?? ''}?">
  <p>The world goes back to how it was on {restoring ? date(restoring.w.backups.find((b) => b.file === restoring!.file)?.created ?? 0) : ""}. Arcadia backs up the world as it is now first, so you can undo this.</p>
  {#snippet actions()}
    <button class="ghost" onclick={() => (confirmRestore = false)}>Cancel</button>
    <button onclick={restore}>Restore backup</button>
  {/snippet}
</Modal>

<Modal bind:open={confirmDelete} title="Delete {deleting?.name ?? ''}?">
  <label class="keep"><input type="checkbox" bind:checked={keep} /> Keep a backup first (recommended)</label>
  {#snippet actions()}
    <button class="ghost" onclick={() => (confirmDelete = false)}>Keep world</button>
    <button class="danger" onclick={del}>Delete world</button>
  {/snippet}
</Modal>

<style>
  .bar { margin-bottom: 0.6rem; }
  .bar button, .acts button, .more button { display: inline-flex; align-items: center; gap: 0.35rem; }
  .n { color: var(--faint); font-weight: 500; }
  .hint { font-size: 0.85rem; margin-bottom: 0.8rem; }
  .warn { color: var(--peach); }
  .none { color: var(--muted); padding: 1.2rem 0; }
  .worlds { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.5rem; }
  .worlds > li { border: 1px solid var(--line-soft); border-radius: var(--r-lg); background: var(--sunk); }
  .main { display: flex; align-items: center; gap: 0.9rem; padding: 0.7rem 0.8rem; }
  img, .noicon { width: 56px; height: 56px; border-radius: var(--r-md); flex: none; image-rendering: pixelated; }
  .noicon { display: grid; place-items: center; color: var(--faint); background: linear-gradient(145deg, #4a7d4f, #2f5233); color: rgb(255 255 255 / 0.7); }
  .txt { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 0.2rem; }
  .nm { font-weight: 700; font-size: 1.02rem; }
  .facts { display: flex; flex-wrap: wrap; gap: 0.4rem; align-items: center; font-size: 0.8rem; }
  .tag { padding: 0.05rem 0.45rem; border-radius: 99px; background: var(--raised); border: 1px solid var(--line-soft); font-weight: 600; font-size: 0.72rem; }
  .tag.hard { color: var(--rose); border-color: color-mix(in srgb, var(--rose) 40%, transparent); }
  .acts { display: flex; align-items: center; gap: 0.3rem; }
  .icon { padding: 0.35rem; }
  .more { padding: 0.2rem 0.9rem 0.8rem calc(56px + 1.7rem); display: flex; flex-direction: column; align-items: flex-start; gap: 0.3rem; }
  .seed .mono { color: var(--muted); }
  .small { font-size: 0.8rem; }
  .backups { list-style: none; margin: 0.2rem 0 0; padding: 0; width: 100%; }
  .backups li { display: flex; align-items: center; gap: 0.6rem; padding: 0.3rem 0; border-top: 1px solid var(--line-soft); color: var(--muted); font-size: 0.88rem; }
  .grow { flex: 1; }
  .keep { display: flex; gap: 0.5rem; align-items: center; color: var(--text); }
</style>
