// How far through a line the singer is, for the karaoke fill.
//
// With word timings, each word fills in its own time. Without them, the
// line fills over its length, but not evenly: vowels are held longer than
// consonants, so they get more weight, and the end of a line is usually
// drawn out, so the last fifth counts extra. The idea and the weights come
// from Riff (github.com/rootscripts/riff, MIT licence, © 2026 rootscripts);
// this is a fresh implementation of it.

const PUNCT_OR_SPACE = /^[\s\p{P}]$/u;
const HELD = /^[\p{sc=Han}\p{sc=Hiragana}\p{sc=Katakana}\p{sc=Hangul}\p{M}]$/u;
const VOWELS = new Set("aeiouyæøœıаеёиоуыэюяєіїў");

function weight(c: string): number {
  if (PUNCT_OR_SPACE.test(c)) return 0.4;
  if (HELD.test(c)) return 3;
  const base = c.normalize("NFD")[0]?.toLowerCase() ?? c;
  return VOWELS.has(base) || VOWELS.has(c.toLowerCase()) ? 3 : 1;
}

const cache = new Map<string, Float64Array>();

/** Running total of character weights, 0 first, total last. */
function weights(text: string): Float64Array {
  let w = cache.get(text);
  if (w) return w;
  const chars = Array.from(text);
  const outro = Math.floor(chars.length * 0.8);
  w = new Float64Array(chars.length + 1);
  for (let i = 0; i < chars.length; i++) w[i + 1] = w[i] + weight(chars[i]) * (i >= outro ? 1.6 : 1);
  if (cache.size > 400) cache.clear();
  cache.set(text, w);
  return w;
}

/** Share of the line's characters sung, 0–1, given the share of its time
 *  that has passed. */
export function sweep(text: string, timeShare: number): number {
  if (timeShare <= 0) return 0;
  if (timeShare >= 1) return 1;
  const w = weights(text);
  const n = w.length - 1;
  if (n <= 0 || w[n] <= 0) return timeShare;
  const target = timeShare * w[n];
  let lo = 0;
  let hi = n - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (w[mid] <= target) lo = mid;
    else hi = mid - 1;
  }
  const span = w[lo + 1] - w[lo];
  return Math.min(1, (lo + (span > 0 ? (target - w[lo]) / span : 0)) / n);
}

/** Backing vocals: a trailing "(…)" is shown smaller, Riff-style. */
export function splitBacking(text: string): [string, string] {
  const m = /^(.*?\S)\s*(\([^()]*\))\s*$/.exec(text);
  return m ? [m[1], m[2]] : [text, ""];
}

/** Easing curves for line moves. */
export const EASES = {
  smooth: "cubic-bezier(0.786, 0.648, 0, 0.935)",
  snappy: "cubic-bezier(0.16, 1, 0.3, 1)",
  gentle: "cubic-bezier(0.37, 0, 0.63, 1)",
  springy: "cubic-bezier(0.34, 1.36, 0.5, 1)",
  linear: "linear",
} as const;
export type Ease = keyof typeof EASES;
