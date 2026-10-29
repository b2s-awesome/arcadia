<script lang="ts">
  // Music → Stats: recap cards (Tonal), terminal tables (Tiled) or glass
  // widgets, all from the same numbers.
  import { music } from "../../lib/api";
  import { app, saveUi } from "../../lib/state.svelte";
  import { skinFor, type StatsPrefs } from "../../lib/theme";
  import type { ListenStats } from "../../lib/types";
  import Art from "../../music/Art.svelte";
  import { artUrl, go, lib, play } from "../../music/store.svelte";

  const p = $derived(app.ui.stats);
  const skin = $derived(skinFor(p.skin, app.ui.music.layout));
  const RANGES: { id: StatsPrefs["range"]; label: string; short: string; days: number | null }[] = [
    { id: "4w", label: "4 weeks", short: "4w", days: 28 },
    { id: "6m", label: "6 months", short: "6m", days: 182 },
    { id: "1y", label: "This year", short: "1y", days: 365 },
    { id: "all", label: "All time", short: "all", days: null },
  ];
  const range = $derived(RANGES.find((r) => r.id === p.range) ?? RANGES[0]);

  let s = $state<ListenStats | null>(null);
  let error: string | null = $state(null);
  $effect(() => {
    const days = range.days;
    const since = days === null ? null : Math.floor(Date.now() / 1000) - days * 86400;
    error = null;
    music.stats(since).then((v) => (s = v)).catch((e) => (error = String(e)));
  });
  function setRange(r: StatsPrefs["range"]) {
    app.ui.stats.range = r;
    saveUi();
  }

  const hrs = (secs: number) => (secs >= 3600 ? `${Math.round(secs / 360) / 10} h` : `${Math.round(secs / 60)} min`);
  const hoursNum = (secs: number) => Math.round(secs / 360) / 10;
  const maxHour = $derived(s ? Math.max(1, ...s.hours) : 1);
  const peakHour = $derived(s ? s.hours.indexOf(Math.max(...s.hours)) : 0);
  const when = (h: number) => (h < 5 ? "Mostly after midnight" : h < 12 ? "Mostly in the morning" : h < 17 ? "Mostly in the afternoon" : h < 21 ? "Mostly in the evening" : "Mostly late at night");
  const change = $derived(s && s.previous !== null ? s.listened - s.previous : null);
  const skipRate = $derived(s && s.plays + s.skips ? Math.round((s.skips / (s.plays + s.skips)) * 100) : 0);
  const maxDay = $derived(s ? Math.max(1, ...s.days) : 1);
  const level = (v: number) => (v <= 0 ? 0 : v < maxDay * 0.25 ? 1 : v < maxDay * 0.5 ? 2 : v < maxDay * 0.75 ? 3 : 4);
  const totalSrc = $derived(s ? Math.max(1, s.sources.reduce((a, x) => a + x.listened, 0)) : 1);
  const totalFmt = $derived(s ? Math.max(1, s.formats.reduce((a, x) => a + x.listened, 0)) : 1);
  const SRC: Record<string, string> = { local: "Your files", stream: "Streamed", download: "Downloaded" };
  const bar = (v: number, max: number, w: number) => "█".repeat(Math.max(1, Math.round((v / Math.max(1, max)) * w)));
  const goalPct = $derived(s ? Math.min(1, s.today / 60 / p.goal) : 0);

  function playSong(id: number | null) {
    const t = id === null ? null : lib.byId.get(id);
    if (t) play([t], 0);
  }
  function openArtist(name: string) {
    if (lib.artists.some((a) => a.name === name)) go({ kind: "artist", name });
  }
  function openAlbum(album: string, artist: string) {
    const a = lib.albums.find((x) => x.title === album && x.artist === artist) ?? lib.albums.find((x) => x.title === album);
    if (a) go({ kind: "album", key: a.key });
  }
</script>

