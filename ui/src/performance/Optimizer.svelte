<script lang="ts">
  import { onMount } from "svelte";
  import { ff } from "../lib/api";
  import { toast } from "../lib/state.svelte";
  import { ffs, saveConfig } from "../lib/ff.svelte";
  import { LEVEL_NAME, RESTART_TEXT, type ActionView, type Diagnosis, type Finding, type OptSession } from "../lib/fftypes";
  import Icon from "../components/Icon.svelte";
  import Modal from "../components/Modal.svelte";
  import Toggle from "../components/Toggle.svelte";
  import ProcessManager from "./parts/ProcessManager.svelte";
  import OptHistory from "./parts/OptHistory.svelte";

  type View = "analysis" | "actions" | "processes" | "history";
  // `?optview=` opens a section directly (used by UI tests and screenshots).
  let view: View = $state((new URLSearchParams(location.search).get("optview") as View | null) ?? "analysis");

  let diagnosis: Diagnosis | null = $state(null);
  let analysing = $state(false);
  let actions: ActionView[] = $state([]);
  let loadingActions = $state(false);
  let selected: Record<string, boolean> = $state({});
  let open: Record<string, boolean> = $state({});
  let confirming = $state(false);
  let applying = $state(false);
  let result: OptSession | null = $state(null);
  let showResult = $state(false);
  let expertPrompt = $state(false);
  let historyKey = $state(0);

  const level = $derived(ffs.config?.optimizerLevel ?? 1);
  const CATEGORY: Record<string, string> = { power: "Power", windows: "Windows gaming settings", startup: "Start-up apps", graphics: "Graphics", driver: "Graphics driver", process: "Processes" };
  const LEVEL_NOTE = [
    "",
    "Low-risk, documented, reversible changes. This is the recommended level.",
    "Changes how Windows behaves while you play. Each one is recorded and can be reverted. You approve every change before it is made.",
    "Vendor driver controls and tuning for experienced users. Nothing here is applied automatically, and nothing overclocks or changes voltage by itself.",
  ];

  const visible = $derived(actions.filter((a) => a.level <= level));
  const hidden = $derived(actions.length - visible.length);
  const groups = $derived.by(() => {
    const out: { level: number; category: string; items: ActionView[] }[] = [];
    for (const a of visible) {
      let g = out.find((x) => x.level === a.level && x.category === a.category);
      if (!g) out.push((g = { level: a.level, category: a.category, items: [] }));
      g.items.push(a);
    }
    return out.sort((a, b) => a.level - b.level || a.category.localeCompare(b.category));
  });
  const chosen = $derived(visible.filter((a) => selected[a.key] && a.state === "available"));
  const chosenLevel = $derived(chosen.reduce((m, a) => Math.max(m, a.level), 1));
  const restarts = $derived([...new Set(chosen.map((a) => a.restart).filter((r) => r !== "none"))]);

  async function analyse() {
    analysing = true;
    try {
      diagnosis = await ff.diagnose();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      analysing = false;
    }
  }

  async function loadActions() {
    loadingActions = true;
    try {
      actions = await ff.optList();
      for (const k of Object.keys(selected)) if (!actions.some((a) => a.key === k && a.state === "available")) delete selected[k];
    } catch (e) {
      toast(String(e), "error");
    } finally {
      loadingActions = false;
    }
  }

  onMount(() => {
    analyse();
    loadActions();
  });

  function setLevel(n: number) {
    if (!ffs.config) return;
    if (n === 3 && !ffs.config.expertAcknowledged) {
      expertPrompt = true;
      return;
    }
    saveConfig({ ...ffs.config, optimizerLevel: n });
  }

  async function acknowledgeExpert() {
    if (!ffs.config) return;
    expertPrompt = false;
    await saveConfig({ ...ffs.config, expertAcknowledged: true, optimizerLevel: 3 });
  }

  /** Tick the actions a finding points at and show them. */
  function selectFor(f: Finding) {
    const matches = actions.filter((a) => f.actions.includes(a.id) && a.state === "available");
    if (!matches.length) {
      toast("Those actions are already applied or not available on this PC");
      return;
    }
    const need = Math.max(...matches.map((a) => a.level));
    if (need > level) {
      toast(`These are ${LEVEL_NAME[need]} actions. Switch the level to ${LEVEL_NAME[need]} to use them.`);
      return;
    }
    for (const a of matches) selected[a.key] = true;
    view = "actions";
  }

  async function apply() {
    confirming = false;
    applying = true;
    try {
      const s = await ff.optApply(chosen.map((a) => ({ id: a.id, params: a.params })), chosenLevel, `${LEVEL_NAME[chosenLevel]} optimisation`);
      result = s;
      selected = {};
      if (s.status === "applied") toast(`${s.changes.filter((c) => c.status === "applied").length} change(s) applied and recorded`);
      else showResult = true;
    } catch (e) {
      toast(String(e), "error");
    } finally {
      applying = false;
      historyKey += 1;
      loadActions();
    }
  }

  async function revert(a: ActionView) {
    if (!a.sessionId || a.changeId === null) return;
    try {
      await ff.optRevertChange(a.sessionId, a.changeId);
      toast(`Reverted: ${a.label ?? a.name}`);
    } catch (e) {
      toast(String(e), "error");
    }
    historyKey += 1;
    loadActions();
  }

  async function copyReport() {
    if (!diagnosis) return;
    try {
      await navigator.clipboard.writeText(diagnosis.report);
      toast("Report copied");
    } catch {
      toast("The report could not be copied", "error");
    }
  }

  const sevText = { info: "Information", notice: "Worth a look", warning: "Needs attention" };
  const actionName = (id: string) => actions.find((a) => a.id === id)?.name ?? id;
