// A pretend music library and player, for designing the Music page in a
// browser. Artists, albums and covers are all made up.
import type { MusicPlaylist, MusicStatus, RepeatMode, Track } from "./types";

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
    };
  }),
);

let playlists: (MusicPlaylist & { items: number[] })[] = [
  { id: 1, name: "Late night", count: 0, duration: 0, updated: now - 3600, arts: [], items: [2, 3, 10, 22, 26, 31] },
  { id: 2, name: "Revision", count: 0, duration: 0, updated: now - DAY, arts: [], items: [14, 15, 16, 17, 18, 19, 20] },
  { id: 3, name: "Gym 140 bpm+", count: 0, duration: 0, updated: now - 4 * DAY, arts: [], items: [35, 36, 37, 40, 41] },
];
let folders = ["C:\\Users\\you\\Music"];

const st: { list: number[]; original: number[] | null; pos: number | null; playing: boolean; t: number; volume: number; shuffle: boolean; repeat: RepeatMode } = {
  list: [1, 2, 3, 4, 5, 6, 7, 8],
  original: null,
  pos: 1,
  playing: false,
  t: 74,
  volume: 0.8,
  shuffle: false,
  repeat: "off",
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
  };
}

let ticking: ReturnType<typeof setInterval> | null = null;
function tick(emit: Emit) {
  if (ticking) return;
  ticking = setInterval(() => {
    const t = cur();
    if (!st.playing || !t) return;
    st.t += 0.25;
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

export async function mockMusic(cmd: string, args: Record<string, unknown>, emit: Emit): Promise<unknown> {
  tick(emit);
  const changed = () => emit("music-status", status());
  switch (cmd) {
    case "music_library": return structuredClone(tracks);
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
  }
  throw new Error(`mock: unknown ${cmd}`);
}
