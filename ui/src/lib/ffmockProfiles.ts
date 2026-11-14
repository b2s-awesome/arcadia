// Browser-only preview data for Game Profiles and Benchmark History (see
// ffmock.ts: inside Arcadia every call goes to the Rust backend instead).
import type { BenchComparison, BenchMetric, BenchSession, FrameStats, GameChoice, GameProfile, Inventory, LifecycleEvent, ProcessInfo, ProfileRun, ProfileView } from "./fftypes";

type Handler = (args: Record<string, unknown>) => unknown;

interface Deps {
  processes: () => ProcessInfo[];
  presets: () => { id: string; name: string }[];
  actions: () => { id: string; params: unknown; name: string; label: string | null; level: number; state: string }[];
  monitor: () => { pid: number; name: string } | null;
  recording: () => boolean;
  record: (on: boolean) => void;
  recordedMs: () => number;
  stats: (frames: number[]) => FrameStats;
  inventory: () => Inventory;
}

/** Deterministic frame times, so the preview looks the same on every load. */
function frames(seed: number, n: number, base: number, hitchEvery: number): number[] {
  let x = seed;
  const rnd = () => ((x = (x * 1664525 + 1013904223) % 4294967296) / 4294967296);
  return Array.from({ length: n }, (_, i) => (hitchEvery && i % hitchEvery === hitchEvery - 1 ? base * 2.6 : base * (0.92 + rnd() * 0.16)));
}

