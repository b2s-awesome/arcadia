// LRC in the browser: enough for the timing editor (writing) and the mock
// backend (reading). The app's real parser is in Rust.
import type { LyricLine, Lyrics } from "../../lib/types";

const STAMP = /\[(\d+):(\d+(?:[.:]\d+)?)\]/g;

export function parseLrc(text: string): Lyrics {
  const lines: LyricLine[] = [];
  for (const raw of text.split(/\r?\n/)) {
    const times: number[] = [];
    let m: RegExpExecArray | null;
    STAMP.lastIndex = 0;
    let end = 0;
    while ((m = STAMP.exec(raw)) && m.index === end) {
      times.push(+m[1] * 60 + +m[2].replace(":", "."));
      end = STAMP.lastIndex;
    }
    const rest = raw.slice(end).trim();
    if (!times.length) {
      if (rest && !/^\[[a-z]+:/i.test(raw)) lines.push({ t: null, text: rest });
      continue;
    }
    for (const t of times) lines.push({ t, text: rest });
  }
  const synced = lines.some((l) => l.t !== null);
  if (synced) lines.sort((a, b) => (a.t ?? 0) - (b.t ?? 0));
  return { synced, words: false, source: "yours", lines: synced ? lines.filter((l) => l.t !== null) : lines, instrumental: false };
}

const two = (n: number) => String(Math.floor(n)).padStart(2, "0");

export function stamp(t: number): string {
  const m = Math.floor(t / 60);
  const s = t - m * 60;
  return `${two(m)}:${two(s)}.${String(Math.round((s % 1) * 100) % 100).padStart(2, "0")}`;
}

export function toLrc(lines: { t: number | null; text: string }[]): string {
  return lines.map((l) => (l.t === null ? l.text : `[${stamp(l.t)}]${l.text}`)).join("\n") + "\n";
}
