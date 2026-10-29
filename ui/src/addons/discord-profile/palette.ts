// Suggested profile colours, worked out from the banner, the avatar, or both.
//
// Discord only suggests colours from the avatar, and picks them from raw
// pixel averages. This does better in three ways:
//   * colours are clustered in OKLab, where distance matches what eyes see,
//     so a small bright detail isn't drowned out by a big dull background;
//   * only what Discord actually shows is sampled: the avatar's circle, not
//     its hidden corners, and only the banner's opaque pixels;
//   * the gradient pair is tuned for legibility (a deep top colour, a lively
//     bottom one) and its text contrast is reported.

export interface Swatch {
  hex: string;
  /** Share of the sampled pixels, 0–1. */
  weight: number;
}

export type PaletteSource = "banner" | "avatar" | "both";

export interface Suggestion {
  id: PaletteSource;
  title: string;
  swatches: Swatch[];
  primary: string;
  accent: string;
  bannerColor: string;
  /** WCAG contrast of white text on the darker end of the gradient. */
  contrast: number;
  /** Set when this palette can't be made (e.g. no banner yet). */
  missing: string | null;
}

type Lab = [number, number, number];

// ── colour maths ──────────────────────────────────────────────

const toLin = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const toSrgb = (c: number) => (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055);

export function rgbToLab(r: number, g: number, b: number): Lab {
  const R = toLin(r / 255), G = toLin(g / 255), B = toLin(b / 255);
  const l = Math.cbrt(0.4122214708 * R + 0.5363325363 * G + 0.0514459929 * B);
  const m = Math.cbrt(0.2119034982 * R + 0.6806995451 * G + 0.1073969566 * B);
  const s = Math.cbrt(0.0883024619 * R + 0.2817188376 * G + 0.6299787005 * B);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}

function labToLinear([L, a, b]: Lab): [number, number, number] {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
}

const inGamut = (rgb: number[]) => rgb.every((c) => c >= -0.0005 && c <= 1.0005);

/** OKLab to hex, pulling chroma in until the colour fits sRGB. */
export function labToHex(lab: Lab): string {
  let [L, a, b] = lab;
  L = Math.min(1, Math.max(0, L));
  let lin = labToLinear([L, a, b]);
  if (!inGamut(lin)) {
    let lo = 0, hi = 1;
    for (let i = 0; i < 18; i++) {
      const mid = (lo + hi) / 2;
      if (inGamut(labToLinear([L, a * mid, b * mid]))) lo = mid;
      else hi = mid;
    }
    lin = labToLinear([L, a * lo, b * lo]);
  }
  const hex = lin.map((c) => Math.round(Math.min(1, Math.max(0, toSrgb(Math.max(0, c)))) * 255).toString(16).padStart(2, "0"));
  return `#${hex.join("")}`;
}

export function hexToLab(hex: string): Lab {
  const n = parseInt(hex.slice(1), 16);
  return rgbToLab((n >> 16) & 255, (n >> 8) & 255, n & 255);
}

const dist = (p: Lab, q: Lab) => Math.hypot(p[0] - q[0], p[1] - q[1], p[2] - q[2]);
const chroma = (p: Lab) => Math.hypot(p[1], p[2]);
const hueOf = (p: Lab) => Math.atan2(p[2], p[1]);

/** Same hue, with lightness and chroma clamped into a range. */
function shape(p: Lab, Lmin: number, Lmax: number, Cmax: number, Cmin = 0): Lab {
  const L = Math.min(Lmax, Math.max(Lmin, p[0]));
  const c = chroma(p);
  const C = Math.min(Cmax, Math.max(Cmin, c));
  const h = c < 1e-4 ? 0 : hueOf(p);
  return [L, C * Math.cos(h), C * Math.sin(h)];
}

