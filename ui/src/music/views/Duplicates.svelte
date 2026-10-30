<script lang="ts">
  // Duplicate songs, grouped, each with how sure Arcadia is. The best copy
  // (lossless, then higher quality, then most played) comes first and is
  // kept; the rest can go to the Recycle Bin, so nothing is lost for good.
  import { onMount } from "svelte";
  import Icon from "../../components/Icon.svelte";
  import Modal from "../../components/Modal.svelte";
  import { music } from "../../lib/api";
  import { toast } from "../../lib/state.svelte";
  import type { DupeGroup, DupePair, Track } from "../../lib/types";
  import Art from "../Art.svelte";
  import { ms, play, quality, reloadLibrary, time } from "../store.svelte";

  let min = $state(85);
  let groups: DupeGroup[] = $state([]);
  let busy = $state(false);
  let done = $state(false);
  /** Group index → ids ticked for removal. */
  let picked: Record<number, number[]> = $state({});
  let ignored = $state(new Set<string>());

  const byId = $derived(new Map(ms.tracks.map((t) => [t.id, t])));
  const shown = $derived(groups.filter((g) => !ignored.has(g.ids.join(","))));

  // Only the newest answer counts, if the slider moved while one was out.
  let run = 0;
  async function find() {
    const mine = ++run;
    busy = true;
    try {
      const g = await music.duplicates(min);
      if (mine !== run) return;
      groups = g;
      // Tick everything but the best copy, for groups we're very sure of.
      picked = Object.fromEntries(groups.map((g, i) => [i, g.certainty >= 95 ? g.ids.slice(1) : []]));
      done = true;
    } catch (e) {
      toast(String(e), "error");
    } finally {
      busy = false;
    }
  }
  onMount(find);
  let timer: ReturnType<typeof setTimeout> | undefined;
  function setMin(v: number) {
    min = v;
    clearTimeout(timer);
    timer = setTimeout(find, 250);
  }

  const level = (c: number) => (c >= 95 ? "Almost certain" : c >= 85 ? "Likely" : c >= 70 ? "Possible" : "Unlikely");
  const pct = (x: number) => `${Math.round(x * 100)}%`;
  function why(p: DupePair): string {
    if (p.sameFileSize) return "Same size and length: an exact copy";
    const bits = [`title ${pct(p.title)}`, `artist ${pct(p.artist)}`, `length ${pct(p.length)}`, `file name ${pct(p.file)}`];
    return bits.join(" · ") + (p.versionClash ? " · marked as a different version" : "");
  }
  function pairFor(g: DupeGroup, id: number): DupePair | undefined {
    return g.pairs.find((p) => (p.a === id || p.b === id) && (p.a === g.ids[0] || p.b === g.ids[0])) ?? g.pairs.find((p) => p.a === id || p.b === id);
  }
  const folder = (t: Track) => t.path.replace(/[\\/][^\\/]*$/, "");

  function toggle(gi: number, id: number) {
    const cur = picked[gi] ?? [];
    picked[gi] = cur.includes(id) ? cur.filter((x) => x !== id) : [...cur, id];
  }
  const total = $derived(shown.reduce((n, g) => n + (picked[groups.indexOf(g)]?.length ?? 0), 0));

  let confirm = $state(false);
  async function recycle() {
    confirm = false;
    const ids = shown.flatMap((g) => picked[groups.indexOf(g)] ?? []);
    if (!ids.length) return;
    try {
      const n = await music.recycle(ids);
      toast(`Moved ${n} song${n === 1 ? "" : "s"} to the Recycle Bin`);
      await reloadLibrary();
      await find();
    } catch (e) {
      toast(String(e), "error");
    }
  }
</script>

<div class="bar">
  <h2>Duplicates</h2>
  <span class="muted">{done ? `${shown.length} group${shown.length === 1 ? "" : "s"}` : ""}</span>
  <span class="grow"></span>
  <label class="min">
    <span class="muted">Show from</span>
    <input type="range" min="50" max="99" step="1" value={min} oninput={(e) => setMin(+e.currentTarget.value)} aria-label="Lowest certainty to show" />
    <span class="mono">{min}%</span>
  </label>
  <button class="danger" disabled={!total} onclick={() => (confirm = true)}><Icon name="trash" size={15} /> Recycle {total || ""}</button>
</div>
<p class="muted how">
  Each pair gets a certainty from its title, artist, length and file name. Songs marked as another version (slowed, sped up, remix, live…) don't count as copies.
  The best copy in each group is listed first and kept.
</p>