<div class="stats {skin}">
  <div class="top">
    {#if skin === "tiled"}<b class="g-name">music:stats</b>{:else if skin === "glass"}<h2>Your listening</h2>{:else}<h2>Stats</h2>{/if}
    <div class="ranges" role="radiogroup" aria-label="Time range">
      {#each RANGES as r}<button role="radio" aria-checked={p.range === r.id} onclick={() => setRange(r.id)}>{skin === "tiled" ? r.short : r.label}</button>{/each}
    </div>
    <span class="grow"></span>
    <span class="note">{skin === "tiled" ? "local only · library.db" : "Counted from your own plays, on this PC only"}</span>
  </div>

  {#if error}
    <p class="empty">{error}</p>
  {:else if !s}
    <p class="empty">Counting…</p>
  {:else if s.plays + s.skips === 0}
    <p class="empty">Nothing played {range.days === null ? "yet" : `in the last ${range.label.toLowerCase()}`}. Arcadia counts plays from this version on; play some music and come back.</p>
  {:else if skin === "tonal"}
    <div class="r-grid">
      <section class="card hero">
        <span class="lbl">Listening time</span>
        <span class="huge">{hoursNum(s.listened) >= 1 ? `${hoursNum(s.listened)} h` : `${Math.round(s.listened / 60)} min`}</span>
        <span class="soft">{s.plays.toLocaleString()} plays · {s.songs.toLocaleString()} different songs</span>
        <span class="grow"></span>
        {#if change !== null}<span class="soft">{change >= 0 ? "Up" : "Down"} {hrs(Math.abs(change))} on the {range.label.toLowerCase()} before</span>{/if}
      </section>
      <section class="card wide" aria-label="Top artists">
        <span class="lbl">Top artists</span>
        <div class="artists">
          {#each s.topArtists.slice(0, 5) as a, i}
            <button class="artist" onclick={() => openArtist(a.name)}><Art src={a.art} name={a.name} color={a.artColor} round /><b>{i + 1}. {a.name}</b><span>{a.plays} plays</span></button>
          {/each}
        </div>
      </section>
      <section class="card" aria-label="When you listen">
        <span class="lbl">When you listen</span>
        <div class="hours">{#each s.hours as h, i}<span style:height="{Math.max(3, (h / maxHour) * 100)}%" class:peak={i === peakHour} title="{i}:00 · {hrs(h)}"></span>{/each}</div>
        <div class="axis"><span>00</span><span>06</span><span>12</span><span>18</span><span>23</span></div>
        <span class="soft">{when(peakHour)}</span>
      </section>
      <section class="card wide2" aria-label="Top songs">
        <span class="lbl">Top songs</span>
        <ol class="songs">
          {#each s.topSongs.slice(0, 5) as t, i}
            <li><span class="n">{i + 1}</span><button class="cover" onclick={() => playSong(t.id)} aria-label="Play {t.title}" disabled={t.id === null}><Art src={t.art} name={t.title} color={t.artColor} radius="8px" /></button><span class="tt"><b>{t.title}</b><span>{t.artist}</span></span><span class="soft">{t.plays} plays</span></li>
          {/each}
        </ol>
      </section>
      {#if s.topAlbums[0]}
        {@const al = s.topAlbums[0]}
        <section class="card" aria-label="Top album">
          <span class="lbl">Top album</span>
          <button class="album" onclick={() => openAlbum(al.album, al.artist)}><Art src={al.art} name={al.album} color={al.artColor} radius="14px" /></button>
          <b>{al.album}</b><span class="soft small">{al.artist} · {al.plays} plays</span>
        </section>
      {/if}
      <section class="card" aria-label="Habits">
        <span class="lbl">Habits</span>
        <dl>
          <dt>Skipped early</dt><dd>{skipRate}%</dd>
          <dt>Longest streak</dt><dd>{s.streak} day{s.streak === 1 ? "" : "s"}</dd>
          <dt>New to you</dt><dd>{s.newSongs} song{s.newSongs === 1 ? "" : "s"}</dd>
          {#if s.busiestDay}<dt>Busiest day</dt><dd>{new Date(s.busiestDay.at * 1000).toLocaleDateString([], { day: "numeric", month: "short" })}</dd>{/if}
        </dl>
      </section>
    </div>
  {:else if skin === "tiled"}
    <div class="g-grid">
      <section class="g-pane g-focus" aria-label="Top artists">
        <div class="g-ph"><b>top artists</b><span>by plays</span></div>
        <table><thead><tr><th>#</th><th>artist</th><th>plays</th><th>time</th><th class="share">share</th></tr></thead>
          <tbody>{#each s.topArtists.slice(0, 8) as a, i}<tr><td class="f">{i + 1}</td><td><button class="lnk" onclick={() => openArtist(a.name)}>{a.name}</button></td><td>{a.plays}</td><td>{hrs(a.listened)}</td><td class="acc">{bar(a.plays, s.topArtists[0].plays, 24)}</td></tr>{/each}</tbody></table>
      </section>
      <section class="g-pane" aria-label="Top songs">
        <div class="g-ph"><b>top songs</b><span>plays · skips · avg listened</span></div>
        <table><thead><tr><th>#</th><th>title</th><th>plays</th><th>skips</th><th>avg</th></tr></thead>
          <tbody>{#each s.topSongs.slice(0, 8) as t, i}<tr><td class="f">{i + 1}</td><td><button class="lnk" disabled={t.id === null} onclick={() => playSong(t.id)}>{t.title}</button></td><td>{t.plays}</td><td class:bad={t.skips > 4}>{t.skips}</td><td>{Math.floor(t.avg / 60)}:{String(Math.round(t.avg % 60)).padStart(2, "0")}</td></tr>{/each}</tbody></table>
      </section>
      <section class="g-pane" aria-label="Activity">
        <div class="g-ph"><b>activity</b><span>last {Math.round(s.days.length / 7)} weeks · listening per day</span></div>
        <div class="heat" style:--weeks={Math.ceil(s.days.length / 7)}>{#each s.days as d, i}<span class="l{level(d)}" title="{new Date(Date.now() - (s.days.length - 1 - i) * 86400000).toLocaleDateString()} · {hrs(d)}"></span>{/each}</div>
        <div class="legend">less <span class="l0"></span><span class="l1"></span><span class="l2"></span><span class="l3"></span><span class="l4"></span> more · streak {s.streak}d · skip rate {skipRate}%</div>
      </section>
      <section class="g-pane" aria-label="Sources and formats">
        <div class="g-ph"><b>sources &amp; formats</b><span>{hrs(s.listened)} total</span></div>
        <div class="mix">
          {#each s.sources as x}<span class="k">{(SRC[x.key] ?? x.key).toLowerCase()}</span><span class="acc">{bar(x.listened, totalSrc, 36)}</span><span class="v">{Math.round((x.listened / totalSrc) * 100)}%</span>{/each}
          <span class="sp"></span>
          {#each s.formats as x}<span class="k">{x.key.toLowerCase()}</span><span class="alt">{bar(x.listened, totalFmt, 36)}</span><span class="v">{Math.round((x.listened / totalFmt) * 100)}%</span>{/each}
        </div>
      </section>
    </div>
  {:else}
    <div class="l-wall" style:background-image={s.topSongs[0]?.art ? `url("${artUrl(s.topSongs[0].art)}")` : undefined} aria-hidden="true"></div>
    <div class="l-grid">
      <section class="lg w" aria-label="Listening time"><span class="sheen"></span><span class="lbl">{range.label}</span><span class="huge">{hoursNum(s.listened) >= 1 ? hoursNum(s.listened) : Math.round(s.listened / 60)}<small>{hoursNum(s.listened) >= 1 ? " h" : " min"}</small></span><span class="grow"></span><b>{s.plays} plays · {s.songs} songs</b></section>
      {#if s.topArtists[0]}
        {@const a = s.topArtists[0]}
        <section class="lg w l-artist" aria-label="Top artist"><span class="sheen"></span><div class="l-ac"><Art src={a.art} name={a.name} color={a.artColor} round /></div><div><span class="lbl">Top artist</span><h3>{a.name}</h3><b>{a.plays} plays · {hrs(a.listened)}</b>{#if s.topArtists[1]}<span class="soft">Ahead of {s.topArtists[1].name} by {a.plays - s.topArtists[1].plays}</span>{/if}</div></section>
      {/if}
      <section class="lg w l-ring" aria-label="Today"><span class="sheen"></span><span class="lbl">Today</span>
        <svg viewBox="0 0 120 120" role="img" aria-label="{Math.round(s.today / 60)} of {p.goal} minutes"><circle cx="60" cy="60" r="48" fill="none" stroke="rgba(255,255,255,.22)" stroke-width="12"></circle><circle cx="60" cy="60" r="48" fill="none" stroke="#fff" stroke-width="12" stroke-linecap="round" stroke-dasharray="{goalPct * 301.6} 302" transform="rotate(-90 60 60)"></circle><text x="60" y="58" text-anchor="middle" fill="#fff" font-size="26" font-weight="900">{Math.round(s.today / 60)}</text><text x="60" y="78" text-anchor="middle" fill="#fff" font-size="11" font-weight="700">of {p.goal} min</text></svg>
      </section>
      <section class="lg w l-songs" aria-label="Top songs"><span class="sheen"></span><span class="lbl">Top songs</span>
        <ol>{#each s.topSongs.slice(0, 4) as t}<li><button class="cover" onclick={() => playSong(t.id)} aria-label="Play {t.title}" disabled={t.id === null}><Art src={t.art} name={t.title} color={t.artColor} radius="12px" /></button><span class="tt"><b>{t.title}</b><span>{t.artist}</span></span><b>{t.plays}</b></li>{/each}</ol>
      </section>
      <section class="lg w l-hours" aria-label="When you listen"><span class="sheen"></span><span class="lbl">When you listen</span>
        <div class="hours">{#each s.hours as h, i}<span style:height="{Math.max(4, (h / maxHour) * 100)}%" class:peak={i === peakHour}></span>{/each}</div>
        <div class="axis"><span>midnight</span><span>6 am</span><span>noon</span><span>6 pm</span><span>11 pm</span></div>
      </section>
    </div>
  {/if}
</div>

<style>
  .stats { position: relative; min-height: calc(100% - 1rem); }
  .grow { flex: 1; }
  .top { display: flex; align-items: center; gap: 0.6rem; margin-bottom: 1rem; flex-wrap: wrap; position: relative; z-index: 1; }
  .top h2 { font-size: 1.5rem; margin-right: 0.4rem; }
  .ranges { display: flex; gap: 0.3rem; }
  .ranges button { border-radius: 999px; padding: 0.35rem 0.8rem; font-size: 0.82rem; background: transparent; color: var(--muted); }
  .ranges button[aria-checked="true"] { background: var(--text); color: var(--ink); border-color: transparent; font-weight: 700; }
  .note { font-size: 0.8rem; color: var(--muted); }
  .empty { color: var(--muted); padding: 2rem 0.4rem; max-width: 520px; }
  .lbl { font-size: 0.74rem; font-weight: 700; letter-spacing: 0.1em; text-transform: uppercase; color: var(--muted); }
  .soft { color: var(--muted); font-size: 0.86rem; }
  .small { font-size: 0.8rem; }
  .huge { font-size: clamp(2.6rem, 5vw, 4rem); font-weight: 900; letter-spacing: -0.05em; line-height: 1; }
  .cover { border: 0; padding: 0; background: transparent; width: 40px; border-radius: 8px; flex-shrink: 0; }

  /* Recap cards */
  .r-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 14px; }
  .card { background: var(--raised); border: 1px solid var(--line-soft); border-radius: calc(var(--r-lg) + 4px); padding: 18px; display: flex; flex-direction: column; gap: 10px; min-width: 0; }
  .hero { background: linear-gradient(160deg, color-mix(in srgb, var(--m-accent, var(--peach)) 45%, var(--raised)), var(--raised) 80%); }
  .wide { grid-column: span 2; }
  .wide2 { grid-column: span 2; }
  .artists { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 12px; }
  .artist { border: 0; background: transparent; padding: 0; display: flex; flex-direction: column; gap: 6px; text-align: left; min-width: 0; }
  .artist b { font-size: 0.82rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .artist span { font-size: 0.76rem; color: var(--muted); }
  .hours { flex: 1; min-height: 90px; display: flex; align-items: flex-end; gap: 3px; }
  .hours span { flex: 1; border-radius: 3px; background: color-mix(in srgb, var(--text) 18%, transparent); }
  .hours span.peak { background: var(--m-accent, var(--peach)); }
  .axis { display: flex; justify-content: space-between; font-size: 0.7rem; color: var(--muted); }
  .songs { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
  .songs li { display: grid; grid-template-columns: 20px 40px minmax(0, 1fr) auto; gap: 12px; align-items: center; }
  .n { color: var(--muted); font-weight: 700; }
  .tt { display: flex; flex-direction: column; min-width: 0; }
  .tt b, .tt span { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tt span { font-size: 0.78rem; color: var(--muted); }
  .album { border: 0; padding: 0; background: transparent; border-radius: 14px; }
  dl { display: grid; grid-template-columns: 1fr auto; gap: 8px; font-size: 0.9rem; }
  dt { color: var(--muted); }
  dd { margin: 0; font-weight: 700; text-align: right; }
  @media (max-width: 1100px) { .r-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); } .artists { grid-template-columns: repeat(3, minmax(0, 1fr)); } .artists :global(.artist:nth-child(n + 4)) { display: none; } }

  /* Terminal tables */
  .tiled { font-family: "JetBrains Mono Variable", Consolas, monospace; font-size: 0.82rem; }
  .tiled .ranges button { border-radius: 2px; }
  .tiled .ranges button[aria-checked="true"] { background: var(--m-accent, var(--peach)); color: var(--m-ink, var(--peach-ink)); }
  .g-name { color: var(--m-accent, var(--peach)); margin-right: 0.4rem; }
  .g-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
  .g-pane { border: 1px solid var(--line); border-radius: 4px; background: var(--sunk); min-width: 0; overflow: hidden; }
  .g-focus { border: 2px solid var(--m-accent, var(--peach)); }
  .g-ph { display: flex; gap: 0.8rem; padding: 0.45rem 0.7rem; border-bottom: 1px solid var(--line-soft); color: var(--muted); }
  .g-ph b { color: var(--m-accent, var(--peach)); }
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: 0.22rem 0.7rem; text-align: left; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 14rem; }
  th { color: var(--muted); font-weight: 500; }
  th.share { width: 40%; }
  .f { color: var(--faint); }
  .acc { color: var(--m-accent, var(--peach)); }
  .alt { color: var(--lilac); }
  .bad { color: var(--rose); }
  .lnk { font: inherit; border: 0; background: transparent; padding: 0; color: var(--text); }
  .lnk:hover:not(:disabled) { text-decoration: underline; background: transparent; }
  .heat { display: grid; grid-auto-flow: column; grid-template-rows: repeat(7, 13px); grid-template-columns: repeat(var(--weeks), minmax(0, 1fr)); gap: 3px; padding: 0.8rem; }
  .heat span, .legend span { border-radius: 2px; }
  .legend { display: flex; align-items: center; gap: 0.35rem; padding: 0 0.8rem 0.8rem; color: var(--muted); }
  .legend span { width: 12px; height: 12px; display: inline-block; }
  .l0 { background: color-mix(in srgb, var(--text) 8%, transparent); }
  .l1 { background: color-mix(in srgb, var(--m-accent, var(--peach)) 25%, transparent); }
  .l2 { background: color-mix(in srgb, var(--m-accent, var(--peach)) 50%, transparent); }
  .l3 { background: color-mix(in srgb, var(--m-accent, var(--peach)) 75%, transparent); }
  .l4 { background: var(--m-accent, var(--peach)); }
  .mix { display: grid; grid-template-columns: 7.5rem minmax(0, 1fr) 3rem; gap: 0.35rem 0.7rem; padding: 0.8rem; }
  .mix .k { color: var(--muted); }
  .mix .v { text-align: right; }
  .mix .acc, .mix .alt { overflow: hidden; white-space: nowrap; }
  .mix .sp { grid-column: 1 / -1; height: 0.4rem; }
  @media (max-width: 1000px) { .g-grid { grid-template-columns: 1fr; } }

  /* Glass widgets */
  .glass { color: #fff; font-family: "Figtree Variable", "Segoe UI", sans-serif; border-radius: var(--r-lg); overflow: hidden; padding: 1.4rem; }
  .glass .top { color: #fff; }
  .glass .note, .glass .lbl, .glass .soft { color: rgba(255, 255, 255, 0.85); }
  .glass .ranges button { color: #fff; border-color: rgba(255, 255, 255, 0.35); background: rgba(255, 255, 255, 0.12); }
  .glass .ranges button[aria-checked="true"] { background: rgba(255, 255, 255, 0.85); color: #141018; }
  .glass .empty { color: rgba(255, 255, 255, 0.85); }
  .l-wall { position: absolute; inset: 0; background: linear-gradient(200deg, #0f2b3a, #2f6f73) center / cover; filter: blur(30px) saturate(1.4); transform: scale(1.15); }
  .l-grid { position: relative; display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 18px; }
  .lg { position: relative; backdrop-filter: blur(22px) saturate(1.9) brightness(1.06); background: rgba(255, 255, 255, 0.16);
    box-shadow: inset 0 1.5px 0.5px rgba(255, 255, 255, 0.75), inset 0 -1px 0.5px rgba(255, 255, 255, 0.22), inset 0 0 0 1px rgba(255, 255, 255, 0.18), 0 0 0 1px rgba(0, 0, 0, 0.22), 0 14px 34px rgba(0, 0, 0, 0.28); }
  .sheen { position: absolute; inset: 0; border-radius: inherit; pointer-events: none; background: linear-gradient(135deg, rgba(255, 255, 255, 0.42), rgba(255, 255, 255, 0) 38%, rgba(255, 255, 255, 0) 72%, rgba(255, 255, 255, 0.14)); mix-blend-mode: screen; }
  .w { border-radius: 32px; padding: 22px; display: flex; flex-direction: column; gap: 10px; min-width: 0; min-height: 200px; }
  .w small { font-size: 0.45em; }
  .l-artist { grid-column: span 2; flex-direction: row; align-items: center; gap: 22px; }
  .l-ac { width: 150px; flex-shrink: 0; border-radius: 50%; box-shadow: 0 14px 40px rgba(0, 0, 0, 0.35); }
  .l-artist div:last-child { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .l-artist h3 { font-size: 2.4rem; font-weight: 900; letter-spacing: -0.04em; line-height: 1; overflow-wrap: anywhere; }
  .l-ring { align-items: center; }
  .l-ring .lbl { align-self: flex-start; }
  .l-ring svg { width: 140px; height: 140px; }
  .l-songs { grid-column: span 2; }
  .l-songs ol { list-style: none; margin: 0; padding: 0; display: grid; gap: 9px; }
  .l-songs li { display: grid; grid-template-columns: 44px minmax(0, 1fr) auto; gap: 12px; align-items: center; }
  .l-songs .cover { width: 44px; border-radius: 12px; }
  .l-songs .tt span { color: rgba(255, 255, 255, 0.8); }
  .l-hours { grid-column: span 2; }
  .l-hours .hours span { background: rgba(255, 255, 255, 0.4); border-radius: 6px; }
  .l-hours .hours span.peak { background: rgba(255, 255, 255, 0.95); }
  .l-hours .axis { color: rgba(255, 255, 255, 0.85); font-weight: 700; }
  @media (max-width: 1100px) { .l-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
</style>