function luminance(hex: string): number {
  const n = parseInt(hex.slice(1), 16);
  const ch = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => toLin(v / 255));
  return 0.2126 * ch[0] + 0.7152 * ch[1] + 0.0722 * ch[2];
}

export function contrast(a: string, b: string): number {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}

/** Black or white, whichever reads better on `bg`. */
export function inkOn(bg: string): string {
  return contrast("#ffffff", bg) >= contrast("#000000", bg) ? "#ffffff" : "#000000";
}

// ── sampling ──────────────────────────────────────────────────

async function decode(src: string): Promise<HTMLImageElement> {
  return new Promise((res, rej) => {
    const i = new Image();
    i.onload = () => res(i);
    i.onerror = () => rej(new Error("image"));
    i.src = src;
  });
}

/** Pixels Discord actually shows, in OKLab. */
export async function samplePixels(src: string, opts: { circle: boolean }): Promise<Lab[]> {
  const img = await decode(src);
  const max = 96;
  const k = Math.min(1, max / Math.max(img.naturalWidth, img.naturalHeight, 1));
  const w = Math.max(1, Math.round(img.naturalWidth * k));
  const h = Math.max(1, Math.round(img.naturalHeight * k));
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  const g = c.getContext("2d", { willReadFrequently: true })!;
  g.drawImage(img, 0, 0, w, h);
  const d = g.getImageData(0, 0, w, h).data;
  const out: Lab[] = [];
  const cx = w / 2, cy = h / 2, r2 = (Math.min(w, h) / 2) ** 2;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (opts.circle && (x + 0.5 - cx) ** 2 + (y + 0.5 - cy) ** 2 > r2) continue;
      const i = (y * w + x) * 4;
      if (d[i + 3] < 200) continue;
      out.push(rgbToLab(d[i], d[i + 1], d[i + 2]));
    }
  }
  return out;
}

// ── clustering ────────────────────────────────────────────────