</script>

<div class="ff-page">
  <div class="top">
    <div class="seg" role="radiogroup" aria-label="Optimizer section">
      {#each [["analysis", "Analysis"], ["actions", "Actions"], ["processes", "Background apps"], ["history", "History"]] as [id, label]}
        <button role="radio" aria-checked={view === id} onclick={() => (view = id as View)}>{label}</button>
      {/each}
    </div>
    <div class="levels">
      <span class="ff-note">Level</span>
      <div class="seg" role="radiogroup" aria-label="Optimisation level">
        {#each [1, 2, 3] as n}
          <button role="radio" aria-checked={level === n} onclick={() => setLevel(n)}>{LEVEL_NAME[n]}</button>
        {/each}
      </div>
    </div>
  </div>

  {#if view === "analysis"}
    <div class="bar">
      <span class="ff-note">{diagnosis?.basis ?? (analysing ? "Analysing…" : "")}</span>
      <div class="row">
        <button class="ghost" disabled={!diagnosis} onclick={copyReport}>Copy report</button>
        <button disabled={analysing} onclick={analyse}><Icon name="refresh" size={15} /> {analysing ? "Analysing…" : "Analyse again"}</button>
      </div>
    </div>
    {#if diagnosis && !diagnosis.frames}
      <div class="ff-banner"><Icon name="warning" size={18} /><span>Frame-based checks (what limits the frame rate, pacing, dropped frames) need a game being measured. <button class="link" onclick={() => (ffs.tab = "monitor")}>Start Live Monitor</button>, play for a while, then analyse again.</span></div>
    {/if}
    {#if diagnosis && !diagnosis.findings.length}
      <p class="ff-note">Nothing that needs attention was found with the data available.</p>
    {/if}
    <div class="findings">
      {#each diagnosis?.findings ?? [] as f (f.id)}
        <article class="ff-card finding {f.severity}">
          <header>
            <div class="lead">{f.title}</div>
            <div class="tags">
              <span class="pill" class:warn={f.severity === "warning"}>{sevText[f.severity]}</span>
              <span class="pill" title="How strongly the measurements support this">{f.confidence} confidence</span>
            </div>
          </header>
          {#if f.evidence.length}
            <ul class="evidence">{#each f.evidence as e}<li>{e}</li>{/each}</ul>
          {/if}
          {#if f.why}<p><span class="k">Why it matters</span>{f.why}</p>{/if}
          {#if f.recommendation}<p><span class="k">Recommended</span>{f.recommendation}</p>{/if}
          <dl class="facts">
            <div><dt>Trade-off</dt><dd>{f.tradeOff}</dd></div>
            <div><dt>Risk</dt><dd>{f.risk}</dd></div>
            <div><dt>Restart</dt><dd>{RESTART_TEXT[f.restart] || "Not needed"}</dd></div>
            <div><dt>Reversible</dt><dd>{f.reversible ? "Yes" : "No"}</dd></div>
          </dl>
          {#if f.actions.length || f.settingsUri}
            <div class="row wrap">
              {#if f.actions.length}<button onclick={() => selectFor(f)} title={f.actions.map(actionName).join(", ")}>Review {f.actions.length === 1 ? "the action" : `${f.actions.length} actions`}</button>{/if}
              {#if f.settingsUri}<button class="ghost" onclick={() => ff.openSettings(f.settingsUri!).catch((e) => toast(String(e), "error"))}>Open Windows Settings</button>{/if}
            </div>
          {/if}
        </article>
      {/each}
    </div>
  {:else if view === "actions"}
    <div class="bar">
      <span class="ff-note">{LEVEL_NOTE[level]}{#if hidden > 0}{" "}{hidden} more action(s) are shown at a higher level.{/if}</span>
      <div class="row">
        <button class="ghost" disabled={loadingActions} onclick={loadActions}><Icon name="refresh" size={15} /> Refresh</button>
        <button class="play" disabled={!chosen.length || applying} onclick={() => (confirming = true)}>{applying ? "Applying…" : `Review ${chosen.length || ""} change${chosen.length === 1 ? "" : "s"}`}</button>
      </div>
    </div>
    {#if !actions.length && !loadingActions}<p class="ff-note">No optimisation actions are available on this system.</p>{/if}
    {#each groups as g (g.level + g.category)}
      <section class="ff-card group">
        <header><h3>{CATEGORY[g.category] ?? g.category}</h3><span class="pill">{LEVEL_NAME[g.level]}</span></header>
        {#each g.items as a (a.key)}
          <div class="action" class:off={a.state === "notApplicable"}>
            <div class="pick">
              {#if a.state === "available"}
                <input type="checkbox" aria-label="Select {a.label ?? a.name}" bind:checked={selected[a.key]} />
              {:else if a.state === "applied"}
                <span class="dot on" title="Applied by Arcadia"></span>
              {:else}
                <span class="dot" title={a.state === "alreadySet" ? "Already set" : "Not available"}></span>
              {/if}
            </div>
            <div class="main">
              <div class="name">{a.name}{#if a.label}<span class="target">{a.label}</span>{/if}</div>
              <div class="desc">{a.description}</div>
              {#if a.state === "notApplicable"}
                <div class="why">{a.reason}</div>
              {:else}
                <div class="values">
                  <span class="faint">Now</span> <strong>{a.current ?? "unknown"}</strong>
                  {#if a.state === "available"}<span class="faint">→ would become</span> <strong>{a.proposed}</strong>{/if}
                  {#if a.state === "alreadySet"}<span class="pill">Already set</span>{/if}
                  {#if a.state === "applied"}<span class="pill live">Applied by Arcadia</span>{/if}
                </div>
              {/if}
              {#if open[a.key]}
                <dl class="facts">
                  <div><dt>Trade-off</dt><dd>{a.tradeOff}</dd></div>
                  <div><dt>How it is done</dt><dd>{a.source}</dd></div>
                  <div><dt>Applies to</dt><dd>{a.hardware} · {a.os}</dd></div>
                  <div><dt>Reversible</dt><dd>{a.reversible ? "Yes, the old value is saved first" : "No"}</dd></div>
                  <div><dt>ID</dt><dd class="mono">{a.key}</dd></div>
                </dl>
              {/if}
            </div>
            <div class="side">
              <span class="pill" class:warn={a.risk === "high"}>{a.risk} risk</span>
              {#if a.restart !== "none"}<span class="pill">{RESTART_TEXT[a.restart]}</span>{/if}
              {#if a.state === "applied"}<button onclick={() => revert(a)}>Revert</button>{/if}
              <button class="ghost" aria-expanded={!!open[a.key]} onclick={() => (open[a.key] = !open[a.key])}>{open[a.key] ? "Less" : "Details"}</button>
            </div>
          </div>
        {/each}
      </section>
    {/each}
  {:else if view === "processes"}
    <ProcessManager onchanged={() => (historyKey += 1)} />
  {:else}
    {#key historyKey}<OptHistory onchanged={loadActions} />{/key}
  {/if}
</div>

<Modal bind:open={confirming} title="Apply {chosen.length} change{chosen.length === 1 ? '' : 's'}?" wide>
  <p>Arcadia saves each current value first, applies the change, reads it back to check it, and records it in History where it can be reverted.</p>
  <table class="diff">
    <thead><tr><th>Change</th><th>Now</th><th>After</th><th>Risk</th></tr></thead>
    <tbody>
      {#each chosen as a (a.key)}
        <tr><td>{a.name}{#if a.label} <span class="faint">({a.label})</span>{/if}</td><td>{a.current ?? "unknown"}</td><td>{a.proposed}</td><td>{a.risk}</td></tr>
      {/each}
    </tbody>
  </table>
  {#if restarts.length}
    <div class="ff-banner"><Icon name="warning" size={18} /><span>Some changes take effect only after: <strong>{restarts.map((r) => RESTART_TEXT[r].toLowerCase()).join(", ")}</strong>.</span></div>
  {/if}
  {#if chosenLevel >= 2}<p>This includes {LEVEL_NAME[chosenLevel]} changes. Applying them is your explicit approval for the changes listed above only.</p>{/if}
  {#snippet actions()}
    <button class="ghost" onclick={() => (confirming = false)}>Cancel</button>
    <button class="play" onclick={apply}>Apply</button>
  {/snippet}
</Modal>

<Modal bind:open={showResult} title="The optimisation did not complete" wide>
  {#if result}
    <p>{result.error}</p>
    <p>{result.status === "failedDirty" ? "At least one change could not be put back automatically. It is listed below and in History so you can fix it by hand." : "Everything that had already been applied in this session was put back."}</p>
    <table class="diff">
      <thead><tr><th>Change</th><th>Before</th><th>Result</th></tr></thead>
      <tbody>
        {#each result.changes as c (c.id)}
          <tr><td>{c.name}</td><td>{c.beforeText}</td><td>{c.status === "rolledBack" ? "Put back" : c.status === "failed" ? "Failed" : c.status === "revertFailed" ? "Could not be put back" : c.status}{#if c.error} <span class="faint">({c.error})</span>{/if}</td></tr>
        {/each}
      </tbody>
    </table>
  {/if}
  {#snippet actions()}<button onclick={() => (showResult = false)}>Close</button>{/snippet}
</Modal>

<Modal bind:open={expertPrompt} title="Turn on Expert level?">
  <p>Expert level shows controls the graphics driver exposes for tuning, such as power limits and fan settings, where the vendor's API supports them on your card.</p>
  <p>Arcadia only offers values inside the limits the driver reports, never overclocks or changes voltage by itself, and records every change so it can be reverted. Tuning can still make a system unstable.</p>
  {#snippet actions()}
    <button class="ghost" onclick={() => (expertPrompt = false)}>Cancel</button>
    <button onclick={acknowledgeExpert}>I understand, turn it on</button>
  {/snippet}
</Modal>

<style>
  .top { display: flex; justify-content: space-between; align-items: center; gap: 1rem; flex-wrap: wrap; margin-bottom: 1rem; }
  .levels { display: flex; align-items: center; gap: 0.6rem; }
  .bar { display: flex; justify-content: space-between; align-items: center; gap: 1rem; margin-bottom: 1rem; }
  .row { display: flex; gap: 0.5rem; align-items: center; flex-shrink: 0; }
  .row.wrap { flex-wrap: wrap; }
  .findings { display: grid; gap: 1rem; grid-template-columns: repeat(auto-fit, minmax(380px, 1fr)); align-items: start; }
  .finding header { align-items: flex-start; }
  .finding.warning { border-color: color-mix(in srgb, var(--rose) 35%, var(--line-soft)); }
  .tags { display: flex; gap: 0.35rem; flex-shrink: 0; flex-wrap: wrap; justify-content: flex-end; }
  .evidence { margin: 0; padding-left: 1.1rem; font-size: 0.86rem; color: var(--muted); font-variant-numeric: tabular-nums; display: grid; gap: 0.15rem; }
  .finding p { font-size: 0.9rem; line-height: 1.45; }
  .k { display: block; font-size: 0.7rem; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; color: var(--faint); margin-bottom: 0.15rem; }
  .facts { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 0.6rem 1rem; margin: 0; font-size: 0.84rem; }
  .facts dt { font-size: 0.68rem; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; color: var(--faint); }
  .facts dd { margin: 0.1rem 0 0; color: var(--muted); overflow-wrap: anywhere; }
  .group { margin-bottom: 1rem; gap: 0; }
  .group > header { margin-bottom: 0.4rem; }
  .action { display: grid; grid-template-columns: 26px 1fr auto; gap: 0.75rem; padding: 0.8rem 0; border-top: 1px solid var(--line-soft); align-items: start; }
  .action.off .name, .action.off .desc { color: var(--faint); }
  .pick { padding-top: 0.2rem; display: grid; place-items: center; }
  .pick input { width: 16px; height: 16px; accent-color: var(--peach); padding: 0; }
  .dot { width: 9px; height: 9px; border-radius: 50%; border: 1px solid var(--line); background: var(--sunk); }
  .dot.on { background: var(--mint); border-color: var(--mint); }
  .main { display: grid; gap: 0.3rem; min-width: 0; }
  .name { font-weight: 600; }
  .target { margin-left: 0.5rem; font-weight: 700; color: var(--lilac); }
  .desc { font-size: 0.86rem; color: var(--muted); line-height: 1.4; }
  .values { font-size: 0.86rem; display: flex; flex-wrap: wrap; gap: 0.4rem; align-items: center; }
  .why { font-size: 0.84rem; color: var(--faint); }
  .side { display: flex; gap: 0.35rem; align-items: center; flex-wrap: wrap; justify-content: flex-end; max-width: 300px; }
  .side button { padding: 0.3rem 0.65rem; font-size: 0.82rem; }
  .diff { width: 100%; border-collapse: collapse; font-size: 0.86rem; }
  .diff th { text-align: left; font-size: 0.7rem; letter-spacing: 0.08em; text-transform: uppercase; color: var(--faint); padding: 0 0.6rem 0.35rem 0; }
  .diff td { padding: 0.4rem 0.6rem 0.4rem 0; border-top: 1px solid var(--line-soft); color: var(--text); vertical-align: top; }
  .link { border: 0; background: transparent; padding: 0; color: var(--lilac); font-weight: 600; text-decoration: underline; }
  .link:hover { background: transparent; }
  .ff-banner { margin-bottom: 1rem; }
</style>
