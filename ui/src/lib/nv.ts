// Helpers for showing and editing NVIDIA driver values in the UI.
import type { NvKind, NvValue } from "./fftypes";

export const hex = (n: number | bigint, width = 8) => "0x" + n.toString(16).toUpperCase().padStart(width, "0");

/** Base64 → "AB 01 FF". */
export function bytesToHex(b64: string): string {
  try {
    return Array.from(atob(b64), (c) => c.charCodeAt(0).toString(16).toUpperCase().padStart(2, "0")).join(" ");
  } catch {
    return "";
  }
}

/** "AB 01 ff" or "ab01ff" → base64, or null when it is not hex bytes. */
export function hexToBase64(text: string): string | null {
  const clean = text.replace(/[\s,]/g, "");
  if (clean.length % 2 !== 0 || !/^[0-9a-fA-F]*$/.test(clean)) return null;
  let bin = "";
  for (let i = 0; i < clean.length; i += 2) bin += String.fromCharCode(parseInt(clean.slice(i, i + 2), 16));
  return btoa(bin);
}

/** What goes in the edit box for a value. */
export function editText(v: NvValue | null): string {
  if (!v) return "";
  if (v.type === "dword") return hex(Number(v.value));
  if (v.type === "qword") return hex(BigInt(v.value), 16);
  if (v.type === "binary") return bytesToHex(v.value);
  return v.value;
}

/** Parse what the user typed into a value of the given kind. Returns an error string when it does not fit. */
export function parseValue(kind: NvKind, text: string): NvValue | string {
  const t = text.trim();
  if (kind === "dword" || kind === "qword") {
    if (!/^(0x[0-9a-fA-F]+|\d+)$/.test(t)) return "Enter a number (decimal, or hex starting with 0x)";
    let n: bigint;
    try {
      n = BigInt(t);
    } catch {
      return "Enter a number (decimal, or hex starting with 0x)";
    }
    const max = kind === "dword" ? 0xffffffffn : 0xffffffffffffffffn;
    if (n < 0n || n > max) return kind === "dword" ? "The value must fit in 32 bits" : "The value must fit in 64 bits";
    return { type: kind, value: n.toString() };
  }
  if (kind === "binary") {
    const b64 = hexToBase64(t);
    if (b64 === null) return "Enter bytes as hex pairs, for example 0A FF 10";
    if (atob(b64).length > 4096) return "Binary values are limited to 4096 bytes";
    return { type: kind, value: b64 };
  }
  if (text.length > 2047) return "Text values are limited to 2047 characters";
  if (kind === "ansi" && !/^[\x20-\x7e]*$/.test(text)) return "This setting only takes plain ASCII text";
  return { type: kind, value: text };
}

export const sameValue = (a: NvValue | null, b: NvValue | null) => !!a && !!b && a.type === b.type && a.value === b.value;
