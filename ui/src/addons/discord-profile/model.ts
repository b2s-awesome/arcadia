// Profile Studio's data: one editable Discord profile, and Discord's limits.

export type Status = "online" | "idle" | "dnd" | "offline";
export type PreviewView = "popout" | "profile" | "chat";

export interface ProfileTheme {
  /** Top of the profile gradient. */
  primary: string;
  /** Bottom of the profile gradient. */
  accent: string;
}

export interface DProfile {
  id: string;
  /** Name on the shelf; not part of the Discord profile. */
  label: string;
  username: string;
  displayName: string;
  pronouns: string;
  bio: string;
  /** data: URLs, kept whole so GIFs stay animated. */
  avatar: string | null;
  banner: string | null;
  decoration: string | null;
  /** Banner colour shown when there's no banner image. */
  bannerColor: string;
  /** Nitro profile colours; null for a regular profile. */
  theme: ProfileTheme | null;
  status: Status;
  customStatus: string;
  /** Account creation time (ms), shown as "Member since". */
  createdAt: number | null;
  /** Where it was pulled from, if anywhere. */
  source: { userId: string; via: "discord" | "lanyard" | "file"; at: number } | null;
  updatedAt: number;
}

/** Discord's own limits. */
export const LIMITS = {
  username: 32,
  displayName: 32,
  pronouns: 40,
  bio: 190,
  customStatus: 128,
} as const;

export const STATUS_LABEL: Record<Status, string> = {
  online: "Online",
  idle: "Idle",
  dnd: "Do Not Disturb",
  offline: "Invisible",
};

export const DEFAULT_BANNER = "#5865f2";

export function newId(): string {
  return Math.random().toString(36).slice(2, 10) + Date.now().toString(36).slice(-4);
}

export function blank(): DProfile {
  return {
    id: newId(),
    label: "New profile",
    username: "you",
    displayName: "",
    pronouns: "",
    bio: "",
    avatar: null,
    banner: null,
    decoration: null,
    bannerColor: DEFAULT_BANNER,
    theme: null,
    status: "online",
    customStatus: "",
    createdAt: Date.now(),
    source: null,
    updatedAt: Date.now(),
  };
}

/** Problems Discord itself would refuse, keyed by field. */
export function problems(p: DProfile): Partial<Record<keyof DProfile, string>> {
  const out: Partial<Record<keyof DProfile, string>> = {};
  const u = p.username;
  if (u.length < 2 || u.length > LIMITS.username) out.username = `Usernames are 2–${LIMITS.username} characters`;
  else if (!/^[a-z0-9_.]+$/.test(u)) out.username = "Only lowercase letters, numbers, _ and .";
  else if (u.includes("..")) out.username = "No two periods in a row";
  if ([...p.displayName].length > LIMITS.displayName) out.displayName = `Up to ${LIMITS.displayName} characters`;
  if ([...p.pronouns].length > LIMITS.pronouns) out.pronouns = `Up to ${LIMITS.pronouns} characters`;
  if ([...p.bio].length > LIMITS.bio) out.bio = `Up to ${LIMITS.bio} characters`;
  if ([...p.customStatus].length > LIMITS.customStatus) out.customStatus = `Up to ${LIMITS.customStatus} characters`;
  return out;
}

/** The fields that make up the profile itself (not shelf bookkeeping). */
export function fingerprint(p: DProfile): string {
  const { id: _i, label: _l, updatedAt: _u, ...rest } = p;
  return JSON.stringify(rest);
}

/** Small, light copy for the shelf (avatar shrunk to a thumbnail). */
export interface ShelfEntry {
  id: string;
  label: string;
  name: string;
  thumb: string | null;
  bannerColor: string;
  theme: ProfileTheme | null;
  updatedAt: number;
}

export async function thumbnail(src: string | null, size = 64): Promise<string | null> {
  if (!src) return null;
  try {
    const img = await loadImage(src);
    const c = document.createElement("canvas");
    c.width = c.height = size;
    const g = c.getContext("2d")!;
    const s = Math.min(img.naturalWidth, img.naturalHeight);
    g.drawImage(img, (img.naturalWidth - s) / 2, (img.naturalHeight - s) / 2, s, s, 0, 0, size, size);
    return c.toDataURL("image/webp", 0.85);
  } catch {
    return null;
  }
}

export function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((res, rej) => {
    const i = new Image();
    i.onload = () => res(i);
    i.onerror = () => rej(new Error("That image couldn't be read"));
    i.src = src;
  });
}

/** Read a picked file as a data: URL, refusing anything that isn't an image. */
export function readImageFile(f: File, maxMb = 10): Promise<string> {
  return new Promise((res, rej) => {
    if (!f.type.startsWith("image/")) return rej(new Error("Pick an image (PNG, JPG, GIF or WebP)"));
    if (f.size > maxMb * 1024 * 1024) return rej(new Error(`That image is over ${maxMb} MB`));
    const r = new FileReader();
    r.onload = () => res(String(r.result));
    r.onerror = () => rej(new Error("That file couldn't be read"));
    r.readAsDataURL(f);
  });
}

/** Load a profile someone exported, filling in anything missing. */
export function fromJson(text: string): DProfile {
  const v = JSON.parse(text) as Partial<DProfile> & { format?: string };
  if (!v || typeof v !== "object" || typeof v.username !== "string") throw new Error("That isn't a Profile Studio file");
  const b = blank();
  const str = (x: unknown, d: string) => (typeof x === "string" ? x : d);
  const img = (x: unknown) => (typeof x === "string" && x.startsWith("data:image/") ? x : null);
  const hex = (x: unknown, d: string) => (typeof x === "string" && /^#[0-9a-f]{6}$/i.test(x) ? x : d);
  return {
    ...b,
    label: str(v.label, "Imported profile"),
    username: str(v.username, b.username),
    displayName: str(v.displayName, ""),
    pronouns: str(v.pronouns, ""),
    bio: str(v.bio, ""),
    avatar: img(v.avatar),
    banner: img(v.banner),
    decoration: img(v.decoration),
    bannerColor: hex(v.bannerColor, b.bannerColor),
    theme: v.theme && typeof v.theme === "object" ? { primary: hex(v.theme.primary, "#1e1f22"), accent: hex(v.theme.accent, "#5865f2") } : null,
    status: (["online", "idle", "dnd", "offline"] as const).includes(v.status as Status) ? (v.status as Status) : "online",
    customStatus: str(v.customStatus, ""),
    createdAt: typeof v.createdAt === "number" ? v.createdAt : null,
    source: { userId: "", via: "file", at: Date.now() },
  };
}

export function toJson(p: DProfile): string {
  const { id: _id, source: _s, ...rest } = p;
  return JSON.stringify({ format: "arcadia-profile-studio/1", ...rest }, null, 2);
}
