<script lang="ts">
  // NVIDIA driver profiles: browse, edit, import/export .nip, back up, restore.
  // Edits are collected here and sent as one request; the backend snapshots
  // the profile, applies through DRS, saves, reads back and records.
  import { onMount } from "svelte";
  import { nv, pickFile, pickSavePath } from "../lib/api";
  import { toast } from "../lib/state.svelte";
  import { ffs, saveConfig } from "../lib/ff.svelte";
  import { NV_NAME_SOURCE, type NvChange, type NvDiffLine, type NvImportPreview, type NvProfileDetail, type NvProfileSummary, type NvSetting, type NvStatus, type NvValue } from "../lib/fftypes";
  import { editText, hex, parseValue, sameValue } from "../lib/nv";
  import Icon from "../components/Icon.svelte";
  import Modal from "../components/Modal.svelte";
  import NvImport from "./parts/NvImport.svelte";
  import NvBackups from "./parts/NvBackups.svelte";
  import NvCompare from "./parts/NvCompare.svelte";

  const NIP = [{ name: "NVIDIA Profile Inspector profile", extensions: ["nip"] }];
  const LIST_CAP = 300;
  const KIND_TEXT = { global: "Global", predefined: "NVIDIA profile", user: "Your profile" };
  const STATE_TEXT = { user: "Changed by you", nvidia: "NVIDIA's value", inherited: "From global profile", default: "Driver default", unset: "Not set" };

  let status = $state<NvStatus | null>(null);
  let list: NvProfileSummary[] = $state([]);
  let loadingList = $state(false);
  let query = $state("");
  let listFilter: "all" | "changed" | "yours" = $state("all");
  let listSort: "name" | "changes" = $state("name");

  let selected: string | null = $state(null);
  let detail = $state<NvProfileDetail | null>(null);
  let loadingDetail = $state(false);

  let search = $state("");
  let scope: "common" | "advanced" | "all" = $state("common");
  let modifiedOnly = $state(false);
  let favouritesOnly = $state(false);
  let collapsed: Record<string, boolean> = $state({});
  let expanded: Record<number, boolean> = $state({});

  // Pending edits for the open profile.
  let edits: Record<number, NvValue | "restore"> = $state({});
  let drafts: Record<number, string> = $state({});
  let errors: Record<number, string> = $state({});
  let custom: Record<number, boolean> = $state({});
  let addApps: string[] = $state([]);
  let removeApps: string[] = $state([]);
  let restoreAll = $state(false);
  let newExe = $state("");

  let reviewOpen = $state(false);
  let plan: NvDiffLine[] = $state([]);
  let applying = $state(false);
  let bits: NvSetting | null = $state(null);
  let bitsOpen = $state(false);
  let bitsValue = $state(0);

  let importPreview: NvImportPreview | null = $state(null);
  let importPath: string | null = $state(null);
  let importOpen = $state(false);
  let backupsOpen = $state(false);
  let compareOpen = $state(false);
  let createOpen = $state(false);
  let createName = $state("");
  let createExe = $state("");
  let deleteOpen = $state(false);
  let moreOpen = $state(false);

  const favourites = $derived(new Set(ffs.config?.nvidiaFavourites ?? []));

  const shownList = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const out = list.filter((p) => {
      if (listFilter === "changed" && !(p.customisedCount > 0 || p.userExecutables > 0 || p.kind === "user")) return false;
      if (listFilter === "yours" && p.kind !== "user") return false;
      return !q || p.name.toLowerCase().includes(q) || p.executables.some((e) => e.toLowerCase().includes(q));
    });
    if (listSort === "changes") out.sort((a, b) => b.customisedCount - a.customisedCount || a.name.localeCompare(b.name));
    return out;
  });

  const shownSettings = $derived.by(() => {
    if (!detail) return [];
    const q = search.trim().toLowerCase();
    return detail.settings.filter((s) => {
      const pending = s.id in edits;
      if (modifiedOnly && !(s.modified || pending)) return false;
      if (favouritesOnly && !favourites.has(s.id)) return false;
      if (q) return s.name.toLowerCase().includes(q) || s.idHex.toLowerCase().includes(q) || String(s.id) === q || s.alternateNames.some((n) => n.toLowerCase().includes(q));
      if (modifiedOnly || favouritesOnly || scope === "all") return true;
      // Anything the profile actually stores stays visible in both views.
      const stored = s.state === "user" || s.state === "nvidia";
      return scope === "common" ? s.common || s.modified || pending : !s.common || stored;
    });
  });
  const groups = $derived((detail?.groups ?? []).map((g) => ({ name: g, items: shownSettings.filter((s) => s.group === g) })).filter((g) => g.items.length));
  const pendingCount = $derived(Object.keys(edits).length + addApps.length + removeApps.length + (restoreAll ? 1 : 0));
  const hasErrors = $derived(Object.keys(errors).length > 0);
  const groupLabel = (g: string) => g.replace(/^\d+\s-\s/, "");

  async function loadList(refresh = false) {
    loadingList = true;
    try {
      list = await nv.profiles(refresh);
      if (!selected && list.length) select(list[0].name);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      loadingList = false;
    }
  }

  function clearEdits() {
    edits = {};
    drafts = {};
    errors = {};
    custom = {};
    addApps = [];
    removeApps = [];
    restoreAll = false;
    newExe = "";
  }

  async function select(name: string) {
    if (pendingCount && name !== selected) {
      toast("Apply or discard your changes to this profile first");
      return;
    }
    selected = name;
    clearEdits();
    await loadDetail();
  }

  async function loadDetail() {
    if (!selected) return;
    loadingDetail = true;
    try {
      detail = await nv.profile(selected);
    } catch (e) {
      detail = null;
      toast(String(e), "error");
    } finally {
      loadingDetail = false;
    }
  }

  onMount(async () => {
    try {
      status = await nv.status();
    } catch (e) {
      status = { available: false, reason: String(e), driver: { version: null, branch: null }, knownSettings: 0, referenceLoaded: false };
    }
    if (status.available) loadList();
  });

  /** The value a row shows: the pending edit if there is one, else the live value. */
  function shown(s: NvSetting): NvValue | null {
    const e = edits[s.id];
    if (e === "restore") return s.default;
    return e ?? s.value;
  }

  function setValue(s: NvSetting, v: NvValue) {
    delete errors[s.id];
    if (sameValue(v, s.value)) delete edits[s.id];
    else edits[s.id] = v;
  }

  function typed(s: NvSetting, text: string) {
    drafts[s.id] = text;
    const r = parseValue(s.kind, text);
    if (typeof r === "string") errors[s.id] = r;
    else setValue(s, r);
  }

  function picked(s: NvSetting, v: string) {
    if (v === "custom") {
      custom[s.id] = true;
      drafts[s.id] = editText(shown(s));
      return;
    }
    setValue(s, { type: s.kind, value: v });
  }

  function restore(s: NvSetting) {
    delete errors[s.id];
    delete drafts[s.id];
    delete custom[s.id];
    if (s.modified) edits[s.id] = "restore";
    else delete edits[s.id];
  }

  function undo(s: NvSetting) {
    delete edits[s.id];
    delete drafts[s.id];
    delete errors[s.id];
    delete custom[s.id];
  }

  function toggleFavourite(id: number) {
    if (!ffs.config) return;
    const next = favourites.has(id) ? ffs.config.nvidiaFavourites.filter((x) => x !== id) : [...ffs.config.nvidiaFavourites, id];
    saveConfig({ ...ffs.config, nvidiaFavourites: next });
  }

  function openBits(s: NvSetting) {
    bits = s;
    bitsValue = Number(shown(s)?.value ?? 0);
    bitsOpen = true;
  }
  const bitLabel = (s: NvSetting, bit: number) => s.options.find((o) => o.value === 2 ** bit)?.label ?? "";
  function applyBits() {
    if (bits) setValue(bits, { type: "dword", value: String(bitsValue >>> 0) });
    bitsOpen = false;
  }

  function addExe() {
    const exe = newExe.trim();
    if (!exe || !detail) return;
    if (detail.apps.some((a) => a.name.toLowerCase() === exe.toLowerCase()) || addApps.some((a) => a.toLowerCase() === exe.toLowerCase())) {
      toast("That executable is already in this profile");
      return;
    }
    addApps.push(exe);
    newExe = "";
  }

  function changes(): NvChange[] {
    const out: NvChange[] = [];
    if (restoreAll) out.push({ op: "restoreProfile" });
    for (const [id, e] of Object.entries(edits)) out.push(e === "restore" ? { op: "restore", id: Number(id) } : { op: "set", id: Number(id), value: e });
    for (const exe of removeApps) out.push({ op: "removeApp", exe });
    for (const exe of addApps) out.push({ op: "addApp", exe });
    return out;
  }

  async function review() {
    if (!selected) return;
    try {
      plan = await nv.plan(selected, changes());
      reviewOpen = true;
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function apply() {
    if (!selected) return;
    reviewOpen = false;
    applying = true;
    try {
      const out = await nv.apply(selected, changes());
      if (out.verified) toast(`${out.changes.length} change(s) saved to the driver and verified. Restart the game for them to take effect.`);
      else toast(`Saved, but the driver did not read back as expected: ${out.mismatches.join("; ")}`, "error");
      clearEdits();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      applying = false;
      await loadDetail();
      loadList();
    }
  }

  function askRestoreAll() {
    edits = {};
    drafts = {};
    errors = {};
    custom = {};
    restoreAll = true;
    review();
  }

  async function openImport() {
    const path = await pickFile(NIP);
    if (!path) return;
    try {
      importPreview = await nv.importPreview(path);
      importPath = path;
      importOpen = true;
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function exportProfiles(names: string[]) {
    const fallback = names.length === 1 ? names[0].replace(/[^\w .-]+/g, "_") : "customised-profiles";
    const path = await pickSavePath(`${fallback}.nip`, NIP);
    if (!path) return;
    try {
      const n = await nv.exportTo(path, names);
      toast(`Exported ${n} profile${n === 1 ? "" : "s"}`);
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function saveDatabase() {
    moreOpen = false;
    const path = await pickSavePath("nvidia-driver-profiles.nvdrs", [{ name: "NVIDIA driver database backup", extensions: ["nvdrs"] }]);
    if (!path) return;
    try {
      await nv.saveDatabase(path);
      toast("Driver database backup saved");
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function createProfile() {
    createOpen = false;
    try {
      const out = await nv.createProfile(createName, createExe.trim() || null);
      toast(out.verified ? `Created ${createName.trim()}` : `Created, but not verified: ${out.mismatches.join("; ")}`, out.verified ? "ok" : "error");
      const name = createName.trim();
      createName = "";
      createExe = "";
      await loadList();
      select(name);
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function deleteProfile() {
    deleteOpen = false;
    if (!selected) return;
    try {
      await nv.deleteProfile(selected);
      toast(`Deleted ${selected}. It can be restored from Backups.`);
      selected = null;
      detail = null;
      clearEdits();
      await loadList();
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function chooseNpi() {
    const path = await pickFile([{ name: "NVIDIA Profile Inspector", extensions: ["exe"] }]);
    if (!path) return;
    try {
      ffs.config = await nv.setNpi(path);
      toast("NVIDIA Profile Inspector location saved");
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function loadReference() {
    try {
      const n = await nv.loadReference();
      status = await nv.status();
      toast(`Reference.xml loaded: ${n} settings are now known`);
      loadDetail();
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function npiExport() {
    try {
      toast((await nv.npiExport()).message);
    } catch (e) {
      toast(String(e), "error");
    }
  }

  function afterDriverChange() {
    loadDetail();
    loadList();
  }
</script>

<div class="nvp">
  {#if status && !status.available}
    <div class="ff-page">
      <div class="ff-banner"><Icon name="warning" size={18} /><span><strong>NVIDIA driver profiles are not available on this PC.</strong> {status.reason}</span></div>
      <section class="ff-card">
        <div class="lead">What still works</div>
        <p class="ff-note">Profile files do not need the driver. You can open a .nip file to inspect and edit it, and compare two files. Importing into the driver, the profile browser and backups need an NVIDIA GPU with its driver installed. AMD and Intel graphics have no .nip equivalent; their controls are under GPU Control.</p>
        <div class="row">
          <button onclick={openImport}>Open a .nip file</button>
          <button class="ghost" onclick={() => (compareOpen = true)}>Compare two files</button>
        </div>
      </section>
    </div>
  {:else if status}
    <div class="toolbar">
      <span class="pill" title="Installed NVIDIA driver">Driver {status.driver.version ?? "unknown"}{status.driver.branch ? ` · ${status.driver.branch}` : ""}</span>
      <div class="row">
        <button onclick={openImport}>Import .nip</button>
        <button disabled={!selected} onclick={() => selected && exportProfiles([selected])}>Export profile</button>
        <button class="ghost" onclick={() => exportProfiles([])} title="Every profile you changed, in one .nip file">Export all changed</button>
        <button class="ghost" onclick={() => (backupsOpen = true)}>Backups</button>
        <button class="ghost" onclick={() => (compareOpen = true)}>Compare files</button>
        <button class="ghost" onclick={() => (moreOpen = true)}>More</button>
      </div>
    </div>

    <div class="split">
      <aside class="list">
        <div class="list-head">
          <input type="search" placeholder="Search profiles or executables" bind:value={query} aria-label="Search profiles" />
          <div class="row">
            <div class="seg" role="radiogroup" aria-label="Which profiles">
              {#each [["all", "All"], ["changed", "Changed"], ["yours", "Yours"]] as [id, label]}
                <button role="radio" aria-checked={listFilter === id} onclick={() => (listFilter = id as typeof listFilter)}>{label}</button>
              {/each}
            </div>
            <select bind:value={listSort} aria-label="Sort profiles">
              <option value="name">A–Z</option>
              <option value="changes">Most changes</option>
            </select>
          </div>
        </div>
        <div class="rows" role="listbox" aria-label="Driver profiles">
          {#each shownList.slice(0, LIST_CAP) as p (p.name)}
            <button class="prow" role="option" aria-selected={selected === p.name} class:on={selected === p.name} onclick={() => select(p.name)}>
              <span class="pn">{p.name}{#if p.kind === "global"}<span class="pill">Global</span>{/if}{#if p.kind === "user"}<span class="pill">Yours</span>{/if}</span>
              <span class="pe">{p.executables.length ? p.executables.slice(0, 2).join(", ") + (p.executables.length > 2 ? ` +${p.executables.length - 2}` : "") : p.kind === "global" ? "Everything without its own profile" : "No executables"}</span>
              {#if p.customisedCount > 0}<span class="count" title="{p.customisedCount} setting(s) changed by you">{p.customisedCount}</span>{/if}
            </button>
          {/each}
          {#if loadingList && !list.length}<p class="ff-note pad">Reading the driver's profile database…</p>{/if}
          {#if !loadingList && !shownList.length}<p class="ff-note pad">No profiles match.</p>{/if}
          {#if shownList.length > LIST_CAP}<p class="ff-note pad">Showing {LIST_CAP} of {shownList.length}. Search to narrow the list.</p>{/if}
        </div>
        <div class="list-foot">
          <span class="ff-note">{list.length} profiles</span>
          <div class="row">
            <button class="ghost small" disabled={loadingList} onclick={() => loadList(true)} aria-label="Reload profiles"><Icon name="refresh" size={14} /></button>
            <button class="small" onclick={() => (createOpen = true)}>New profile</button>
          </div>
        </div>
      </aside>

      <section class="editor">
        {#if detail}
          <header class="phead">
            <div>
              <h3>{detail.summary.name}</h3>
              <div class="tags">
                <span class="pill">{KIND_TEXT[detail.summary.kind]}</span>
                <span class="ff-note">{detail.summary.customisedCount} changed by you · {detail.summary.settingCount} stored</span>
              </div>
            </div>
            <div class="row">
              <button class="ghost" disabled={detail.summary.customisedCount === 0 || applying} onclick={askRestoreAll}>Restore defaults</button>
              {#if detail.summary.kind === "user"}<button class="danger" onclick={() => (deleteOpen = true)}>Delete</button>{/if}
            </div>
          </header>

          <div class="apps">
            <span class="k">Executables</span>
            {#each detail.apps as a (a.name + a.fileInFolder)}
              {@const gone = removeApps.includes(a.name)}
              <span class="chip" class:gone title={a.fileInFolder ? `Only when ${a.fileInFolder} is in the same folder` : a.isPredefined ? "Assigned by NVIDIA" : "Added by you"}>
                {a.name}{#if a.fileInFolder}<span class="faint"> + {a.fileInFolder}</span>{/if}
                <button aria-label={gone ? `Keep ${a.name}` : `Remove ${a.name}`} onclick={() => (removeApps = gone ? removeApps.filter((x) => x !== a.name) : [...removeApps, a.name])}>{gone ? "↺" : "×"}</button>
              </span>
            {/each}
            {#each addApps as exe (exe)}
              <span class="chip new">{exe}<button aria-label="Do not add {exe}" onclick={() => (addApps = addApps.filter((x) => x !== exe))}>×</button></span>
            {/each}
            {#if detail.summary.kind !== "global"}
              <form class="addexe" onsubmit={(e) => { e.preventDefault(); addExe(); }}>
                <input placeholder="game.exe" bind:value={newExe} aria-label="Executable to add" />
                <button class="small" disabled={!newExe.trim()}>Add</button>
              </form>
            {/if}
          </div>

          <div class="filters">
            <input type="search" placeholder="Search by name or setting ID" bind:value={search} aria-label="Search settings" />
            <div class="seg" role="radiogroup" aria-label="Which settings">
              {#each [["common", "Common"], ["advanced", "Advanced"], ["all", "All"]] as [id, label]}
                <button role="radio" aria-checked={scope === id} onclick={() => (scope = id as typeof scope)}>{label}</button>
              {/each}
            </div>
            <label class="check"><input type="checkbox" bind:checked={modifiedOnly} /> Modified only</label>
            <label class="check"><input type="checkbox" bind:checked={favouritesOnly} /> Favourites</label>
          </div>
          {#if !detail.internalScan}
            <p class="ff-note tight">This driver does not let Arcadia tell which of NVIDIA's own values are stored encoded. Treat "NVIDIA's value" entries of undocumented settings as raw data.</p>
          {/if}

          <div class="settings">
            {#each groups as g (g.name)}
              <div class="group">
                <button class="ghead" aria-expanded={!collapsed[g.name]} onclick={() => (collapsed[g.name] = !collapsed[g.name])}>
                  <span>{groupLabel(g.name)}</span><span class="faint">{g.items.length}</span>
                </button>
                {#if !collapsed[g.name]}
                  {#each g.items as s (s.id)}
                    {@const pending = edits[s.id]}
                    {@const v = shown(s)}
                    <div class="srow" class:pending={pending !== undefined} class:bad={errors[s.id]}>
                      <button class="star" class:on={favourites.has(s.id)} aria-label={favourites.has(s.id) ? "Remove from favourites" : "Add to favourites"} aria-pressed={favourites.has(s.id)} onclick={() => toggleFavourite(s.id)}>{favourites.has(s.id) ? "★" : "☆"}</button>
                      <div class="sname">
                        <button class="title" aria-expanded={!!expanded[s.id]} onclick={() => (expanded[s.id] = !expanded[s.id])}>{s.name || "Unnamed setting"}</button>
                        <div class="meta">
                          <span class="mono" title="Setting ID">{s.idHex}</span>
                          {#if !s.documented}<span class="pill warn" title="NVIDIA does not document this setting. Its meaning comes from the community and can change between drivers.">Undocumented</span>{/if}
                          {#if s.internal}<span class="pill" title={s.readOnlyReason ?? ""}>Encoded</span>{/if}
                          {#if s.note}<span class="pill warn">{s.note}</span>{/if}
                        </div>
                      </div>
                      <div class="sval">
                        {#if !s.editable}
                          <span class="mono ro" title={s.readOnlyReason ?? ""}>{s.raw || "—"}</span>
                        {:else if (s.kind === "dword" || s.kind === "qword") && s.options.length && !custom[s.id] && (!v || s.options.some((o) => String(o.value) === v.value))}
                          <select aria-label="Value of {s.name || s.idHex}" value={v?.value ?? ""} onchange={(e) => picked(s, e.currentTarget.value)}>
                            {#if !v}<option value="" disabled>Not set</option>{/if}
                            {#each s.options as o}<option value={String(o.value)}>{o.label} · {hex(o.value, s.kind === "qword" ? 16 : 8)}</option>{/each}
                            <option value="custom">Other value…</option>
                          </select>
                        {:else}
                          <input class="mono" aria-label="Value of {s.name || s.idHex}" aria-invalid={!!errors[s.id]} value={drafts[s.id] ?? editText(v)} placeholder={s.kind === "binary" ? "hex bytes" : s.kind === "wstring" || s.kind === "ansi" ? "text" : "0x00000000"} oninput={(e) => typed(s, e.currentTarget.value)} />
                        {/if}
                        {#if s.editable && s.kind === "dword"}<button class="ghost small" title="Edit as individual bits" onclick={() => openBits(s)}>Bits</button>{/if}
                      </div>
                      <div class="sstate">
                        {#if errors[s.id]}<span class="err">{errors[s.id]}</span>
                        {:else if pending === "restore"}<span class="pill live">Will be restored</span>
                        {:else if pending !== undefined}<span class="pill live">Pending</span>
                        {:else}<span class="pill" class:mine={s.state === "user"}>{STATE_TEXT[s.state]}</span>{/if}
                      </div>
                      <div class="sact">
                        {#if pending !== undefined || errors[s.id]}<button class="ghost small" onclick={() => undo(s)}>Undo</button>
                        {:else if s.modified}<button class="ghost small" title="Back to {s.defaultLabel ?? (s.default ? editText(s.default) : 'the inherited value')}" onclick={() => restore(s)}>Restore</button>{/if}
                      </div>
                      {#if expanded[s.id]}
                        <div class="sinfo">
                          {#if s.description}<p class="desc">{s.description}</p>{/if}
                          <dl>
                            <div><dt>Raw value</dt><dd class="mono">{s.raw || "none"}{#if s.valueLabel} <span class="faint">({s.valueLabel})</span>{/if}</dd></div>
                            <div><dt>Restores to</dt><dd class="mono">{s.default ? editText(s.default) : "nothing stored"}{#if s.defaultLabel} <span class="faint">({s.defaultLabel})</span>{/if}</dd></div>
                            <div><dt>Type</dt><dd>{s.kindLabel}</dd></div>
                            <div><dt>ID</dt><dd class="mono">{s.idHex} · {s.id}</dd></div>
                            <div><dt>Name source</dt><dd>{NV_NAME_SOURCE[s.nameSource]}</dd></div>
                            <div><dt>Listed by this driver</dt><dd>{s.exposed ? "Yes" : "No"}</dd></div>
                            {#if s.alternateNames.length}<div><dt>Also known as</dt><dd>{s.alternateNames.join(", ")}</dd></div>{/if}
                          </dl>
                          {#if s.readOnlyReason}<p class="desc">{s.readOnlyReason}</p>{/if}
                        </div>
                      {/if}
                    </div>
                  {/each}
                {/if}
              </div>
            {/each}
            {#if !groups.length}<p class="ff-note pad">{loadingDetail ? "Reading the profile…" : "No settings match these filters."}</p>{/if}
          </div>

          {#if pendingCount > 0}
            <div class="pendingbar">
              <span><strong>{pendingCount}</strong> change{pendingCount === 1 ? "" : "s"} not applied yet</span>
              <div class="row">
                <button class="ghost" onclick={clearEdits}>Discard</button>
                <button class="play" disabled={hasErrors || applying} onclick={review}>{applying ? "Applying…" : "Review and apply"}</button>
              </div>
            </div>
          {/if}
        {:else}
          <p class="ff-note pad">{loadingDetail || loadingList ? "Reading the driver…" : "Choose a profile on the left."}</p>
        {/if}
      </section>
    </div>
  {/if}
</div>

<Modal bind:open={reviewOpen} title="Apply to {selected}?" wide>
  <p>Arcadia saves a snapshot of this profile first, writes the changes through the NVIDIA driver's own settings API, then reads every value back to check it. The snapshot stays under Backups.</p>
  <table class="diff">
    <thead><tr><th>Setting</th><th>Now</th><th>After</th></tr></thead>
    <tbody>
      {#each plan as l}
        <tr>
          <td>{l.what}{#if l.settingId !== null}<div class="mono faint">{hex(l.settingId)}</div>{/if}{#if l.note}<div class="note">{l.note}</div>{/if}</td>
          <td class="mono">{l.before}</td>
          <td class="mono">{l.after}</td>
        </tr>
      {/each}
    </tbody>
  </table>
  <p class="ff-note">Driver profile changes take effect the next time the game starts.</p>
  {#snippet actions()}
    <button class="ghost" onclick={() => { reviewOpen = false; restoreAll = false; }}>Cancel</button>
    <button class="play" onclick={apply}>Apply to driver</button>
  {/snippet}
</Modal>

<Modal bind:open={bitsOpen} title="{bits?.name || bits?.idHex} as bits">
  {#if bits}
    <p class="mono">{hex(bitsValue >>> 0)}</p>
    <div class="bits">
      {#each Array.from({ length: 32 }, (_, i) => 31 - i) as bit}
        <label title={bitLabel(bits, bit)}>
          <input type="checkbox" checked={((bitsValue >>> bit) & 1) === 1} onchange={(e) => (bitsValue = (e.currentTarget.checked ? bitsValue | (1 << bit) : bitsValue & ~(1 << bit)) >>> 0)} />
          <span class="mono">{bit}</span>{#if bitLabel(bits, bit)}<span class="bl">{bitLabel(bits, bit)}</span>{/if}
        </label>
      {/each}
    </div>
  {/if}
  {#snippet actions()}
    <button class="ghost" onclick={() => (bitsOpen = false)}>Cancel</button>
    <button onclick={applyBits}>Use this value</button>
  {/snippet}
</Modal>

<Modal bind:open={createOpen} title="New driver profile">
  <label class="field"><span>Profile name</span><input bind:value={createName} placeholder="My Game" /></label>
  <label class="field"><span>Executable (optional)</span><input bind:value={createExe} placeholder="mygame.exe" /></label>
  <p class="ff-note">The profile applies to the executables you add to it. An executable can belong to one profile only.</p>
  {#snippet actions()}
    <button class="ghost" onclick={() => (createOpen = false)}>Cancel</button>
    <button disabled={!createName.trim()} onclick={createProfile}>Create</button>
  {/snippet}
</Modal>

<Modal bind:open={deleteOpen} title="Delete {selected}?">
  <p>This removes your profile from the driver. Arcadia saves a snapshot first, so it can be brought back from Backups.</p>
  {#snippet actions()}
    <button class="ghost" onclick={() => (deleteOpen = false)}>Cancel</button>
    <button class="danger" onclick={deleteProfile}>Delete profile</button>
  {/snippet}
</Modal>

<Modal bind:open={moreOpen} title="More" wide>
  <div class="more">
    <div>
      <div class="lead">Driver database backup</div>
      <p class="ff-note">NVIDIA's own dump of the whole profile database. This is not a .nip file and Arcadia does not read it back; keep it for NVIDIA's tools.</p>
      <button onclick={saveDatabase}>Save database backup…</button>
    </div>
    <div>
      <div class="lead">NVIDIA Profile Inspector (optional fallback)</div>
      <p class="ff-note">Arcadia edits profiles itself. If you have Profile Inspector installed, Arcadia can hand a .nip file to it when its own import cannot do the job, and can read extra setting names from its Reference.xml. Arcadia never downloads it.</p>
      <p class="mono path">{ffs.config?.npiExe ?? "Not set"}</p>
      <div class="row wrap">
        <button onclick={chooseNpi}>Choose nvidiaProfileInspector.exe…</button>
        {#if ffs.config?.npiExe}
          <button class="ghost" onclick={loadReference}>{status?.referenceLoaded ? "Reload Reference.xml" : "Load names from Reference.xml"}</button>
          <button class="ghost" onclick={npiExport} title="Profile Inspector writes every customised profile to a time-stamped file next to its executable">Export all with Profile Inspector</button>
          <button class="ghost" onclick={async () => { try { ffs.config = await nv.setNpi(null); } catch (e) { toast(String(e), "error"); } }}>Forget</button>
        {/if}
      </div>
    </div>
    <p class="ff-note">Setting names: {status?.knownSettings ?? 0} known. Names come from NVIDIA's SDK and from the NVIDIA Profile Inspector project (MIT licence, © Orbmu2k).</p>
  </div>
  {#snippet actions()}<button onclick={() => (moreOpen = false)}>Close</button>{/snippet}
</Modal>

{#if importPreview}
  <NvImport bind:open={importOpen} preview={importPreview} path={importPath} npiReady={!!ffs.config?.npiExe} ondone={afterDriverChange} />
{/if}
<NvBackups bind:open={backupsOpen} selected={selected} available={!!status?.available} ondone={afterDriverChange} />
<NvCompare bind:open={compareOpen} />

<style>
  .nvp { height: 100%; display: flex; flex-direction: column; min-height: 0; }
  .toolbar { display: flex; justify-content: space-between; align-items: center; gap: 1rem; padding: 0.8rem 2.2rem; border-bottom: 1px solid var(--line-soft); flex-wrap: wrap; }
  .row { display: flex; gap: 0.5rem; align-items: center; }
  .row.wrap { flex-wrap: wrap; }
  .split { flex: 1; min-height: 0; display: grid; grid-template-columns: 320px 1fr; }
  .list { border-right: 1px solid var(--line-soft); display: flex; flex-direction: column; min-height: 0; min-width: 0; }
  .list-head { padding: 0.8rem 0.9rem; display: grid; gap: 0.5rem; border-bottom: 1px solid var(--line-soft); }
  .list-head input { width: 100%; box-sizing: border-box; }
  .list-head select { width: auto; flex: 1; min-width: 0; }
  .rows { flex: 1; overflow-y: auto; padding: 0.4rem; }
  .prow { width: 100%; display: grid; grid-template-columns: 1fr auto; gap: 0.1rem 0.5rem; text-align: left; border: 0; background: transparent; border-radius: var(--r-md); padding: 0.5rem 0.6rem; }
  .prow:hover { background: var(--sunk); }
  .prow.on { background: var(--raised); box-shadow: inset 2px 0 0 var(--peach); }
  .pn { font-weight: 600; font-size: 0.9rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; display: flex; gap: 0.4rem; align-items: center; }
  .pe { grid-column: 1; font-size: 0.76rem; color: var(--faint); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .count { grid-row: 1 / span 2; grid-column: 2; align-self: center; min-width: 1.5rem; text-align: center; font-size: 0.75rem; font-weight: 700; color: var(--text); border: 1px solid var(--peach); border-radius: 999px; padding: 0.05rem 0.4rem; font-variant-numeric: tabular-nums; }
  .list-foot { display: flex; justify-content: space-between; align-items: center; padding: 0.6rem 0.9rem; border-top: 1px solid var(--line-soft); }
  .small { padding: 0.25rem 0.6rem; font-size: 0.8rem; }
  .pad { padding: 1rem; }
  .editor { display: flex; flex-direction: column; min-height: 0; min-width: 0; }
  .phead { display: flex; justify-content: space-between; align-items: flex-start; gap: 1rem; padding: 1rem 1.6rem 0.6rem; }
  .phead h3 { font-size: 1.25rem; }
  .tags { display: flex; gap: 0.6rem; align-items: center; margin-top: 0.3rem; }
  .apps { display: flex; flex-wrap: wrap; gap: 0.4rem; align-items: center; padding: 0 1.6rem 0.8rem; }
  .k { font-size: 0.68rem; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; color: var(--faint); margin-right: 0.3rem; }
  .chip { display: inline-flex; align-items: center; gap: 0.3rem; font-size: 0.8rem; font-family: var(--mono, monospace); background: var(--sunk); border: 1px solid var(--line-soft); border-radius: 999px; padding: 0.1rem 0.25rem 0.1rem 0.6rem; }
  .chip button { border: 0; background: transparent; padding: 0 0.3rem; color: var(--faint); font-size: 0.9rem; line-height: 1; }
  .chip button:hover { color: var(--text); background: transparent; }
  .chip.gone { text-decoration: line-through; opacity: 0.6; }
  .chip.new { border-color: var(--mint); }
  .addexe { display: inline-flex; gap: 0.3rem; }
  .addexe input { width: 150px; padding: 0.2rem 0.5rem; font-size: 0.8rem; }
  .filters { display: flex; gap: 0.6rem; align-items: center; flex-wrap: wrap; padding: 0.6rem 1.6rem; border-top: 1px solid var(--line-soft); border-bottom: 1px solid var(--line-soft); }
  .filters input[type="search"] { width: 260px; }
  .check { display: flex; gap: 0.35rem; align-items: center; font-size: 0.84rem; color: var(--muted); white-space: nowrap; }
  .check input { width: 15px; height: 15px; padding: 0; accent-color: var(--peach); }
  .tight { padding: 0.5rem 1.6rem 0; }
  .settings { flex: 1; overflow-y: auto; padding: 0.4rem 1.6rem 1.2rem; }
  .ghead { width: 100%; display: flex; justify-content: space-between; border: 0; background: transparent; border-radius: 0; padding: 0.9rem 0 0.35rem; font-size: 0.72rem; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; color: var(--muted); border-bottom: 1px solid var(--line-soft); }
  .ghead:hover { background: transparent; color: var(--text); }
  .srow { display: grid; grid-template-columns: 24px minmax(180px, 1.2fr) minmax(220px, 1fr) 150px 70px; gap: 0.6rem; align-items: center; padding: 0.4rem 0; border-bottom: 1px solid var(--line-soft); }
  .srow.pending { background: color-mix(in srgb, var(--peach) 7%, transparent); }
  .srow.bad { background: color-mix(in srgb, var(--rose) 8%, transparent); }
  .star { border: 0; background: transparent; padding: 0; color: var(--faint); font-size: 1rem; }
  .star.on { color: var(--peach); }
  .star:hover { background: transparent; color: var(--text); }
  .sname { min-width: 0; }
  .title { border: 0; background: transparent; padding: 0; text-align: left; font-weight: 600; font-size: 0.9rem; color: var(--text); }
  .title:hover { background: transparent; text-decoration: underline; }
  .meta { display: flex; gap: 0.4rem; align-items: center; flex-wrap: wrap; margin-top: 0.1rem; font-size: 0.74rem; color: var(--faint); }
  .meta .pill { font-size: 0.66rem; padding: 0 0.4rem; }
  .sval { display: flex; gap: 0.3rem; align-items: center; min-width: 0; }
  .sval select, .sval input { flex: 1; min-width: 0; padding: 0.3rem 0.5rem; font-size: 0.84rem; }
  .sval input[aria-invalid="true"] { border-color: var(--rose); }
  .ro { color: var(--muted); font-size: 0.84rem; overflow: hidden; text-overflow: ellipsis; }
  .sstate .pill { font-size: 0.7rem; }
  .pill.mine { border-color: var(--peach); color: var(--text); }
  .err { color: var(--rose); font-size: 0.76rem; }
  .sact { text-align: right; }
  .sinfo { grid-column: 2 / -1; padding: 0.2rem 0 0.6rem; display: grid; gap: 0.5rem; }
  .desc { font-size: 0.84rem; color: var(--muted); white-space: pre-line; line-height: 1.45; }
  .sinfo dl { display: grid; grid-template-columns: repeat(auto-fit, minmax(190px, 1fr)); gap: 0.4rem 1rem; margin: 0; font-size: 0.82rem; }
  .sinfo dt { font-size: 0.66rem; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; color: var(--faint); }
  .sinfo dd { margin: 0; color: var(--muted); overflow-wrap: anywhere; }
  .pendingbar { display: flex; justify-content: space-between; align-items: center; gap: 1rem; padding: 0.7rem 1.6rem; border-top: 1px solid var(--line); background: var(--raised); }
  .diff { width: 100%; border-collapse: collapse; font-size: 0.86rem; }
  .diff th { text-align: left; font-size: 0.7rem; letter-spacing: 0.08em; text-transform: uppercase; color: var(--faint); padding: 0 0.6rem 0.35rem 0; }
  .diff td { padding: 0.4rem 0.6rem 0.4rem 0; border-top: 1px solid var(--line-soft); color: var(--text); vertical-align: top; overflow-wrap: anywhere; }
  .note { font-size: 0.78rem; color: var(--rose); margin-top: 0.15rem; }
  .bits { display: grid; grid-template-columns: repeat(4, 1fr); gap: 0.3rem 0.8rem; }
  .bits label { display: flex; gap: 0.35rem; align-items: center; font-size: 0.8rem; min-width: 0; }
  .bits input { width: 14px; height: 14px; padding: 0; accent-color: var(--peach); }
  .bl { color: var(--faint); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .field { display: grid; gap: 0.3rem; }
  .field span { font-size: 0.8rem; color: var(--muted); }
  .more { display: grid; gap: 1.2rem; }
  .more > div { display: grid; gap: 0.5rem; justify-items: start; }
  .path { font-size: 0.82rem; color: var(--muted); overflow-wrap: anywhere; }
  .ff-banner { margin-bottom: 1rem; }
</style>
