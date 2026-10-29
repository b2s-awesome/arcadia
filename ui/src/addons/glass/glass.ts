// Liquid Glass: sets the switches glass.css reads.
import type { UiState } from "../../lib/theme";

export const GLASS_ID = "liquid-glass";

export function applyGlass(ui: UiState) {
  const root = document.documentElement;
  const g = ui.glass;
  if (!ui.addons.installed.includes(GLASS_ID) || !g.enabled) {
    delete root.dataset.glass;
    delete root.dataset.glassSheen;
    delete root.dataset.glassTone;
    return;
  }
  root.dataset.glass = g.scope;
  root.dataset.glassSheen = String(g.sheen);
  root.style.setProperty("--lg-frost", `${g.frost}px`);
  root.style.setProperty("--lg-tint", String(g.tint));
  const frost = g.shade === "frost" || (g.shade === "auto" && root.dataset.scheme === "light");
  root.style.setProperty("--lg-rgb", frost ? "255, 255, 255" : "18, 14, 26");
  root.dataset.glassTone = frost ? "frost" : "smoke";
  root.style.setProperty("--lg-light", String(g.edge));
}
