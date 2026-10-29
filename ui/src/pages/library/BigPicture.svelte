<script lang="ts">
  // Big Picture: the library full screen, for the sofa. Arrow keys or a
  // controller move around; Enter/A plays, Y favourites, B/Esc leaves.
  import { onMount } from "svelte";
  import { api } from "../../lib/api";
  import {
    lib, metaOf, saveMeta, nameOf, secondsOf, lastOf, coverOf, heroOf, logoOf, playtime, when, launch,
  } from "../../lib/games.svelte";
  import type { Game } from "../../lib/types";
  import Icon from "../../components/Icon.svelte";

  const shelves = $derived.by(() => {
    const games = lib.games.filter((g) => !metaOf(g.id).hidden);
    const out: { title: string; games: Game[] }[] = [];
    const recent = games.filter((g) => lastOf(g) > 0).sort((a, b) => lastOf(b) - lastOf(a)).slice(0, 12);
    if (recent.length) out.push({ title: "Jump back in", games: recent });
    const favs = games.filter((g) => metaOf(g.id).favourite);
    if (favs.length) out.push({ title: "Favourites", games: favs });
    for (const c of lib.collections) {
      const list = games.filter((g) => metaOf(g.id).collections.includes(c.name));
      if (list.length) out.push({ title: `${c.icon ? c.icon + " " : ""}${c.name}`, games: list });
    }
    out.push({ title: "All games", games: [...games].sort((a, b) => nameOf(a).localeCompare(nameOf(b))) });
    return out;
  });

  let row = $state(0);
  let col = $state<number[]>([]);
  const at = (r: number) => col[r] ?? 0;
  const focused = $derived(shelves[row]?.games[at(row)] ?? null);
  let rows: HTMLElement[] = $state([]);
  let shelfEls: HTMLElement[] = $state([]);

  function move(dr: number, dc: number) {
    if (!shelves.length) return;
    if (dr) row = Math.max(0, Math.min(shelves.length - 1, row + dr));
    if (dc) {
      const n = shelves[row].games.length;
      const c = Math.max(0, Math.min(n - 1, at(row) + dc));
      col[row] = c;
    }
    queueMicrotask(scrollIntoView);
  }
  function scrollIntoView() {
    shelfEls[row]?.scrollIntoView({ block: "nearest", behavior: "smooth" });
    const card = rows[row]?.children[at(row)] as HTMLElement | undefined;
    card?.scrollIntoView({ inline: "center", block: "nearest", behavior: "smooth" });
  }
  function act(what: "play" | "fav" | "back") {
    if (what === "back") return close();
    const g = focused;
    if (!g) return;
    if (what === "play") launch(g);
    else saveMeta(g.id, { favourite: !metaOf(g.id).favourite });
  }
  function close() {
    lib.bigPicture = false;
  }

  function key(e: KeyboardEvent) {
    const map: Record<string, () => void> = {
      ArrowUp: () => move(-1, 0), ArrowDown: () => move(1, 0), ArrowLeft: () => move(0, -1), ArrowRight: () => move(0, 1),
      Enter: () => act("play"), " ": () => act("play"), Escape: () => act("back"), Backspace: () => act("back"), f: () => act("fav"), y: () => act("fav"),
    };
    const f = map[e.key];
    if (f) {
      e.preventDefault();
      e.stopPropagation();
      f();
    }
  }

  // Controllers: the standard mapping (A=0, B=1, Y=3, LB=4, RB=5, d-pad 12–15).
  let pads = $state(0);
  let raf = 0;
  const held = new Map<string, number>();
  function press(id: string, down: boolean, fn: () => void, repeat = false) {
    const now = performance.now();
    const t = held.get(id);
    if (!down) {
      held.delete(id);
      return;
    }
    if (t === undefined) {
      held.set(id, now + 380);
      fn();
    } else if (repeat && now >= t) {
      held.set(id, now + 110);
      fn();
    }
  }
  function poll() {
    const gps = navigator.getGamepads?.() ?? [];
    let n = 0;
    for (const gp of gps) {
      if (!gp) continue;
      n++;
      const b = (i: number) => !!gp.buttons[i]?.pressed;
      const ax = (i: number) => gp.axes[i] ?? 0;
      press(`${gp.index}u`, b(12) || ax(1) < -0.55, () => move(-1, 0), true);
      press(`${gp.index}d`, b(13) || ax(1) > 0.55, () => move(1, 0), true);
      press(`${gp.index}l`, b(14) || ax(0) < -0.55, () => move(0, -1), true);
      press(`${gp.index}r`, b(15) || ax(0) > 0.55, () => move(0, 1), true);
      press(`${gp.index}a`, b(0), () => act("play"));
      press(`${gp.index}b`, b(1), () => act("back"));
      press(`${gp.index}y`, b(3), () => act("fav"));
      press(`${gp.index}lb`, b(4), () => move(-1, 0));
      press(`${gp.index}rb`, b(5), () => move(1, 0));
    }
    pads = n;
    raf = requestAnimationFrame(poll);
  }

  let clock = $state(new Date());
  onMount(() => {
    api.libraryBigPicture(true).catch(() => {});
    raf = requestAnimationFrame(poll);
    const t = setInterval(() => (clock = new Date()), 10_000);
    window.addEventListener("keydown", key, true);
    return () => {
      cancelAnimationFrame(raf);
      clearInterval(t);
      window.removeEventListener("keydown", key, true);
      api.libraryBigPicture(false).catch(() => {});
    };
  });

  const bg = $derived(focused ? heroOf(focused) ?? coverOf(focused) : null);
  let broken = $state(new Set<string>());
