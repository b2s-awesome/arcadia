<script lang="ts">
  import { onMount } from "svelte";
  import { ff, pickFile, pickSavePath } from "../lib/api";
  import { toast } from "../lib/state.svelte";
  import { ffs, loadConfig, saveConfig } from "../lib/ff.svelte";
  import { LEVEL_NAME, PRIORITY_LABEL, VENDOR_LABEL, type ActionView, type FfConfig, type GameChoice, type GameProfile, type GpuPreset, type LifecycleEvent, type ProfileRun, type ProfileView } from "../lib/fftypes";
  import Icon from "../components/Icon.svelte";
  import Modal from "../components/Modal.svelte";
  import Toggle from "../components/Toggle.svelte";

  let profiles: ProfileView[] = $state([]);
  let games: GameChoice[] = $state([]);
  let actions: ActionView[] = $state([]);
  let presets: GpuPreset[] = $state([]);
  let events: LifecycleEvent[] = $state([]);
  let loading = $state(true);
  let failed = $state<string | null>(null);
  let busy = $state(false);

  let editOpen = $state(false);
  let draft = $state<GameProfile | null>(null);
  /** Index into `games`, or -1 for an executable typed by hand. */
  let gamePick = $state(-1);

  let confirmOpen = $state(false);
  let confirming = $state<ProfileView | null>(null);
  let resultOpen = $state(false);
  let result = $state<ProfileRun | null>(null);

  const level = $derived(ffs.config?.optimizerLevel ?? 1);
  /** Actions a profile may hold: system settings, not per-process or per-GPU ones. */
  const pickable = $derived(actions.filter((a) => !a.id.startsWith("process.") && a.id !== "gpu.control" && a.state !== "notApplicable"));
  const same = (a: unknown, b: unknown) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);
  const actionOf = (id: string, params: unknown) => actions.find((a) => a.id === id && same(a.params, params));
  const actionName = (id: string, params: unknown) => {
    const a = actionOf(id, params);
    return a ? (a.label ? `${a.name}: ${a.label}` : a.name) : id;
  };

  /** The optimiser level a profile's contents need. */
  function needed(p: GameProfile): number {
    let n = 1;
    if (p.priority) n = Math.max(n, 2);
    if (p.presetId) n = Math.max(n, 2);
    for (const a of p.actions) n = Math.max(n, actionOf(a.id, a.params)?.level ?? 1);
    return n;
  }

  function contents(p: GameProfile): string[] {
    const out = p.actions.map((a) => actionName(a.id, a.params));
    if (p.priority) out.push(`Priority: ${PRIORITY_LABEL[p.priority] ?? p.priority}`);
    const preset = presets.find((x) => x.id === p.presetId);
    if (p.presetId) out.push(preset ? `GPU preset: ${preset.name} (${VENDOR_LABEL[preset.vendor]})` : "GPU preset: missing");
    if (p.monitor) out.push(p.benchmark ? "Live Monitor + benchmark recording" : "Live Monitor");
    return out;
  }

  async function load() {
    try {
      [profiles, games, actions, presets, events] = await Promise.all([ff.profiles(), ff.profileGames(), ff.optList(), ff.presets(), ff.profileEvents()]);
      failed = null;
    } catch (e) {
      failed = String(e);
    } finally {
      loading = false;
    }
  }
  onMount(() => {
    if (!ffs.config) loadConfig();
    load();
    const t = setInterval(async () => {
      if (editOpen || confirmOpen || busy) return;
      try {
        [profiles, events] = await Promise.all([ff.profiles(), ff.profileEvents()]);
      } catch {
        // The next pass tries again; the page keeps what it last showed.
      }
    }, 4000);
    return () => clearInterval(t);
  });

  async function setting(patch: Partial<FfConfig>) {
    if (ffs.config) await saveConfig({ ...ffs.config, ...patch });
  }

  function blank(): GameProfile {
    return { schema: 1, id: "", name: "", gameId: null, gameName: "", exe: "", enabled: true, autoApply: false, priority: null, presetId: null, actions: [], monitor: false, benchmark: false, approvedLevel: 0, notes: "", createdAt: 0, updatedAt: 0 };
  }

  function create() {
    draft = blank();
    gamePick = -1;
    const running = games.findIndex((g) => g.runningPid !== null && g.exe);
    if (running >= 0) pickGame(running);
    editOpen = true;
  }

  function edit(p: ProfileView) {
    const { runningPid: _r, applied: _a, presetName: _n, presetMissing: _m, benchmarks: _b, ...rest } = $state.snapshot(p) as ProfileView;
    draft = rest;
    gamePick = games.findIndex((g) => g.gameId !== null && g.gameId === p.gameId);
    editOpen = true;
  }

  function pickGame(i: number) {
    gamePick = i;
    if (!draft) return;
    const g = games[i];
    if (!g) {
      draft.gameId = null;
      return;
    }
    draft.gameId = g.gameId;
    draft.gameName = g.name;
    if (g.exe) draft.exe = g.exe;
    if (!draft.name.trim()) draft.name = g.name;
  }

  function toggleAction(a: ActionView, on: boolean) {
    if (!draft) return;
    draft.actions = draft.actions.filter((x) => !(x.id === a.id && same(x.params, a.params)));
    if (on) draft.actions.push({ id: a.id, params: a.params ?? null });
  }

  async function save() {
    if (!draft) return;
    const p = $state.snapshot(draft) as GameProfile;
    if (!p.gameName.trim()) p.gameName = p.exe;
    if (!p.monitor) p.benchmark = false;
    busy = true;
    try {
      const saved = await ff.profileSave(p, needed(p));
      editOpen = false;
      toast(`Saved "${saved.name}"`);
      await load();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function duplicate(p: ProfileView) {
    try {
      const copy = await ff.profileDuplicate(p.id);
      toast(`Created "${copy.name}". It does not apply automatically until you switch that on.`);
      await load();
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function remove(p: ProfileView) {
    try {
      await ff.profileDelete(p.id);
      toast(`Deleted "${p.name}"`);
      await load();
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function importProfile() {
    const path = await pickFile([{ name: "Arcadia game profile", extensions: ["json"] }]);
    if (!path) return;
    try {
      const p = await ff.profileImport(path);
      toast(`Imported "${p.name}". It is switched off: open it, check what it changes, and save it to approve.`);
      await load();
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function exportProfile(p: ProfileView) {
    const path = await pickSavePath(`${p.name.replace(/[^\w.-]+/g, "_")}.json`, [{ name: "Arcadia game profile", extensions: ["json"] }]);
    if (!path) return;
    try {
      await ff.profileExport(p.id, path);
      toast("Profile exported");
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function apply() {
    const p = confirming;
    if (!p) return;
    confirmOpen = false;
    busy = true;
    try {
      result = await ff.profileApply(p.id, level);
      resultOpen = true;
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = false;
      load();
    }
  }

  async function revert(p: ProfileView) {
    busy = true;
    try {
      const notes = await ff.profileRevert(p.id);
      toast(notes.length ? notes.join(". ") : `Reverted "${p.name}"`);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = false;
      load();
    }
  }

  const when = (secs: number) => new Date(secs * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
</script>

<div class="ff-page">
  <div class="intro">
    <div>
      <h3 class="page-title">Game Profiles</h3>
      <p class="ff-note">A profile belongs to one game executable and lists what Arcadia does while that game runs. Everything it applies goes through the Optimizer, so it is checked first, recorded, and can be undone.</p>
    </div>
    <div class="row">
      <button class="ghost" onclick={importProfile}>Import profile</button>
      <button class="play" onclick={create}><Icon name="plus" size={15} /> New profile</button>
    </div>
  </div>

  {#if failed}
    <div class="ff-banner warn"><Icon name="warning" size={18} /><span><strong>Profiles could not be read.</strong> {failed}</span></div>
  {/if}

  <section class="ff-card">
    <header><h3>Automation</h3><span class="ff-note">Off until you switch it on</span></header>
    <div class="opt">
      <div>
        <div class="name">Apply profiles when their game starts</div>
        <div class="ff-note">Only profiles with "Apply automatically" switched on, only to that game's own process, and never above the level you approved for the profile or the Optimizer level ({LEVEL_NAME[level]}).</div>
      </div>
      <Toggle checked={ffs.config?.autoProfiles ?? false} label="Apply profiles when their game starts" onchange={(v) => setting({ autoProfiles: v })} />
    </div>
    <div class="opt">
      <div>
        <div class="name">Put system changes back when the game exits</div>
        <div class="ff-note">Process priority always goes back. With this on, system settings a profile changed (such as the power plan) are restored too. GPU driver settings from a preset are kept; revert the profile by hand to undo those.</div>
      </div>
      <Toggle checked={ffs.config?.restoreOnExit ?? false} label="Put system changes back when the game exits" onchange={(v) => setting({ restoreOnExit: v })} />
    </div>
    <div class="opt">
      <div>
        <div class="name">Start Live Monitor when a library game starts</div>
        <div class="ff-note">For any game in your library, with or without a profile. Needs PresentMon; monitoring stops when the game exits.</div>
      </div>
      <Toggle checked={ffs.config?.autoMonitor ?? false} label="Start Live Monitor when a library game starts" onchange={(v) => setting({ autoMonitor: v })} />
    </div>
  </section>

  {#if !loading && !profiles.length}
    <section class="ff-card empty">
      <div class="lead">No profiles yet</div>
      <p class="ff-note">Create one for a game in your library or for a game that is running now.</p>
    </section>
  {/if}

  {#each profiles as p (p.id)}
    {@const list = contents(p)}
    <section class="ff-card profile">
      <header>
        <div class="title">
          <div class="lead">{p.name}</div>
          <div class="ff-note">{p.gameName || p.exe} · <span class="mono">{p.exe}</span>{p.benchmarks ? ` · ${p.benchmarks} benchmark(s)` : ""}</div>
        </div>
        <div class="tags">
          {#if p.runningPid !== null}<span class="pill live">Running</span>{/if}
          {#if p.applied}<span class="pill live">Applied</span>{/if}
          {#if !p.enabled}<span class="pill">Switched off</span>
          {:else if p.autoApply}<span class="pill" class:warn={!ffs.config?.autoProfiles} title={ffs.config?.autoProfiles ? "Applies when the game starts" : "Automatic profiles are switched off above"}>Automatic{ffs.config?.autoProfiles ? "" : " (paused)"}</span>
          {:else}<span class="pill">Manual</span>{/if}
          {#if p.approvedLevel === 0}<span class="pill warn" title="Open the profile and save it to approve what it changes">Not reviewed</span>{/if}
        </div>
      </header>
      {#if list.length}
        <ul class="contents">{#each list as line}<li>{line}</li>{/each}</ul>
      {:else}
        <p class="ff-note">This profile does not change anything yet.</p>
      {/if}
      {#if p.presetMissing}<div class="ff-banner warn"><Icon name="warning" size={18} /><span>The GPU preset this profile used was deleted. Edit the profile to choose another or remove it.</span></div>{/if}
      {#if p.notes}<p class="ff-note">{p.notes}</p>{/if}
      <footer>
        {#if p.applied}
          <button disabled={busy} onclick={() => revert(p)}>Revert</button>
        {:else}
          <button class="play" disabled={busy || !list.length} onclick={() => { confirming = p; confirmOpen = true; }}>Apply now</button>
        {/if}
        <button class="ghost" onclick={() => edit(p)}>Edit</button>
        <button class="ghost" onclick={() => duplicate(p)}>Duplicate</button>
        <button class="ghost" onclick={() => exportProfile(p)}>Export</button>
        <button class="ghost danger" disabled={p.applied} title={p.applied ? "Revert the profile first" : ""} onclick={() => remove(p)}>Delete</button>
      </footer>
    </section>
  {/each}

  <section class="ff-card">
    <header><h3>Recent activity</h3></header>
    {#if !events.length}
      <p class="ff-note">Nothing yet. Profile runs and game exits appear here.</p>
    {/if}
    {#each events.slice(0, 12) as e}
      <div class="event" class:bad={e.kind === "error"}>
        <span class="mono faint">{when(e.at)}</span>
        <span><strong>{e.profile ?? e.process}</strong>{e.profile ? ` (${e.process})` : ""}: {e.text}</span>
      </div>
    {/each}
  </section>
</div>

<Modal bind:open={editOpen} title={draft?.id ? "Edit profile" : "New profile"} wide>
  {#if draft}
    <div class="grid2">
      <label class="field">Game
        <select value={gamePick} onchange={(e) => pickGame(Number(e.currentTarget.value))}>
          <option value={-1}>Another executable (type it below)</option>
          {#each games as g, i}<option value={i}>{g.name}{g.runningPid !== null ? " (running)" : ""}</option>{/each}
        </select>
      </label>
      <label class="field">Profile name<input bind:value={draft.name} maxlength="120" placeholder="e.g. Competitive" /></label>
    </div>
    <label class="field">Game executable
      <input bind:value={draft.exe} maxlength="120" placeholder="game.exe" class="mono" />
      <span class="ff-note">The file name only. The profile follows the game if it is updated or moved to another folder. {#if gamePick >= 0 && !games[gamePick]?.exe}The library does not know this game's executable yet: start the game once, or type its name.{/if}</span>
    </label>

    <h4>While the game runs</h4>
    <label class="field">Process priority
      <select value={draft.priority ?? ""} onchange={(e) => { if (draft) draft.priority = e.currentTarget.value || null; }}>
        <option value="">Leave as it is</option>
        <option value="AboveNormal">Above normal</option>
        <option value="High">High</option>
        <option value="Normal">Normal</option>
      </select>
      <span class="ff-note">Advanced. Helps only when the processor is the limit and other programs compete for it. Goes back when the game exits. Realtime is never offered.</span>
    </label>

    <div class="field">Optimizer actions
      <div class="actions">
        {#each pickable as a (a.key)}
          {@const on = draft.actions.some((x) => x.id === a.id && same(x.params, a.params))}
          <label class="pick">
            <input type="checkbox" checked={on} onchange={(e) => toggleAction(a, e.currentTarget.checked)} />
            <span>{a.label ? `${a.name}: ${a.label}` : a.name} <span class="pill" class:warn={a.level === 3}>{LEVEL_NAME[a.level]}</span>{#if a.state !== "available"} <span class="faint">already set</span>{/if}</span>
          </label>
        {/each}
        {#if !pickable.length}<span class="ff-note">No optimiser actions are available on this PC.</span>{/if}
      </div>
    </div>

    <label class="field">GPU preset
      <select value={draft.presetId ?? ""} onchange={(e) => { if (draft) draft.presetId = e.currentTarget.value || null; }}>
        <option value="">None</option>
        {#each presets as pr}<option value={pr.id}>{pr.name} ({VENDOR_LABEL[pr.vendor]})</option>{/each}
      </select>
      <span class="ff-note">Presets are made on the GPU Control page. A preset is only applied to a graphics card of its own vendor.</span>
    </label>

    <h4>Measuring</h4>
    <div class="opt"><div class="name">Start Live Monitor with the game</div><Toggle bind:checked={draft.monitor} label="Start Live Monitor with the game" /></div>
    <div class="opt" class:dim={!draft.monitor}><div><div class="name">Record the whole session as a benchmark</div><div class="ff-note">Saved to Benchmark History when the game exits.</div></div><Toggle bind:checked={draft.benchmark} label="Record the whole session as a benchmark" /></div>

    <h4>Activation</h4>
    <div class="opt"><div class="name">Profile switched on</div><Toggle bind:checked={draft.enabled} label="Profile switched on" /></div>
    <div class="opt"><div><div class="name">Apply automatically when the game starts</div><div class="ff-note">Also needs "Apply profiles when their game starts" on this page.</div></div><Toggle bind:checked={draft.autoApply} label="Apply automatically when the game starts" /></div>

    <label class="field">Notes<textarea bind:value={draft.notes} maxlength="4000" rows="2"></textarea></label>

    <div class="ff-banner">
      <Icon name="info" size={18} />
      <span>Saving approves this profile up to the <strong>{LEVEL_NAME[needed(draft)]}</strong> level.
        {#if needed(draft) > level}The Optimizer level is {LEVEL_NAME[level]}, so anything above it is left out (and reported) until you raise the level on the Optimizer page.{/if}
      </span>
    </div>
  {/if}
  {#snippet actions()}
    <button class="ghost" onclick={() => (editOpen = false)}>Cancel</button>
    <button class="play" disabled={busy || !draft?.name.trim() || !draft?.exe.trim()} onclick={save}>Save profile</button>
  {/snippet}
</Modal>

<Modal bind:open={confirmOpen} title={confirming ? `Apply "${confirming.name}"` : "Apply profile"}>
  {#if confirming}
    <p class="ff-note">{confirming.runningPid !== null ? `${confirming.exe} is running, so its process settings are applied too.` : `${confirming.exe} is not running. System and driver settings are applied now; process priority needs the running game.`}</p>
    <ul class="contents">{#each contents(confirming) as line}<li>{line}</li>{/each}</ul>
    <p class="ff-note">Applied at the {LEVEL_NAME[level]} level. Anything that needs a higher level, or is not available on this PC, is left out and listed afterwards.</p>
  {/if}
  {#snippet actions()}
    <button class="ghost" onclick={() => (confirmOpen = false)}>Cancel</button>
    <button class="play" onclick={apply}>Apply</button>
  {/snippet}
</Modal>

<Modal bind:open={resultOpen} title="Profile applied">
  {#if result}
    {#if result.applied.length}
      <h4>Applied</h4>
      <ul class="contents">{#each result.applied as line}<li>{line}</li>{/each}</ul>
    {:else}
      <p class="ff-note">Nothing needed changing: everything in the profile was already set or was left out.</p>
    {/if}
    {#if result.skipped.length}
      <h4>Left out</h4>
      <ul class="contents">{#each result.skipped as line}<li>{line}</li>{/each}</ul>
    {/if}
    {#if result.preset && !result.preset.verified}<div class="ff-banner warn"><Icon name="warning" size={18} /><span>{result.preset.message}</span></div>{/if}
    {#if result.applied.length}<p class="ff-note">Undo it with Revert on the profile, or change by change from Optimizer, History.</p>{/if}
  {/if}
  {#snippet actions()}
    <button class="play" onclick={() => (resultOpen = false)}>Done</button>
  {/snippet}
</Modal>

<style>
  .intro { display: flex; justify-content: space-between; align-items: flex-start; gap: 1rem; margin-bottom: 1rem; }
  .page-title { font-family: var(--display); font-size: 1.15rem; margin-bottom: 0.3rem; }
  .intro .ff-note { max-width: 72ch; }
  section { margin-bottom: 1rem; }
  .row, .tags { display: flex; gap: 0.4rem; flex-wrap: wrap; align-items: center; }
  .title { min-width: 0; display: grid; gap: 0.15rem; }
  .opt { display: flex; justify-content: space-between; align-items: center; gap: 1.25rem; padding: 0.6rem 0; border-top: 1px solid var(--line-soft); }
  .opt.dim { opacity: 0.55; }
  .name { font-weight: 600; }
  .contents { margin: 0; padding-left: 1.1rem; display: grid; gap: 0.2rem; font-size: 0.9rem; }
  footer { display: flex; gap: 0.5rem; flex-wrap: wrap; }
  .empty { text-align: center; }
  .event { display: flex; gap: 0.75rem; padding: 0.4rem 0; border-top: 1px solid var(--line-soft); font-size: 0.86rem; }
  .event.bad { color: var(--rose); }
  h4 { font-size: 0.72rem; letter-spacing: 0.12em; text-transform: uppercase; color: var(--faint); font-weight: 700; margin: 1rem 0 0.3rem; }
  .grid2 { display: grid; grid-template-columns: 1fr 1fr; gap: 0.75rem; }
  .field { display: grid; gap: 0.3rem; font-size: 0.82rem; color: var(--muted); margin-top: 0.7rem; }
  .field input:not([type="checkbox"]), .field textarea, .field select { width: 100%; }
  .actions { display: grid; gap: 0.3rem; max-height: 13rem; overflow: auto; padding: 0.5rem 0.6rem; border: 1px solid var(--line-soft); border-radius: var(--r-md); background: var(--sunk); }
  .pick { display: flex; gap: 0.5rem; align-items: flex-start; color: var(--text); font-size: 0.86rem; }
  .pick input { margin-top: 0.2rem; }
  :global(dialog) .ff-banner { margin: 0.9rem 0 0; }
</style>
