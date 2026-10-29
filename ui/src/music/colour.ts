/** Black or white ink for text on `hex`. */
export function inkFor(hex: string | null): string {
  const m = hex && /^#([0-9a-f]{6})$/i.exec(hex);
  if (!m) return "var(--peach-ink)";
  const n = parseInt(m[1], 16);
  const lin = (c: number) => {
    const v = c / 255;
    return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  const l = 0.2126 * lin((n >> 16) & 255) + 0.7152 * lin((n >> 8) & 255) + 0.0722 * lin(n & 255);
  return (l + 0.05) / 0.05 > 1.05 / (l + 0.05) ? "#111114" : "#ffffff";
}
