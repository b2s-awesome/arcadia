<script lang="ts">
  // Home: a dashboard of widgets you pick, size and arrange.
  import { onMount } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { app, saveUi } from "../lib/state.svelte";
  import { WIDGET_KINDS, type Widget, type WidgetKind } from "../lib/theme";
  import { lib, startLibrary, metaOf, nameOf, secondsOf, lastOf, coverOf, playtime, when, launch } from "../lib/games.svelte";
  import { ms, artUrl, toggle, next, prev } from "../music/store.svelte";
  import { rbx, startRbx, play as playRoblox, hours } from "./roblox/rbx.svelte";
  import { mcs, initMinecraft, play as playMc, playtime as mcPlaytime } from "../lib/mc.svelte";
  import Icon from "../components/Icon.svelte";
  import Thumb from "./roblox/Thumb.svelte";

  onMount(() => {
    startLibrary();
    startRbx();
    initMinecraft();
  });

  let editing = $state(false);
  const widgets = $derived(app.ui.home.widgets);

  // ── editing ──
  let dragging = $state<string | null>(null);
  let over = $state<string | null>(null);
  function move(id: string, to: string) {
    const list = [...app.ui.home.widgets];
    const from = list.findIndex((w) => w.id === id);
    const at = list.findIndex((w) => w.id === to);
    if (from < 0 || at < 0 || from === at) return;
    list.splice(at, 0, ...list.splice(from, 1));
    app.ui.home.widgets = list;
    saveUi();
  }
  function step(id: string, by: number) {
    const list = [...app.ui.home.widgets];
    const i = list.findIndex((w) => w.id === id);
    const j = i + by;
    if (i < 0 || j < 0 || j >= list.length) return;
    [list[i], list[j]] = [list[j], list[i]];
    app.ui.home.widgets = list;
    saveUi();
  }
  function resize(w: Widget, size: 1 | 2 | 3) {
    w.size = size;
    saveUi();
  }
  function remove(w: Widget) {
    app.ui.home.widgets = app.ui.home.widgets.filter((x) => x.id !== w.id);
    saveUi();
  }
  function add(kind: WidgetKind) {
    const k = WIDGET_KINDS.find((x) => x.kind === kind)!;
    app.ui.home.widgets = [...app.ui.home.widgets, { id: `w${Date.now().toString(36)}`, kind, size: k.size }];
    saveUi();
  }

  // ── data ──
  let now = $state(new Date());
  onMount(() => {
    const t = setInterval(() => (now = new Date()), 1000);
    return () => clearInterval(t);
  });
  const greeting = $derived(now.getHours() < 5 ? "Up late" : now.getHours() < 12 ? "Good morning" : now.getHours() < 18 ? "Good afternoon" : "Good evening");
  const visibleGames = $derived(lib.games.filter((g) => !metaOf(g.id).hidden));
  const recent = $derived([...visibleGames].filter((g) => lastOf(g) > 0).sort((a, b) => lastOf(b) - lastOf(a)));
  const favourites = $derived(visibleGames.filter((g) => metaOf(g.id).favourite));
  const mostPlayed = $derived([...visibleGames].filter((g) => secondsOf(g) > 0).sort((a, b) => secondsOf(b) - secondsOf(a)).slice(0, 6));
  const totalPlayed = $derived(visibleGames.reduce((n, g) => n + secondsOf(g), 0));
  const robloxGames = $derived([...rbx.history.games].sort((a, b) => Number(b.favourite) - Number(a.favourite) || b.last - a.last).slice(0, 4));
  const instances = $derived(mcs.instances.slice(0, 4));
  const track = $derived(ms.status?.track ?? null);

  let noteTimer: ReturnType<typeof setTimeout> | undefined;
  function note(w: Widget, text: string) {
    w.data = { ...(w.data ?? {}), text };
    clearTimeout(noteTimer);
    noteTimer = setTimeout(saveUi, 600);
  }
  const brokenCovers = new SvelteSet<string>();
</script>

