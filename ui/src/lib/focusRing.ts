// The focus ring is for keyboard users. Chromium shows it on a button that
// was *clicked* as soon as any key is pressed afterwards, so pressing Space
// to pause music left a ring on whatever play button you last clicked (the
// stray box beside the song list). Buttons and tabs focused by the mouse are
// marked, and marked ones don't show the ring until Tab moves focus.

const MARK = "data-pointer-focus";

export function installFocusRing(): () => void {
  const down = (e: PointerEvent) => {
    const el = (e.target as Element | null)?.closest?.("button, [role=tab], [role=button], a, summary");
    if (el) el.setAttribute(MARK, "");
  };
  const out = (e: FocusEvent) => (e.target as Element | null)?.removeAttribute?.(MARK);
  const key = (e: KeyboardEvent) => {
    if (e.key === "Tab") document.activeElement?.removeAttribute(MARK);
  };
  window.addEventListener("pointerdown", down, true);
  window.addEventListener("focusout", out, true);
  window.addEventListener("keydown", key, true);
  return () => {
    window.removeEventListener("pointerdown", down, true);
    window.removeEventListener("focusout", out, true);
    window.removeEventListener("keydown", key, true);
  };
}
