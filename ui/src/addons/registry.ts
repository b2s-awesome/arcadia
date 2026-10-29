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
  /** Its own page, with a sidebar entry. */
  load?: () => Promise<{ default: Component }>;
  /** A settings panel, shown under Appearance → Addons. */
  settings?: () => Promise<{ default: Component }>;
  /** Where "Open" goes for an addon without a page. */
  opens?: "appearance" | "music";
  /** Shown on the card: "Listening", "Visual", "Customisation"… */
  group?: string;
}

/** A set of addons installed together. */
export interface Bundle {
  id: string;
  name: string;
  description: string;
  addons: string[];
  /** Addons in the bundle that start unticked. */
  optional?: string[];
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
  {
    id: "see-through",
    name: "Translucent window",
    tagline: "See your desktop through Arcadia, blurred, like a Linux rice.",
    description:
      "Makes Arcadia's background see-through and lets Windows blur whatever is behind it, so the app floats over your desktop the way panels do on a riced Linux setup. Choose Acrylic for real blur of the windows behind, or Mica for a lighter wallpaper-tinted look, then tune the tint and how solid the panel stays.",
    author: "Arcadia",
    official: true,
    version: "1.0.0",
    icon: "sparkle",
    art: ["#0e1a2b", "#6bb3ff", "#d6f0ff"],
    features: [
      "Acrylic: blurs the windows and desktop behind Arcadia (Windows 10 and 11)",
      "Mica and Mica Alt: lighter, wallpaper-tinted backdrops (Windows 11)",
      "Tint colour and strength, and how see-through the panel is",
      "Works with every Look, and with the popped-out Minecraft window",
      "Off in one switch; nothing changes until you restart Arcadia",
    ],
    permissions: [
      { label: "Window", detail: "Makes Arcadia's windows transparent and asks Windows for a blurred backdrop. Switching it on or off takes a restart." },
      { label: "Performance", detail: "Acrylic can make dragging and resizing the window stutter on some Windows builds. Mica doesn't." },
    ],
    settings: () => import("./see-through/Settings.svelte"),
    opens: "appearance",
  },
  {
    id: "streaming",
    name: "Streaming & downloads",
    tagline: "Play and download music from YouTube Music, SoundCloud and more.",
    description:
      "Adds Online and Downloads to Music. Search YouTube Music or SoundCloud, or paste a link to a song, album, playlist or set from anywhere yt-dlp supports. Songs play through Arcadia's own engine (gapless, full seeking) after a quick fetch, and can be saved as M4A, MP3 or FLAC with their covers. Link a playlist and check it for new songs any time. Only download what you're allowed to: subscriber-only and DRM-protected audio isn't fetched.",
    author: "Arcadia",
    official: true,
    version: "1.0.0",
    icon: "download",
    art: ["#2a0f1c", "#ff5f6d", "#ff8a3d"],
    features: [
      "Search YouTube Music and SoundCloud, or paste any link",
      "Fetch-then-play through the native engine: gapless, seek anywhere, the next song pre-fetched",
      "Download as M4A (untouched), MP3 320 or FLAC, with tags and covers",
      "SoundCloud: the artist's original file when they allow downloads",
      "Import playlists, albums and sets; check them for new songs later",
      "A size-limited cache for streamed songs, cleared oldest first",
    ],
    permissions: [
      { label: "Runs yt-dlp", detail: "Fetches yt-dlp from its official GitHub release (checked against the published checksum), or uses one you already have. ffmpeg likewise, only if you pick MP3/FLAC or want covers embedded." },
      { label: "Internet", detail: "Talks to the sites you search or link, through yt-dlp, and downloads their thumbnails." },
      { label: "Files", detail: "Streamed songs go to a cache in Arcadia's folder; downloads go to Music\\Arcadia Downloads (changeable)." },
    ],
    opens: "music",
  },
  {
    id: "full-app-display",
    name: "Full App Display",
    tagline: "What's playing, big, filling the whole screen.",
    description:
      "A full-screen now playing view for when music is the main thing on screen: on a second monitor, at a party, or while you work. It follows your Music layout (Tonal gets the blurred-cover look, Tiled gets panes and a live spectrum visualiser) or you can pick Liquid Glass with a clock. Press F in Music, or the full-screen button in the player.",
    author: "Arcadia",
    official: true,
    version: "1.0.0",
    icon: "fullscreen",
    art: ["#1f1147", "#6a2c91", "#f08a5d"],
    group: "Visual",
    features: [
      "Three looks: Ambient (blurred cover), Tiled (panes + visualiser), Liquid Glass (clock + glass card)",
      "A real spectrum visualiser from the audio engine, only running while it's on screen",
      "The current lyric line under the title, with Lyrics Plus",
      "Clock, up next, cover blur and layout all adjustable",
    ],
    permissions: [{ label: "Window", detail: "Can switch Arcadia's window to full screen while it's open." }],
    settings: () => import("./fad/Settings.svelte"),
    opens: "music",
  },
  {
    id: "lyrics-plus",
    name: "Lyrics Plus",
    tagline: "Synced lyrics, a pop-up lyrics window and a desktop lyrics bar.",
    description:
      "Adds Lyrics to Music. Lyrics come from a .lrc file beside the song, the song's own tags, or LRCLIB (a free, open lyrics database), and scroll in time, word by word when the lyrics have word timings. Fix timings by tapping along, shift them with an offset, pop them out into a small always-on-top glass window, or float one line over your games with the click-through desktop bar.",
    author: "Arcadia",
    official: true,
    version: "1.0.0",
    icon: "lyrics",
    art: ["#0f2b3a", "#2f6f73", "#9ad1d4"],
    group: "Visual",
    features: [
      "Three page styles: Karaoke, Tiled (with timestamps and sources) and Glass",
      "Word-by-word highlighting for enhanced LRC",
      "Pop-up lyrics window and a click-through desktop lyrics bar",
      "Tap-along timing editor; per-song offset",
      "Romanise Korean, Japanese kana, Chinese, Cyrillic and Greek, offline",
      "Translate lyrics into your language (free, no account)",
      "Works offline with your own .lrc files and tags",
    ],
    permissions: [
      { label: "Internet: LRCLIB", detail: "Looks up lyrics on lrclib.net by title, artist, album and length, when nothing is on your PC. Can be turned off." },
      { label: "Internet: MyMemory", detail: "Only if you switch Translate on: sends a song's lyric lines to api.mymemory.translated.net, once per song and language." },
      { label: "Files", detail: "Keeps found lyrics in its own folder. Only writes .lrc files beside your songs if you switch that on." },
      { label: "Windows", detail: "Opens the pop-up lyrics window and the desktop bar (always on top; the bar lets clicks through)." },
    ],
    settings: () => import("./lyrics/Settings.svelte"),
    opens: "music",
  },
  {
    id: "stats",
    name: "Stats",
    tagline: "Your listening, counted on your own PC.",
    description:
      "Adds Stats to Music: how long you've listened, your top artists, songs and albums, when in the day you listen, a half-year activity heatmap, where your music comes from and in what formats, and habits like skips and streaks. Everything is worked out from Arcadia's own play log; nothing is sent anywhere. Its look follows your Music layout, or pick one.",
    author: "Arcadia",
    official: true,
    version: "1.0.0",
    icon: "chart",
    art: ["#141417", "#ff9f6e", "#8b7bff"],
    group: "Visual",
    features: [
      "Four weeks, six months, a year or all time",
      "Top artists, songs and albums with covers",
      "Listening by hour, a 26-week heatmap and a daily goal",
      "Sources and formats, skips, streaks and new finds",
      "Recap cards, terminal tables or glass widgets",
    ],
    permissions: [{ label: "Storage", detail: "Reads the play log Arcadia keeps in its music library (what you played, when, and for how long). It never leaves your PC." }],
    settings: () => import("./stats/Settings.svelte"),
    opens: "music",
  },
  {
    id: "trash-bin",
    name: "Trash Bin + Auto Skip",
    tagline: "Never hear that song again, and skip the intros you always skip.",
    description:
      "Put a song or a whole artist in the trash and Arcadia passes over them whenever it moves on by itself. Auto skip does the same for explicit songs, anything under 30 seconds, live versions or remixes. Choosing a song yourself still plays it.",
    author: "Arcadia",
    official: true,
    version: "1.0.0",
    icon: "skip",
    art: ["#2a0f1c", "#ff5f6d", "#ffc371"],
    group: "Listening",
    features: [
      "Trash songs and artists from any song's menu",
      "Auto skip explicit, short, live or remix tracks",
      "Works with gapless playback: the next song is chosen ahead of time",
      "See and empty the trash any time",
    ],
    permissions: [{ label: "Storage", detail: "Keeps your trash and rules in its own folder. Reads a song's tags to check the explicit mark." }],
    settings: () => import("./listening/TrashSettings.svelte"),
    opens: "music",
  },
  {
    id: "shuffle-plus",
    name: "Shuffle+",
    tagline: "Shuffle that feels random: artists spread out, fewer repeats.",
    description:
      "True random shuffle often plays the same artist three times in a row or the song you heard an hour ago. Shuffle+ adds modes that feel fairer: spread artists through the queue, push recently played songs back, or bring forward the songs you play least. The song playing keeps playing when you switch.",
    author: "Arcadia",
    official: true,
    version: "1.0.0",
    icon: "shuffle",
    art: ["#191b22", "#2b3a55", "#7ee787"],
    group: "Listening",
    features: ["Spread artists", "Fewer repeats", "Favour less played", "True random, as before", "Pick from the shuffle button's menu or the queue"],
    permissions: [{ label: "Library", detail: "Uses your play counts and when songs were last played to order the queue." }],
    settings: () => import("./listening/ShuffleSettings.svelte"),
    opens: "music",
  },
  {
    id: "loopy-loop",
    name: "Loopy Loop + Bookmarks",
    tagline: "Loop any part of a song, and jump back to your favourite moments.",
    description:
      "Set A and B on the seek bar to loop a section exactly (the engine loops it sample by sample, no gap or click from the app). Bookmark moments in songs and play from them later. Keys: [ sets A, ] sets B, \\ stops the loop, M adds a bookmark.",
    author: "Arcadia",
    official: true,
    version: "1.0.0",
    icon: "loop",
    art: ["#3d1d4f", "#ff7b86", "#ffd28a"],
    group: "Listening",
    features: ["Sample-accurate A–B loops", "Loop markers on the seek bar", "Named bookmarks that play from the moment", "Keyboard shortcuts"],
    permissions: [{ label: "Storage", detail: "Keeps your bookmarks in its own folder." }],
    settings: () => import("./listening/LoopSettings.svelte"),
    opens: "music",
  },
  {
    id: "sort-play",
    name: "Sort & Play",
    tagline: "Play any list sorted your way, and tidy the queue.",
    description:
      "Adds a Sort & Play button to songs, albums, artists and playlists: play by most played, least played, recently added, recently played, longest, shortest, title, artist or album shuffle. Queue tools remove duplicates, clear what's played and shuffle only what's still to come.",
    author: "Arcadia",
    official: true,
    version: "1.0.0",
    icon: "sort",
    art: ["#0b1640", "#3d6bff", "#e9e9f2"],
    group: "Listening",
    features: ["Ten ways to sort, then play", "Album shuffle: albums in random order, songs in order", "Queue: remove duplicates, clear played, shuffle the rest"],
    permissions: [{ label: "Library", detail: "Reads play counts and dates to sort." }],
    opens: "music",
  },
  {
    id: "themes",
    name: "Themes & CSS snippets",
    tagline: "Colour themes, one-click tweaks and your own CSS.",
    description:
      "Theme packs recolour the whole app on top of your Look. Snippets are small one-click tweaks (rounder covers, compact rows, thin scrollbars…). And for full control, write your own CSS, for the whole app or just Music, with a live preview. Hold Shift while Arcadia starts to skip your CSS for that run if something goes wrong.",
    author: "Arcadia",
    official: true,
    version: "1.0.0",
    icon: "palette",
    art: ["#1e1e2e", "#fab387", "#cba6f7"],
    group: "Customisation",
    features: ["Six theme packs, light and dark", "Built-in snippets, and save your own", "Your CSS for the whole app or Music only", "Safe mode: hold Shift at startup"],
    permissions: [{ label: "Appearance", detail: "Adds your CSS to Arcadia's pages. CSS can change how anything looks, but can't run code." }],
    settings: () => import("./themes/Settings.svelte"),
    opens: "appearance",
  },
];

