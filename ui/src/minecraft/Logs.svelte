<script lang="ts">
  // Live output while the game runs, saved logs and crash reports, and the
  // crash explainer.
  import { mc } from "../lib/api";
  import { bytes, toast } from "../lib/state.svelte";
  import { ago, mcs } from "../lib/mc.svelte";
  import type { McCrashInfo, McInstance, McLogFile } from "../lib/types";
  import Icon from "../components/Icon.svelte";
  import CrashCard from "./CrashCard.svelte";

  let { inst, running }: { inst: McInstance; running: boolean } = $props();

  let files = $state<McLogFile[]>([]);
  let which = $state<string>("live");
  let text = $state("");
  let info = $state<McCrashInfo | null>(null);
  let filter = $state<"all" | "warn" | "error">("all");
  let q = $state("");
  let follow = $state(true);
  let pre: HTMLPreElement | undefined = $state();

  async function load() {
    files = await mc.logs(inst.id).catch(() => []);
    if (!running && which === "live" && files.length) which = files[0].path;
  }
  $effect(() => {
    void inst.id;
    load();
  });
  $effect(() => {
    const w = which;
    info = null;
    if (w === "live") return;
    mc.logRead(inst.id, w).then((t) => (text = t)).catch((e) => (text = String(e)));
    const f = files.find((x) => x.path === w);
    if (f?.kind !== "log" || w.endsWith("latest.log")) mc.explain(inst.id, w).then((i) => (info = i)).catch(() => {});
  });

  const raw = $derived(which === "live" ? (mcs.logs[inst.id] ?? []) : text.split("\n"));
  const lines = $derived.by(() => {
    const term = q.trim().toLowerCase();
    return raw.filter((l) => {
      if (filter === "error" && !/\b(ERROR|FATAL|Exception|Caused by)\b/.test(l)) return false;
      if (filter === "warn" && !/\b(WARN|ERROR|FATAL|Exception)\b/.test(l)) return false;
      return !term || l.toLowerCase().includes(term);
    });
  });
  $effect(() => {
    void lines.length;
    if (follow && pre) queueMicrotask(() => pre && (pre.scrollTop = pre.scrollHeight));
  });
  const tone = (l: string) => (/\b(ERROR|FATAL|Exception|Caused by)\b/.test(l) ? "err" : /\bWARN\b/.test(l) ? "warn" : "");
  const label = (f: McLogFile) => (f.kind === "crash" ? "Crash report" : f.kind === "jvm" ? "Java crash" : f.path.replace("logs/", ""));

  async function copy() {
    await navigator.clipboard.writeText(raw.join("\n"));
    toast("Log copied");
  }
</script>

<div class="row spread bar">
  <h3>Logs & crashes</h3>
  <button class="ghost" onclick={() => mc.openFolder(inst.id, "logs")}><Icon name="folder" size={15} /> Logs folder</button>
</div>

<div class="files" role="radiogroup" aria-label="Log">
  <button class="file" role="radio" aria-checked={which === "live"} onclick={() => (which = "live")}>
    <span class="dot" class:on={running}></span> Live output {#if running}<span class="faint">running</span>{/if}
  </button>
  {#each files as f (f.path)}
    <button class="file" class:crash={f.kind !== "log"} role="radio" aria-checked={which === f.path} onclick={() => (which = f.path)} title={f.path}>
      {#if f.kind !== "log"}<Icon name="shield" size={13} />{/if}
      {label(f)} <span class="faint">{ago(f.modified)} · {bytes(f.size)}</span>
    </button>
  {/each}
</div>

{#if info}
  <div class="explain"><CrashCard {info} /></div>
{/if}

<div class="row tools">
  <input class="grow" placeholder="Search the log" bind:value={q} aria-label="Search the log" />
  <div class="seg" role="radiogroup" aria-label="Show">
    <button role="radio" aria-checked={filter === "all"} onclick={() => (filter = "all")}>All</button>
    <button role="radio" aria-checked={filter === "warn"} onclick={() => (filter = "warn")}>Warnings</button>
    <button role="radio" aria-checked={filter === "error"} onclick={() => (filter = "error")}>Errors</button>
  </div>
  <button class="ghost" onclick={copy} disabled={!raw.length}><Icon name="copy" size={15} /> Copy</button>
</div>

{#if lines.length === 0}
  <p class="none">{which === "live" ? (running ? "Waiting for output…" : "Start the game to see its output here.") : "Nothing matches."}</p>
{:else}
  <pre bind:this={pre} onscroll={() => pre && (follow = pre.scrollTop + pre.clientHeight >= pre.scrollHeight - 20)}>{#each lines as l}<span class={tone(l)}>{l}
</span>{/each}</pre>
{/if}

<style>
  .bar { margin-bottom: 0.6rem; }
  .bar button, .tools button { display: inline-flex; align-items: center; gap: 0.35rem; }
  .files { display: flex; flex-wrap: wrap; gap: 0.35rem; margin-bottom: 0.8rem; }
  .file { display: inline-flex; align-items: center; gap: 0.4rem; padding: 0.3rem 0.7rem; border-radius: 999px; font-size: 0.82rem; font-weight: 600; background: var(--sunk); }
  .file .faint { font-weight: 500; }
  .file[aria-checked="true"] { background: color-mix(in srgb, var(--lilac) 18%, var(--raised)); border-color: color-mix(in srgb, var(--lilac) 45%, transparent); }
  .file.crash { color: var(--rose); }
  .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--faint); }
  .dot.on { background: var(--mint); box-shadow: 0 0 0 3px color-mix(in srgb, var(--mint) 25%, transparent); }
  .explain { margin-bottom: 0.9rem; }
  .tools { margin-bottom: 0.5rem; }
  .none { color: var(--muted); padding: 1.2rem 0; }
  pre {
    margin: 0; height: min(52vh, 520px); overflow: auto;
    background: var(--sunk); border: 1px solid var(--line-soft); border-radius: var(--r-md);
    padding: 0.7rem 0.8rem; font-size: 0.74rem; line-height: 1.5; color: var(--text);
    white-space: pre-wrap; word-break: break-word; user-select: text;
  }
  .err { color: var(--rose); }
  .warn { color: var(--peach); }
</style>
