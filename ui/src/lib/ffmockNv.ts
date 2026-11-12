// Browser-only preview data for the NVIDIA Profiles page (see ffmock.ts).
// Inside Arcadia every one of these calls goes to the driver through NVAPI.
// `?nv=none` previews a PC without an NVIDIA driver.
import type { NipDiff, NipFile, NvApp, NvBackup, NvChange, NvDiffLine, NvImportPreview, NvPreviewSetting, NvProfileDetail, NvProfileSummary, NvSetting, NvSettingState, NvValue } from "./fftypes";

const none = typeof location !== "undefined" && new URLSearchParams(location.search).get("nv") === "none";
const REASON = "nvapi64.dll was not found. Install the NVIDIA graphics driver.";
const hex = (n: number, w = 8) => "0x" + n.toString(16).toUpperCase().padStart(w, "0");
const dword = (n: number): NvValue => ({ type: "dword", value: String(n) });

interface Def { id: number; name: string; group: string; description: string; options: [number, string][]; def: number; documented: boolean; common: boolean; source: NvSetting["nameSource"] }
const defs: Def[] = [
  { id: 0x00a879cf, name: "Vertical Sync", group: "02 - Sync and Refresh", description: "Controls vertical sync for the application.", options: [[0x60925292, "Use the 3D application setting"], [0x08416747, "Force off"], [0x47814940, "Force on"], [0x18888888, "Fast Sync"]], def: 0x60925292, documented: true, common: true, source: "profileInspector" },
  { id: 0x10835002, name: "Frame Rate Limiter V3", group: "02 - Sync and Refresh", description: "Limits the frame rate. 0 is off.", options: [[0, "Off"], [60, "60 FPS"], [120, "120 FPS"], [141, "141 FPS"]], def: 0, documented: true, common: true, source: "profileInspector" },
  { id: 0x1057eb71, name: "Power Management - Mode", group: "05 - Common", description: "How the GPU balances performance and power use.", options: [[0, "Adaptive"], [1, "Prefer maximum performance"], [5, "Optimal power"], [3, "Prefer consistent performance"]], def: 5, documented: true, common: true, source: "profileInspector" },
  { id: 0x007ba09e, name: "Maximum Pre-Rendered Frames", group: "02 - Sync and Refresh", description: "How many frames the CPU prepares ahead of the GPU.", options: [[0, "Use the 3D application setting"], [1, "1"], [2, "2"], [3, "3"]], def: 0, documented: true, common: true, source: "profileInspector" },
  { id: 0x00ce2691, name: "Texture Filtering - Quality", group: "04 - Texture Filtering", description: "Trades texture filtering quality for speed.", options: [[0xfffffff6, "High quality"], [0, "Quality"], [0x0a, "Performance"], [0x14, "High performance"]], def: 0, documented: true, common: true, source: "profileInspector" },
  { id: 0x10e41e01, name: "DLSS - Forced Preset Letter", group: "05 - Upscaling and Frame Generation", description: "Overrides the DLSS preset the game asks for.\nNot documented by NVIDIA.", options: [[0, "Off"], [0x0b, "Preset K"], [0x00ffffff, "Always use latest"]], def: 0, documented: false, common: true, source: "profileInspector" },
  { id: 0x20d690f8, name: "Vulkan/OpenGL present method", group: "Other NVIDIA settings", description: "", options: [[0, "Prefer native"], [1, "Prefer layered on dxgi swapchain"], [2, "Auto"]], def: 2, documented: true, common: false, source: "nvidia" },
  { id: 0x00f00ba5, name: "", group: "Unidentified", description: "", options: [], def: 0, documented: false, common: false, source: "none" },
];

interface P { name: string; kind: NvProfileSummary["kind"]; apps: NvApp[]; nvidia: Record<number, number>; user: Record<number, number> }
const app = (name: string, isPredefined = true): NvApp => ({ name, friendlyName: "", launcher: "", fileInFolder: "", commandLine: "", isPredefined });
let profiles: P[] = [
  { name: "Base Profile", kind: "global", apps: [], nvidia: { 0x00a879cf: 0x60925292, 0x1057eb71: 5 }, user: { 0x1057eb71: 1 } },
  { name: "Counter-Strike 2", kind: "predefined", apps: [app("cs2.exe")], nvidia: { 0x007ba09e: 1, 0x00f00ba5: 0xcafe }, user: { 0x00a879cf: 0x08416747, 0x10835002: 0 } },
  { name: "Cyberpunk 2077", kind: "predefined", apps: [app("cyberpunk2077.exe"), app("redprelauncher.exe")], nvidia: { 0x007ba09e: 1 }, user: {} },
  { name: "Elden Ring", kind: "predefined", apps: [app("eldenring.exe")], nvidia: {}, user: {} },
  { name: "Minecraft (Java)", kind: "predefined", apps: [app("javaw.exe")], nvidia: { 0x20d690f8: 1 }, user: {} },
  { name: "My Indie Game", kind: "user", apps: [app("indie.exe", false)], nvidia: {}, user: { 0x10e41e01: 0x0b } },
];
let backups: NvBackup[] = [];

