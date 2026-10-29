// Interface sounds, made on the fly with Web Audio (no sound files).
import type { FeelPrefs } from "./theme";

export type Cue = "click" | "on" | "off" | "ok" | "error" | "launch";

let ctx: AudioContext | null = null;
function audio(): AudioContext | null {
  if (typeof window === "undefined" || !("AudioContext" in window)) return null;
  ctx ??= new AudioContext();
  if (ctx.state === "suspended") void ctx.resume();
  return ctx;
}

function tone(a: AudioContext, at: number, freq: number, len: number, vol: number, type: OscillatorType, slideTo?: number) {
  const o = a.createOscillator();
  const g = a.createGain();
  o.type = type;
  o.frequency.setValueAtTime(freq, at);
  if (slideTo) o.frequency.exponentialRampToValueAtTime(slideTo, at + len);
  g.gain.setValueAtTime(0.0001, at);
  g.gain.exponentialRampToValueAtTime(vol, at + 0.006);
  g.gain.exponentialRampToValueAtTime(0.0001, at + len);
  o.connect(g).connect(a.destination);
  o.start(at);
  o.stop(at + len + 0.02);
}

function noise(a: AudioContext, at: number, len: number, vol: number, hp: number) {
  const n = Math.max(1, Math.floor(a.sampleRate * len));
  const buf = a.createBuffer(1, n, a.sampleRate);
  const d = buf.getChannelData(0);
  for (let i = 0; i < n; i++) d[i] = (Math.random() * 2 - 1) * (1 - i / n) ** 3;
  const src = a.createBufferSource();
  src.buffer = buf;
  const f = a.createBiquadFilter();
  f.type = "highpass";
  f.frequency.value = hp;
  const g = a.createGain();
  g.gain.value = vol;
  src.connect(f).connect(g).connect(a.destination);
  src.start(at);
}

/** Notes for each cue, per sound set: [frequency, start offset s, length s]. */
const SETS: Record<Exclude<FeelPrefs["sounds"], "off">, { wave: OscillatorType; gain: number; cues: Record<Cue, [number, number, number, number?][]> }> = {
  soft: {
    wave: "sine",
    gain: 0.14,
    cues: {
      click: [[880, 0, 0.05]],
      on: [[740, 0, 0.06], [990, 0.05, 0.08]],
      off: [[740, 0, 0.06], [560, 0.05, 0.08]],
      ok: [[660, 0, 0.09], [880, 0.08, 0.12]],
      error: [[440, 0, 0.12], [330, 0.1, 0.16]],
      launch: [[440, 0, 0.32, 1320]],
    },
  },
  clicky: {
    wave: "triangle",
    gain: 0.12,
    cues: {
      click: [],
      on: [[1200, 0, 0.03]],
      off: [[800, 0, 0.03]],
      ok: [[1046, 0, 0.05], [1568, 0.06, 0.06]],
      error: [[300, 0, 0.08], [220, 0.08, 0.1]],
      launch: [[600, 0, 0.18, 1800]],
    },
  },
  retro: {
    wave: "square",
    gain: 0.05,
    cues: {
      click: [[1046, 0, 0.03]],
      on: [[784, 0, 0.05], [1046, 0.05, 0.06]],
      off: [[1046, 0, 0.05], [784, 0.05, 0.06]],
      ok: [[523, 0, 0.06], [659, 0.06, 0.06], [784, 0.12, 0.1]],
      error: [[220, 0, 0.1], [165, 0.1, 0.16]],
      launch: [[262, 0, 0.06], [392, 0.06, 0.06], [523, 0.12, 0.06], [1046, 0.18, 0.16]],
    },
  },
};

export function play(cue: Cue, f: FeelPrefs) {
  if (f.sounds === "off" || f.volume <= 0) return;
  const a = audio();
  if (!a) return;
  const set = SETS[f.sounds];
  const now = a.currentTime + 0.005;
  if (f.sounds === "clicky" && cue === "click") {
    noise(a, now, 0.025, 0.5 * f.volume, 2400);
    return;
  }
  for (const [freq, at, len, slide] of set.cues[cue]) tone(a, now + at, freq, len, set.gain * f.volume, set.wave, slide);
}
