// The sidebar's entries, in the user's order. Settings and Search always
// stay pinned at the bottom so nothing can hide the way back.
import { addonById } from "../addons/registry";
import type { Page } from "./state.svelte";
import type { UiState } from "./theme";

export interface NavItem {
  id: Page;
  label: string;
  icon: string;
  /** Addons can be removed from the Marketplace, not just hidden. */
  addon?: string;
}

export const CORE_NAV: NavItem[] = [
  { id: "home", label: "Home", icon: "sparkle" },
  { id: "library", label: "Library", icon: "library" },
  { id: "music", label: "Music", icon: "music" },
  { id: "minecraft", label: "Minecraft", icon: "cube" },
  { id: "roblox", label: "Roblox", icon: "roblox" },
  { id: "proxy", label: "Asset proxy", icon: "proxy" },
  { id: "inspector", label: "Inspector", icon: "inspector" },
  { id: "appearance", label: "Appearance", icon: "brush" },
  { id: "marketplace", label: "Marketplace", icon: "store" },
];

/** Every entry that can be in the sidebar, in the user's order. */
export function allNav(ui: UiState): NavItem[] {
  const pool = new Map<string, NavItem>(CORE_NAV.map((n) => [n.id, n]));
  for (const id of ui.addons.installed) {
    const a = addonById(id);
    if (a?.load) pool.set(`addon:${id}`, { id: `addon:${id}`, label: a.name, icon: a.icon, addon: id });
  }
  const out: NavItem[] = [];
  for (const id of ui.rail.order) {
    const n = pool.get(id);
    if (n) {
      out.push(n);
      pool.delete(id);
    }
  }
  // Home goes first the first time it appears; anything else new (a fresh
  // install, a page added in an update) goes last.
  const home = pool.get("home");
  pool.delete("home");
  return [...(home ? [home] : []), ...out, ...pool.values()];
}

/** The entries actually shown. */
export function visibleNav(ui: UiState): NavItem[] {
  return allNav(ui).filter((n) => !ui.rail.hidden.includes(n.id));
}

/** Freeze the current order into the prefs (after a move). */
export function setOrder(ui: UiState, ids: string[]) {
  ui.rail.order = ids;
}
