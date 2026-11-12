// Browser-only preview data for GPU Control and presets (see ffmock.ts: none
// of this exists inside Arcadia, where every call goes to the Rust backend).
import type { ApplyRequest, GpuControl, GpuControlsView, GpuDevice, OptSession, GpuPreset, PresetCheck, PresetOutcome, PresetSettingCheck } from "./fftypes";

type Handler = (args: Record<string, unknown>) => unknown;

const igcl = (id: string, name: string, description: string, kind: GpuControl["kind"], value: number, def: number, extra: Partial<GpuControl> = {}): GpuControl => ({
  id, vendor: "intel", name, description, category: "3D settings", kind, value, default: def, supported: true, unsupportedReason: null, risk: "low", level: 2, requiresRestart: false, ...extra,
});

function intelControls(): GpuControl[] {
  return [
    igcl("igcl.3d.low_latency", "Low latency mode", "Reduces the frames queued ahead of the display.", { type: "choice", options: [{ value: 0, label: "Off" }, { value: 1, label: "On" }, { value: 2, label: "On + Boost" }] }, 0, 0),
    igcl("igcl.3d.frame_limit", "Frame rate limit", "Caps the frame rate in the driver.", { type: "range", min: 30, max: 240, step: 1, unit: "FPS" }, 60, 60),
    igcl("igcl.3d.sharpening", "Adaptive sharpening", "Sharpens the image after scaling.", { type: "toggle" }, 0, 0),
    igcl("igcl.3d.anisotropic", "Anisotropic filtering", "Texture filtering quality at shallow angles.", { type: "choice", options: [{ value: 0, label: "Application choice" }, { value: 2, label: "2x" }, { value: 4, label: "4x" }, { value: 8, label: "8x" }, { value: 16, label: "16x" }] }, 0, 0),
    igcl("igcl.3d.vrr_windowed", "Variable refresh rate in windowed games", "Lets the display follow the game's frame rate outside exclusive fullscreen.", { type: "toggle" }, 0, 0, { supported: false, unsupportedReason: "The driver reports this feature as not supported on this adapter." }),
    igcl("igcl.power_limit", "Sustained power limit", "The power the card may draw over time, in the driver's own units and range.", { type: "range", min: 95, max: 228, step: 1, unit: "W" }, 190, 190, { category: "Limits", level: 3, risk: "high" }),
    igcl("igcl.temperature_limit", "Temperature limit", "The temperature at which the card starts to slow down.", { type: "range", min: 65, max: 100, step: 1, unit: "°C" }, 90, 90, { category: "Limits", level: 3, risk: "high" }),
  ];
}

