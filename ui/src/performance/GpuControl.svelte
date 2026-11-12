<script lang="ts">
  import { onMount } from "svelte";
  import { ff, pickFile, pickSavePath } from "../lib/api";
  import { toast } from "../lib/state.svelte";
  import { ffs } from "../lib/ff.svelte";
  import { LEVEL_NAME, VENDOR_LABEL, type GpuControl, type GpuControlsView, type GpuPreset, type PresetCheck } from "../lib/fftypes";
  import Icon from "../components/Icon.svelte";
  import Modal from "../components/Modal.svelte";
  import Toggle from "../components/Toggle.svelte";

  let views: GpuControlsView[] = $state([]);
  let presets: GpuPreset[] = $state([]);
  let loading = $state(true);
  let failed = $state<string | null>(null);
  /** Staged values, keyed `<gpu id>|<control id>`. Nothing reaches a driver until Apply. */
  let edits: Record<string, number> = $state({});
  let reviewing = $state<GpuControlsView | null>(null);
  let reviewOpen = $state(false);
  let applying = $state(false);

  let captureFor = $state<GpuControlsView | null>(null);
  let captureOpen = $state(false);
  let captureName = $state("");
  let captureExe = $state("");
  let captureNotes = $state("");

  let checking = $state<GpuPreset | null>(null);
  let check = $state<PresetCheck | null>(null);
  let checkOpen = $state(false);

  const level = $derived(ffs.config?.optimizerLevel ?? 1);
  const key = (v: GpuControlsView, c: GpuControl) => `${v.gpu.id}|${c.id}`;
  const staged = (v: GpuControlsView, c: GpuControl) => edits[key(v, c)] ?? c.value;
  const changed = (v: GpuControlsView) => v.controls.filter((c) => key(v, c) in edits && edits[key(v, c)] !== c.value);
  const locked = (c: GpuControl) => c.level > level;

  function show(c: GpuControl, v: number | null): string {
    if (v === null) return "not reported";
    if (c.kind.type === "toggle") return v ? "On" : "Off";
    if (c.kind.type === "choice") return c.kind.options.find((o) => o.value === v)?.label ?? String(v);
    if (c.kind.type === "range") return `${v} ${c.kind.unit}`.trim();
    return String(v);
  }

  function categories(v: GpuControlsView): { name: string; items: GpuControl[] }[] {
    const out: { name: string; items: GpuControl[] }[] = [];
    for (const c of v.controls) {
      let g = out.find((x) => x.name === c.category);
      if (!g) out.push((g = { name: c.category, items: [] }));
      g.items.push(c);
    }
    return out;
  }

  async function load() {
    try {
      [views, presets] = await Promise.all([ff.gpuControls(), ff.presets()]);
      failed = null;
      for (const k of Object.keys(edits)) {
        const hit = views.some((v) => v.controls.some((c) => key(v, c) === k && c.value !== edits[k]));
        if (!hit) delete edits[k];
      }
    } catch (e) {
      failed = String(e);
    } finally {
      loading = false;
    }
  }
  onMount(load);

  function stage(v: GpuControlsView, c: GpuControl, value: number) {
    if (!Number.isFinite(value)) return;
    if (c.kind.type === "range") value = Math.min(c.kind.max, Math.max(c.kind.min, value));
    if (value === c.value) delete edits[key(v, c)];
    else edits[key(v, c)] = value;
  }

  function discard(v: GpuControlsView) {
    for (const c of v.controls) delete edits[key(v, c)];
  }

  async function apply() {
    const v = reviewing;
    if (!v) return;
    const list = changed(v);
    const need = list.reduce((m, c) => Math.max(m, c.level), 1);
    reviewOpen = false;
    applying = true;
    try {
      const s = await ff.optApply(list.map((c) => ({ id: "gpu.control", params: { gpu: v.gpu.id, control: c.id, value: edits[key(v, c)] } })), need, `GPU Control: ${v.gpu.name}`);
      if (s.status === "applied") {
        toast(`${s.changes.length} driver control(s) changed and read back. Undo them from Optimizer, History.`);
        discard(v);
      } else toast(s.error ?? "The driver did not accept the change. Anything already changed was put back.", "error");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      applying = false;
      load();
    }
  }

  function startCapture(v: GpuControlsView) {
    captureFor = v;
    captureName = "";
    captureExe = "";
    captureNotes = "";
    captureOpen = true;
  }

  async function capture() {
    if (!captureFor) return;
    try {
      const p = await ff.presetCapture(captureFor.gpu.id, captureName, captureExe.trim() || null, captureNotes);
      captureOpen = false;
      toast(`Saved "${p.name}" with ${p.settings.length} setting(s)`);
      presets = await ff.presets();
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function importPreset() {
    const path = await pickFile([{ name: "Arcadia preset", extensions: ["json"] }]);
    if (!path) return;
    try {
      const p = await ff.presetImport(path);
      toast(`Imported "${p.name}". Nothing is applied until you choose Apply.`);
      presets = await ff.presets();
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function exportPreset(p: GpuPreset) {
    const path = await pickSavePath(`${p.name.replace(/[^\w.-]+/g, "_")}.json`, [{ name: "Arcadia preset", extensions: ["json"] }]);
    if (!path) return;
    try {
      await ff.presetExport(p.id, path);
      toast("Preset exported");
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function removePreset(p: GpuPreset) {
    try {
      await ff.presetDelete(p.id);
      presets = presets.filter((x) => x.id !== p.id);
      toast(`Deleted "${p.name}". Settings it applied earlier are unchanged.`);
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function openCheck(p: GpuPreset) {
    checking = p;
    check = null;
    checkOpen = true;
    try {
      check = await ff.presetCheck(p.id);
    } catch (e) {
      checkOpen = false;
      toast(String(e), "error");
    }
  }

  async function applyPreset() {
    if (!checking || !check) return;
    const p = checking;
    checkOpen = false;
    applying = true;
    try {
      const out = await ff.presetApply(p.id, check.level);
      toast(out.message, out.verified ? "ok" : "error");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      applying = false;
      load();
    }
  }

  const when = (secs: number) => (secs ? new Date(secs * 1000).toLocaleDateString() : "");
</script>

<div class="ff-page">
  <div class="intro">
    <div>
      <h3 class="page-title">GPU Control</h3>
      <p class="ff-note">Controls come from each vendor's own API, with the limits the installed driver reports. A control the driver does not offer is not shown as available. Every change is recorded and can be undone from Optimizer, History.</p>
    </div>
    <button class="ghost" disabled={loading} onclick={load}><Icon name="refresh" size={15} /> Refresh</button>
  </div>

  {#if failed}
    <div class="ff-banner warn"><Icon name="warning" size={18} /><span><strong>GPU controls could not be read.</strong> {failed}</span></div>
  {:else if !loading && !views.length}
    <div class="ff-banner"><Icon name="warning" size={18} /><span><strong>No graphics adapter was detected.</strong> There is nothing to control on this PC.</span></div>
  {/if}

  {#each views as v (v.gpu.id)}
    {@const pending = changed(v)}
    <section class="ff-card gpu">
      <header>
        <div class="lead">{v.gpu.name}</div>
        <div class="tags">
          <span class="pill">{VENDOR_LABEL[v.gpu.vendor]}</span>
          {#if v.api.state === "available"}
            <span class="pill live" title="Vendor API in use">{v.api.api}{v.api.version ? ` · ${v.api.version}` : ""}</span>
          {:else if v.api.state === "unavailable"}
            <span class="pill warn">{v.api.api} unavailable</span>
          {/if}
        </div>
      </header>

      {#if v.api.state === "unavailable"}
        <div class="ff-banner warn"><Icon name="warning" size={18} /><span><strong>{v.api.api} cannot be used for this adapter.</strong> {v.api.reason} No controls are shown because none can be read or changed.</span></div>
      {/if}
      {#if v.note}
        <div class="ff-banner">
          <Icon name="info" size={18} />
          <span>{v.note}
            {#if v.gpu.vendor === "nvidia"}<button class="link" onclick={() => (ffs.tab = "nvidia")}>Open NVIDIA Profiles</button>{/if}
          </span>
        </div>
      {/if}
      {#if v.api.state === "available" && !v.controls.length && v.gpu.vendor !== "nvidia"}
        <p class="ff-note">The driver reported no adjustable controls for this adapter.</p>
      {/if}

      {#each categories(v) as cat (cat.name)}
        <div class="cat">
          <h4>{cat.name}</h4>
          {#each cat.items as c (c.id)}
            {@const value = staged(v, c)}
            {@const off = !c.supported || locked(c) || c.kind.type === "readOnly"}
            <div class="ctl" class:off={!c.supported} class:edited={key(v, c) in edits}>
              <div class="what">
                <div class="name">
                  {c.name}
                  {#if c.level === 3}<span class="pill warn" title="Expert control">Expert</span>{/if}
                  {#if c.risk === "high"}<span class="pill warn">High risk</span>{/if}
                  {#if c.requiresRestart}<span class="pill">Restart the game</span>{/if}
                </div>
                {#if c.description}<div class="ff-note">{c.description}</div>{/if}
                {#if !c.supported}
                  <div class="ff-note why">Not available: {c.unsupportedReason ?? "the driver reports it as unsupported on this adapter."}</div>
                {:else if locked(c)}
                  <div class="ff-note why">{LEVEL_NAME[c.level]} control. Set the level to {LEVEL_NAME[c.level]} on the Optimizer page to change it.</div>
                {/if}
                {#if c.supported}
                  <div class="ff-note meta">
                    Now: <span class="mono">{show(c, c.value)}</span>
                    {#if c.default !== null} · Driver default: <span class="mono">{show(c, c.default)}</span>{/if}
                    {#if c.kind.type === "range"} · Range: <span class="mono">{c.kind.min} to {c.kind.max} {c.kind.unit}</span>{/if}
                  </div>
                {/if}
              </div>
              <div class="input">
                {#if !c.supported}
                  <span class="faint">Unsupported</span>
                {:else if c.kind.type === "toggle"}
                  {#if off}<span class="mono">{show(c, value)}</span>
                  {:else}<Toggle checked={!!value} label={c.name} onchange={(on) => stage(v, c, on ? 1 : 0)} />{/if}
                {:else if c.kind.type === "choice"}
                  <select disabled={off} aria-label={c.name} value={value} onchange={(e) => stage(v, c, Number(e.currentTarget.value))}>
                    {#if value === null}<option value="">not reported</option>{/if}
                    {#each c.kind.options as o}<option value={o.value}>{o.label}</option>{/each}
                  </select>
                {:else if c.kind.type === "range"}
                  <div class="range">
                    <input type="number" disabled={off} aria-label={c.name} min={c.kind.min} max={c.kind.max} step={c.kind.step} value={value ?? ""} onchange={(e) => stage(v, c, Number(e.currentTarget.value))} />
                    <span class="faint">{c.kind.unit}</span>
                  </div>
                {:else}
                  <span class="mono">{show(c, value)}</span>
                {/if}
                {#if c.supported && !off && c.default !== null && value !== c.default}
                  <button class="link" title="Stage the driver default" onclick={() => stage(v, c, c.default as number)}>Default</button>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/each}

      <footer>
        {#if pending.length}
          <span class="ff-note">{pending.length} change(s) not applied yet</span>
          <button class="ghost" onclick={() => discard(v)}>Discard</button>
          <button class="play" disabled={applying} onclick={() => { reviewing = v; reviewOpen = true; }}>Review and apply</button>
        {/if}
        {#if v.api.state === "available"}
          <button class="ghost" onclick={() => startCapture(v)}>Save current settings as a preset</button>
        {/if}
      </footer>
    </section>
  {/each}

  <section class="ff-card">
    <header>
      <h3>Arcadia presets</h3>
      <button class="ghost" onclick={importPreset}>Import preset</button>
    </header>
    <p class="ff-note">A preset is a JSON file holding settings for one GPU vendor. It can be shared, but it is only ever applied to a GPU of the same vendor, and each setting is checked against the installed driver first. NVIDIA settings are not converted into AMD or Intel settings, or the other way round.</p>
    {#if !presets.length}
      <p class="ff-note">No presets yet. Save the current settings of a GPU above, or import one.</p>
    {/if}
    {#each presets as p (p.id)}
      <div class="preset">
        <div class="what">
          <div class="name">{p.name} <span class="pill">{VENDOR_LABEL[p.vendor]}</span></div>
          <div class="ff-note">
            {p.settings.length} setting(s){p.gameExe ? ` · ${p.gameExe}` : ""}{p.gpuModel ? ` · made on ${p.gpuModel}` : ""}{p.driverVersion ? ` · driver ${p.driverVersion}` : ""}{p.createdAt ? ` · ${when(p.createdAt)}` : ""}
          </div>
          {#if p.notes}<div class="ff-note">{p.notes}</div>{/if}
        </div>
        <div class="row">
          <button onclick={() => openCheck(p)}>Check and apply</button>
          <button class="ghost" onclick={() => exportPreset(p)}>Export</button>
          <button class="ghost danger" onclick={() => removePreset(p)}>Delete</button>
        </div>
      </div>
    {/each}
  </section>
</div>

<Modal bind:open={reviewOpen} title="Apply driver controls" wide>
  {#if reviewing}
    <p class="ff-note">These values are sent to {reviewing.gpu.name} through {reviewing.api.state === "available" ? reviewing.api.api : "the vendor API"}, then read back to confirm the driver accepted them. The previous values are recorded first.</p>
    <table class="diff">
      <thead><tr><th>Control</th><th>Now</th><th>New</th></tr></thead>
      <tbody>
        {#each changed(reviewing) as c (c.id)}
          <tr><td>{c.name}{#if c.level === 3} <span class="pill warn">Expert</span>{/if}</td><td class="mono">{show(c, c.value)}</td><td class="mono">{show(c, edits[key(reviewing, c)])}</td></tr>
        {/each}
      </tbody>
    </table>
    {#if changed(reviewing).some((c) => c.level === 3)}
      <div class="ff-banner warn"><Icon name="warning" size={18} /><span><strong>Expert controls change hardware limits.</strong> They stay inside the range the driver reports, but higher limits mean more heat and power draw. Arcadia never raises them by itself.</span></div>
    {/if}
  {/if}
  {#snippet actions()}
    <button class="ghost" onclick={() => (reviewOpen = false)}>Cancel</button>
    <button class="play" onclick={apply}>Apply</button>
  {/snippet}
</Modal>

<Modal bind:open={captureOpen} title="Save current settings as a preset">
  {#if captureFor}
    <p class="ff-note">
      {#if captureFor.gpu.vendor === "nvidia"}
        Saves the settings you changed in one NVIDIA driver profile: the profile that contains the game executable below, or the global profile when it is left empty.
      {:else}
        Saves the current value of every adjustable {VENDOR_LABEL[captureFor.gpu.vendor]} control on {captureFor.gpu.name}. Expert limits are left out.
      {/if}
    </p>
    <label class="field">Name<input bind:value={captureName} maxlength="120" placeholder="e.g. Competitive" /></label>
    <label class="field">Game executable (optional)<input bind:value={captureExe} maxlength="120" placeholder="game.exe" /></label>
    <label class="field">Notes (optional)<textarea bind:value={captureNotes} maxlength="4000" rows="2"></textarea></label>
  {/if}
  {#snippet actions()}
    <button class="ghost" onclick={() => (captureOpen = false)}>Cancel</button>
    <button class="play" disabled={!captureName.trim()} onclick={capture}>Save preset</button>
  {/snippet}
</Modal>

<Modal bind:open={checkOpen} title={checking ? `Preset: ${checking.name}` : "Preset"} wide>
  {#if !check}
    <p class="ff-note">Checking the preset against this PC…</p>
  {:else}
    {#each check.problems as p}
      <div class="ff-banner warn"><Icon name="warning" size={18} /><span><strong>Cannot be applied.</strong> {p}</span></div>
    {/each}
    {#each check.warnings as w}
      <div class="ff-banner"><Icon name="info" size={18} /><span>{w}</span></div>
    {/each}
    {#if check.target}<p class="ff-note">Applies to: <strong>{check.target}</strong></p>{/if}
    {#if check.settings.length}
      <table class="diff">
        <thead><tr><th>Setting</th><th>Now</th><th>Preset</th><th></th></tr></thead>
        <tbody>
          {#each check.settings as s (s.id)}
            <tr class:skip={s.status === "skip"}>
              <td>{s.name || s.id}<div class="faint mono">{s.id}</div></td>
              <td class="mono">{s.current ?? "—"}</td>
              <td class="mono">{s.target}</td>
              <td>{s.status === "change" ? "Will change" : s.status === "same" ? "Already set" : `Skipped: ${s.reason ?? "not available"}`}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
    {#if check.usable && check.level > level}
      <div class="ff-banner warn"><Icon name="warning" size={18} /><span>This preset changes {LEVEL_NAME[check.level]} controls. Set the level to {LEVEL_NAME[check.level]} on the Optimizer page first.</span></div>
    {/if}
  {/if}
  {#snippet actions()}
    <button class="ghost" onclick={() => (checkOpen = false)}>Close</button>
    <button class="play" disabled={!check?.usable || (check?.level ?? 1) > level} onclick={applyPreset}>Apply preset</button>
  {/snippet}
</Modal>

<style>
  .intro { display: flex; justify-content: space-between; align-items: flex-start; gap: 1rem; margin-bottom: 1rem; }
  .page-title { font-family: var(--display); font-size: 1.15rem; margin-bottom: 0.3rem; }
  .intro .ff-note { max-width: 72ch; }
  section { margin-bottom: 1rem; }
  .tags, .row { display: flex; gap: 0.4rem; flex-wrap: wrap; align-items: center; }
  .cat { display: grid; gap: 0; }
  .cat h4 { font-size: 0.72rem; letter-spacing: 0.12em; text-transform: uppercase; color: var(--faint); font-weight: 700; padding-bottom: 0.35rem; }
  .ctl, .preset {
    display: flex; justify-content: space-between; align-items: center; gap: 1.25rem;
    padding: 0.7rem 0; border-top: 1px solid var(--line-soft); min-width: 0;
  }
  .ctl.off .name { color: var(--muted); }
  .ctl.edited { box-shadow: inset 3px 0 0 var(--peach); padding-left: 0.75rem; }
  .what { min-width: 0; display: grid; gap: 0.2rem; }
  .name { font-weight: 600; display: flex; gap: 0.45rem; align-items: center; flex-wrap: wrap; }
  .why { color: var(--text); }
  .meta { font-size: 0.78rem; }
  .input { display: flex; align-items: center; gap: 0.6rem; flex-shrink: 0; }
  .range { display: flex; align-items: center; gap: 0.35rem; }
  .range input { width: 6.5rem; text-align: right; }
  select { max-width: 15rem; }
  footer { display: flex; justify-content: flex-end; align-items: center; gap: 0.6rem; flex-wrap: wrap; }
  footer:empty { display: none; }
  .link { border: 0; background: transparent; padding: 0; color: var(--peach); font-weight: 600; text-decoration: underline; }
  .diff { width: 100%; border-collapse: collapse; font-size: 0.86rem; margin: 0.6rem 0; }
  .diff th { text-align: left; color: var(--muted); font-weight: 600; font-size: 0.75rem; padding: 0.3rem 0.5rem 0.3rem 0; }
  .diff td { padding: 0.4rem 0.5rem 0.4rem 0; border-top: 1px solid var(--line-soft); vertical-align: top; overflow-wrap: anywhere; }
  .diff tr.skip td { color: var(--muted); }
  .field { display: grid; gap: 0.3rem; font-size: 0.82rem; color: var(--muted); margin-top: 0.7rem; }
  .field input, .field textarea { width: 100%; }
  :global(dialog) .ff-banner { margin: 0.5rem 0; }
</style>
