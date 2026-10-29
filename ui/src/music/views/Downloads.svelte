<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "../../components/Icon.svelte";
  import Toggle from "../../components/Toggle.svelte";
  import { pickFile, pickFolder, streaming } from "../../lib/api";
  import { toast } from "../../lib/state.svelte";
  import type { DownloadFormat, StreamingConfig } from "../../lib/types";
  import { go, ms, reloadLibrary, reloadPlaylists } from "../store.svelte";
  import { bytes, initStreaming, install, refresh, sx } from "../streaming.svelte";

  onMount(async () => {
    await initStreaming();
    refresh();
  });

  const cfg = $derived(sx.status?.config ?? null);
  async function save(patch: Partial<StreamingConfig>) {
    if (!sx.status) return;
    const next = { ...sx.status.config, ...patch };
    sx.status.config = next;
    try {
      await streaming.setConfig(next);
      if ("downloadDir" in patch || "ytdlpPath" in patch || "ffmpegPath" in patch) refresh();
    } catch (e) {
      toast(String(e), "error");
    }
  }

  const formats: { id: DownloadFormat; name: string; tag?: string; what: string; changes: string; ffmpeg: boolean }[] = [
    { id: "m4a", name: "M4A", tag: "Recommended", ffmpeg: false,
      what: "Exactly what the site sends (AAC), saved untouched.",
      changes: "Best quality and smallest files. If a site only offers MP3, you get that MP3 as-is." },
    { id: "mp3", name: "MP3 320", ffmpeg: true,
      what: "Re-encoded so it plays on anything: old car stereos, MP3 players.",
      changes: "A tiny, mostly inaudible quality loss from re-encoding; files about 1.5–2× bigger." },
    { id: "flac", name: "FLAC", ffmpeg: true,
      what: "The same sound, stored losslessly.",
      changes: "Sounds identical to M4A (it can't add detail the source never had) but 3–5× bigger. Only if your gear needs FLAC." },
  ];
  const hasFfmpeg = $derived(!!sx.status?.ffmpeg);

  async function chooseDir() {
    const d = await pickFolder();
    if (d) save({ downloadDir: d });
  }
  async function chooseTool(tool: "ytdlpPath" | "ffmpegPath") {
    const p = await pickFile([{ name: "Programs", extensions: ["exe"] }]);
    if (p) save({ [tool]: p } as Partial<StreamingConfig>);
  }
  async function update() {
    try {
      toast(await streaming.update());
      refresh();
    } catch (e) {
      toast(String(e), "error");
    }
  }
  async function clearCache() {
    try {
      const freed = await streaming.clearCache();
      toast(`Freed ${bytes(freed)}`);
      refresh();
      reloadLibrary();
    } catch (e) {
      toast(String(e), "error");
    }
  }
  async function sync(pid: number) {
    try {
      const n = await streaming.sync(pid);
      toast(n ? `${n} new ${n === 1 ? "song" : "songs"} added` : "Nothing new");
      await Promise.all([reloadPlaylists(), reloadLibrary(), refresh()]);
    } catch (e) {
      toast(String(e), "error");
    }
  }
  async function unlink(pid: number) {
    await streaming.unlink(pid);
    refresh();
  }
  const run = (p: Promise<unknown>) => p.catch((e) => toast(String(e), "error"));
  const active = $derived(sx.jobs.filter((j) => j.state === "queued" || j.state === "running").length);
  const finished = $derived(sx.jobs.some((j) => j.state !== "queued" && j.state !== "running"));
  const plName = (id: number) => ms.playlists.find((p) => p.id === id)?.name ?? "Deleted playlist";
  const ago = (t: number) => {
    const m = Math.round((Date.now() / 1000 - t) / 60);
    return m < 1 ? "just now" : m < 60 ? `${m} min ago` : m < 1440 ? `${Math.round(m / 60)} h ago` : new Date(t * 1000).toLocaleDateString();
  };
  const cacheLimits = [256, 512, 1024, 2048, 4096, 10240];
</script>

