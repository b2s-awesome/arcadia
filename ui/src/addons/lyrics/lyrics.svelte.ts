// Lyrics Plus: lyrics for the playing song, fetched only while something
// shows them (the Lyrics page, Now Playing, Full App Display, the pop-up
// window or the desktop bar).
import { lyricsApi } from "../../lib/api";
import { app, saveUi } from "../../lib/state.svelte";
import type { LyricLine, Lyrics } from "../../lib/types";
import { ms } from "../../music/store.svelte";

export const LYRICS = "lyrics-plus";

export const ly = $state({
  /** The song these lyrics are for. */
  id: null as number | null,
  lyrics: null as Lyrics | null,
  loading: false,
  error: null as string | null,
  /** How many things on screen want lyrics. */
  users: 0,
  /** Lines under the lyrics: romanisation and/or translation, per line. */
  under: null as (string | null)[] | null,
  translating: false,
  translateError: null as string | null,
});

/** Language to translate into: the setting, or this PC's language. */
export const targetLang = () => app.ui.lyrics.target || (typeof navigator !== "undefined" ? navigator.language.slice(0, 2) : "en");

const cache = new Map<number, Lyrics | null>();

async function load(id: number) {
  ly.id = id;
  ly.error = null;
  if (cache.has(id)) {
    ly.lyrics = cache.get(id)!;
    ly.loading = false;
    return;
  }
  ly.lyrics = null;
  ly.loading = true;
  try {
    const l = await lyricsApi.get(id, app.ui.lyrics.online, app.ui.lyrics.sidecar);
    cache.set(id, l);
    if (ly.id === id) ly.lyrics = l;
  } catch (e) {
    if (ly.id === id) ly.error = String(e);
  } finally {
    if (ly.id === id) ly.loading = false;
  }
}

let watching = false;
function watch() {
  if (watching) return;
  watching = true;
  $effect.root(() => {
    $effect(() => {
      const id = ms.status?.track?.id ?? null;
      const on = ly.users > 0 && app.ui.addons.installed.includes(LYRICS);
      if (!on || id === null) return;
      if (id !== ly.id) load(id);
    });
    // Romanisation and translation, worked out when the lyrics or the
    // switches change.
    $effect(() => {
      const l = ly.lyrics;
      const id = ly.id;
      const p = app.ui.lyrics;
      const want = { rom: p.romanise, tr: p.translate, to: targetLang(), email: p.email };
      void extras(id, l, want);
    });
  });
}

let extrasRun = 0;
async function extras(id: number | null, l: Lyrics | null, want: { rom: boolean; tr: boolean; to: string; email: string }) {
  const run = ++extrasRun;
  ly.under = null;
  ly.translateError = null;
  if (id === null || !l || l.instrumental || (!want.rom && !want.tr)) return;
  const lines = l.lines.map((x) => x.text);
  const all = lines.join("\n");
  const { detectScript, guessLanguage, romanise } = await import("./romanise");
  const script = detectScript(all);
  const rom = want.rom && script !== "latin" ? await romanise(lines, script) : null;
  let tr: string[] | null = null;
  const from = guessLanguage(all);
  if (want.tr && from !== want.to) {
    ly.translating = true;
    try {
      tr = await lyricsApi.translate(id, from, want.to, lines, want.email || null);
    } catch (e) {
      if (run === extrasRun) ly.translateError = String(e);
    } finally {
      if (run === extrasRun) ly.translating = false;
    }
  }
  if (run !== extrasRun) return;
  ly.under = lines.map((_, i) => [rom?.[i], tr?.[i]].filter((x) => x && x.trim()).join("\n") || null);
}

/** Call from a component that shows lyrics; returns the cleanup. */
export function useLyrics(): () => void {
  watch();
  ly.users++;
  return () => {
    ly.users = Math.max(0, ly.users - 1);
  };
}

/** Look again (after fixing tags, or to try LRCLIB again). */
export async function reloadLyrics() {
  if (ly.id === null) return;
  cache.delete(ly.id);
  await lyricsApi.forget(ly.id).catch(() => {});
  await load(ly.id);
}

/** Saved from the timing editor. */
export function replaceLyrics(id: number, l: Lyrics) {
  cache.set(id, l);
  if (ly.id === id) ly.lyrics = l;
}

// ── timing ──────────────────────────────────────────────────

export const offsetFor = (id: number | null) => (id === null ? 0 : (app.ui.lyrics.offsets[String(id)] ?? 0));

export function nudge(by: number) {
  const id = ly.id;
  if (id === null) return;
  const v = Math.round((offsetFor(id) + by) * 10) / 10;
  if (v === 0) delete app.ui.lyrics.offsets[String(id)];
  else app.ui.lyrics.offsets[String(id)] = v;
  saveUi();
}

/** Position in the lyrics' own time (with this song's offset). */
export const lyricTime = () => ms.pos + offsetFor(ly.id);

/** The line being sung at `t`: the last one that has started. */
export function lineAt(lines: LyricLine[], t: number): number {
  let lo = 0;
  let hi = lines.length - 1;
  let found = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if ((lines[mid].t ?? 0) <= t) {
      found = mid;
      lo = mid + 1;
    } else hi = mid - 1;
  }
  return found;
}

/** How far through each word of a line we are, 0–1 per word. */
export function wordProgress(line: LyricLine, next: LyricLine | undefined, t: number): number[] {
  const ws = line.words ?? [];
  return ws.map((w, i) => {
    const end = ws[i + 1]?.t ?? next?.t ?? w.t + 0.6;
    if (t <= w.t) return 0;
    if (t >= end) return 1;
    return (t - w.t) / Math.max(0.05, end - w.t);
  });
}

/** The line now, for one-line places (Full App Display, desktop bar). */
export function currentLine(): { text: string; next: string } | null {
  const l = ly.lyrics;
  if (!l || !l.synced || ly.id !== ms.status?.track?.id) return null;
  const i = lineAt(l.lines, lyricTime());
  if (i < 0) return { text: "", next: l.lines[0]?.text ?? "" };
  return { text: l.lines[i].text, next: l.lines[i + 1]?.text ?? "" };
}
