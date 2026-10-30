// The two-colour gradient engine: Discord's Nitro profile colours, for
// Discord previews and for Arcadia's own look.
//
// Two colours go in (top and bottom) and come out as one of three styles:
//   faithful  Discord's own recipe: a straight top-to-bottom gradient with a
//             see-through white or black layer for panels, text picked by
//             brightness. Used for Discord cards, always.
//   glow      The same colours lit: soft pools of light, frosted panels,
//             the bottom colour as the button colour.
//   tonal     A calm palette worked out from the two hues in OKLCH. The
//             gradient stays on the edges; panels are tinted solids, so long
//             pages stay readable. Arcadia's default for its own look.
//
// Colours can come from a screenshot (pullFromImage), pasted hex codes
// (parsePair), a banner and avatar (Profile Studio's palette.ts) or a
// Discord accent colour (fromAccent).

export type GradientStyle = "faithful" | "glow" | "tonal";
export interface Gradient {
  top: string;
  bottom: string;
  style: GradientStyle;
}

export const STYLE_INFO: Record<GradientStyle, { name: string; blurb: string }> = {
  tonal: { name: "Tonal", blurb: "A calm palette from the two colours; the gradient stays on the edges. Easiest to read." },
  faithful: { name: "Faithful", blurb: "Discord's own recipe: the full gradient behind see-through panels." },
  glow: { name: "Glow", blurb: "Soft pools of light and frosted panels. The most eye-catching." },
};

// ── colour maths ──────────────────────────────────────────────