</script>

<div class="bp" role="dialog" aria-modal="true" aria-label="Big Picture">
  {#key bg}
    <div class="bg" style:background-image={bg ? `url("${bg}")` : undefined}></div>
  {/key}
  <div class="shade"></div>

  <header>
    <span class="brand">Arcadia</span>
    <span class="grow"></span>
    {#if pads}<span class="pad"><Icon name="gamepad" size={18} /> {pads} controller{pads === 1 ? "" : "s"}</span>{/if}
    <span class="clock">{clock.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })}</span>
    <button class="exit" onclick={close} aria-label="Leave Big Picture"><Icon name="close" size={20} /></button>
  </header>

  {#if focused}
    {@const logo = logoOf(focused)}
    <section class="detail">
      {#if logo}<img class="logo" src={logo} alt={nameOf(focused)} />{:else}<h1>{nameOf(focused)}</h1>{/if}
      <div class="facts">
        {#if secondsOf(focused) >= 60}<span>{playtime(secondsOf(focused))} played</span>{/if}
        {#if lastOf(focused)}<span>Last played {when(lastOf(focused))}</span>{/if}
        {#if metaOf(focused.id).favourite}<span class="fav"><Icon name="star" size={14} filled /> Favourite</span>{/if}
      </div>
    </section>
  {/if}

  <div class="shelves">
    {#each shelves as s, r (s.title)}
      <section class="shelf" class:active={r === row} bind:this={shelfEls[r]}>
        <h2>{s.title}</h2>
        <div class="cards" bind:this={rows[r]}>
          {#each s.games as g, c (g.id)}
            {@const cv = coverOf(g)}
            <button
              class="card"
              class:on={r === row && c === at(r)}
              onclick={() => { row = r; col[r] = c; }}
              ondblclick={() => launch(g)}
              aria-label={nameOf(g)}
            >
              {#if cv && !broken.has(g.id)}<img src={cv} alt="" loading="lazy" onerror={() => (broken = new Set(broken).add(g.id))} />{:else}<span class="mark">{nameOf(g)}</span>{/if}
            </button>
          {/each}
        </div>
      </section>
    {/each}
  </div>

  <footer>
    <span><kbd>A</kbd><kbd>Enter</kbd> Play</span>
    <span><kbd>Y</kbd><kbd>F</kbd> Favourite</span>
    <span><kbd>LB</kbd><kbd>RB</kbd> Shelves</span>
    <span><kbd>B</kbd><kbd>Esc</kbd> Leave</span>
  </footer>
</div>

<style>
  .bp { position: fixed; inset: 0; z-index: 200; color: #fff; background: #07070a; display: flex; flex-direction: column; overflow: hidden; font-family: var(--body); }
  .bg { position: absolute; inset: -40px; background-size: cover; background-position: center; filter: blur(26px) saturate(1.25); opacity: 0.55; animation: fade 0.5s ease; }
  @keyframes fade { from { opacity: 0; } to { opacity: 0.55; } }
  .shade { position: absolute; inset: 0; background: linear-gradient(180deg, rgb(7 7 10 / 0.35), rgb(7 7 10 / 0.25) 30%, rgb(7 7 10 / 0.92) 75%); }
  header, .detail, .shelves, footer { position: relative; }
  header { display: flex; align-items: center; gap: 1.2rem; padding: 1.4rem 3rem 0; font-size: 1.05rem; }
  .brand { font-family: var(--display); font-weight: 800; letter-spacing: 0.04em; text-transform: uppercase; font-size: 1rem; opacity: 0.85; }
  .grow { flex: 1; }
  .pad { display: inline-flex; gap: 0.4rem; align-items: center; opacity: 0.8; font-size: 0.9rem; }
  .clock { font-variant-numeric: tabular-nums; font-weight: 600; }
  .exit { width: 42px; height: 42px; padding: 0; border-radius: 50%; display: grid; place-items: center; background: rgb(255 255 255 / 0.1); border-color: rgb(255 255 255 / 0.15); color: #fff; }
  .detail { padding: 1.4rem 3rem 0.4rem; min-height: 150px; display: flex; flex-direction: column; justify-content: flex-end; }
  .detail h1 { font-size: clamp(2.6rem, 5vw, 4.6rem); line-height: 0.95; letter-spacing: -0.03em; font-weight: 800; text-shadow: 0 4px 24px rgb(0 0 0 / 0.5); }
  .logo { max-width: 460px; max-height: 150px; object-fit: contain; object-position: left bottom; filter: drop-shadow(0 6px 24px rgb(0 0 0 / 0.5)); }
  .facts { display: flex; gap: 1.2rem; margin-top: 0.8rem; font-size: 1.05rem; opacity: 0.85; }
  .fav { color: #ffd66b; display: inline-flex; gap: 0.35rem; align-items: center; }
  .shelves { flex: 1; overflow-y: auto; padding: 0.6rem 0 2rem; scrollbar-width: none; mask-image: linear-gradient(to bottom, transparent 0, #000 28px, #000 calc(100% - 28px), transparent 100%); }
  .shelves::-webkit-scrollbar { display: none; }
  .shelf { scroll-margin: 1rem; padding: 0.6rem 0 0.2rem; opacity: 0.55; transition: opacity 0.2s; }
  .shelf.active { opacity: 1; }
  .shelf h2 { font-size: 1.2rem; padding: 0 3rem; margin-bottom: 0.7rem; font-weight: 700; }
  .cards { display: flex; gap: 1.1rem; overflow-x: auto; padding: 1.1rem 3rem 1.2rem; scrollbar-width: none; }
  .cards::-webkit-scrollbar { display: none; }
  .card { flex: none; width: clamp(110px, min(11vw, 16vh), 190px); aspect-ratio: 2 / 3; padding: 0; border-radius: 12px; overflow: hidden; border: 2px solid transparent; background: rgb(255 255 255 / 0.07); position: relative; transition: transform 0.18s ease, box-shadow 0.18s ease, border-color 0.18s; }
  .card img { width: 100%; height: 100%; object-fit: cover; }
  .card.on { transform: scale(1.1); border-color: #fff; box-shadow: 0 14px 40px rgb(0 0 0 / 0.55), 0 0 0 6px rgb(255 255 255 / 0.12); z-index: 1; }
  .mark { position: absolute; left: 0.7rem; right: 0.7rem; bottom: 0.9rem; font-family: var(--display); font-weight: 800; font-size: 1.3rem; line-height: 1.05; text-align: left; }
  footer { display: flex; gap: 2rem; padding: 0.9rem 3rem 1.4rem; font-size: 0.9rem; opacity: 0.75; }
  kbd { display: inline-block; min-width: 1.6em; padding: 0.05rem 0.4rem; margin-right: 0.3rem; border-radius: 6px; background: rgb(255 255 255 / 0.14); border: 1px solid rgb(255 255 255 / 0.2); font-family: var(--body); font-size: 0.78rem; font-weight: 700; text-align: center; }
  @media (prefers-reduced-motion: reduce) { .card, .bg { transition: none; animation: none; } }
</style>