export function profileMock(d: Deps): Record<string, Handler> {
  const now = () => Math.floor(Date.now() / 1000);
  const inv = d.inventory();
  const display = inv.displays.find((x) => x.primary) ?? inv.displays[0];
  const gpu = inv.gpus[0];

  function session(id: string, label: string, startedAt: number, seed: number, base: number, hitch: number, seconds: number, extra: Partial<BenchSession> = {}): BenchSession {
    const ft = frames(seed, Math.round((seconds * 1000) / base), base, hitch);
    const st = d.stats(ft);
    return {
      schema: 1, id, label, gameId: "steam:1091500", gameName: "Cyberpunk 2077", exe: "Cyberpunk2077.exe", pid: 9120, startedAt, durationMs: st.durationMs, source: "PresentMon service",
      width: display?.width ?? null, height: display?.height ?? null, refreshHz: display?.refreshHz ?? null, cpu: inv.cpu.model, gpu: gpu?.name ?? null, gpuDriver: gpu?.driverVersionVendor ?? gpu?.driverVersion ?? null, os: `${inv.system.osName} ${inv.system.osVersion}`,
      displayed: st, presented: st, cpuBusyMs: base * 0.45, cpuWaitMs: base * 0.55, gpuBusyMs: base * 0.94, gpuWaitMs: base * 0.06, displayLatencyMs: base * 2.4, droppedPct: 0.2, generatedPct: 0,
      cpuUtilPct: 34, cpuTempAvgC: 66, cpuTempMaxC: 74, gpuUtilPct: 96, gpuTempAvgC: 71, gpuTempMaxC: 75, gpuPowerAvgW: 212, vramUsedMaxBytes: 7.4 * 1024 ** 3, ramUsedAvgBytes: 17.2 * 1024 ** 3,
      activeOptimizations: [], profile: null, truncated: false, notes: st.low01pctFps === null ? [`Too few frames for a 0.1% low (${ft.length} captured, 3000 needed).`] : [],
      frameTimesMs: ft.filter((_, i) => i % Math.max(1, Math.floor(ft.length / 600)) === 0).slice(0, 600), ...extra,
    };
  }

  let benches: BenchSession[] = [
    session("preview-b3", "Preview: short test", 1762300000, 9, 12.5, 150, 12, { notes: ["Short capture (12 s). Use at least 30 s for figures you want to compare."] }),
    session("preview-b2", "Preview: after tuning", 1762200000, 5, 12.9, 400, 90, { activeOptimizations: ["High performance power plan"], profile: "Preview: Competitive" }),
    session("preview-b1", "Preview: baseline", 1762100000, 3, 14.2, 180, 90),
  ];

  let profiles: GameProfile[] = [
    { schema: 1, id: "preview-p1", name: "Preview: Competitive", gameId: "steam:1091500", gameName: "Cyberpunk 2077", exe: "Cyberpunk2077.exe", enabled: true, autoApply: false, priority: "AboveNormal", presetId: null, actions: [{ id: "power.plan.high_performance", params: null }], monitor: true, benchmark: false, approvedLevel: 2, notes: "Sample data.", createdAt: 1762000000, updatedAt: 1762000000 },
  ];
  const applied = new Set<string>();
  let events: LifecycleEvent[] = [];

  const view = (p: GameProfile): ProfileView => {
    const presets = d.presets();
    const preset = presets.find((x) => x.id === p.presetId);
    return { ...structuredClone(p), runningPid: d.processes().find((x) => x.name.toLowerCase() === p.exe.toLowerCase())?.pid ?? null, applied: applied.has(p.id), presetName: preset?.name ?? null, presetMissing: !!p.presetId && !preset, benchmarks: benches.filter((b) => b.exe.toLowerCase() === p.exe.toLowerCase()).length };
  };
  const get = (id: unknown) => {
    const p = profiles.find((x) => x.id === id);
    if (!p) throw new Error("That profile does not exist");
    return p;
  };
  const bench = (id: unknown) => {
    const b = benches.find((x) => x.id === id);
    if (!b) throw new Error("That benchmark does not exist");
    return b;
  };

  function validate(p: GameProfile) {
    if (!p.name.trim()) throw new Error("The profile needs a name");
    if (!/^[^\\/:]+\.exe$/i.test(p.exe.trim())) throw new Error("The game executable must be a file name ending in .exe, not a path");
    if (["csrss.exe", "lsass.exe", "explorer.exe", "msmpeng.exe", "vgc.exe", "easyanticheat.exe"].includes(p.exe.trim().toLowerCase())) throw new Error(`${p.exe} cannot have a profile: Windows, driver, security or anti-cheat component`);
    if (p.priority && !["Idle", "BelowNormal", "Normal", "AboveNormal", "High"].includes(p.priority)) throw new Error("Realtime priority is never set: it can starve Windows itself and freeze the PC");
  }

  function compare(ids: string[]): BenchComparison {
    if (ids.length < 2) throw new Error("Choose at least two benchmarks to compare");
    const s = ids.map(bench);
    const defs: [string, string, string, boolean | null, (b: BenchSession) => number | null][] = [
      ["avgFps", "Average FPS", "FPS", true, (b) => b.displayed.avgFps], ["low1", "1% low FPS", "FPS", true, (b) => b.displayed.low1pctFps], ["low01", "0.1% low FPS", "FPS", true, (b) => b.displayed.low01pctFps],
      ["avgFrameTime", "Average frame time", "ms", false, (b) => b.displayed.avgFrameTimeMs], ["p95", "Frame time, 95th percentile", "ms", false, (b) => b.displayed.p95Ms], ["p99", "Frame time, 99th percentile", "ms", false, (b) => b.displayed.p99Ms],
      ["consistency", "Frame-time consistency", "%", true, (b) => b.displayed.consistencyPct], ["cpuBusy", "CPU busy per frame", "ms", false, (b) => b.cpuBusyMs], ["gpuBusy", "GPU busy per frame", "ms", false, (b) => b.gpuBusyMs],
      ["gpuUtil", "GPU utilisation", "%", null, (b) => b.gpuUtilPct], ["gpuTemp", "GPU temperature, average", "°C", null, (b) => b.gpuTempAvgC], ["cpuTemp", "CPU temperature, average", "°C", null, (b) => b.cpuTempAvgC],
    ];
    const metrics: BenchMetric[] = defs.map(([key, name, unit, higherIsBetter, f]) => {
      const values = s.map(f);
      const delta = values.map((v, i) => (i === 0 || v === null || values[0] === null ? null : v - values[0]));
      return { key, name, unit, higherIsBetter, values, delta, deltaPct: delta.map((x) => (x === null || !values[0] ? null : (x / values[0]) * 100)) };
    });
    const flags: BenchComparison["flags"] = [];
    for (const b of s) if (b.durationMs < 30000) flags.push({ severity: "warning", text: `"${b.label}" is a short capture (${Math.round(b.durationMs / 1000)} s). Its figures, the lows above all, are not reliable enough to compare.` });
    if (s.some((b) => JSON.stringify(b.activeOptimizations) !== JSON.stringify(s[0].activeOptimizations))) flags.push({ severity: "info", text: "Different optimisations were active for these runs." });
    flags.push({ severity: "info", text: "Arcadia cannot tell whether the runs covered the same scene. Repeat the same route or built-in benchmark for each run." });
    const comparable = !flags.some((f) => f.severity === "warning");
    const avg = metrics[0];
    const summary = s.slice(1).map((b, i) => {
      const dv = avg.delta[i + 1] ?? 0, p = avg.deltaPct[i + 1] ?? 0;
      return `Average FPS: "${b.label}" measured ${(avg.values[i + 1] ?? 0).toFixed(1)} FPS, ${Math.abs(p) < 3 ? "about the same as" : dv > 0 ? "higher than" : "lower than"} "${s[0].label}" at ${(avg.values[0] ?? 0).toFixed(1)} FPS (${dv >= 0 ? "+" : ""}${dv.toFixed(1)} FPS, ${p >= 0 ? "+" : ""}${p.toFixed(1)}%).`;
    });
    summary.push(comparable ? "These are measurements of two or more runs. One run each does not prove what caused a difference; repeat the runs to confirm it." : "The runs were not taken under matching conditions (see the warnings), so the differences cannot be attributed to any one change.");
    return { ids, labels: s.map((b) => b.label), metrics, flags, summary, comparable };
  }

  let benchLabel = "";
  return {
    ff_profiles: () => profiles.map(view),
    ff_profile_games: (): GameChoice[] => {
      const running = d.processes().filter((p) => p.category === "game");
      return [
        ...running.map((p) => ({ gameId: p.gameId, name: p.gameName ?? p.name, exe: p.name, runningPid: p.pid })),
        { gameId: "steam:730", name: "Counter-Strike 2", exe: "cs2.exe", runningPid: null },
        { gameId: "epic:fortnite", name: "Fortnite", exe: null, runningPid: null },
      ];
    },
    ff_profile_save: (a) => {
      const p = structuredClone(JSON.parse(JSON.stringify(a.profile))) as GameProfile;
      validate(p);
      p.name = p.name.trim();
      p.exe = p.exe.trim();
      p.approvedLevel = Math.min(3, a.level as number);
      p.updatedAt = now();
      if (!p.id) {
        p.id = `preview-p${Date.now()}`;
        p.createdAt = now();
        profiles.push(p);
      } else profiles = profiles.map((x) => (x.id === p.id ? p : x));
      return structuredClone(p);
    },
    ff_profile_delete: (a) => {
      if (applied.has(get(a.id).id)) throw new Error("This profile is applied right now. Revert it first, then delete it.");
      profiles = profiles.filter((x) => x.id !== a.id);
    },
    ff_profile_duplicate: (a) => {
      const p = { ...structuredClone(get(a.id)), id: `preview-p${Date.now()}`, autoApply: false };
      p.name = `${p.name} (copy)`;
      profiles.push(p);
      return structuredClone(p);
    },
    ff_profile_apply: (a): ProfileRun => {
      const p = get(a.id);
      if (applied.has(p.id)) throw new Error("This profile is already applied. Revert it before applying it again.");
      const level = a.level as number;
      const v = view(p);
      const acts = d.actions();
      const done: string[] = [], skipped: string[] = [];
      for (const act of p.actions) {
        const hit = acts.find((x) => x.id === act.id);
        if (!hit) skipped.push(`${act.id}: not available on this PC`);
        else if (hit.level > level) skipped.push(`${hit.label ?? hit.name}: needs the ${["", "Safe", "Advanced", "Expert"][hit.level]} level`);
        else if (hit.state === "available") done.push(hit.label ?? hit.name);
      }
      if (p.priority) {
        if (v.runningPid === null) skipped.push("Priority: the game is not running, so there is no process to change");
        else if (level < 2) skipped.push("Priority: needs the Advanced level");
        else done.push("Change a process's priority while you play");
      }
      if (done.length) applied.add(p.id);
      events.unshift({ at: now(), kind: "applied", profile: p.name, process: p.exe, pid: v.runningPid, text: done.length ? `Applied: ${done.join(", ")}` : "Nothing needed changing" });
      return { profileId: p.id, profileName: p.name, pid: v.runningPid, applied: done, skipped, sessionId: done.length ? "preview" : null, preset: null, monitoring: false };
    },
    ff_profile_revert: (a) => {
      const p = get(a.id);
      if (!applied.delete(p.id)) throw new Error("Arcadia has nothing applied for this profile right now");
      return ["Restored: everything this profile changed"];
    },
    ff_profile_export: () => undefined,
    ff_profile_import: () => {
      const src = profiles[0];
      if (!src) throw new Error("The file is not an Arcadia game profile");
      const p = { ...structuredClone(src), id: `preview-p${Date.now()}`, name: `${src.name} (imported)`, enabled: false, autoApply: false, approvedLevel: 0 };
      profiles.push(p);
      return structuredClone(p);
    },
    ff_profile_events: () => structuredClone(events),

    ff_bench_start: (a) => {
      if (!d.monitor()) throw new Error("Start Live Monitor on the game first: a benchmark records the frames it measures");
      if (d.recording()) throw new Error("A benchmark is already being recorded");
      benchLabel = String(a.label ?? "").trim();
      d.record(true);
    },
    ff_bench_stop: () => {
      if (!d.recording()) throw new Error("No benchmark is being recorded");
      const seconds = Math.max(1, d.recordedMs() / 1000);
      d.record(false);
      const b = session(`preview-b${Date.now()}`, benchLabel, now(), 11, 13.6, 100, seconds, seconds < 30 ? { notes: [`Short capture (${Math.round(seconds)} s). Use at least 30 s for figures you want to compare.`] } : {});
      const m = d.monitor();
      if (m) Object.assign(b, { exe: m.name, pid: m.pid });
      benches.unshift(b);
      return structuredClone(b);
    },
    ff_bench_list: () => structuredClone(benches),
    ff_bench_rename: (a) => {
      const b = bench(a.id);
      b.label = String(a.label).trim();
      return structuredClone(b);
    },
    ff_bench_delete: (a) => {
      bench(a.id);
      benches = benches.filter((x) => x.id !== a.id);
    },
    ff_bench_compare: (a) => compare(a.ids as string[]),
    ff_bench_export: () => undefined,
  };
}
