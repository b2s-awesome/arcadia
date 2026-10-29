<script lang="ts">
  // Trash Bin + Auto Skip settings.
  import { onMount } from "svelte";
  import Toggle from "../../components/Toggle.svelte";
  import { lib } from "../../music/store.svelte";
  import { initListening, lx, saveRules, untrashArtist, untrashSong, type SkipRules } from "./listening.svelte";

  onMount(() => {
    initListening();
  });
  type Rule = keyof Pick<SkipRules, "trashed" | "explicit" | "short" | "live" | "remix">;
  const rules: { id: Rule; label: string; desc: string }[] = [
    { id: "trashed", label: "Skip the trash", desc: "Songs and artists you've trashed." },
    { id: "explicit", label: "Skip explicit songs", desc: "Marked explicit in their tags, or “(Explicit)” in the title." },
    { id: "short", label: "Skip anything under 30 seconds", desc: "Intros, skits and interludes." },
    { id: "live", label: "Skip live versions", desc: "“(Live)”, “- Live” or “Live at” in the title." },
    { id: "remix", label: "Skip remixes", desc: "“Remix” in the title." },
  ];
  function set(id: Rule, v: boolean) {
    lx.rules[id] = v;
    saveRules();
  }
  const songs = $derived(lx.rules.trashSongs.map((id) => ({ id, t: lib.byId.get(id) })));
  async function empty() {
    lx.rules.trashSongs = [];
    lx.rules.trashArtists = [];
    await saveRules();
  }
</script>

<p class="note">Only applies when Arcadia moves on by itself. Picking a song yourself always plays it.</p>
{#each rules as r}
  <div class="setting"><div><div class="label">{r.label}</div><div class="desc">{r.desc}</div></div><Toggle label={r.label} checked={lx.rules[r.id]} onchange={(v) => set(r.id, v)} /></div>
{/each}

<div class="setting col">
  <div class="row full"><div class="label">In the trash</div><span class="grow"></span>{#if songs.length || lx.rules.trashArtists.length}<button onclick={empty}>Empty the trash</button>{/if}</div>
  {#if !songs.length && !lx.rules.trashArtists.length}
    <p class="note">Nothing yet. Right-click a song and choose Trash this song or Trash the artist.</p>
  {:else}
    <ul>
      {#each lx.rules.trashArtists as a}<li><span><b>{a}</b> <span class="muted">artist</span></span><button onclick={() => untrashArtist(a)}>Take out</button></li>{/each}
      {#each songs as s (s.id)}<li><span>{#if s.t}<b>{s.t.title}</b> <span class="muted">{s.t.artist}</span>{:else}<span class="muted">A song no longer in your library</span>{/if}</span><button onclick={() => untrashSong(s.id)}>Take out</button></li>{/each}
    </ul>
  {/if}
</div>

<style>
  .note { color: var(--muted); font-size: 0.86rem; padding: 0.3rem 0; }
  .setting.col { flex-direction: column; align-items: stretch; gap: 0.5rem; }
  .full { display: flex; align-items: center; gap: 0.6rem; }
  .grow { flex: 1; }
  ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.3rem; max-height: 260px; overflow-y: auto; }
  li { display: flex; justify-content: space-between; align-items: center; gap: 0.8rem; padding: 0.35rem 0.5rem; border-radius: var(--r-sm); background: var(--sunk); }
  li button { padding: 0.25rem 0.7rem; font-size: 0.82rem; }
</style>
