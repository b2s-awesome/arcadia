// Romanisation for lyrics, all offline: Korean (Revised Romanisation),
// Japanese kana (Hepburn), Chinese (pinyin, loaded only when needed),
// Russian/Ukrainian Cyrillic and Greek. Japanese kanji stay as they are.

export type Script = "ko" | "ja" | "zh" | "cyrillic" | "greek" | "latin";

/** The main non-Latin script in some lyrics, if any. */
export function detectScript(text: string): Script {
  let ko = 0, kana = 0, han = 0, cyr = 0, gr = 0;
  for (const ch of text) {
    const c = ch.codePointAt(0)!;
    if (c >= 0xac00 && c <= 0xd7a3) ko++;
    else if ((c >= 0x3040 && c <= 0x30ff) || (c >= 0x31f0 && c <= 0x31ff)) kana++;
    else if (c >= 0x4e00 && c <= 0x9fff) han++;
    else if (c >= 0x0400 && c <= 0x04ff) cyr++;
    else if (c >= 0x0370 && c <= 0x03ff) gr++;
  }
  const max = Math.max(ko, kana, han, cyr, gr);
  if (max < 3) return "latin";
  if (ko === max) return "ko";
  if (kana > 0 && kana >= han * 0.2) return "ja";
  if (han === max) return "zh";
  if (cyr === max) return "cyrillic";
  return "greek";
}

// ── Korean ───────────────────────────────────────────────────
const KO_INITIAL = ["g", "kk", "n", "d", "tt", "r", "m", "b", "pp", "s", "ss", "", "j", "jj", "ch", "k", "t", "p", "h"];
const KO_VOWEL = ["a", "ae", "ya", "yae", "eo", "e", "yeo", "ye", "o", "wa", "wae", "oe", "yo", "u", "wo", "we", "wi", "yu", "eu", "ui", "i"];
const KO_FINAL = ["", "k", "k", "k", "n", "n", "n", "t", "l", "k", "m", "l", "l", "l", "p", "l", "m", "p", "p", "t", "t", "ng", "t", "t", "k", "t", "p", "t"];

// A final consonant read as the next syllable's start, before a silent ㅇ.
const KO_CARRY: Record<number, string> = { 1: "g", 2: "kk", 4: "n", 7: "d", 8: "r", 16: "m", 17: "b", 19: "s", 20: "ss", 22: "j", 23: "ch", 24: "k", 25: "t", 26: "p", 27: "" };

function korean(s: string): string {
  let out = "";
  let prev = 0;
  for (const ch of s) {
    const c = ch.codePointAt(0)!;
    if (c < 0xac00 || c > 0xd7a3) {
      out += ch;
      prev = 0;
      continue;
    }
    const n = c - 0xac00;
    const ini = Math.floor(n / 588);
    const vow = Math.floor((n % 588) / 28);
    const fin = n % 28;
    let start = KO_INITIAL[ini];
    if (ini === 11 && prev in KO_CARRY) {
      out = out.slice(0, out.length - KO_FINAL[prev].length);
      start = KO_CARRY[prev];
    }
    out += start + KO_VOWEL[vow] + KO_FINAL[fin];
    prev = fin;
  }
  return out;
}

// ── Japanese kana ────────────────────────────────────────────
const KANA: Record<string, string> = {
  あ: "a", い: "i", う: "u", え: "e", お: "o", か: "ka", き: "ki", く: "ku", け: "ke", こ: "ko", さ: "sa", し: "shi", す: "su", せ: "se", そ: "so",
  た: "ta", ち: "chi", つ: "tsu", て: "te", と: "to", な: "na", に: "ni", ぬ: "nu", ね: "ne", の: "no", は: "ha", ひ: "hi", ふ: "fu", へ: "he", ほ: "ho",
  ま: "ma", み: "mi", む: "mu", め: "me", も: "mo", や: "ya", ゆ: "yu", よ: "yo", ら: "ra", り: "ri", る: "ru", れ: "re", ろ: "ro", わ: "wa", を: "o", ん: "n",
  が: "ga", ぎ: "gi", ぐ: "gu", げ: "ge", ご: "go", ざ: "za", じ: "ji", ず: "zu", ぜ: "ze", ぞ: "zo", だ: "da", ぢ: "ji", づ: "zu", で: "de", ど: "do",
  ば: "ba", び: "bi", ぶ: "bu", べ: "be", ぼ: "bo", ぱ: "pa", ぴ: "pi", ぷ: "pu", ぺ: "pe", ぽ: "po", ゔ: "vu",
  ぁ: "a", ぃ: "i", ぅ: "u", ぇ: "e", ぉ: "o", ゎ: "wa",
};
const SMALL_Y: Record<string, string> = { ゃ: "ya", ゅ: "yu", ょ: "yo" };

