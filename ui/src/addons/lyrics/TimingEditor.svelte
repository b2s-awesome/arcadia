<script lang="ts">
  // Fix or write lyrics: edit the words, then tap along with the song to
  // time each line. Saved as LRC; it wins over anything found online.
  import { onMount } from "svelte";
  import Icon from "../../components/Icon.svelte";
  import { lyricsApi, music } from "../../lib/api";
  import { app, toast } from "../../lib/state.svelte";
  import type { Lyrics } from "../../lib/types";
  import { ms, seek, time } from "../../music/store.svelte";
  import { stamp, toLrc } from "./lrc";
  import { replaceLyrics } from "./lyrics.svelte";

  let { id, start, onclose }: { id: number; start: Lyrics | null; onclose: () => void } = $props();

  let step: "words" | "tap" = $state("words");
  let text = $state("");
  let rows: { t: number | null; text: string }[] = $state([]);
  let at = $state(0);
  let saving = $state(false);

  onMount(() => {
    text = (start?.lines ?? []).map((l) => l.text).join("\n");
  });

  function beginTap() {
    const lines = text.split(/\r?\n/).map((s) => s.trim());
    while (lines.length && !lines[lines.length - 1]) lines.pop();
    if (!lines.length) {
      toast("Type or paste the words first, one line per line.", "error");
      return;
    }
    // Keep existing times where the words still line up.
    rows = lines.map((s, i) => ({ text: s, t: start?.lines[i]?.text === s ? (start.lines[i].t ?? null) : null }));
    at = 0;
    step = "tap";
    seek(0);
    if (!ms.status?.playing) music.pause(false).catch(() => {});
  }

  function tap() {
    if (at >= rows.length) return;
    rows[at].t = Math.round(ms.pos * 100) / 100;
    at++;
  }
  function undo() {
    if (at === 0) return;
    at--;
    rows[at].t = null;
    // Back to just before that line, to try again.
    const prev = rows[at - 1]?.t ?? 0;
    seek(Math.max(0, prev - 1));
  }
  function keys(e: KeyboardEvent) {
    if (step !== "tap") return;
    if (e.key === " " || e.key === "t" || e.key === "Enter") {
      e.preventDefault();
      e.stopImmediatePropagation();
      tap();
    } else if (e.key === "Backspace") {
      e.preventDefault();
      e.stopImmediatePropagation();
      undo();
    }
  }

  async function save() {
    // Lines left untimed take their neighbour's time so nothing is lost.
    let last = 0;
    const out = rows.map((r) => ({ text: r.text, t: (last = r.t ?? last) }));
    saving = true;
    try {
      const l = await lyricsApi.save(id, toLrc(out), app.ui.lyrics.sidecar);
      replaceLyrics(id, l);
      toast(app.ui.lyrics.sidecar ? "Lyrics saved, and written beside the song as .lrc" : "Lyrics saved");
      onclose();
    } catch (e) {
      toast(String(e), "error");
    } finally {
      saving = false;
    }
  }
</script>

<svelte:window onkeydowncapture={keys} />

<div class="ed">
  <header>
    <h3>{step === "words" ? "Words" : "Tap along"}</h3>
    <span class="muted">{step === "words" ? "One line per line. Blank lines are pauses." : "Press Space (or T) as each line starts. Backspace undoes."}</span>
    <span class="grow"></span>
    <button onclick={onclose}>Cancel</button>
    {#if step === "words"}
      <button class="go" onclick={beginTap}>Next: timing <Icon name="play" size={13} filled /></button>
    {:else}
      <button onclick={() => (step = "words")}>Back to words</button>
      <button class="go" onclick={save} disabled={saving}>{saving ? "Saving…" : "Save"}</button>
    {/if}
  </header>
  {#if step === "words"}
    <textarea bind:value={text} spellcheck="false" aria-label="Lyrics, one line per line" placeholder="Paste or type the lyrics here"></textarea>
  {:else}
    <div class="tap">
      <ol>
        {#each rows as r, i}
          <li class:cur={i === at} class:done={r.t !== null && i < at}><span class="ts">{r.t === null ? "--:--.--" : stamp(r.t)}</span><span>{r.text || "♪"}</span></li>
        {/each}
      </ol>
      <div class="pad">
        <span class="now mono">{time(ms.pos)}</span>
        <button class="big" onclick={tap} disabled={at >= rows.length}>{at >= rows.length ? "All timed" : "Stamp this line"}</button>
        <button onclick={undo} disabled={at === 0}>Undo</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .ed { display: flex; flex-direction: column; gap: 0.8rem; height: 100%; min-height: 0; }
  header { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; }
  header h3 { font-size: 1.1rem; }
  .grow { flex: 1; }
  .go { background: var(--m-accent, var(--peach)); color: var(--m-ink, var(--peach-ink)); border-color: transparent; display: inline-flex; gap: 0.4rem; align-items: center; }
  textarea { flex: 1; min-height: 240px; resize: none; font: inherit; font-size: 1rem; line-height: 1.6; padding: 0.9rem; border-radius: var(--r-md); }
  .tap { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(0, 1fr) 220px; gap: 1rem; }
  ol { list-style: none; margin: 0; padding: 0; overflow-y: auto; border: 1px solid var(--line); border-radius: var(--r-md); }
  li { display: flex; gap: 0.9rem; padding: 0.45rem 0.8rem; color: var(--muted); }
  li.done { color: var(--text); }
  li.cur { background: color-mix(in srgb, var(--m-accent, var(--peach)) 18%, transparent); color: var(--text); font-weight: 700; }
  .ts { font-family: "JetBrains Mono Variable", Consolas, monospace; font-size: 0.8rem; color: var(--faint); width: 5.6rem; flex-shrink: 0; }
  .pad { display: flex; flex-direction: column; gap: 0.6rem; }
  .now { font-size: 2rem; font-weight: 700; }
  .big { padding: 1.2rem; font-size: 1rem; font-weight: 800; background: var(--m-accent, var(--peach)); color: var(--m-ink, var(--peach-ink)); border-color: transparent; }
</style>