export function gpuMock(gpus: GpuDevice[], optApply: Handler): Record<string, Handler> {
  const controls: Record<string, GpuControl[]> = {};
  for (const g of gpus) controls[g.id] = g.vendor === "intel" && g.api.state === "available" ? intelControls() : [];

  const views = (): GpuControlsView[] =>
    gpus.map((gpu) => ({
      gpu, api: gpu.api, controls: structuredClone(controls[gpu.id]),
      note: gpu.vendor === "nvidia" ? "NVIDIA keeps 3D settings in driver profiles, one per game plus a global one. They are edited on the NVIDIA Profiles page." : null,
    }));

  const first = gpus[0];
  let presets: GpuPreset[] = !first ? [] : [first.vendor === "nvidia"
    ? { schema: 1, id: "preview-nv", name: "Preview: low latency", vendor: "nvidia", gpuModel: first.name, driverVersion: "566.14", minDriver: null, gameExe: "cs2.exe", settings: [{ id: "drs.0x007BA09E", value: { type: "dword", value: "1" }, name: "Maximum pre-rendered frames" }, { id: "drs.0x00A879CF", value: { type: "dword", value: "138504007" }, name: "Vertical Sync" }], createdAt: 1762000000, createdWith: "Arcadia (browser preview)", notes: "Sample data." }
    : { schema: 1, id: "preview-gpu", name: "Preview: capped and sharp", vendor: first.vendor, gpuModel: first.name, driverVersion: first.driverVersion, minDriver: null, gameExe: null, settings: first.vendor === "intel" ? [{ id: "igcl.3d.frame_limit", value: 120, name: "Frame rate limit" }, { id: "igcl.3d.sharpening", value: 1, name: "Adaptive sharpening" }, { id: "igcl.3d.vrr_windowed", value: 1, name: "Variable refresh rate in windowed games" }] : [{ id: "adlx.antilag", value: 1, name: "Radeon Anti-Lag" }, { id: "adlx.chill", value: 0, name: "Radeon Chill" }], createdAt: 1762000000, createdWith: "Arcadia (browser preview)", notes: "Sample data." }];

  const text = (c: GpuControl, v: number) => (c.kind.type === "toggle" ? (v ? "On" : "Off") : c.kind.type === "choice" ? (c.kind.options.find((o) => o.value === v)?.label ?? String(v)) : c.kind.type === "range" ? `${v} ${c.kind.unit}` : String(v));

  function check(p: GpuPreset): PresetCheck {
    const gpu = gpus.find((g) => g.vendor === p.vendor);
    if (!gpu) return { usable: false, problems: [`This preset is for ${p.vendor} graphics, which this PC does not have.`], warnings: [], target: "", settings: [], level: 1 };
    if (p.vendor === "nvidia") {
      const settings: PresetSettingCheck[] = p.settings.map((s, i) => ({ id: s.id, name: s.name, current: i ? "Use the 3D application setting (1620202130)" : "Use the 3D application setting (0)", target: typeof s.value === "number" ? String(s.value) : s.value.value, status: "change", reason: null }));
      return { usable: true, problems: [], warnings: [], target: `NVIDIA driver profile "Counter-Strike 2"`, settings, level: 2 };
    }
    if (gpu.api.state !== "available") return { usable: false, problems: [`${gpu.api.state === "unavailable" ? gpu.api.api : "The vendor API"} is not available: ${gpu.api.state === "unavailable" ? gpu.api.reason : ""}`], warnings: [], target: gpu.name, settings: [], level: 1 };
    let level = 1;
    const settings: PresetSettingCheck[] = p.settings.map((s) => {
      const c = controls[gpu.id].find((x) => x.id === s.id);
      const v = s.value as number;
      if (!c) return { id: s.id, name: s.name, current: null, target: String(v), status: "skip", reason: "The driver does not offer this setting on this adapter." };
      if (!c.supported) return { id: s.id, name: c.name, current: null, target: String(v), status: "skip", reason: c.unsupportedReason };
      const same = c.value === v;
      if (!same) level = Math.max(level, c.level);
      return { id: s.id, name: c.name, current: c.value === null ? null : text(c, c.value), target: text(c, v), status: same ? "same" : "change", reason: null };
    });
    const usable = settings.some((s) => s.status === "change");
    return { usable, problems: [], warnings: usable ? [] : ["Everything in this preset is already set."], target: gpu.name, settings, level };
  }

  const get = (id: unknown) => {
    const p = presets.find((x) => x.id === id);
    if (!p) throw new Error("That preset does not exist");
    return p;
  };

  return {
    ff_gpu_controls: views,
    ff_opt_apply: (args) => {
      const session = optApply(args) as OptSession;
      for (const r of args.requests as ApplyRequest[]) {
        if (r.id !== "gpu.control") continue;
        const p = r.params as { gpu: string; control: string; value: number };
        const c = controls[p.gpu]?.find((x) => x.id === p.control);
        const change = session.changes.find((x) => x.params === r.params || JSON.stringify(x.params) === JSON.stringify(r.params));
        if (c && change) Object.assign(change, { name: c.name, category: "driver", level: c.level, risk: c.risk, reversible: true, temporary: false, beforeText: c.value === null ? "unknown" : text(c, c.value), proposedText: text(c, p.value), afterText: text(c, p.value) });
        if (c) c.value = p.value;
      }
      return session;
    },
    ff_presets: () => structuredClone(presets),
    ff_preset_capture: (a) => {
      const gpu = gpus.find((g) => g.id === a.gpu);
      if (!gpu) throw new Error("That graphics adapter is no longer present");
      const settings = gpu.vendor === "nvidia"
        ? [{ id: "drs.0x00A879CF", value: { type: "dword" as const, value: "138504007" }, name: "Vertical Sync" }]
        : controls[gpu.id].filter((c) => c.supported && c.level < 3 && c.value !== null).map((c) => ({ id: c.id, value: c.value as number, name: c.name }));
      if (!settings.length) throw new Error("The driver reports no adjustable settings for this adapter, so there is nothing to save");
      const p: GpuPreset = { schema: 1, id: `preview-${presets.length + 1}-${Date.now()}`, name: String(a.name).trim(), vendor: gpu.vendor, gpuModel: gpu.name, driverVersion: gpu.driverVersionVendor ?? gpu.driverVersion, minDriver: null, gameExe: (a.gameExe as string | null) || null, settings, createdAt: Math.floor(Date.now() / 1000), createdWith: "Arcadia (browser preview)", notes: String(a.notes ?? "") };
      if (!p.name) throw new Error("The preset needs a name of at most 120 characters");
      presets.push(p);
      return structuredClone(p);
    },
    ff_preset_check: (a) => check(get(a.id)),
    ff_preset_apply: (a): PresetOutcome => {
      const p = get(a.id);
      const c = check(p);
      if (!c.usable) throw new Error(c.problems[0] ?? "Everything in this preset is already set.");
      const gpu = gpus.find((g) => g.vendor === p.vendor)!;
      const wanted = c.settings.filter((s) => s.status === "change");
      for (const s of wanted) {
        const ctl = controls[gpu.id].find((x) => x.id === s.id);
        const v = p.settings.find((x) => x.id === s.id)?.value;
        if (ctl && typeof v === "number") ctl.value = v;
      }
      return { applied: wanted.length, skipped: c.settings.filter((s) => s.status === "skip"), verified: true, message: p.vendor === "nvidia" ? "Applied to Counter-Strike 2 and verified. Undo it from NVIDIA Profiles, Backups." : `Applied to ${gpu.name} and read back. Undo it from Optimizer, History.`, sessionId: p.vendor === "nvidia" ? null : "preview", backupId: p.vendor === "nvidia" ? "preview" : null };
    },
    ff_preset_import: () => {
      const src = presets[0];
      if (!src) throw new Error("The file is not an Arcadia preset");
      const p = { ...structuredClone(src), id: `preview-import-${Date.now()}`, name: `${src.name} (imported)` };
      presets.push(p);
      return structuredClone(p);
    },
    ff_preset_export: () => undefined,
    ff_preset_delete: (a) => {
      get(a.id);
      presets = presets.filter((x) => x.id !== a.id);
    },
  };
}