type Rgb = [number, number, number];
const toLin = (c: number) => ((c /= 255) <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const toSrgb = (c: number) => 255 * (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055);

export const isHex = (s: unknown): s is string => typeof s === "string" && /^#[0-9a-f]{6}$/i.test(s);

export function hexToRgb(hex: string): Rgb {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}
export function rgbToHex(c: number[]): string {
  return "#" + c.slice(0, 3).map((v) => Math.round(Math.min(255, Math.max(0, v))).toString(16).padStart(2, "0")).join("");
}

export function luminance(hex: string): number {
  const [r, g, b] = hexToRgb(hex).map(toLin);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
export function contrast(a: string, b: string): number {
  const x = luminance(a), y = luminance(b);
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}
export const mix = (a: string, b: string, t: number) => {
  const x = hexToRgb(a), y = hexToRgb(b);
  return rgbToHex(x.map((v, i) => v + (y[i] - v) * t));
};
export const rgba = (hex: string, a: number) => {
  const [r, g, b] = hexToRgb(hex);
  return `rgba(${r}, ${g}, ${b}, ${a})`;
};

/** [lightness 0–1, chroma, hue degrees] */
export function toOklch(hex: string): [number, number, number] {
  const [r, g, b] = hexToRgb(hex).map(toLin);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  const L = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s;
  const A = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
  const B = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  return [L, Math.hypot(A, B), ((Math.atan2(B, A) * 180) / Math.PI + 360) % 360];
}
function oklchToLinear(L: number, C: number, H: number): Rgb {
  const a = C * Math.cos((H * Math.PI) / 180), b = C * Math.sin((H * Math.PI) / 180);
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
}
/** OKLCH to hex, pulling chroma in until it fits sRGB. */
export function fromOklch(L: number, C: number, H: number): string {
  L = Math.min(1, Math.max(0, L));
  const fits = (c: Rgb) => c.every((v) => v >= -0.0005 && v <= 1.0005);
  let lin = oklchToLinear(L, C, H);
  if (!fits(lin)) {
    let lo = 0, hi = C;
    for (let i = 0; i < 16; i++) {
      const mid = (lo + hi) / 2;
      if (fits(oklchToLinear(L, mid, H))) lo = mid;
      else hi = mid;
    }
    lin = oklchToLinear(L, lo, H);
  }
  return rgbToHex(lin.map((v) => toSrgb(Math.min(1, Math.max(0, v)))));
}

/** Hue halfway between two, the short way round. */
function midHue(a: number, b: number): number {
  const d = ((b - a + 540) % 360) - 180;
  return (a + d / 2 + 360) % 360;
}

// ── light or dark ─────────────────────────────────────────────

/** Discord picks a light or dark card from the top colour: light colours get
 *  a white layer and dark text, dark ones a black layer and light text. */
export function schemeFor(top: string): "light" | "dark" {
  return contrast(top, "#000000") > contrast(top, "#ffffff") ? "light" : "dark";
}

/** Light or dark for a whole pair: the average of both. */
export function pairScheme(top: string, bottom: string): "light" | "dark" {
  return (luminance(top) + luminance(bottom)) / 2 > 0.36 ? "light" : "dark";
}

// ── the engine ────────────────────────────────────────────────

/** Arcadia's theme tokens for a gradient, plus the window's fill. */
export interface GradientTheme {
  scheme: "light" | "dark";
  /** What the main window is painted with. */
  fill: string;
  vars: Record<string, string>;
}

export function buildTheme(g: Gradient): GradientTheme {
  const { top, bottom } = g;
  const scheme = g.style === "tonal" ? pairScheme(top, bottom) : schemeFor(top);
  const light = scheme === "light";
  const [L1, C1, H1] = toOklch(top);
  const [L2, C2, H2] = toOklch(bottom);
  const Hm = midHue(H1, H2);

  // The accent: the bottom colour, pushed to a strength buttons can carry.
  const accent = fromOklch(light ? 0.56 : 0.76, Math.max(Math.min(C2, 0.2), 0.1), H2);
  const accentInk = contrast(accent, "#ffffff") >= contrast(accent, "#111111") ? "#ffffff" : "#111111";
  const ok = fromOklch(light ? 0.55 : 0.8, 0.13, 155);
  const bad = fromOklch(light ? 0.58 : 0.76, 0.17, 18);
  const info = fromOklch(light ? 0.52 : 0.78, 0.12, (H1 + 60) % 360);

  if (g.style === "tonal") {
    const base = fromOklch(light ? 0.975 : 0.17, Math.min(0.025, C1 * 0.25), H1);
    const raised = fromOklch(light ? 0.995 : 0.215, Math.min(0.03, C1 * 0.3), Hm);
    const sunk = fromOklch(light ? 0.94 : 0.145, Math.min(0.035, Hm ? C2 * 0.3 : 0), Hm);
    const line = fromOklch(light ? 0.87 : 0.3, Math.min(0.045, C2 * 0.35), H2);
    return {
      scheme,
      fill: `linear-gradient(180deg, ${mix(base, top, light ? 0.55 : 0.32)} 0, ${base} 300px, ${base} calc(100% - 240px), ${mix(base, bottom, light ? 0.5 : 0.3)} 100%)`,
      vars: {
        "--panel": base, "--ink": base, "--raised": raised, "--sunk": sunk, "--rail": "transparent",
        "--line": line, "--line-soft": fromOklch(light ? 0.92 : 0.25, Math.min(0.035, C2 * 0.3), H2),
        "--text": fromOklch(light ? 0.24 : 0.95, Math.min(0.03, C2 * 0.2), H2),
        "--muted": fromOklch(light ? 0.46 : 0.74, Math.min(0.04, C2 * 0.3), H2),
        "--faint": fromOklch(light ? 0.62 : 0.56, Math.min(0.035, C2 * 0.25), H2),
        "--peach": accent, "--peach-ink": accentInk, "--mint": ok, "--rose": bad, "--lilac": info,
        "--group": light ? "rgba(255,255,255,0.55)" : rgba(raised, 0.6),
        "--desk": mix(top, bottom, 0.5),
      },
    };
  }

  const ink = light ? "#2e3035" : "#f2f3f5";
  const muted = light ? "#55585f" : "#b5bac1";
  const faint = light ? "#7b7e86" : "#8e939b";
  const overlay = (a: number) => (light ? `rgba(255,255,255,${a})` : `rgba(0,0,0,${a})`);
  const hair = (a: number) => (light ? `rgba(0,0,0,${a})` : `rgba(255,255,255,${a})`);

  if (g.style === "faithful") {
    return {
      scheme,
      fill: `linear-gradient(180deg, ${top}, ${bottom})`,
      vars: {
        "--panel": overlay(0.5), "--ink": light ? "#ffffff" : "#111214", "--raised": overlay(0.45), "--sunk": overlay(0.3), "--rail": overlay(0.18),
        "--line": hair(0.12), "--line-soft": hair(0.07), "--text": ink, "--muted": muted, "--faint": faint,
        "--peach": accent, "--peach-ink": accentInk, "--mint": ok, "--rose": bad, "--lilac": info,
        "--group": overlay(0.35), "--desk": top,
      },
    };
  }

  // Glow
  const poolA = fromOklch(Math.min(0.97, L1 + 0.06), Math.min(0.25, C1 * 1.25), H1);
  const poolB = fromOklch(Math.min(0.95, L2 + 0.04), Math.min(0.25, C2 * 1.3), H2);
  return {
    scheme,
    fill: `radial-gradient(120% 70% at 12% 0%, ${poolA} 0%, transparent 60%), radial-gradient(110% 80% at 100% 100%, ${poolB} 0%, transparent 65%), linear-gradient(160deg, ${top}, ${bottom})`,
    vars: {
      "--panel": overlay(0.3), "--ink": light ? "#ffffff" : "#111214",
      "--raised": light ? "rgba(255,255,255,0.46)" : "rgba(20,20,28,0.42)", "--sunk": light ? "rgba(255,255,255,0.32)" : "rgba(10,10,16,0.32)", "--rail": overlay(0.12),
      "--line": light ? "rgba(255,255,255,0.75)" : "rgba(255,255,255,0.16)", "--line-soft": light ? "rgba(255,255,255,0.5)" : "rgba(255,255,255,0.09)",
      "--text": ink, "--muted": muted, "--faint": faint,
      "--peach": accent, "--peach-ink": accentInk, "--mint": ok, "--rose": bad, "--lilac": info,
      "--group": light ? "rgba(255,255,255,0.38)" : "rgba(20,20,28,0.32)", "--desk": mix(top, bottom, 0.5),
      "--glass-blur": "16px",
    },
  };
}

/** WCAG contrast of body text on panels: what the "readable" pill shows. */
export function readability(g: Gradient): number {
  const t = buildTheme(g);
  const text = t.vars["--text"];
  // Translucent panels: judge against the colour the panel sits over.
  const under = g.style === "tonal" ? t.vars["--panel"] : mix(mix(g.top, g.bottom, 0.5), t.scheme === "light" ? "#ffffff" : "#000000", g.style === "faithful" ? 0.5 : 0.3);
  return contrast(text, under);
}

// ── pulling colours in ────────────────────────────────────────

/** Two hex codes from pasted text: "#f7cfcf, #f19ba7", "f7cfcf f19ba7",
 *  CSS like linear-gradient(#a, #b), or Discord's decimal colours. */
export function parsePair(text: string): [string, string] | null {
  const hex = [...text.matchAll(/#?\b([0-9a-f]{6}|[0-9a-f]{3})\b/gi)].map((m) => {
    const h = m[1].length === 3 ? [...m[1]].map((c) => c + c).join("") : m[1];
    return `#${h.toLowerCase()}`;
  });
  if (hex.length >= 2) return [hex[0], hex[1]];
  // Discord stores colours as decimal integers (e.g. 16242639).
  const dec = [...text.matchAll(/\b(\d{4,8})\b/g)].map((m) => +m[1]).filter((n) => n <= 0xffffff);
  if (dec.length >= 2) return [rgbToHex([(dec[0] >> 16) & 255, (dec[0] >> 8) & 255, dec[0] & 255]), rgbToHex([(dec[1] >> 16) & 255, (dec[1] >> 8) & 255, dec[1] & 255])];
  return null;
}

/** A pair from one accent colour (all bots and Lanyard can see): a deep top
 *  in the accent's hue and the accent itself, lifted, at the bottom. */
export function fromAccent(accent: string): [string, string] {
  const [L, C, H] = toOklch(accent);
  return [fromOklch(Math.min(0.42, Math.max(0.24, L * 0.55)), Math.min(0.12, C), H), fromOklch(Math.min(0.78, Math.max(0.58, L)), Math.max(0.08, Math.min(0.2, C)), H)];
}

/** Read the profile colours off a screenshot of a Discord profile.
 *
 *  Samples a thin strip down both edges (the card's border shows the raw
 *  gradient best), between the banner and the bottom, taking medians so
 *  avatars and text don't skew it. Discord draws its see-through layer
 *  over the gradient inside the card; with `unblend`, that layer is taken
 *  back off to recover the colours as you'd type them into Discord. */
export async function pullFromImage(src: string, unblend = true): Promise<{ top: string; bottom: string; seen: [string, string] }> {
  const img = await new Promise<HTMLImageElement>((res, rej) => {
    const i = new Image();
    i.onload = () => res(i);
    i.onerror = () => rej(new Error("That image couldn't be read"));
    i.src = src;
  });
  const w = img.naturalWidth, h = img.naturalHeight;
  if (w < 20 || h < 40) throw new Error("That image is too small to read colours from");
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  const g = c.getContext("2d", { willReadFrequently: true })!;
  g.drawImage(img, 0, 0);
  const data = g.getImageData(0, 0, w, h).data;

  // The card may not fill the image: find the rows and columns that aren't
  // the plain background around it (the colour in the corners).
  const at = (x: number, y: number): Rgb => {
    const i = (y * w + x) * 4;
    return [data[i], data[i + 1], data[i + 2]];
  };
  const bg = at(1, 1);
  const differs = (p: Rgb) => Math.abs(p[0] - bg[0]) + Math.abs(p[1] - bg[1]) + Math.abs(p[2] - bg[2]) > 30;
  let left = 0, right = w - 1;
  const midY = Math.floor(h * 0.6);
  while (left < w / 2 && !differs(at(left, midY))) left++;
  while (right > w / 2 && !differs(at(right, midY))) right--;
  if (right - left < 20) { left = 0; right = w - 1; }

  const strip = (y0: number, y1: number): Rgb => {
    const xs = [0.02, 0.035, 0.05].flatMap((f) => [left + Math.floor((right - left) * f), right - Math.floor((right - left) * f)]);
    const px: Rgb[] = [];
    for (let y = Math.floor(h * y0); y < Math.floor(h * y1); y += Math.max(1, Math.floor(h / 240))) for (const x of xs) px.push(at(x, y));
    const med = (k: 0 | 1 | 2) => px.map((p) => p[k]).sort((a, b) => a - b)[Math.floor(px.length / 2)] ?? 0;
    return [med(0), med(1), med(2)];
  };
  const topSeen = strip(0.3, 0.4);
  const bottomSeen = strip(0.86, 0.96);
  const fix = (p: Rgb): Rgb => {
    if (!unblend) return p;
    // Discord's layer is about 60% white (light) or black (dark).
    const light = (p[0] + p[1] + p[2]) / 3 > 128;
    const a = 0.6, over = light ? 255 : 0;
    return p.map((v) => (v - over * a) / (1 - a)) as Rgb;
  };
  // Undoing the layer amplifies small differences; tame the result in OKLCH.
  const tame = (hex: string) => {
    const [L, C, H] = toOklch(hex);
    return fromOklch(L, Math.min(C, 0.22), H);
  };
  return {
    top: tame(rgbToHex(fix(topSeen))),
    bottom: tame(rgbToHex(fix(bottomSeen))),
    seen: [rgbToHex(topSeen), rgbToHex(bottomSeen)],
  };
}
