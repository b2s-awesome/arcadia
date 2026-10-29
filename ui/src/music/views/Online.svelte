<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "../../components/Icon.svelte";
  import { openUrl, streaming } from "../../lib/api";
  import { toast } from "../../lib/state.svelte";
  import type { OnlineFound, OnlineItem, OnlineSource } from "../../lib/types";
  import TrackList from "../TrackList.svelte";
  import { ask, openMenu, type MenuItem } from "../menu.svelte";
  import { go, lib, ms, play, time } from "../store.svelte";
  import { downloadOnline, importLink, initStreaming, install, newPlaylistOnline, playlistOnline, playOnline, queueOnline, sx } from "../streaming.svelte";

  let source: OnlineSource = $state("youtube-music");
  let q = $state("");
  let busy = $state(false);
  let found: OnlineFound | null = $state(null);
  let error: string | null = $state(null);
  let lastKey = "";

  onMount(() => {
    initStreaming();
  });

  const isLink = $derived(/^https?:\/\//i.test(q.trim()));
  const ready = $derived(!!sx.status?.ytdlp);

  async function go_() {
    const query = q.trim();
    if (!query || busy) return;
    const key = `${isLink ? "link" : source}:${query}`;
    if (key === lastKey && found) return;
    busy = true;
    error = null;
    try {
      found = isLink ? await streaming.resolve(query) : await streaming.search(source, query);
      lastKey = key;
    } catch (e) {
      error = String(e);
      found = null;
    }
    busy = false;
  }
  function switchSource(s: OnlineSource) {
    source = s;
    lastKey = "";
    if (q.trim() && !isLink) go_();
  }

  function itemMenu(e: MouseEvent, items: OnlineItem[]) {
    const one = items.length === 1 ? items[0] : null;
    const m: MenuItem[] = [
      { label: "Play", icon: "play", run: () => playOnline(items, 0) },
      { label: "Play next", icon: "queue", run: () => queueOnline(items, true) },
      { label: "Add to queue", icon: "plus", run: () => queueOnline(items, false) },
      { label: "Add to playlist", icon: "music", sub: [
        { label: "New playlist…", icon: "plus", run: async () => {
          const name = await ask("New playlist", found?.playlist ?? (one?.title ?? ""));
          if (name) newPlaylistOnline(name, items);
        } },
        ...ms.playlists.map((p) => ({ label: p.name, run: () => playlistOnline(p.id, items) })),
      ] },
      { label: items.length === 1 ? "Download" : `Download ${items.length} songs`, icon: "download", run: () => downloadOnline(items) },
      ...(one ? [{ label: "", sep: true }, { label: "Open in browser", run: () => openUrl(one.url) }] : []),
    ];
    openMenu(e, m);
  }

  async function saveAsPlaylist(download: boolean) {
    if (!found) return;
    const name = await ask(download ? "Save and download as" : "Save as playlist", found.playlist ?? "Imported");
    if (name === null) return;
    const id = await importLink(q.trim(), name || null, download);
    if (id !== null) {
      toast(download ? `Saved “${name}” and downloading it` : `Saved “${name}”. Check it for new songs from its page.`);
      go({ kind: "playlist", id });
    }
  }

  const siteLabel = (s: string) => (s === "youtube" ? "YouTube" : s === "soundcloud" ? "SoundCloud" : s.charAt(0).toUpperCase() + s.slice(1));
  const playingUrl = $derived(ms.status?.track?.sourceUrl ?? null);
</script>

<div class="online">
  <header class="bar">
    <h2>Online</h2>
    <div class="seg" role="radiogroup" aria-label="Search">
      <button role="radio" aria-checked={source === "youtube-music"} onclick={() => switchSource("youtube-music")}>YouTube Music</button>
      <button role="radio" aria-checked={source === "soundcloud"} onclick={() => switchSource("soundcloud")}>SoundCloud</button>
    </div>
  </header>

  <form class="search" onsubmit={(e) => { e.preventDefault(); go_(); }}>
    <Icon name={isLink ? "link" : "search"} size={17} />
    <input bind:value={q} placeholder={source === "soundcloud" ? "Search SoundCloud, or paste any link" : "Search YouTube Music, or paste any link"} aria-label="Search online or paste a link" disabled={!ready} />
    <button type="submit" class="go" disabled={!q.trim() || busy || !ready}>{busy ? "Looking…" : isLink ? "Open link" : "Search"}</button>
  </form>

  {#if sx.status && !ready}
    <div class="setup">
      <Icon name="download" size={22} />
      <div>
        <b>One quick setup step</b>
        <p>Online music is fetched with yt-dlp, a free open-source tool. Arcadia downloads it from its official release (about 18 MB) and checks it hasn't been tampered with.</p>
      </div>
      {#if sx.installing}
        <span class="muted">{sx.installing.total ? `${Math.round((sx.installing.done / sx.installing.total) * 100)}%` : "Starting…"}</span>
      {:else}
        <button class="go" onclick={() => install("yt-dlp")}>Get yt-dlp</button>
        <button class="ghost" onclick={() => go({ kind: "downloads" })}>Use my own</button>
      {/if}
    </div>
  {/if}

  {#if error}<p class="err">{error}</p>{/if}

  {#if busy}
    <div class="skeleton" aria-hidden="true">{#each Array(6) as _}<div></div>{/each}</div>
  {:else if found}
    {#if found.playlist}
      <section class="pl">
        <div>
          <span class="kind">From a link</span>
          <h3>{found.playlist}</h3>
          <p class="muted">{found.items.length} songs · {time(found.items.reduce((a, i) => a + (i.duration ?? 0), 0))}</p>
        </div>
        <div class="row wrap">
          <button class="go" onclick={() => playOnline(found!.items, 0)} disabled={!found.items.length}><Icon name="play" size={15} filled /> Play</button>
          <button onclick={() => saveAsPlaylist(false)}>Save as playlist</button>
          <button onclick={() => saveAsPlaylist(true)}><Icon name="download" size={15} /> Save and download</button>
          <button class="ghost icon" aria-label="More" onclick={(e) => itemMenu(e, found!.items)}><Icon name="more" size={18} /></button>
        </div>
      </section>
    {/if}
    {#if !found.items.length}
      <p class="muted pad">Nothing found{isLink ? " at that link" : ` for “${q}”`}.</p>
    {/if}
    <ul class="results">
      {#each found.items as it, i (it.url + i)}
        <li class:now={it.url === playingUrl} oncontextmenu={(e) => itemMenu(e, [it])}>
          <button class="hit" ondblclick={() => playOnline(found!.items, i)} onclick={(e) => { if (e.detail === 0) playOnline(found!.items, i); }} aria-label="Play {it.title}">
            <span class="thumb">
              {#if it.thumbnail}<img src={it.thumbnail} alt="" loading="lazy" referrerpolicy="no-referrer" />{:else}<span>{it.title[0]?.toUpperCase()}</span>{/if}
              <span class="pi"><Icon name="play" size={16} filled /></span>
            </span>
            <span class="meta">
              <span class="t">{it.title}</span>
              <span class="s">{it.artist}{it.album ? ` · ${it.album}` : ""}</span>
            </span>
          </button>
          <span class="site {it.site}">{siteLabel(it.site)}</span>
          <span class="d">{it.duration ? time(it.duration) : ""}</span>
          <button class="ic" aria-label="Download {it.title}" title="Download" onclick={() => downloadOnline([it])}><Icon name="download" size={16} /></button>
          <button class="ic" aria-label="More for {it.title}" onclick={(e) => itemMenu(e, [it])}><Icon name="more" size={18} /></button>
        </li>
      {/each}
    </ul>
  {:else if ready}
    {#if lib.streamed.length}
      <h3 class="sec">Recently streamed</h3>
      <p class="muted small">Songs you've played from online. They stay in a cache until it's full; right-click one to download it for good.</p>
      <TrackList tracks={lib.streamed.slice(0, 50)} onplay={(i) => play(lib.streamed.slice(0, 50), i)} />
    {:else}
      <div class="hint">
        <p><b>Search</b> YouTube Music or SoundCloud, or <b>paste a link</b> to a song, album, playlist or set (YouTube, SoundCloud, Bandcamp and anything else yt-dlp reads).</p>
        <p class="muted small">Songs play after a quick fetch, through Arcadia's own player. SoundCloud Go+ tracks only give the 30-second preview SoundCloud shows everyone, and DRM-protected audio isn't fetched. Only download what you're allowed to.</p>
      </div>
    {/if}
  {/if}
</div>

<style>
  .online { display: grid; gap: 1rem; }
  .bar { display: flex; align-items: center; gap: 1rem; flex-wrap: wrap; }
  .bar h2 { font-size: 1.5rem; }
  .search { display: flex; align-items: center; gap: 0.6rem; padding: 0.35rem 0.4rem 0.35rem 0.9rem; border-radius: 999px; background: var(--sunk); border: 1px solid var(--line); color: var(--faint); }
  .search:focus-within { border-color: var(--lilac); }
  .search input { flex: 1; border: 0; background: transparent; padding: 0.4rem 0; outline: none; min-width: 0; font-size: 0.98rem; }
  .go { display: inline-flex; align-items: center; gap: 0.4rem; background: var(--m-accent, var(--peach)); border-color: transparent; color: var(--m-ink, var(--peach-ink)); border-radius: 999px; padding: 0.5rem 1.1rem; }
  .go:hover:not(:disabled) { background: var(--m-accent, var(--peach)); filter: brightness(1.08); border-color: transparent; }
  .setup { display: flex; align-items: center; gap: 1rem; padding: 1rem 1.1rem; border-radius: var(--r-lg); background: color-mix(in srgb, var(--m-accent, var(--peach)) 12%, var(--raised)); border: 1px solid var(--line-soft); flex-wrap: wrap; }
  .setup > div { flex: 1; min-width: 260px; }
  .setup p { color: var(--muted); font-size: 0.88rem; margin-top: 0.2rem; }
  .err { color: var(--rose); font-size: 0.9rem; }
  .pl { display: flex; align-items: flex-end; justify-content: space-between; gap: 1rem; flex-wrap: wrap; padding: 1rem 1.1rem; border-radius: var(--r-lg); background: color-mix(in srgb, var(--m-accent, var(--peach)) 14%, var(--raised)); border: 1px solid var(--line-soft); }
  .pl h3 { font-size: 1.4rem; }
  .kind { font-size: 0.72rem; font-weight: 700; letter-spacing: 0.12em; text-transform: uppercase; color: var(--muted); }
  .row { gap: 0.4rem; }
  .row button { display: inline-flex; align-items: center; gap: 0.35rem; border-radius: 999px; }
  .ghost.icon { padding: 0.45rem; }
  .results { list-style: none; margin: 0; padding: 0; display: grid; gap: 2px; }
  .results li { display: grid; grid-template-columns: minmax(0, 1fr) auto 52px 32px 32px; align-items: center; gap: 0.6rem; padding: 0.3rem 0.5rem 0.3rem 0.3rem; border-radius: var(--r-md); }
  .results li:hover, .results li:focus-within { background: color-mix(in srgb, var(--text) 6%, transparent); }
  .results li.now .t { color: var(--m-accent, var(--peach)); }
  .hit { display: flex; align-items: center; gap: 0.75rem; border: 0; background: transparent; padding: 0; text-align: left; min-width: 0; }
  .hit:hover { background: transparent; }
  .thumb { position: relative; width: 48px; height: 48px; border-radius: 8px; overflow: hidden; flex-shrink: 0; background: var(--raised); display: grid; place-items: center; font-weight: 800; color: var(--muted); }
  .thumb img { width: 100%; height: 100%; object-fit: cover; }
  .pi { position: absolute; inset: 0; display: grid; place-items: center; background: rgba(0, 0, 0, 0.45); color: white; opacity: 0; transition: opacity 0.12s; }
  li:hover .pi { opacity: 1; }
  .meta { display: flex; flex-direction: column; min-width: 0; line-height: 1.25; }
  .t { font-weight: 650; font-size: 0.93rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .s { font-size: 0.8rem; color: var(--muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .site { font-size: 0.68rem; font-weight: 700; letter-spacing: 0.06em; text-transform: uppercase; padding: 0.15rem 0.5rem; border-radius: 999px; background: var(--sunk); color: var(--muted); border: 1px solid var(--line-soft); }
  .site.youtube { color: #ff5a6e; }
  .site.soundcloud { color: #ff7a2e; }
  .d { font-size: 0.82rem; color: var(--muted); text-align: right; font-variant-numeric: tabular-nums; }
  .ic { border: 0; background: transparent; padding: 0.3rem; color: var(--faint); display: grid; place-items: center; }
  .ic:hover { color: var(--text); background: transparent; }
  .skeleton { display: grid; gap: 8px; }
  .skeleton div { height: 54px; border-radius: var(--r-md); background: linear-gradient(90deg, var(--raised), color-mix(in srgb, var(--raised) 60%, var(--text) 6%), var(--raised)); background-size: 200% 100%; animation: shimmer 1.2s linear infinite; }
  @keyframes shimmer { from { background-position: 200% 0; } to { background-position: -200% 0; } }
  .sec { font-size: 1.1rem; }
  .small { font-size: 0.84rem; margin-top: -0.6rem; }
  .hint { display: grid; gap: 0.6rem; padding: 1.4rem; border-radius: var(--r-lg); border: 1px dashed var(--line); max-width: 70ch; }
  .hint .small { margin-top: 0; }
  .pad { padding: 0.6rem; }
</style>