<div class="dl">
  <header class="bar"><h2>Downloads</h2>{#if active}<span class="pill live"><span class="dot"></span>{active} going</span>{/if}</header>

  {#if sx.jobs.length}
    <section class="card">
      <div class="row spread"><h3>Queue</h3>{#if finished}<button class="ghost small" onclick={() => run(streaming.clearFinished())}>Clear finished</button>{/if}</div>
      <ul class="jobs">
        {#each [...sx.jobs].reverse() as j (j.id)}
          <li class={j.state}>
            <div class="jm">
              <span class="t">{j.title}</span>
              <span class="s">{j.artist}{j.state === "running" && j.progress.speed ? ` · ${j.progress.speed}` : ""}{j.state === "running" && j.progress.eta ? ` · ${j.progress.eta} left` : ""}{j.state === "running" && j.progress.stage && j.progress.stage !== "downloading" ? ` · ${j.progress.stage}` : ""}</span>
              {#if j.state === "running" || j.state === "queued"}
                <div class="track"><div class="fill" style:width="{j.progress.percent}%"></div></div>
              {:else if j.state === "failed"}
                <span class="err">{j.error}</span>
              {:else if j.state === "done" && j.path}
                <span class="path" title={j.path}>{j.path}</span>
              {/if}
            </div>
            <span class="state">{j.state === "running" ? `${Math.round(j.progress.percent)}%` : j.state === "queued" ? "Waiting" : j.state === "done" ? "Done" : j.state === "failed" ? "Failed" : "Cancelled"}</span>
            {#if j.state === "running" || j.state === "queued"}
              <button class="ic" aria-label="Cancel {j.title}" onclick={() => run(streaming.cancel(j.id))}><Icon name="close" size={15} /></button>
            {:else if j.state === "failed" || j.state === "cancelled"}
              <button class="ic" aria-label="Try {j.title} again" onclick={() => run(streaming.retry(j.id))}><Icon name="refresh" size={15} /></button>
            {:else}
              <span></span>
            {/if}
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  <section class="card">
    <h3>Setup</h3>
    <div class="tool">
      <div>
        <div class="label">yt-dlp {#if sx.status?.ytdlp}<span class="pill live"><span class="dot"></span>{sx.status.ytdlp.version ?? "ready"}</span>{:else}<span class="pill warn">needed</span>{/if}</div>
        <div class="desc">Finds and fetches the music. Sites change often, so update it if things stop working.</div>
        {#if sx.status?.ytdlp}<div class="path">{sx.status.ytdlp.path}</div>{/if}
      </div>
      {#if sx.installing?.tool === "yt-dlp"}
        <span class="muted">{sx.installing.total ? `${Math.round((sx.installing.done / sx.installing.total) * 100)}%` : "…"}</span>
      {:else if !sx.status?.ytdlp}
        <button class="go" onclick={() => install("yt-dlp")}>Get yt-dlp · 18 MB</button>
      {:else if sx.status.ytdlp.managed}
        <button onclick={update}>Update</button>
      {/if}
    </div>
    <div class="tool">
      <div>
        <div class="label">ffmpeg {#if sx.status?.ffmpeg}<span class="pill live"><span class="dot"></span>ready</span>{:else}<span class="pill">optional</span>{/if}</div>
        <div class="desc">Only needed for MP3 or FLAC, for embedding covers and tags in downloads, and for the odd song a site only offers as Opus.</div>
        {#if sx.status?.ffmpeg}<div class="path">{sx.status.ffmpeg.path}</div>{/if}
      </div>
      {#if sx.installing?.tool === "ffmpeg"}
        <span class="muted">{sx.installing.stage === "unpacking" ? "Unpacking…" : sx.installing.total ? `${Math.round((sx.installing.done / sx.installing.total) * 100)}%` : "…"}</span>
      {:else if !sx.status?.ffmpeg}
        {#if sx.status?.windows}<button onclick={() => install("ffmpeg")}>Get ffmpeg · 87 MB</button>{:else}<span class="desc">Install it with your package manager</span>{/if}
      {/if}
    </div>
    <details>
      <summary>Use copies I already have</summary>
      <div class="custom">
        <div class="row"><span class="desc">yt-dlp</span><code>{cfg?.ytdlpPath || "found automatically"}</code><button class="small" onclick={() => chooseTool("ytdlpPath")}>Choose…</button>{#if cfg?.ytdlpPath}<button class="ghost small" onclick={() => save({ ytdlpPath: "" })}>Reset</button>{/if}</div>
        <div class="row"><span class="desc">ffmpeg</span><code>{cfg?.ffmpegPath || "found automatically"}</code><button class="small" onclick={() => chooseTool("ffmpegPath")}>Choose…</button>{#if cfg?.ffmpegPath}<button class="ghost small" onclick={() => save({ ffmpegPath: "" })}>Reset</button>{/if}</div>
      </div>
    </details>
  </section>

  <section class="card">
    <h3>Download format</h3>
    <div class="formats" role="radiogroup" aria-label="Download format">
      {#each formats as f}
        {@const off = f.ffmpeg && !hasFfmpeg}
        <button role="radio" aria-checked={cfg?.format === f.id} class:on={cfg?.format === f.id} disabled={off} onclick={() => save({ format: f.id })}>
          <span class="fname">{f.name}{#if f.tag}<span class="tag">{f.tag}</span>{/if}</span>
          <span class="what">{f.what}</span>
          <span class="changes">{f.changes}</span>
          {#if off}<span class="need">Needs ffmpeg</span>{/if}
        </button>
      {/each}
    </div>
    <div class="setting">
      <div><div class="label">SoundCloud: the artist's original file</div><div class="desc">When an artist allows downloads, take their own upload. It can be the real master (WAV or FLAC), better than the stream.</div></div>
      <Toggle label="SoundCloud originals" checked={cfg?.scOriginal ?? true} onchange={(v) => save({ scOriginal: v })} />
    </div>
    <div class="setting">
      <div><div class="label">Save to</div><div class="path">{sx.status?.downloadDir}</div><div class="desc">Sorted into Artist\Album folders, and added to your library.</div></div>
      <div class="row"><button onclick={chooseDir}>Change…</button>{#if cfg?.downloadDir}<button class="ghost" onclick={() => save({ downloadDir: "" })}>Default</button>{/if}</div>
    </div>
  </section>

  <section class="card">
    <h3>Streaming cache</h3>
    <div class="setting">
      <div><div class="label">{bytes(sx.status?.cacheBytes ?? 0)} used</div><div class="desc">Songs you stream are kept here so replays are instant. When it's full, the least recently played go first.</div></div>
      <div class="row">
        <select value={cfg?.cacheLimitMb ?? 1024} onchange={(e) => save({ cacheLimitMb: Number((e.currentTarget as HTMLSelectElement).value) })} aria-label="Cache size">
          {#each cacheLimits as l}<option value={l}>{l >= 1024 ? `${l / 1024} GB` : `${l} MB`}</option>{/each}
        </select>
        <button class="ghost" onclick={clearCache}>Clear</button>
      </div>
    </div>
  </section>

  <section class="card">
    <h3>Linked playlists</h3>
    {#if !sx.sources.length}
      <p class="desc">Paste a playlist, album or set link under <button class="link" onclick={() => go({ kind: "online" })}>Online</button> and choose “Save as playlist”. You can check it for new songs here later.</p>
    {/if}
    <ul class="sources">
      {#each sx.sources as s (s.playlistId)}
        <li>
          <button class="link name" onclick={() => go({ kind: "playlist", id: s.playlistId })}>{plName(s.playlistId)}</button>
          <span class="desc">{s.download ? "Downloads new songs" : "Streams new songs"} · checked {ago(s.lastSync)}</span>
          <span class="grow"></span>
          <button class="small" onclick={() => sync(s.playlistId)}><Icon name="refresh" size={14} /> Check now</button>
          <button class="ghost small" onclick={() => unlink(s.playlistId)}>Unlink</button>
        </li>
      {/each}
    </ul>
  </section>
</div>

<style>
  .dl { display: grid; gap: 1rem; max-width: 900px; }
  .bar { display: flex; align-items: center; gap: 0.8rem; }
  .bar h2 { font-size: 1.5rem; }
  .card { padding: 1rem 1.15rem; border-radius: var(--r-lg); background: var(--group); border: 1px solid var(--line-soft); display: grid; gap: 0.7rem; }
  h3 { font-size: 0.74rem; letter-spacing: 0.12em; text-transform: uppercase; color: var(--muted); font-family: var(--body); }
  .label { font-weight: 650; display: flex; align-items: center; gap: 0.5rem; }
  .desc { color: var(--muted); font-size: 0.85rem; }
  .path, code { font-family: "JetBrains Mono Variable", Consolas, monospace; font-size: 0.76rem; color: var(--faint); overflow-wrap: anywhere; user-select: text; }
  .tool { display: flex; align-items: center; justify-content: space-between; gap: 1rem; }
  .tool > button, .tool > span { white-space: nowrap; flex-shrink: 0; }
  .tool + .tool { border-top: 1px solid var(--line-soft); padding-top: 0.7rem; }
  .go { background: var(--m-accent, var(--peach)); border-color: transparent; color: var(--m-ink, var(--peach-ink)); white-space: nowrap; }
  .go:hover { background: var(--m-accent, var(--peach)); filter: brightness(1.08); }
  details summary { cursor: pointer; color: var(--muted); font-size: 0.85rem; }
  .custom { display: grid; gap: 0.4rem; margin-top: 0.5rem; }
  .custom .row { gap: 0.6rem; }
  .custom code { flex: 1; }
  .small { padding: 0.3rem 0.7rem; font-size: 0.82rem; display: inline-flex; align-items: center; gap: 0.3rem; }
  .formats { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 0.6rem; }
  .formats button { display: flex; flex-direction: column; align-items: flex-start; gap: 0.35rem; text-align: left; padding: 0.8rem; border-radius: var(--r-md); background: var(--sunk); font-weight: 500; }
  .formats button.on { border-color: var(--m-accent, var(--peach)); box-shadow: 0 0 0 1px var(--m-accent, var(--peach)); }
  .fname { font-weight: 800; font-size: 1rem; display: flex; align-items: center; gap: 0.4rem; }
  .tag { font-size: 0.64rem; font-weight: 700; letter-spacing: 0.06em; text-transform: uppercase; padding: 0.1rem 0.45rem; border-radius: 999px; background: color-mix(in srgb, var(--m-accent, var(--peach)) 22%, transparent); color: var(--m-accent, var(--peach)); }
  .what { font-size: 0.84rem; }
  .changes { font-size: 0.78rem; color: var(--muted); }
  .need { font-size: 0.72rem; color: var(--rose); font-weight: 700; }
  .setting { display: flex; align-items: center; justify-content: space-between; gap: 1rem; padding-top: 0.6rem; border-top: 1px solid var(--line-soft); }
  .row { gap: 0.4rem; }
  .jobs, .sources { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; }
  .jobs li { display: grid; grid-template-columns: minmax(0, 1fr) 70px 32px; align-items: center; gap: 0.7rem; padding: 0.45rem 0.5rem; border-radius: var(--r-md); background: var(--sunk); }
  .jm { display: flex; flex-direction: column; gap: 0.2rem; min-width: 0; }
  .t { font-weight: 650; font-size: 0.9rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .s { font-size: 0.78rem; color: var(--muted); }
  .jm .path { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .track { height: 4px; border-radius: 4px; background: color-mix(in srgb, var(--text) 12%, transparent); overflow: hidden; }
  .fill { height: 100%; background: var(--m-accent, var(--peach)); transition: width 0.25s; }
  .state { font-size: 0.8rem; color: var(--muted); text-align: right; font-variant-numeric: tabular-nums; }
  .done .state { color: var(--mint); }
  .failed .state, .err { color: var(--rose); font-size: 0.8rem; }
  .ic { border: 0; background: transparent; padding: 0.3rem; color: var(--muted); display: grid; place-items: center; }
  .ic:hover { color: var(--text); background: transparent; }
  .sources li { display: flex; align-items: center; gap: 0.7rem; padding: 0.45rem 0.5rem; border-radius: var(--r-md); background: var(--sunk); flex-wrap: wrap; }
  .link { border: 0; background: transparent; padding: 0; color: var(--lilac); font: inherit; font-weight: 600; }
  .link:hover { text-decoration: underline; background: transparent; }
  .name { color: var(--text); }
  @media (max-width: 900px) { .formats { grid-template-columns: 1fr; } }
</style>
