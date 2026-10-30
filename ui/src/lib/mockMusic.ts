// A pretend music library and player, for designing the Music page in a
// browser. Artists, albums and covers are all made up.
import type { DownloadJob, ListenStats, Lyrics, MusicPlaylist, MusicStatus, OnlineItem, RepeatMode, ShuffleMode, StreamingConfig, SyncedSource, Track } from "./types";

type Emit = (event: string, payload: unknown) => void;

const ALBUMS: { title: string; artist: string; year: number; genre: string; colors: [string, string, string]; songs: string[] }[] = [
  { title: "Night Bus Home", artist: "Ivy Lantern", year: 2024, genre: "Indie pop", colors: ["#1f1147", "#6a2c91", "#f08a5d"], songs: ["Porch Static", "Lantern Lights", "Top Deck", "Cold Chips at 2am", "Ring the Bell Twice", "Northern Line Lullaby", "Bus Shelter Choir", "Last Stop Lovers"] },
  { title: "Glass Harbour", artist: "Nell Okafor", year: 2023, genre: "Soul", colors: ["#0f2b3a", "#2f6f73", "#9ad1d4"], songs: ["Undertow Radio", "Saltwater Letters", "Lighthouse Hours", "Harbour Glass", "Tide Tables"] },
  { title: "Paper Moons", artist: "Kai Ren", year: 2022, genre: "Electronic", colors: ["#ff8fa8", "#ffb38a", "#fff1c1"], songs: ["Origami Weather", "Fold Lines", "Paper Moon", "Crane Season", "Tissue Thin", "Kites at Dusk"] },
  { title: "Soft Machinery", artist: "Juno Vale", year: 2021, genre: "Synthpop", colors: ["#191b22", "#2b3a55", "#7ee787"], songs: ["Low Battery Love", "Firmware", "Cogs in Spring", "Soft Reset", "Quiet Engine"] },
  { title: "Static Bloom", artist: "The Low Orbit", year: 2020, genre: "Shoegaze", colors: ["#0b1640", "#3d6bff", "#e9e9f2"], songs: ["Satellite Kiss", "Static Bloom", "Orbit Decay", "Radio Silence", "Blue Noise", "Re-entry", "Afterglow"] },
  { title: "Copper Season", artist: "Marlow & Finch", year: 2019, genre: "Folk", colors: ["#3b2414", "#fe8019", "#fabd2f"], songs: ["Copper Leaves", "Woodsmoke", "The Long Field", "Harvest Moon Motel"] },
  { title: "Sunburn Postcards", artist: "Delphine Rae", year: 2024, genre: "Pop", colors: ["#ff5f6d", "#ffc371", "#ffe066"], songs: ["Postcard From Nowhere", "SPF Heart", "Pier Lights", "Coastal Drive", "Sunburn"] },
  { title: "Cinema Exit", artist: "Halo Theory", year: 2018, genre: "Alt rock", colors: ["#3d1d4f", "#ff7b86", "#0b0b0d"], songs: ["Credits Roll", "Popcorn Ceiling", "Exit Sign Glow", "Matinee", "Projector Heart", "Final Reel"] },
];

