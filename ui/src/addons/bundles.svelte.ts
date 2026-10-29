// Addon bundles: official ones from the catalogue, and ones you make and
// share as a code (arc-bundle:v1:…) or a .arcbundle file. A bundle only
// names catalogue addons and carries settings; it can't carry code.
import { app, saveSettingsNow } from "../lib/state.svelte";
import {
  normalizeFad, normalizeGlass, normalizeLyrics, normalizeSeeThrough, normalizeStats, normalizeThemes,
  type BundleKey, type UiState, type UserBundle,
} from "../lib/theme";
import { CATALOG, addonById } from "./registry";

export interface SharedBundle {
  v: 1;
  name: string;
  addons: string[];
  settings: Partial<Record<BundleKey, unknown>> | null;
}

const PREFIX = "arc-bundle:v1:";

/** Settings that belong to each addon, by UiState key. */
const OWNED: Partial<Record<BundleKey, string>> = { fad: "full-app-display", lyrics: "lyrics-plus", stats: "stats", themes: "themes", seeThrough: "see-through", glass: "liquid-glass" };

/** Install addons (skipping ones already in), then save. */
export async function installAddons(ids: string[]) {
  const add = ids.filter((id) => addonById(id) && !app.ui.addons.installed.includes(id));
  if (!add.length) return add;
  app.ui.addons.installed = [...app.ui.addons.installed, ...add];
  // Shown in the sidebar straight away, even if hidden before.
  app.ui.rail.hidden = app.ui.rail.hidden.filter((h) => !add.includes(h.replace(/^addon:/, "")));
  if (app.settings) app.settings.ui = $state.snapshot(app.ui);
  await saveSettingsNow();
  return add;
}

/** A copy of the settings for these addons. */
export function snapshotSettings(addons: string[], withTheme: boolean): SharedBundle["settings"] {
  const out: Partial<Record<BundleKey, unknown>> = {};
  for (const [key, owner] of Object.entries(OWNED) as [BundleKey, string][]) {
    if (!addons.includes(owner)) continue;
    if (key === "themes" && !withTheme) continue;
    const v = $state.snapshot(app.ui[key as keyof UiState]) as Record<string, unknown>;
    // Per-song timing shifts mean nothing on someone else's PC.
    if (key === "lyrics") out[key] = { ...v, offsets: {} };
    else out[key] = v;
  }
  return Object.keys(out).length ? out : null;
}

/** Only known addons and known settings, each cleaned up. */
export function validate(raw: unknown): SharedBundle {
  if (!raw || typeof raw !== "object") throw new Error("That isn't a bundle.");
  const r = raw as Record<string, unknown>;
  if (r.v !== 1) throw new Error("That bundle is from a newer Arcadia. Update to open it.");
  const addons = Array.isArray(r.addons) ? [...new Set(r.addons.filter((x): x is string => typeof x === "string" && CATALOG.some((a) => a.id === x)))] : [];
  if (!addons.length) throw new Error("That bundle doesn't list any addons Arcadia knows.");
  const s = r.settings && typeof r.settings === "object" ? (r.settings as Record<string, unknown>) : null;
  const clean: Partial<Record<BundleKey, unknown>> = {};
  if (s) {
    const fix: Partial<Record<BundleKey, (v: never) => unknown>> = { fad: normalizeFad, lyrics: normalizeLyrics, stats: normalizeStats, themes: normalizeThemes, seeThrough: normalizeSeeThrough, glass: normalizeGlass };
    for (const [k, f] of Object.entries(fix) as [BundleKey, (v: never) => unknown][]) {
      if (s[k] && typeof s[k] === "object" && addons.includes(OWNED[k]!)) clean[k] = f(s[k] as never);
    }
  }
  return { v: 1, name: typeof r.name === "string" && r.name.trim() ? r.name.trim().slice(0, 60) : "Shared bundle", addons, settings: Object.keys(clean).length ? clean : null };
}

const b64 = (bytes: Uint8Array) => btoa(String.fromCharCode(...bytes)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
const unb64 = (s: string) => Uint8Array.from(atob(s.replace(/-/g, "+").replace(/_/g, "/")), (c) => c.charCodeAt(0));

async function pipe(bytes: Uint8Array, stream: CompressionStream | DecompressionStream): Promise<Uint8Array> {
  const out = new Blob([bytes as BlobPart]).stream().pipeThrough(stream);
  return new Uint8Array(await new Response(out).arrayBuffer());
}

export async function encode(b: SharedBundle): Promise<string> {
  const json = new TextEncoder().encode(JSON.stringify(b));
  return PREFIX + b64(await pipe(json, new CompressionStream("deflate-raw")));
}

/** A code, or the text of a .arcbundle file. */
export async function decode(text: string): Promise<SharedBundle> {
  const t = text.trim();
  if (t.startsWith("{")) return validate(JSON.parse(t));
  if (!t.startsWith(PREFIX)) throw new Error("Bundle codes start with arc-bundle:v1:");
  try {
    const bytes = await pipe(unb64(t.slice(PREFIX.length).replace(/\s+/g, "")), new DecompressionStream("deflate-raw"));
    if (bytes.length > 512 * 1024) throw new Error("too big");
    return validate(JSON.parse(new TextDecoder().decode(bytes)));
  } catch (e) {
    if (e instanceof Error && /bundle/i.test(e.message)) throw e;
    throw new Error("That code is damaged or cut short. Copy the whole thing.");
  }
}

/** What applying a bundle changes, for the preview. */
export function preview(b: SharedBundle) {
  return {
    add: b.addons.filter((id) => !app.ui.addons.installed.includes(id)),
    have: b.addons.filter((id) => app.ui.addons.installed.includes(id)),
    settings: Object.keys(b.settings ?? {}) as BundleKey[],
  };
}

export async function apply(b: SharedBundle, withSettings: boolean) {
  if (withSettings && b.settings) {
    for (const [k, v] of Object.entries(b.settings) as [BundleKey, unknown][]) {
      (app.ui as unknown as Record<string, unknown>)[k] = v;
    }
  }
  const added = await installAddons(b.addons);
  if (!added.length) {
    if (app.settings) app.settings.ui = $state.snapshot(app.ui);
    await saveSettingsNow();
  }
  return added;
}

export function fromUser(u: UserBundle): SharedBundle {
  return { v: 1, name: u.name, addons: u.addons, settings: u.settings };
}

export const SETTING_NAMES: Record<BundleKey, string> = { fad: "Full App Display", lyrics: "Lyrics Plus", stats: "Stats", themes: "Theme, snippets and your CSS", seeThrough: "Translucent window", music: "Music layout", glass: "Liquid Glass" };