const find = (name: string) => {
  const p = profiles.find((x) => x.name.toLowerCase() === name.toLowerCase());
  if (!p) throw new Error(`The NVIDIA driver has no profile named '${name}'`);
  return p;
};
const summary = (p: P): NvProfileSummary => ({ name: p.name, kind: p.kind, executables: p.apps.map((a) => a.name), settingCount: new Set([...Object.keys(p.nvidia), ...Object.keys(p.user)]).size, customisedCount: Object.keys(p.user).length, userExecutables: p.kind === "predefined" ? p.apps.filter((a) => !a.isPredefined).length : 0 });

function setting(p: P, d: Def): NvSetting {
  const global = profiles[0];
  let state: NvSettingState = "default";
  let v = d.def;
  if (d.id in p.user) { state = "user"; v = p.user[d.id]; }
  else if (d.id in p.nvidia) { state = "nvidia"; v = p.nvidia[d.id]; }
  else if (p !== global && (d.id in global.user || d.id in global.nvidia)) { state = "inherited"; v = global.user[d.id] ?? global.nvidia[d.id]; }
  const dv = d.id in p.nvidia ? p.nvidia[d.id] : p !== global ? (global.user[d.id] ?? global.nvidia[d.id] ?? d.def) : d.def;
  const label = (n: number) => d.options.find((o) => o[0] === n)?.[1] ?? null;
  const internal = d.id === 0x00f00ba5 && state === "nvidia";
  return {
    id: d.id, idHex: hex(d.id), name: d.name, nameSource: d.source, alternateNames: [], group: d.group, description: d.description, kind: "dword", kindLabel: "32-bit number",
    value: dword(v), valueLabel: label(v), raw: hex(v), default: dword(dv), defaultLabel: label(dv), state, modified: state === "user",
    options: d.options.map(([value, l]) => ({ value, label: l })), documented: d.documented, exposed: d.documented, common: d.common, hidden: false, internal,
    editable: !internal, readOnlyReason: internal ? "NVIDIA stores this value encoded. It is shown exactly as the driver returns it and is not edited by Arcadia." : null, note: null,
  };
}

function detail(name: string): NvProfileDetail {
  const p = find(name);
  const settings = defs.filter((d) => d.common || d.id in p.user || d.id in p.nvidia || d.documented).map((d) => setting(p, d)).sort((a, b) => a.group.localeCompare(b.group) || a.name.localeCompare(b.name));
  return { summary: summary(p), apps: structuredClone(p.apps), settings, groups: [...new Set(settings.map((s) => s.group))], internalScan: true };
}

function plan(profile: string, changes: NvChange[]): NvDiffLine[] {
  const p = find(profile);
  return changes.map((c): NvDiffLine => {
    if (c.op === "set" || c.op === "restore") {
      const d = defs.find((x) => x.id === c.id);
      const s = d ? setting(p, d) : null;
      const text = (v: NvValue | null, l: string | null) => (v ? (l ? `${l} (${hex(Number(v.value))})` : hex(Number(v.value))) : "not set");
      const after = c.op === "set" ? text(c.value, d?.options.find((o) => o[0] === Number(c.value.value))?.[1] ?? null) : `${text(s?.default ?? null, s?.defaultLabel ?? null)} (restored)`;
      return { profile: p.name, settingId: c.id, what: d?.name || "Unnamed setting", before: text(s?.value ?? null, s?.valueLabel ?? null), after, note: d && !d.documented ? "Not documented by NVIDIA. Its effect can differ between driver versions." : null };
    }
    if (c.op === "restoreProfile") return { profile: p.name, settingId: null, what: "All settings", before: `${Object.keys(p.user).length} changed by you`, after: "NVIDIA's values", note: null };
    if (c.op === "addApp") return { profile: p.name, settingId: null, what: "Executable", before: "not in this profile", after: c.exe, note: null };
    return { profile: p.name, settingId: null, what: "Executable", before: c.exe, after: "removed from this profile", note: null };
  });
}