function cover(c: [string, string, string], i: number): string {
  const shapes = [
    `<circle cx="180" cy="70" r="46" fill="${c[2]}" opacity=".85"/><path d="M0 200 Q64 150 128 190 T256 170 V256 H0z" fill="${c[0]}" opacity=".8"/>`,
    `<circle cx="80" cy="170" r="60" fill="${c[2]}" opacity=".9"/>`,
    `<rect x="40" y="40" width="176" height="176" rx="88" fill="none" stroke="${c[2]}" stroke-width="22"/>`,
    `<path d="M0 0 L256 256 M64 0 L256 192 M0 64 L192 256" stroke="${c[2]}" stroke-width="14" opacity=".7"/>`,
  ];
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${c[0]}"/><stop offset="1" stop-color="${c[1]}"/></linearGradient></defs><rect width="256" height="256" fill="url(#g)"/>${shapes[i % shapes.length]}</svg>`;
  return `data:image/svg+xml;base64,${btoa(svg)}`;
}

const DAY = 86400;
const now = Math.floor(Date.now() / 1000);
let nextId = 1;
const tracks: Track[] = ALBUMS.flatMap((a, ai) =>
  a.songs.map((title, i) => {
    const id = nextId++;
    return {
      id,
      path: `C:\\Users\\you\\Music\\${a.artist}\\${a.title}\\${String(i + 1).padStart(2, "0")} ${title}.flac`,
      title,
      artist: a.artist,
      albumArtist: a.artist,
      album: a.title,
      genre: a.genre,
      year: a.year,
      trackNo: i + 1,
      discNo: 1,
      duration: 150 + ((id * 37) % 140),
      sampleRate: ai % 3 === 0 ? 96000 : 44100,
      bitDepth: ai % 3 === 0 ? 24 : 16,
      channels: 2,
      bitrate: ai % 3 === 0 ? 2800 : 900,
      format: ai % 4 === 3 ? "MP3" : "FLAC",
      art: cover(a.colors, ai),
      artColor: a.colors[ai === 3 ? 2 : 1 + (ai % 2)],
      addedAt: now - ai * 9 * DAY - i * 60,
      playCount: (id * 13) % 23,
      lastPlayed: ai < 4 ? now - ai * DAY - i * 300 : null,
      favourite: id % 7 === 0,
      origin: "local" as const,
      sourceUrl: null,
      explicit: id % 11 === 0,
    };
  }),
);

let playlists: (MusicPlaylist & { items: number[] })[] = [
  { id: 1, name: "Late night", count: 0, duration: 0, updated: now - 3600, arts: [], items: [2, 3, 10, 22, 26, 31] },
  { id: 2, name: "Revision", count: 0, duration: 0, updated: now - DAY, arts: [], items: [14, 15, 16, 17, 18, 19, 20] },
  { id: 3, name: "Gym 140 bpm+", count: 0, duration: 0, updated: now - 4 * DAY, arts: [], items: [35, 36, 37, 40, 41] },
];
let folders = ["C:\\Users\\you\\Music"];

const st: { list: number[]; original: number[] | null; pos: number | null; playing: boolean; t: number; volume: number; shuffle: boolean; repeat: RepeatMode; mode: ShuffleMode; ab: [number, number] | null } = {
  list: [1, 2, 3, 4, 5, 6, 7, 8],
  original: null,
  pos: 1,
  playing: false,
  t: 74,
  volume: 0.8,
  shuffle: false,
  repeat: "off",
  mode: "random",
  ab: null,
};

const byId = (id: number) => tracks.find((t) => t.id === id)!;
const cur = () => (st.pos === null ? null : byId(st.list[st.pos]));

function status(): MusicStatus {
  const t = cur();
  return {
    track: t,
    playing: st.playing && !!t,
    pos: st.t,
    duration: t?.duration ?? 0,
    volume: st.volume,
    shuffle: st.shuffle,
    repeat: st.repeat,
    queueIndex: st.pos,
    queueLength: st.list.length,
    output: { device: "Speakers (Realtek Audio)", rate: 48000, channels: 2 },
    outputError: null,
    device: null,
    loading: false,
    shuffleMode: st.mode,
    loopAb: st.ab,
  };
}

let ticking: ReturnType<typeof setInterval> | null = null;
function tick(emit: Emit) {
  if (ticking) return;
  ticking = setInterval(() => {
    const t = cur();
    if (!st.playing || !t) return;
    st.t += 0.25;
    if (st.ab && st.t >= st.ab[1]) st.t = st.ab[0];
    if (st.t >= t.duration) {
      st.t = 0;
      if (st.repeat !== "one") {
        if (st.pos! + 1 < st.list.length) st.pos!++;
        else if (st.repeat === "all") st.pos = 0;
        else st.playing = false;
      }
      emit("music-status", status());
    } else emit("music-position", st.t);
  }, 250);
}

function shuffleList(v: number[]) {
  for (let i = v.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [v[i], v[j]] = [v[j], v[i]];
  }
  return v;
}

function plView() {
  return playlists.map(({ items, ...p }) => {
    const ts = items.map(byId).filter(Boolean);
    return { ...p, count: ts.length, duration: ts.reduce((a, t) => a + t.duration, 0), arts: [...new Set(ts.map((t) => t.art!))].slice(0, 4) };
  });
}

// ── Streaming addon, pretend ───────────────────────────────
const ONLINE: OnlineItem[] = [
  { url: "https://music.youtube.com/watch?v=aaa1", title: "Neon Tide", artist: "Mira Sol", album: "Night Swim", duration: 214, thumbnail: null, site: "youtube" },
  { url: "https://music.youtube.com/watch?v=aaa2", title: "Glass Garden", artist: "Mira Sol", album: "Night Swim", duration: 198, thumbnail: null, site: "youtube" },
  { url: "https://music.youtube.com/watch?v=aaa3", title: "Paper Planes (Live)", artist: "The Kites", album: null, duration: 262, thumbnail: null, site: "youtube" },
  { url: "https://soundcloud.com/ollie/pocket-sun", title: "Pocket Sun", artist: "ollie beats", album: null, duration: 176, thumbnail: null, site: "soundcloud" },
  { url: "https://soundcloud.com/ollie/rain-on-tin", title: "Rain on Tin", artist: "ollie beats", album: null, duration: 151, thumbnail: null, site: "soundcloud" },
];
let cfg: StreamingConfig = { format: "m4a", scOriginal: true, downloadDir: "", cacheLimitMb: 1024, ytdlpPath: "", ffmpegPath: "" };
let haveYtdlp = false;
let haveFfmpeg = false;
let jobs: DownloadJob[] = [];
let jobId = 1;
const synced: SyncedSource[] = [];

function remoteTrack(i: OnlineItem): number {
  const ex = tracks.find((t) => t.sourceUrl === i.url);
  if (ex) return ex.id;
  const id = nextId++;
  const colors: [string, string, string] = i.site === "soundcloud" ? ["#3a1406", "#ff5500", "#ffb38a"] : ["#1a0a12", "#ff0033", "#ffd1dc"];
  tracks.push({
    id, path: `remote:${i.url}`, title: i.title, artist: i.artist, albumArtist: i.artist, album: i.album ?? "Singles", genre: "", year: null, trackNo: null, discNo: null,
    duration: i.duration ?? 200, sampleRate: 44100, bitDepth: null, channels: 2, bitrate: 128, format: "M4A", art: cover(colors, id), artColor: colors[1],
    addedAt: Math.floor(Date.now() / 1000), playCount: 0, lastPlayed: null, favourite: false, origin: "stream", sourceUrl: i.url, explicit: null,
  });
  return id;
}

async function mockStreaming(cmd: string, args: Record<string, unknown>, emit: Emit): Promise<unknown> {
  await new Promise((r) => setTimeout(r, 250));
  switch (cmd) {
    case "streaming_status":
      return {
        ytdlp: haveYtdlp ? { path: "C:\\Users\\you\\AppData\\Local\\Arcadia\\addons\\streaming\\bin\\yt-dlp.exe", version: "2026.10.21", managed: true } : null,
        ffmpeg: haveFfmpeg ? { path: "C:\\…\\bin\\ffmpeg.exe", version: "N-117000-g1b2c3d4", managed: true } : null,
        config: { ...cfg }, downloadDir: cfg.downloadDir || "C:\\Users\\you\\Music\\Arcadia Downloads", cacheBytes: 48_000_000, windows: true,
      };
    case "streaming_install": {
      const tool = args.tool as "yt-dlp" | "ffmpeg";
      const total = tool === "yt-dlp" ? 18_000_000 : 87_000_000;
      for (let i = 1; i <= 8; i++) {
        emit("streaming-install", { tool, done: (total * i) / 8, total, stage: "downloading" });
        await new Promise((r) => setTimeout(r, 120));
      }
      if (tool === "yt-dlp") haveYtdlp = true; else haveFfmpeg = true;
      return;
    }
    case "streaming_update": return "yt-dlp is up to date (2026.10.21)";
    case "streaming_set_config": cfg = args.config as StreamingConfig; return;
    case "streaming_clear_cache": return 48_000_000;
    case "streaming_search": {
      if (!haveYtdlp) throw "Streaming needs yt-dlp. Get it under Music → Downloads → Setup.";
      const q = String(args.query).toLowerCase();
      const items = ONLINE.filter((i) => (args.source === "soundcloud") === (i.site === "soundcloud"));
      return { playlist: null, items: q ? items : [] };
    }
    case "streaming_resolve":
      if (!haveYtdlp) throw "Streaming needs yt-dlp. Get it under Music → Downloads → Setup.";
      return { playlist: "Night Swim (album)", items: ONLINE.slice(0, 2) };
    case "streaming_tracks": return (args.items as OnlineItem[]).map(remoteTrack);
    case "streaming_jobs": return jobs;
    case "streaming_download": {
      const ids = args.ids as number[];
      for (const id of ids) {
        const t = tracks.find((x) => x.id === id);
        if (!t?.sourceUrl) continue;
        const job: DownloadJob = { id: jobId++, trackId: id, title: t.title, artist: t.artist, url: t.sourceUrl, state: "queued", progress: { percent: 0, speed: "", eta: "", stage: "" }, error: null, path: null };
        jobs.push(job);
        (async () => {
          job.state = "running";
          for (let p = 0; p <= 100; p += 12.5) {
            job.progress = { percent: p, speed: "2.4MiB/s", eta: `00:0${Math.max(0, 8 - p / 12.5)}`, stage: "downloading" };
            emit("streaming-jobs", structuredClone(jobs));
            await new Promise((r) => setTimeout(r, 300));
          }
          job.state = "done";
          job.path = `C:\\Users\\you\\Music\\Arcadia Downloads\\${t.artist}\\${t.album}\\${t.title}.m4a`;
          t.origin = "download";
          t.path = job.path;
          emit("streaming-jobs", structuredClone(jobs));
          emit("music-library-changed", null);
        })();
      }
      emit("streaming-jobs", structuredClone(jobs));
      return ids.length;
    }
    case "streaming_cancel": { const j = jobs.find((x) => x.id === args.id); if (j) j.state = "cancelled"; emit("streaming-jobs", structuredClone(jobs)); return; }
    case "streaming_retry": return;
    case "streaming_clear_finished": jobs = jobs.filter((j) => j.state === "queued" || j.state === "running"); emit("streaming-jobs", structuredClone(jobs)); return;
    case "streaming_sources": return [...synced];
    case "streaming_import": {
      const ids = ONLINE.slice(0, 2).map(remoteTrack);
      const id = Math.max(0, ...playlists.map((p) => p.id)) + 1;
      playlists.unshift({ id, name: (args.name as string) || "Night Swim (album)", count: 0, duration: 0, updated: Math.floor(Date.now() / 1000), arts: [], items: ids });
      synced.push({ playlistId: id, url: args.url as string, download: !!args.download, lastSync: Math.floor(Date.now() / 1000) });
      return id;
    }
    case "streaming_sync": return 0;
    case "streaming_unlink": return;
  }
  throw new Error(`mock: unknown ${cmd}`);
}

// Made-up lyrics: the same verse for every song, timed every four seconds.
const VERSE = [
  "Every window holds a little sun", "I count the stops and lose the count", "You said the city never sleeps", "But tonight it is humming just for one",
  "Hold the rail, the corner is coming", "", "Lantern lights along the river", "Every face a borrowed glow", "Tell me where the night bus goes", "Somewhere only dreamers know",
];
function mockLyrics(id: number): Lyrics | null {
  if (id % 9 === 0) return null;
  const lines = [] as Lyrics["lines"];
  for (let r = 0; r < 6; r++) {
    VERSE.forEach((text, i) => {
      const t = 6 + (r * VERSE.length + i) * 4;
      let w = t;
      const words = text ? text.split(" ").map((x) => ({ t: (w += 0.32) - 0.32, text: x + " " })) : [];
      lines.push({ t, text, words: id % 2 ? words : [] });
    });
  }
  return { synced: true, words: id % 2 === 1, source: id % 3 ? "lrclib" : "file", lines, instrumental: false };
}

function mockStats(since: number | null): ListenStats {
  const k = since ? Math.max(0.15, (now - since) / (28 * DAY)) : 6;
  const sc = (x: number) => Math.round(x * k);
  const top = [...tracks].sort((a, b) => b.playCount - a.playCount);
  const artists = ALBUMS.map((a, i) => ({ name: a.artist, plays: sc(141 - i * 17), listened: sc((141 - i * 17) * 210), art: tracks.find((t) => t.artist === a.artist)!.art, artColor: tracks.find((t) => t.artist === a.artist)!.artColor }));
  return {
    listened: sc(41 * 3600), previous: since ? sc(35 * 3600) : null, plays: sc(612), skips: sc(71), songs: Math.min(tracks.length, sc(38)), newSongs: sc(9),
    topArtists: artists,
    topSongs: top.slice(0, 10).map((t, i) => ({ id: t.id, title: t.title, artist: t.artist, album: t.album, plays: sc(34 - i * 2), skips: (i * 3) % 7, avg: t.duration * (0.95 - i * 0.03), art: t.art, artColor: t.artColor })),
    topAlbums: ALBUMS.slice(0, 6).map((a, i) => ({ album: a.title, artist: a.artist, plays: sc(96 - i * 11), art: tracks.find((t) => t.album === a.title)!.art, artColor: tracks.find((t) => t.album === a.title)!.artColor })),
    hours: [3, 2, 1, 1, 1, 2, 5, 9, 12, 10, 8, 9, 14, 12, 10, 11, 15, 22, 30, 41, 55, 62, 48, 20].map((v) => sc(v * 120)),
    days: Array.from({ length: 182 }, (_, i) => (i % 11 === 3 ? 0 : Math.round(((Math.sin(i * 0.37) + Math.sin(i * 1.3) + 2) / 4) * 7200))),
    sources: [{ key: "local", listened: sc(30 * 3600) }, { key: "stream", listened: sc(8 * 3600) }, { key: "download", listened: sc(3 * 3600) }],
    formats: [{ key: "FLAC", listened: sc(19 * 3600) }, { key: "M4A / AAC", listened: sc(15 * 3600) }, { key: "MP3", listened: sc(7 * 3600) }],
    streak: Math.min(9, sc(9)), busiestDay: { at: now - 3 * DAY, listened: 5.2 * 3600 }, today: 74 * 60,
  };
}

export async function mockMusic(cmd: string, args: Record<string, unknown>, emit: Emit): Promise<unknown> {
  if (cmd.startsWith("streaming_")) return mockStreaming(cmd, args, emit);
  if (cmd.startsWith("lyrics_")) {
    switch (cmd) {
      case "lyrics_get": return mockLyrics(args.id as number);
      case "lyrics_translate": await new Promise((r) => setTimeout(r, 400)); return (args.lines as string[]).map((l) => (l ? `(${args.to}) ${l}` : ""));
      case "lyrics_save": return { ...(await import("../addons/lyrics/lrc")).parseLrc(args.lrc as string), source: "yours" };
      default: return;
    }
  }
  tick(emit);
  const changed = () => emit("music-status", status());
  switch (cmd) {
    case "music_library": return structuredClone(tracks);
    case "music_duplicates": {
      // Pretend the first songs have copies, at a few certainties.
      const pick = (i: number) => tracks[i % tracks.length].id;
      const pair = (a: number, b: number, c: number, extra: Partial<Record<string, unknown>> = {}) => ({ a, b, certainty: c, title: c / 100, artist: 1, length: 0.95, file: 0.8, sameFileSize: c >= 99, versionClash: false, ...extra });
      return [
        { ids: [pick(0), pick(1)], certainty: 99.2, pairs: [pair(pick(0), pick(1), 99.2)] },
        { ids: [pick(2), pick(3), pick(4)], certainty: 91.4, pairs: [pair(pick(2), pick(3), 91.4), pair(pick(3), pick(4), 86.8)] },
        { ids: [pick(5), pick(6)], certainty: 74.5, pairs: [pair(pick(5), pick(6), 74.5, { artist: 0.5, length: 0.7 })] },
      ].filter((g) => g.certainty >= (args.min as number));
    }
    case "music_recycle": return (args.ids as number[]).length;
    case "music_folders": return [...folders];
    case "music_set_folders": folders = args.folders as string[]; return;
    case "music_scan": {
      for (let i = 0; i <= 10; i++) {
        emit("music-scan", { done: i * 6, total: 60, current: tracks[(i * 5) % tracks.length].title + ".flac" });
        await new Promise((r) => setTimeout(r, 120));
      }
      return { added: 0, updated: 0, removed: 0, total: tracks.length, missingFolders: [], errors: [] };
    }
    case "music_favourite": byId(args.id as number).favourite = args.on as boolean; return;
    case "music_status": return status();
    case "music_play": {
      const ids = args.ids as number[];
      const start = (args.start as number) ?? 0;
      if (st.shuffle) {
        const first = ids[start];
        st.original = [...ids];
        st.list = [first, ...shuffleList(ids.filter((_, i) => i !== start))];
        st.pos = 0;
      } else {
        st.list = [...ids];
        st.pos = start;
      }
      st.t = 0;
      st.playing = true;
      changed();
      return;
    }
    case "music_pause": st.playing = args.paused == null ? !st.playing : !(args.paused as boolean); changed(); return;
    case "music_next": if (st.pos !== null && st.pos + 1 < st.list.length) { st.pos++; st.t = 0; } changed(); return;
    case "music_prev": if (st.t > 3) st.t = 0; else if (st.pos) { st.pos--; st.t = 0; } changed(); return;
    case "music_seek": st.t = args.secs as number; emit("music-position", st.t); return;
    case "music_volume": st.volume = args.volume as number; return;
    case "music_shuffle": {
      const on = args.on as boolean;
      const c = cur();
      if (on && !st.shuffle && c) {
        st.original = [...st.list];
        st.list = [c.id, ...shuffleList(st.list.filter((_, i) => i !== st.pos))];
        st.pos = 0;
      } else if (!on && st.shuffle && st.original) {
        st.list = st.original;
        st.pos = c ? st.list.indexOf(c.id) : 0;
      }
      st.shuffle = on;
      changed();
      return;
    }
    case "music_repeat": st.repeat = args.mode as RepeatMode; changed(); return;
    case "music_devices": return ["Speakers (Realtek Audio)", "Headphones (WH-1000XM5)", "Digital Output (S/PDIF)"];
    case "music_set_device": return;
    case "music_queue": return { index: st.pos, tracks: st.list.map(byId) };
    case "music_queue_add": {
      const ids = args.ids as number[];
      const at = args.next ? (st.pos ?? -1) + 1 : st.list.length;
      st.list.splice(at, 0, ...ids);
      if (st.pos === null) st.pos = 0;
      emit("music-queue-changed", null);
      changed();
      return;
    }
    case "music_queue_remove": {
      const i = args.index as number;
      st.list.splice(i, 1);
      if (st.pos !== null && i < st.pos) st.pos--;
      if (!st.list.length) st.pos = null;
      emit("music-queue-changed", null);
      changed();
      return;
    }
    case "music_queue_move": {
      const [from, to] = [args.from as number, args.to as number];
      const [id] = st.list.splice(from, 1);
      st.list.splice(to, 0, id);
      if (st.pos === from) st.pos = to;
      else if (st.pos !== null && from < st.pos && to >= st.pos) st.pos--;
      else if (st.pos !== null && from > st.pos && to <= st.pos) st.pos++;
      emit("music-queue-changed", null);
      changed();
      return;
    }
    case "music_queue_jump": st.pos = args.index as number; st.t = 0; st.playing = true; emit("music-queue-changed", null); changed(); return;
    case "music_queue_clear": { const c = cur(); st.list = c ? [c.id] : []; st.pos = c ? 0 : null; emit("music-queue-changed", null); changed(); return; }
    case "music_playlists": return plView();
    case "music_playlist_items": return [...(playlists.find((p) => p.id === args.id)?.items ?? [])];
    case "music_playlist_create": {
      const id = Math.max(0, ...playlists.map((p) => p.id)) + 1;
      playlists.unshift({ id, name: (args.name as string) || "New playlist", count: 0, duration: 0, updated: Math.floor(Date.now() / 1000), arts: [], items: [...(args.ids as number[])] });
      return id;
    }
    case "music_playlist_rename": { const p = playlists.find((x) => x.id === args.id); if (p) p.name = args.name as string; return; }
    case "music_playlist_delete": playlists = playlists.filter((p) => p.id !== args.id); return;
    case "music_playlist_set": { const p = playlists.find((x) => x.id === args.id); if (p) p.items = [...(args.ids as number[])]; return; }
    case "music_playlist_add": { const p = playlists.find((x) => x.id === args.id); if (p) p.items.push(...(args.ids as number[])); return; }
    case "music_reload_rules": return;
    case "music_shuffle_mode": st.mode = args.mode as ShuffleMode; changed(); return;
    case "music_loop": st.ab = args.a != null && args.b != null ? [args.a as number, args.b as number] : null; changed(); return;
    case "music_spectrum": {
      const n = args.bands as number;
      const t = performance.now() / 1000;
      return Array.from({ length: n }, (_, i) => (st.playing ? Math.max(0, Math.min(1, 0.85 * Math.exp((-i / n) * 1.6) * (0.55 + 0.45 * Math.abs(Math.sin(t * (2.1 + (i % 5) * 0.7) + i * 1.3))))) : 0));
    }
    case "music_spectrum_stop": return;
    case "music_queue_tool": {
      const c = st.pos === null ? null : st.list[st.pos];
      if (args.tool === "dedupe") { const seen = new Set<number>(); st.list = st.list.filter((x, i) => i === st.pos || (x !== c && !seen.has(x) && (seen.add(x), true))); }
      if (args.tool === "clearPlayed" && st.pos) { st.list = st.list.slice(st.pos); }
      if (args.tool === "shuffleRest" && st.pos !== null) st.list = [...st.list.slice(0, st.pos + 1), ...shuffleList(st.list.slice(st.pos + 1))];
      st.pos = c === null ? null : st.list.indexOf(c);
      emit("music-queue-changed", null); changed(); return;
    }
    case "music_stats": return mockStats(args.since as number | null);
  }
  throw new Error(`mock: unknown ${cmd}`);
}