function kana(s: string): string {
  // Katakana to hiragana first; the long-vowel mark repeats the last vowel.
  const h = [...s].map((ch) => {
    const c = ch.codePointAt(0)!;
    return c >= 0x30a1 && c <= 0x30f6 ? String.fromCodePoint(c - 0x60) : ch;
  });
  let out = "";
  for (let i = 0; i < h.length; i++) {
    const ch = h[i];
    const next = h[i + 1];
    if (ch === "っ" || ch === "ッ") {
      const r = KANA[next] ?? "";
      out += r ? (r.startsWith("ch") ? "t" : r[0]) : "";
      continue;
    }
    if (ch === "ー") {
      const v = out.match(/[aeiou]$/);
      out += v ? v[0] : "";
      continue;
    }
    const r = KANA[ch];
    if (r === undefined) {
      out += ch === "、" ? ", " : ch === "。" ? ". " : ch === "　" ? " " : ch;
      continue;
    }
    if (next && SMALL_Y[next]) {
      const y = SMALL_Y[next];
      out += r.endsWith("i") ? (["shi", "chi", "ji"].includes(r) ? r.slice(0, -1) + y.slice(1) : r.slice(0, -1) + y) : r + y;
      i++;
      continue;
    }
    // ん before a vowel or y keeps an apostrophe (kan'i).
    out += r === "n" && next && /^[aiueoy]/.test(KANA[next] ?? "") ? "n'" : r;
  }
  return out;
}

// ── Cyrillic and Greek ───────────────────────────────────────
const CYR: Record<string, string> = {
  а: "a", б: "b", в: "v", г: "g", д: "d", е: "e", ё: "yo", ж: "zh", з: "z", и: "i", й: "y", к: "k", л: "l", м: "m", н: "n", о: "o", п: "p",
  р: "r", с: "s", т: "t", у: "u", ф: "f", х: "kh", ц: "ts", ч: "ch", ш: "sh", щ: "shch", ъ: "", ы: "y", ь: "", э: "e", ю: "yu", я: "ya",
  є: "ye", і: "i", ї: "yi", ґ: "g",
};
const GR: Record<string, string> = {
  α: "a", ά: "a", β: "v", γ: "g", δ: "d", ε: "e", έ: "e", ζ: "z", η: "i", ή: "i", θ: "th", ι: "i", ί: "i", ϊ: "i", ΐ: "i", κ: "k", λ: "l", μ: "m", ν: "n",
  ξ: "x", ο: "o", ό: "o", π: "p", ρ: "r", σ: "s", ς: "s", τ: "t", υ: "y", ύ: "y", ϋ: "y", ΰ: "y", φ: "f", χ: "ch", ψ: "ps", ω: "o", ώ: "o",
};
function table(s: string, t: Record<string, string>): string {
  return [...s].map((ch) => {
    const low = ch.toLowerCase();
    const r = t[low];
    if (r === undefined) return ch;
    return ch !== low && r ? r[0].toUpperCase() + r.slice(1) : r;
  }).join("");
}

let pinyinMod: typeof import("pinyin-pro") | null = null;

/** Romanise lines; Chinese loads its dictionary the first time. */
export async function romanise(lines: string[], script: Script): Promise<string[]> {
  switch (script) {
    case "ko": return lines.map(korean);
    case "ja": return lines.map(kana);
    case "cyrillic": return lines.map((l) => table(l, CYR));
    case "greek": return lines.map((l) => table(l, GR));
    case "zh": {
      pinyinMod ??= await import("pinyin-pro");
      const { pinyin } = pinyinMod;
      return lines.map((l) => (l ? pinyin(l, { toneType: "symbol", nonZh: "consecutive" }) : l));
    }
    default: return lines;
  }
}

/** A best guess at a language code for translating, from the script and
 *  some very common words. */
export function guessLanguage(text: string): string {
  const s = detectScript(text);
  if (s === "ko" || s === "ja" || s === "zh") return s;
  if (s === "greek") return "el";
  if (s === "cyrillic") return /[єіїґ]/i.test(text) ? "uk" : "ru";
  const words = text.toLowerCase().match(/[a-zà-ÿ']+/g) ?? [];
  const score = (list: string[]) => words.filter((w) => list.includes(w)).length;
  const langs: [string, string[]][] = [
    ["es", ["que", "el", "los", "las", "y", "mi", "tu", "con", "por", "para", "como", "pero", "yo", "eres", "quiero", "corazón"]],
    ["fr", ["je", "tu", "le", "la", "les", "et", "est", "pas", "que", "dans", "pour", "mon", "moi", "toi", "mais", "avec"]],
    ["pt", ["que", "eu", "você", "não", "meu", "com", "para", "uma", "mais", "sem", "tudo", "amor", "coração", "seu"]],
    ["de", ["ich", "du", "und", "die", "der", "das", "nicht", "ist", "mit", "mich", "dich", "ein", "eine", "wir", "mein"]],
    ["it", ["che", "il", "non", "di", "e", "mi", "ti", "per", "una", "sono", "come", "amore", "cuore", "ancora"]],
    ["nl", ["ik", "je", "de", "het", "een", "niet", "en", "van", "mijn", "jij", "wij", "maar", "voor"]],
    ["en", ["the", "and", "you", "i", "me", "my", "to", "it", "in", "is", "of", "your", "love", "don't", "we"]],
  ];
  let best = "en";
  let top = 0;
  for (const [code, list] of langs) {
    const n = score(list);
    if (n > top) [best, top] = [code, n];
  }
  return best;
}