function rng(seed: number) {
  return () => {
    seed |= 0;
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

interface Cluster {
  c: Lab;
  weight: number;
}

/** k-means++ in OKLab, seeded so the same image always gives the same palette. */
export function cluster(points: Lab[], k = 7): Cluster[] {
  if (!points.length) return [];
  const rand = rng(0xa11ce);
  const centers: Lab[] = [points[Math.floor(rand() * points.length)]];
  const d2 = new Float64Array(points.length).fill(Infinity);
  while (centers.length < Math.min(k, points.length)) {
    const last = centers[centers.length - 1];
    let total = 0;
    for (let i = 0; i < points.length; i++) {
      d2[i] = Math.min(d2[i], dist(points[i], last) ** 2);
      total += d2[i];
    }
    if (total === 0) break;
    let pick = rand() * total;
    let idx = 0;
    for (; idx < points.length - 1 && pick > d2[idx]; idx++) pick -= d2[idx];
    centers.push(points[idx]);
  }
  const assign = new Int32Array(points.length);
  for (let iter = 0; iter < 12; iter++) {
    let moved = 0;
    for (let i = 0; i < points.length; i++) {
      let best = 0, bd = Infinity;
      for (let j = 0; j < centers.length; j++) {
        const dd = dist(points[i], centers[j]);
        if (dd < bd) { bd = dd; best = j; }
      }
      if (assign[i] !== best) moved++;
      assign[i] = best;
    }
    const sums = centers.map(() => [0, 0, 0, 0]);
    for (let i = 0; i < points.length; i++) {
      const s = sums[assign[i]];
      s[0] += points[i][0]; s[1] += points[i][1]; s[2] += points[i][2]; s[3]++;
    }
    sums.forEach((s, j) => { if (s[3]) centers[j] = [s[0] / s[3], s[1] / s[3], s[2] / s[3]]; });
    if (iter > 0 && moved === 0) break;
  }
  const counts = new Array(centers.length).fill(0);
  for (let i = 0; i < points.length; i++) counts[assign[i]]++;
  return centers.map((c, j) => ({ c, weight: counts[j] / points.length })).filter((x) => x.weight > 0).sort((a, b) => b.weight - a.weight);
}

/** Merge near-identical clusters, drop specks, keep up to five. */
function distinct(cs: Cluster[], max = 5): Cluster[] {
  const out: Cluster[] = [];
  for (const c of cs) {
    const near = out.find((o) => dist(o.c, c.c) < 0.065);
    if (near) near.weight += c.weight;
    else if (c.weight >= 0.015) out.push({ ...c });
  }
  return out.sort((a, b) => b.weight - a.weight).slice(0, max);
}

// ── suggestions ───────────────────────────────────────────────

/** Turn clusters into a top/bottom gradient pair. */
function pair(base: Cluster[], highlight: Cluster[]) {
  const dominant = base[0].c;
  const primary = shape(dominant, 0.27, 0.42, 0.13);
  // The bottom colour wants to be lively and clearly different from the top.
  const pool = highlight.length ? highlight : base;
  const vivid = Math.max(...pool.map((c) => chroma(c.c)));
  let accent: Lab;
  if (vivid < 0.035) {
    // A grey picture: a lighter step of the same tone reads as intentional.
    accent = shape(dominant, 0.6, 0.66, 0.04);
  } else {
    const scored = pool
      .map((c) => ({ c: c.c, s: chroma(c.c) * 1.6 + Math.sqrt(c.weight) * 0.35 - (dist(c.c, primary) < 0.14 ? 0.3 : 0) }))
      .sort((a, b) => b.s - a.s);
    accent = shape(scored[0].c, 0.56, 0.74, 0.19, Math.min(0.07, chroma(scored[0].c)));
  }
  const ph = labToHex(primary);
  const ah = labToHex(accent);
  return { primary: ph, accent: ah, bannerColor: labToHex(dominant), contrast: contrast("#ffffff", ph) };
}

const toSwatches = (cs: Cluster[]) => cs.map((c) => ({ hex: labToHex(c.c), weight: c.weight }));

const empty = (id: PaletteSource, title: string, missing: string): Suggestion => ({
  id, title, swatches: [], primary: "#000000", accent: "#000000", bannerColor: "#000000", contrast: 1, missing,
});

/** All three palettes, in order: banner, avatar, both. */
export async function suggest(avatar: string | null, banner: string | null): Promise<Suggestion[]> {
  const [ap, bp] = await Promise.all([
    avatar ? samplePixels(avatar, { circle: true }).catch(() => []) : Promise.resolve([] as Lab[]),
    banner ? samplePixels(banner, { circle: false }).catch(() => []) : Promise.resolve([] as Lab[]),
  ]);
  const ac = ap.length ? distinct(cluster(ap)) : [];
  const bc = bp.length ? distinct(cluster(bp)) : [];
  const out: Suggestion[] = [];

  out.push(bc.length ? { id: "banner", title: "From banner", swatches: toSwatches(bc), ...pair(bc, bc), missing: null } : empty("banner", "From banner", "Add a banner to get this one"));
  out.push(ac.length ? { id: "avatar", title: "From avatar", swatches: toSwatches(ac), ...pair(ac, ac), missing: null } : empty("avatar", "From avatar", "Add an avatar to get this one"));

  if (ac.length && bc.length) {
    // Balance the two so a huge banner doesn't outvote the avatar.
    const both = distinct(
      [...ac.map((c) => ({ ...c, weight: c.weight / 2 })), ...bc.map((c) => ({ ...c, weight: c.weight / 2 }))].sort((a, b) => b.weight - a.weight),
    );
    // Top from the banner (the gradient sits under it), bottom from the avatar.
    out.push({ id: "both", title: "From both", swatches: toSwatches(both), ...pair(bc, ac), missing: null });
  } else {
    out.push(empty("both", "From both", "Needs both an avatar and a banner"));
  }
  return out;
}