{#if busy && !done}
  <p class="muted pad">Comparing {ms.tracks.length.toLocaleString()} songs…</p>
{:else if done && !shown.length}
  <p class="muted pad">No duplicates at {min}% or more. Lower the slider to see less certain matches.</p>
{/if}

{#each shown as g (g.ids.join(","))}
  {@const gi = groups.indexOf(g)}
  <section class="group">
    <header>
      <span class="meter" style:--c="{g.certainty}%" aria-hidden="true"></span>
      <strong>{g.certainty.toFixed(g.certainty >= 99 ? 1 : 0)}%</strong>
      <span class="lvl" class:hi={g.certainty >= 95} class:mid={g.certainty >= 85 && g.certainty < 95}>{level(g.certainty)}</span>
      <span class="grow"></span>
      <button class="ghost small" onclick={() => { ignored = new Set([...ignored, g.ids.join(",")]); }}>Not duplicates</button>
    </header>
    {#each g.ids as id, k (id)}
      {@const t = byId.get(id)}
      {#if t}
        {@const p = k ? pairFor(g, id) : null}
        <div class="row" class:keep={k === 0} class:gone={picked[gi]?.includes(id)}>
          <label class="tick" title={k === 0 ? "Best copy: kept" : "Move to the Recycle Bin"}>
            <input type="checkbox" disabled={k === 0} checked={picked[gi]?.includes(id) ?? false} onchange={() => toggle(gi, id)} aria-label="Remove {t.title} from {folder(t)}" />
          </label>
          <button class="art" onclick={() => play([t], 0)} aria-label="Play {t.title}"><Art src={t.art} name={t.album} color={t.artColor} size={40} radius="6px" /></button>
          <div class="txt">
            <span class="t">{t.title} {#if k === 0}<span class="best">Best copy</span>{/if}</span>
            <span class="muted s">{t.artist} · {t.album} · {time(t.duration)} · {quality(t)}{t.playCount ? ` · ${t.playCount} plays` : ""}</span>
            <span class="faint s path" title={t.path}>{t.path}</span>
            {#if p}<span class="faint s why">{p.certainty.toFixed(0)}% vs best · {why(p)}</span>{/if}
          </div>
        </div>
      {/if}
    {/each}
  </section>
{/each}

<Modal bind:open={confirm} title={`Recycle ${total} song${total === 1 ? "" : "s"}?`}>
  <p>Their files go to the Recycle Bin, so you can restore them from there, and they leave your library and playlists. The best copy of each song stays.</p>
  {#snippet actions()}
    <button onclick={() => (confirm = false)}>Keep them</button>
    <button class="danger" onclick={recycle}>Recycle</button>
  {/snippet}
</Modal>

<style>
  .bar { display: flex; align-items: center; gap: 0.7rem; margin-bottom: 0.4rem; flex-wrap: wrap; }
  .bar h2 { font-size: 1.5rem; }
  .grow { flex: 1; }
  .how { font-size: 0.84rem; margin-bottom: 1rem; max-width: 72ch; }
  .min { display: flex; align-items: center; gap: 0.5rem; font-size: 0.85rem; }
  .mono { font-family: "JetBrains Mono Variable", Consolas, monospace; width: 3ch; }
  .danger { display: inline-flex; align-items: center; gap: 0.35rem; }
  .pad { padding: 1rem 0.2rem; }
  .group { border: 1px solid var(--line); border-radius: var(--r-md); padding: 0.6rem 0.7rem; margin-bottom: 0.7rem; background: var(--group, transparent); }
  .group header { display: flex; align-items: center; gap: 0.6rem; margin-bottom: 0.4rem; }
  .meter { width: 70px; height: 6px; border-radius: 6px; background: linear-gradient(90deg, var(--m-accent, var(--peach)) var(--c), var(--sunk) var(--c)); }
  .lvl { font-size: 0.78rem; color: var(--muted); padding: 0.05rem 0.5rem; border-radius: 999px; border: 1px solid var(--line); }
  .lvl.hi { color: var(--mint); border-color: color-mix(in srgb, var(--mint) 40%, transparent); }
  .lvl.mid { color: var(--peach); border-color: color-mix(in srgb, var(--peach) 40%, transparent); }
  .small { font-size: 0.78rem; padding: 0.2rem 0.6rem; }
  .row { display: grid; grid-template-columns: 26px 40px minmax(0, 1fr); gap: 0.7rem; align-items: center; padding: 0.4rem 0.2rem; border-top: 1px solid var(--line-soft); }
  .row.gone .txt { opacity: 0.5; text-decoration: line-through; text-decoration-color: var(--rose); }
  .tick { display: grid; place-items: center; }
  .art { border: 0; padding: 0; background: none; }
  .txt { display: flex; flex-direction: column; min-width: 0; line-height: 1.3; }
  .t { font-weight: 600; }
  .s { font-size: 0.8rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .path { font-family: "JetBrains Mono Variable", Consolas, monospace; font-size: 0.74rem; }
  .best { font-size: 0.7rem; font-weight: 700; color: var(--m-ink, var(--peach-ink)); background: var(--m-accent, var(--peach)); border-radius: 999px; padding: 0.05rem 0.45rem; margin-left: 0.3rem; vertical-align: 2px; }
</style>