function snapshot(label: string, reason: NvBackup["reason"], names: string[], changes: NvDiffLine[]): NvBackup {
  const now = Math.floor(Date.now() / 1000);
  const b: NvBackup & { _state?: P[] } = { id: `${now}-preview${backups.length}`, createdAt: now, label, reason, profiles: names, createdProfiles: [], file: "preview.nip", changes, verified: true, note: null, driverVersion: "566.14" };
  b._state = structuredClone(profiles.filter((p) => names.includes(p.name)));
  backups.unshift(b);
  return b;
}

function apply(profile: string, changes: NvChange[]) {
  const lines = plan(profile, changes);
  const p = find(profile);
  const b = snapshot(`Before editing ${p.name}`, "edit", [p.name], lines);
  for (const c of changes) {
    if (c.op === "set") p.user[c.id] = Number(c.value.value);
    else if (c.op === "restore") delete p.user[c.id];
    else if (c.op === "restoreProfile") p.user = {};
    else if (c.op === "addApp") p.apps.push(app(c.exe.toLowerCase(), false));
    else p.apps = p.apps.filter((a) => a.name.toLowerCase() !== c.exe.toLowerCase());
  }
  return { backupId: b.id, changes: lines, verified: true, mismatches: [] };
}

const sampleFile: NipFile = {
  encoding: "utf8Bom", hasFindFiles: true, warnings: [],
  profiles: [{
    name: "Counter-Strike 2", executables: ["cs2.exe"], findFiles: [], extra: [],
    settings: [
      { id: 0x00a879cf, nameInfo: "Vertical Sync", valueType: "Dword", rawValue: String(0x08416747), typeAbsent: false },
      { id: 0x10835002, nameInfo: "Frame Rate Limiter V3", valueType: "Dword", rawValue: "141", typeAbsent: false },
      { id: 0x1057eb71, nameInfo: "Power Management - Mode", valueType: "Dword", rawValue: "1", typeAbsent: false },
      { id: 0xdeadbeef, nameInfo: "", valueType: "Dword", rawValue: "7", typeAbsent: false },
    ],
  }],
};

function preview(file: NipFile): NvImportPreview {
  return {
    file, warnings: file.warnings, driverAvailable: !none, driverReason: none ? REASON : null,
    profiles: file.profiles.map((fp) => {
      const p = none ? undefined : profiles.find((x) => x.name.toLowerCase() === fp.name.toLowerCase());
      const settings = fp.settings.map((s): NvPreviewSetting => {
        const d = defs.find((x) => x.id === s.id);
        const live = p && d ? setting(p, d) : null;
        const n = Number(s.rawValue);
        const problem = s.valueType !== "Dword" ? `value type '${s.valueType}' is not known to this version of Arcadia` : !/^\d+$/.test(s.rawValue) ? `'${s.rawValue}' is not a decimal number` : null;
        return { id: s.id, idHex: hex(s.id), name: d?.name || s.nameInfo || "", nameSource: d?.source ?? "none", documented: d?.documented ?? false, valueType: s.valueType, rawValue: s.rawValue, label: d?.options.find((o) => o[0] === n)?.[1] ?? null, problem, live: live ? (live.valueLabel ? `${live.valueLabel} (${live.raw})` : live.raw) : null, sameAsLive: !!live && Number(live.value?.value) === n, options: (d?.options ?? []).map(([value, label]) => ({ value, label })) };
      });
      return { name: fp.name, exists: !!p, kind: p?.kind ?? null, executables: fp.executables, conflicts: [], settings, liveOnly: [], unknownElements: fp.extra.length };
    }),
  };
}

function importFile(a: Record<string, unknown>) {
  const file = a.file as NipFile;
  const names = file.profiles.map((p) => p.name);
  const b = snapshot(a.label as string, "import", names.filter((n) => profiles.some((p) => p.name === n)), []);
  const results = file.profiles.map((fp) => {
    let p = profiles.find((x) => x.name.toLowerCase() === fp.name.toLowerCase());
    const created = !p;
    if (!p) profiles.push((p = { name: fp.name, kind: "user", apps: fp.executables.map((e) => app(e, false)), nvidia: {}, user: {} }));
    if (a.mode === "replace") p.user = {};
    for (const s of fp.settings) if (s.valueType === "Dword" && /^\d+$/.test(s.rawValue)) p.user[s.id] = Number(s.rawValue);
    return { profile: fp.name, created, ok: true, error: null, applied: fp.settings.length, skipped: [] };
  });
  return { backupId: b.id, profiles: results, verified: true, mismatches: [] };
}

