// Browser-only preview data for the Optimizer page (see ffmock.ts: none of
// this exists inside Arcadia, where every call goes to the Rust backend).
import type { ActionView, ApplyRequest, Diagnosis, Finding, OptChange, OptSession } from "./fftypes";

const base = { reversible: true, temporary: false, hardware: "Any", os: "Windows 10 and 11", params: null, label: null, applicable: true, reason: null, sessionId: null, changeId: null } as const;

let actions: ActionView[] = [
  { ...base, id: "power.plan.high_performance", key: "power.plan.high_performance", name: "Use the High performance power plan", description: "Switches Windows to its built-in High performance plan.", category: "power", risk: "low", level: 1, restart: "none", tradeOff: "Higher idle power use and heat.", source: "PowerSetActiveScheme (documented power-management API).", current: "Balanced", proposed: "High performance", state: "available" },
  { ...base, id: "windows.game_mode", key: "windows.game_mode", name: "Turn on Game Mode", description: "Windows gives the game in focus priority and holds back driver installs and restart prompts while you play.", category: "windows", risk: "low", level: 1, restart: "game", tradeOff: "None known for games.", source: "Settings > Gaming > Game Mode (per-user value AutoGameModeEnabled).", current: "On", proposed: "On", state: "alreadySet" },
  { ...base, id: "windows.background_recording_off", key: "windows.background_recording_off", name: "Turn off background recording", description: "Stops Windows recording the last moments of gameplay all the time.", category: "windows", risk: "low", level: 1, restart: "game", tradeOff: "\"Record what happened\" (Win+Alt+G) stops working until you turn it back on.", source: "Settings > Gaming > Captures (per-user value HistoricalCaptureEnabled).", current: "On", proposed: "Off", state: "available" },
  { ...base, id: "power.plan.gaming", key: "power.plan.gaming", name: "Create and use the Arcadia Gaming power plan", description: "Copies High performance into a separate plan that the settings below adjust, leaving the built-in plans untouched.", category: "power", risk: "low", level: 2, restart: "none", tradeOff: "Higher idle power use and heat.", source: "PowerDuplicateScheme and PowerSetActiveScheme.", current: "Balanced", proposed: "Arcadia Gaming", state: "available" },
  { ...base, id: "startup.disable", key: "startup.disable:discord", name: "Stop an app from starting with Windows", label: "Discord", params: { name: "Discord" }, description: "Removes the app's entry from your account's start-up list.", category: "startup", risk: "low", level: 2, restart: "signOut", tradeOff: "The app no longer opens by itself after sign-in.", source: "The documented per-user Run key.", current: "Starts with Windows", proposed: "Does not start with Windows", state: "available" },
  { ...base, id: "graphics.gpu_preference", key: "graphics.gpu_preference", name: "Run a game on the high-performance GPU", description: "Sets the Windows per-app graphics preference.", category: "graphics", risk: "low", level: 2, restart: "game", tradeOff: "More power use on laptops.", source: "Settings > Display > Graphics.", current: null, proposed: null, state: "notApplicable", applicable: false, reason: "This PC has one graphics adapter, so there is nothing to choose between." },
];
let sessions: OptSession[] = [];

const findings: Finding[] = [
  { id: "limit.gpu", title: "The game is limited by the graphics card (GPU-bound)", severity: "info", confidence: "high", evidence: ["GPU busy for 96% of each frame (13.10 of 13.60 ms)", "GPU utilisation averaged 96%", "Average 73.5 FPS"], why: "Each frame waits on the graphics card, so the frame rate follows how much work the card is given.", recommendation: "For more FPS, lower GPU-heavy settings in the game or turn on an upscaler. Processor tweaks and process priority will not help here.", tradeOff: "Lower image quality.", risk: "low", restart: "none", reversible: true, actions: [], settingsUri: null },
  { id: "display.refresh", title: "The display is running below its highest refresh rate", severity: "notice", confidence: "high", evidence: ["Preview 27\" monitor is at 60 Hz", "165 Hz is available at this resolution"], why: "The display cannot show more frames per second than its refresh rate.", recommendation: "Choose the higher refresh rate in Windows display settings.", tradeOff: "None.", risk: "low", restart: "none", reversible: true, actions: [], settingsUri: "ms-settings:display-advanced" },
  { id: "windows.background_recording", title: "Background recording is on", severity: "notice", confidence: "medium", evidence: ["Windows is recording gameplay in the background"], why: "The encoder shares the graphics card with the game.", recommendation: "Turn it off unless you use \"Record what happened\".", tradeOff: "Instant replay stops working.", risk: "low", restart: "game", reversible: true, actions: ["windows.background_recording_off"], settingsUri: "ms-settings:gaming-gamedvr" },
];

function diagnose(): Diagnosis {
  return { generatedAt: Math.floor(Date.now() / 1000), findings, report: "SYSTEM ANALYSIS\n\n(browser preview)\n", frames: null, basis: "Browser preview: these findings are sample data, not measurements." };
}

function apply(args: Record<string, unknown>): OptSession {
  const requests = args.requests as ApplyRequest[];
  const now = Math.floor(Date.now() / 1000);
  const id = `${now}-preview${sessions.length}`;
  const changes: OptChange[] = requests.map((r, i) => {
    const key = r.id.startsWith("process.") ? `${r.id}:${(r.params as { pid: number }).pid}` : (actions.find((a) => a.id === r.id && JSON.stringify(a.params) === JSON.stringify(r.params ?? null))?.key ?? r.id);
    const a = actions.find((x) => x.key === key);
    if (a) Object.assign(a, { state: "applied", sessionId: id, changeId: i + 1, current: a.proposed });
    return { id: i + 1, actionId: r.id, key, name: a?.name ?? r.id, category: a?.category ?? "process", risk: a?.risk ?? "low", level: a?.level ?? 1, restart: a?.restart ?? "none", reversible: a?.reversible ?? false, temporary: !a, params: r.params, beforeText: a ? "(preview) before" : "running", proposedText: a?.proposed ?? "asked to close", afterText: a?.proposed ?? "asked to close", status: "applied", error: null, appliedAt: now, revertedAt: null };
  });
  const s: OptSession = { schema: 1, id, label: args.label as string, startedAt: now, level: args.level as number, status: "applied", error: null, changes };
  sessions.unshift(s);
  return structuredClone(s);
}

function revert(sessionId: string, changeId: number | null): OptSession {
  const s = sessions.find((x) => x.id === sessionId);
  if (!s) throw new Error("That optimisation session no longer exists");
  for (const c of s.changes) {
    if ((changeId === null || c.id === changeId) && c.status === "applied" && c.reversible) {
      c.status = "reverted";
      c.revertedAt = Math.floor(Date.now() / 1000);
      const a = actions.find((x) => x.key === c.key);
      if (a) Object.assign(a, { state: "available", sessionId: null, changeId: null });
    }
  }
  s.status = s.changes.some((c) => c.status === "applied") ? "partlyReverted" : "reverted";
  return structuredClone(s);
}

export const optMock: Record<string, (args: Record<string, unknown>) => unknown> = {
  ff_diagnose: diagnose,
  ff_opt_list: () => structuredClone(actions),
  ff_opt_apply: apply,
  ff_opt_revert_change: (a) => revert(a.sessionId as string, a.changeId as number),
  ff_opt_revert_session: (a) => revert(a.sessionId as string, null),
  ff_opt_history: () => structuredClone(sessions),
  ff_open_settings: () => undefined,
};