<div class="home">
  <header class="head">
    <div>
      <h2>Home</h2>
      <p class="muted">{editing ? "Drag widgets to move them, pick a size, or add more below." : "Your games, music and more at a glance."}</p>
    </div>
    <button class:on={editing} onclick={() => (editing = !editing)}><Icon name={editing ? "check" : "sliders"} size={16} /> {editing ? "Done" : "Customise"}</button>
  </header>

  <div class="grid" class:editing>
    {#each widgets as w (w.id)}
      {@const k = WIDGET_KINDS.find((x) => x.kind === w.kind)!}
      <section
        class="widget {w.kind} s{w.size}"
        class:dragging={dragging === w.id}
        class:over={over === w.id && dragging !== w.id}
        draggable={editing}
        ondragstart={(e) => { dragging = w.id; e.dataTransfer?.setData("text/plain", w.id); }}
        ondragover={(e) => { if (editing) { e.preventDefault(); over = w.id; } }}
        ondragleave={() => { if (over === w.id) over = null; }}
        ondrop={(e) => { e.preventDefault(); if (dragging) move(dragging, w.id); dragging = over = null; }}
        ondragend={() => (dragging = over = null)}
        aria-label={k.label}
      >
        {#if editing}
          <div class="tools">
            <span class="grip"><Icon name="grip" size={15} /> {k.label}</span>
            <div class="seg tiny" role="radiogroup" aria-label="Width of {k.label}">
              {#each [1, 2, 3] as n}<button role="radio" aria-checked={w.size === n} onclick={() => resize(w, n as 1 | 2 | 3)}>{n === 1 ? "S" : n === 2 ? "M" : "L"}</button>{/each}
            </div>
            <button class="ghost i" aria-label="Move {k.label} earlier" onclick={() => step(w.id, -1)}><Icon name="up" size={14} /></button>
            <button class="ghost i" aria-label="Move {k.label} later" onclick={() => step(w.id, 1)}><Icon name="down" size={14} /></button>
            <button class="ghost i danger" aria-label="Remove {k.label}" onclick={() => remove(w)}><Icon name="close" size={14} /></button>
          </div>
        {/if}

        {#if w.kind === "greeting"}
          <div class="greet">
            <div>
              <h1>{greeting}.</h1>
              <p class="muted">{now.toLocaleDateString(undefined, { weekday: "long", day: "numeric", month: "long" })}{#if totalPlayed > 0} · {playtime(totalPlayed)} played in your library{/if}</p>
            </div>
            <span class="big-time">{now.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit", hour12: !app.ui.bar.clock24 })}</span>
          </div>
        {:else if w.kind === "continue"}
          <h3>Continue playing</h3>
          {#if recent.length}
            <div class="covers">
              {#each recent.slice(0, w.size === 3 ? 7 : w.size === 2 ? 4 : 2) as g (g.id)}
                {@const c = coverOf(g)}
                <button class="cover-btn" onclick={() => launch(g)} title="Play {nameOf(g)}">
                  <span class="cover">
                    {#if c && !brokenCovers.has(g.id)}<img src={c} alt="" loading="lazy" onerror={() => brokenCovers.add(g.id)} />{:else}<span class="mark">{nameOf(g)}</span>{/if}
                    <span class="go"><Icon name="play" size={16} filled /></span>
                  </span>
                  <span class="nm">{nameOf(g)}</span>
                  <span class="faint sm">{when(lastOf(g))}</span>
                </button>
              {/each}
            </div>
          {:else}
            <p class="empty">Games you play from the Library show up here.</p>
          {/if}
        {:else if w.kind === "nowplaying"}
          <h3>Now playing</h3>
          {#if track}
            <div class="np">
              {#if track.art}<img class="art" src={artUrl(track.art)} alt="" />{:else}<span class="art none"><Icon name="music" size={22} /></span>{/if}
              <div class="np-txt">
                <strong>{track.title}</strong>
                <span class="muted">{track.artist}</span>
              </div>
            </div>
            <div class="track"><span style:width="{ms.status?.duration ? (ms.pos / ms.status.duration) * 100 : 0}%"></span></div>
            <div class="ctl">
              <button class="ghost i" aria-label="Previous" onclick={prev}><Icon name="skip-prev" size={18} filled /></button>
              <button class="pp" aria-label={ms.status?.playing ? "Pause" : "Play"} onclick={toggle}><Icon name={ms.status?.playing ? "pause" : "play"} size={18} filled /></button>
              <button class="ghost i" aria-label="Next" onclick={next}><Icon name="skip-next" size={18} filled /></button>
            </div>
          {:else}
            <p class="empty">Nothing playing. <button class="ghost link" onclick={() => (app.page = "music")}>Open Music</button></p>
          {/if}
        {:else if w.kind === "roblox"}
          <h3>Roblox</h3>
          {#if robloxGames.length}
            <ul class="rows">
              {#each robloxGames as g (g.placeId)}
                <li>
                  <Thumb src={g.icon} seed={g.placeId} label={g.name || String(g.placeId)} size={34} />
                  <span class="grow"><span class="nm">{g.name || `Place ${g.placeId}`}</span><span class="faint sm">{hours(g.seconds)}</span></span>
                  <button class="ghost i" aria-label="Play {g.name}" onclick={() => playRoblox(g.placeId)}><Icon name="play" size={15} filled /></button>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="empty">Play Roblox through Arcadia and your games appear here.</p>
          {/if}
        {:else if w.kind === "minecraft"}
          <h3>Minecraft</h3>
          {#if instances.length}
            <ul class="rows">
              {#each instances as i (i.id)}
                <li>
                  <span class="mc-ico">{i.name.slice(0, 2)}</span>
                  <span class="grow"><span class="nm">{i.name}</span><span class="faint sm">{i.mcVersion} · {mcPlaytime(i.playtimeSecs)}</span></span>
                  <button class="ghost i" aria-label="Play {i.name}" onclick={() => playMc(i.id)}><Icon name="play" size={15} filled /></button>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="empty">Make an instance on the Minecraft page.</p>
          {/if}
        {:else if w.kind === "playtime"}
          <h3>Most played</h3>
          {#if mostPlayed.length}
            {@const top = secondsOf(mostPlayed[0])}
            <ul class="bars">
              {#each mostPlayed as g (g.id)}
                <li>
                  <span class="nm">{nameOf(g)}</span>
                  <span class="bar"><span style:width="{(secondsOf(g) / top) * 100}%"></span></span>
                  <span class="faint sm v">{playtime(secondsOf(g))}</span>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="empty">Playtime appears once you've played something.</p>
          {/if}
        {:else if w.kind === "quick"}
          <h3>Pinned games</h3>
          {#if favourites.length}
            <div class="pins">
              {#each favourites.slice(0, 8) as g (g.id)}
                {@const c = coverOf(g)}
                <button class="pin" onclick={() => launch(g)} title="Play {nameOf(g)}" style:background-image={c ? `url("${c}")` : undefined}>
                  <span>{nameOf(g)}</span>
                </button>
              {/each}
            </div>
          {:else}
            <p class="empty">Star games in the Library to pin them here.</p>
          {/if}
        {:else if w.kind === "stats"}
          <h3>Listening</h3>
          {#if track}
            <p class="big">{track.artist}</p>
            <p class="muted sm">is on right now · {ms.status?.queueLength ?? 0} in the queue</p>
          {:else}
            <p class="empty">Play some music to see it here.</p>
          {/if}
        {:else if w.kind === "notes"}
          <h3>Notes</h3>
          <textarea class="notes" placeholder="Jot something down…" value={String(w.data?.text ?? "")} oninput={(e) => note(w, e.currentTarget.value)} aria-label="Notes"></textarea>
        {:else if w.kind === "clock"}
          <div class="clock">
            <span class="t">{now.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit", hour12: !app.ui.bar.clock24 })}</span>
            <span class="muted">{now.toLocaleDateString(undefined, { weekday: "long" })}</span>
          </div>
        {/if}
      </section>
    {/each}
  </div>

  {#if editing}
    <section class="gallery">
      <h3>Add a widget</h3>
      <div class="kinds">
        {#each WIDGET_KINDS as k (k.kind)}
          <button class="kind" onclick={() => add(k.kind)}>
            <span class="kico"><Icon name={k.icon} size={18} /></span>
            <span><strong>{k.label}</strong><span class="muted sm">{k.blurb}</span></span>
            <Icon name="plus" size={16} />
          </button>
        {/each}
      </div>
      <label class="start"><input type="checkbox" checked={app.ui.home.start} onchange={(e) => { app.ui.home.start = e.currentTarget.checked; saveUi(); }} /> Open Home when Arcadia starts</label>
    </section>
  {/if}
</div>

<style>
  .home { height: 100%; overflow-y: auto; padding: 1.8rem 2.4rem 4rem; }
  .head { display: flex; align-items: flex-end; justify-content: space-between; gap: 1rem; margin-bottom: 1.2rem; }
  .head button { display: inline-flex; gap: 0.4rem; align-items: center; }
  .head button.on { background: var(--peach); color: var(--peach-ink); border-color: var(--peach); }
  .grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 1rem; grid-auto-flow: dense; }
  @media (max-width: 1000px) { .grid { grid-template-columns: repeat(2, minmax(0, 1fr)); } .s3 { grid-column: span 2 !important; } }
  .s1 { grid-column: span 1; }
  .s2 { grid-column: span 2; }
  .s3 { grid-column: span 3; }
  .widget {
    position: relative; min-width: 0; padding: 1.1rem 1.2rem; border-radius: var(--r-lg);
    background: color-mix(in srgb, var(--raised) 70%, transparent); border: 1px solid var(--line-soft);
    display: flex; flex-direction: column; gap: 0.6rem; min-height: 150px;
  }
  .widget h3 { font-size: 0.78rem; letter-spacing: 0.08em; text-transform: uppercase; color: var(--muted); font-weight: 700; }
  .editing .widget > h3 { display: none; }
  .editing .widget { cursor: grab; outline: 1px dashed var(--line); outline-offset: 3px; }
  .widget.dragging { opacity: 0.4; }
  .widget.over { outline: 2px solid var(--peach); }
  .tools { display: flex; align-items: center; gap: 0.3rem; margin: -0.3rem -0.3rem 0.2rem; padding-bottom: 0.5rem; border-bottom: 1px solid var(--line-soft); }
  .grip { display: inline-flex; align-items: center; gap: 0.35rem; font-size: 0.8rem; font-weight: 600; color: var(--muted); flex: 1; }
  .seg.tiny button { padding: 0.1rem 0.45rem; font-size: 0.72rem; }
  .i { padding: 0.3rem; line-height: 0; }
  .grow { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .nm { font-weight: 600; font-size: 0.9rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sm { font-size: 0.78rem; }
  .empty { color: var(--muted); font-size: 0.88rem; }
  .link { padding: 0 0.3rem; color: var(--lilac); }

  .greeting { min-height: 0; background: linear-gradient(120deg, color-mix(in srgb, var(--lilac) 14%, var(--raised)), color-mix(in srgb, var(--peach) 8%, var(--raised))); }
  .greet { display: flex; align-items: center; justify-content: space-between; gap: 1rem; }
  .greet h1 { font-size: clamp(1.8rem, 3.2vw, 2.6rem); letter-spacing: -0.03em; font-weight: 800; }
  .big-time { font-family: var(--display); font-weight: 700; font-size: clamp(1.6rem, 3vw, 2.4rem); font-variant-numeric: tabular-nums; color: var(--muted); }

  .covers { display: grid; grid-template-columns: repeat(auto-fill, minmax(104px, 1fr)); gap: 0.8rem; }
  .cover-btn { all: unset; cursor: pointer; display: flex; flex-direction: column; gap: 0.2rem; min-width: 0; }
  .cover { position: relative; aspect-ratio: 2 / 3; border-radius: var(--r-md); overflow: hidden; background: var(--sunk); border: 1px solid var(--line-soft); }
  .cover img { width: 100%; height: 100%; object-fit: cover; }
  .mark { position: absolute; left: 0.5rem; right: 0.5rem; bottom: 0.6rem; font-family: var(--display); font-weight: 800; font-size: 1rem; line-height: 1.05; }
  .go { position: absolute; right: 0.4rem; bottom: 0.4rem; width: 32px; height: 32px; border-radius: 50%; display: grid; place-items: center; background: var(--peach); color: var(--peach-ink); opacity: 0; transform: scale(0.9); transition: opacity 0.15s, transform 0.15s; }
  .cover-btn:hover .go, .cover-btn:focus-visible .go { opacity: 1; transform: none; }

  .np { display: flex; gap: 0.8rem; align-items: center; }
  .art { width: 56px; height: 56px; border-radius: var(--r-sm); object-fit: cover; flex: none; }
  .art.none { display: grid; place-items: center; background: var(--sunk); color: var(--faint); }
  .np-txt { display: flex; flex-direction: column; min-width: 0; }
  .np-txt strong, .np-txt span { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .track { height: 4px; border-radius: 99px; background: var(--line-soft); overflow: hidden; }
  .track span { display: block; height: 100%; background: var(--text); }
  .ctl { display: flex; justify-content: center; align-items: center; gap: 0.6rem; }
  .pp { width: 40px; height: 40px; border-radius: 50%; padding: 0; display: grid; place-items: center; background: var(--text); color: var(--ink); border: none; }

  .rows { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.45rem; }
  .rows li { display: flex; align-items: center; gap: 0.6rem; }
  .mc-ico { width: 34px; height: 34px; border-radius: 8px; display: grid; place-items: center; font-weight: 800; font-size: 0.8rem; text-transform: uppercase; background: linear-gradient(145deg, #5a8f4c, #2f5233); color: #e8f5e2; flex: none; }

  .bars { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.5rem; }
  .bars li { display: grid; grid-template-columns: minmax(80px, 1fr) 2fr 5rem; gap: 0.7rem; align-items: center; }
  .bar { height: 8px; border-radius: 99px; background: var(--line-soft); overflow: hidden; }
  .bar span { display: block; height: 100%; border-radius: inherit; background: linear-gradient(90deg, var(--lilac), var(--peach)); }
  .v { text-align: right; }

  .pins { display: grid; grid-template-columns: repeat(auto-fill, minmax(90px, 1fr)); gap: 0.5rem; }
  .pin { position: relative; aspect-ratio: 1; padding: 0; overflow: hidden; display: flex; align-items: flex-end; border-radius: var(--r-md); background-color: var(--sunk); background-size: cover; background-position: center; }
  .pin span { width: 100%; padding: 0.35rem 0.5rem; font-size: 0.75rem; font-weight: 700; text-align: left; background: linear-gradient(transparent, rgb(0 0 0 / 0.7)); color: #fff; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .big { font-family: var(--display); font-weight: 800; font-size: 1.5rem; line-height: 1.1; }
  .notes { flex: 1; min-height: 110px; resize: vertical; background: transparent; border-color: var(--line-soft); }
  .clock { flex: 1; display: flex; flex-direction: column; justify-content: center; align-items: center; gap: 0.2rem; }
  .clock .t { font-family: var(--display); font-weight: 800; font-size: 2.8rem; font-variant-numeric: tabular-nums; letter-spacing: -0.03em; }

  .gallery { margin-top: 1.6rem; }
  .gallery h3 { margin-bottom: 0.7rem; }
  .kinds { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 0.5rem; }
  .kind { display: flex; align-items: center; gap: 0.7rem; text-align: left; padding: 0.65rem 0.8rem; font-weight: 500; }
  .kind > span:nth-child(2) { flex: 1; display: flex; flex-direction: column; }
  .kico { width: 34px; height: 34px; border-radius: 50%; display: grid; place-items: center; background: var(--sunk); color: var(--muted); flex: none; }
  .start { display: inline-flex; gap: 0.5rem; align-items: center; margin-top: 1rem; color: var(--muted); cursor: pointer; }
</style>