function restore(id: string) {
  const b = backups.find((x) => x.id === id) as (NvBackup & { _state?: P[] }) | undefined;
  if (!b) throw new Error("That backup no longer exists");
  const state = structuredClone(b._state ?? []);
  snapshot(`Before restoring "${b.label}"`, "restore", b.profiles, []);
  profiles = profiles.map((p) => state.find((s) => s.name === p.name) ?? p);
  return { backupId: backups[0].id, profiles: b.profiles.map((profile) => ({ profile, created: false, ok: true, error: null, applied: 0, skipped: [] })), verified: true, mismatches: [] };
}

const diff: NipDiff = {
  identical: false,
  profiles: [
    { name: "Counter-Strike 2", status: "changed", settingsSame: 2, executablesOnlyA: [], executablesOnlyB: ["cs2-vulkan.exe"], settings: [{ id: 0x10835002, name: "Frame Rate Limiter V3", a: "0", b: "141", aType: "Dword", bType: "Dword" }, { id: 0xdeadbeef, name: "", a: null, b: "7", aType: null, bType: "Dword" }] },
    { name: "My Indie Game", status: "onlyA", settingsSame: 0, executablesOnlyA: ["indie.exe"], executablesOnlyB: [], settings: [{ id: 0x10e41e01, name: "DLSS - Forced Preset Letter", a: "11", b: null, aType: "Dword", bType: null }] },
  ],
};

const need = <T>(f: () => T): T => {
  if (none) throw new Error(REASON);
  return f();
};
const strip = (b: NvBackup) => { const { _state, ...rest } = b as NvBackup & { _state?: unknown }; void _state; return structuredClone(rest); };

export const nvMock: Record<string, (args: Record<string, unknown>) => unknown> = {
  ff_nv_status: () => (none ? { available: false, reason: REASON, driver: { version: null, branch: null }, knownSettings: 240, referenceLoaded: false } : { available: true, reason: null, driver: { version: "566.14", branch: "r565_00" }, knownSettings: 240, referenceLoaded: false }),
  ff_nv_profiles: () => need(() => profiles.map(summary)),
  ff_nv_profile: (a) => need(() => detail(a.name as string)),
  ff_nv_plan: (a) => need(() => plan(a.profile as string, a.changes as NvChange[])),
  ff_nv_apply: (a) => need(() => apply(a.profile as string, a.changes as NvChange[])),
  ff_nv_create_profile: (a) => need(() => { profiles.push({ name: a.name as string, kind: "user", apps: a.exe ? [app(a.exe as string, false)] : [], nvidia: {}, user: {} }); return { backupId: "preview", changes: [], verified: true, mismatches: [] }; }),
  ff_nv_delete_profile: (a) => need(() => { profiles = profiles.filter((p) => p.name !== a.name); return { backupId: "preview", changes: [], verified: true, mismatches: [] }; }),
  ff_nv_export: (a) => need(() => ((a.names as string[]).length || profiles.filter((p) => Object.keys(p.user).length).length)),
  ff_nv_import_preview: () => preview(structuredClone(sampleFile)),
  ff_nv_preview: (a) => preview(a.file as NipFile),
  ff_nv_import: (a) => need(() => importFile(a)),
  ff_nv_save_file: () => undefined,
  ff_nv_compare: () => structuredClone(diff),
  ff_nv_backups: () => backups.map(strip),
  ff_nv_backup_now: (a) => need(() => strip(snapshot((a.label as string) || "Manual backup", "manual", (a.names as string[]).length ? (a.names as string[]) : profiles.filter((p) => Object.keys(p.user).length).map((p) => p.name), []))),
  ff_nv_backup_contents: () => preview(structuredClone(sampleFile)),
  ff_nv_restore_backup: (a) => need(() => restore(a.id as string)),
  ff_nv_delete_backup: (a) => { backups = backups.filter((b) => b.id !== a.id); },
  ff_nv_save_database: () => need(() => undefined),
  ff_nv_npi_import: () => { throw new Error("Choose your NVIDIA Profile Inspector executable first"); },
  ff_nv_npi_export: () => { throw new Error("Choose your NVIDIA Profile Inspector executable first"); },
  ff_nv_set_npi: () => { throw new Error("The browser preview cannot check a file on disk"); },
  ff_nv_load_reference: () => { throw new Error("Choose your NVIDIA Profile Inspector executable first"); },
};
