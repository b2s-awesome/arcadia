<script lang="ts">
  // Lyrics in Now Playing's side panel.
  import { onMount } from "svelte";
  import { ms } from "../../music/store.svelte";
  import Lines from "./Lines.svelte";
  import { ly, useLyrics } from "./lyrics.svelte";
  onMount(() => useLyrics());
  const l = $derived(ly.id === ms.status?.track?.id ? ly.lyrics : null);
</script>

{#if ly.loading}
  <p class="muted">Looking for lyrics…</p>
{:else if l?.instrumental}
  <p class="muted">♪ Instrumental</p>
{:else if l}
  <Lines lyrics={l} size={30} />
{:else}
  <p class="muted">No lyrics for this song. Open Lyrics in Music to write them.</p>
{/if}
