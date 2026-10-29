<script lang="ts">
  // "Sort & Play ▾" for any list of songs.
  import Icon from "../../components/Icon.svelte";
  import type { Track } from "../../lib/types";
  import { openMenu } from "../../music/contextMenu.svelte";
  import { SORTS, sortAndPlay } from "./listening.svelte";
  let { tracks, cls = "ghosty" }: { tracks: Track[]; cls?: string } = $props();
</script>

<button class={cls} disabled={!tracks.length} aria-haspopup="menu" onclick={(e) => openMenu(e, SORTS.map((s) => ({ label: s.label, run: () => sortAndPlay(tracks, s.id) })))}>
  <Icon name="sort" size={15} /> Sort &amp; Play <Icon name="down" size={13} />
</button>
