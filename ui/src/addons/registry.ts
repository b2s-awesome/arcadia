// The marketplace catalog. Addons ship inside Arcadia for now and are only
// loaded once installed (each is its own lazily imported chunk). The shape
// is ready for a remote catalog: everything a card shows lives here.
import type { Component } from "svelte";

export interface AddonPermission {
  label: string;
  detail: string;
}

export interface AddonMeta {
  id: string;
  name: string;
  /** One line for the card. */
  tagline: string;
  description: string;
  author: string;
  official: boolean;
  version: string;
  /** Icon.svelte name. */
  icon: string;
  /** Card artwork colours. */
  art: [string, string, string];
  features: string[];
  permissions: AddonPermission[];
  load: () => Promise<{ default: Component }>;
}

export const CATALOG: AddonMeta[] = [
  {
    id: "discord-profile",
    name: "Profile Studio",
    tagline: "Design your Discord profile before you change it.",
    description:
      "A sandbox for your Discord profile. Pull anyone's profile in by username or ID, or start blank, then change the avatar, banner, decoration, colours, pronouns and About me and see it the way Discord shows it. Keep as many versions as you like.",
    author: "Arcadia",
    official: true,
    version: "1.0.0",
    icon: "profile",
    art: ["#1e1f4b", "#5865f2", "#eb459e"],
    features: [
      "Live preview: profile popout, full profile, and chat with the member list",
      "Import a profile from a username, user ID or profile link",
      "Three suggested palettes: from the banner, the avatar, and both together",
      "Save, compare and export as many profile versions as you like",
      "Discord's own limits on names, pronouns and About me",
    ],
    permissions: [
      { label: "Internet: Discord", detail: "Looks profiles up through Discord's API using your own bot token, and downloads their images from Discord's CDN." },
      { label: "Internet: Lanyard", detail: "Without a bot token, looks up user IDs through api.lanyard.rest." },
      { label: "Storage", detail: "Saves your profile versions in its own folder. Your bot token goes in Windows Credential Manager." },
    ],
    load: () => import("./discord-profile/Studio.svelte"),
  },
];

export const addonById = (id: string) => CATALOG.find((a) => a.id === id) ?? null;