CATALOG.push({
  id: "liquid-glass",
  name: "Liquid Glass",
  tagline: "Frosted glass panels with lit edges, over your wallpaper or desktop.",
  description:
    "Turns Arcadia's window, sidebar, top bar, sections and the music player into frosted glass, in the style of iOS 27: the blur, a lit edge and a soft sheen. Pick frosted white or smoked dark glass, how much blur and how solid it is, and whether it covers the whole app or just Music. It looks best over a wallpaper, or with the Translucent window addon so your real desktop is behind it.",
  author: "Arcadia",
  official: true,
  version: "1.0.0",
  icon: "sparkle",
  art: ["#3b1f63", "#ff8fa8", "#fff1c1"],
  group: "Customisation",
  features: ["Frosted or smoked glass", "Blur, tint, edge light and sheen are all adjustable", "Whole app or Music only", "Pairs with Translucent window for real see-through glass"],
  permissions: [{ label: "Performance", detail: "Blurring large areas costs some graphics power. Lower the blur if scrolling feels heavy on an older PC." }],
  settings: () => import("./glass/Settings.svelte"),
  opens: "appearance",
});

export const BUNDLES: Bundle[] = [
  {
    id: "spice-rack",
    name: "Spice rack",
    description: "Everything people install Spicetify for, made native. Untick anything you don't want.",
    addons: ["full-app-display", "lyrics-plus", "stats", "trash-bin", "shuffle-plus", "loopy-loop", "sort-play", "themes"],
    optional: ["themes"],
  },
  { id: "listening-tools", name: "Listening tools", description: "Control what plays and how: skip rules, shuffle, loops and queue sorting.", addons: ["trash-bin", "shuffle-plus", "loopy-loop", "sort-play"] },
  { id: "eye-candy", name: "Eye candy", description: "The good-looking ones: full-screen display, lyrics and stats.", addons: ["full-app-display", "lyrics-plus", "stats"] },
  { id: "rice-kit", name: "Rice kit", description: "Make the whole app yours: a see-through window, Liquid Glass, theme packs and your own CSS.", addons: ["see-through", "liquid-glass", "themes"] },
];

export const addonById = (id: string) => CATALOG.find((a) => a.id === id) ?? null;

/** Is this addon installed? (Reactive where `installed` is.) */
export const has = (installed: string[], id: string) => installed.includes(id);
