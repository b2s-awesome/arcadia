<script lang="ts">
  // A game icon or account picture, or a soft colour tile with initials
  // when there's no picture (or it won't load).
  import { hue, initials } from "./rbx.svelte";

  let { src = null, seed, label, size = 48, round = false }: {
    src?: string | null; seed: number | string; label: string; size?: number; round?: boolean;
  } = $props();
  let broken = $state(false);
  $effect(() => {
    void src;
    broken = false;
  });
  const h = $derived(hue(seed));
</script>

<span
  class="thumb"
  class:round
  style:width="{size}px"
  style:height="{size}px"
  style:--h={h}
  style:font-size="{Math.max(10, size * 0.34)}px"
  aria-hidden="true"
>
  {#if src && !broken}
    <img {src} alt="" loading="lazy" draggable="false" onerror={() => (broken = true)} />
  {:else}
    <span class="ini">{initials(label)}</span>
  {/if}
</span>

<style>
  .thumb {
    position: relative;
    display: inline-grid;
    place-items: center;
    flex: none;
    overflow: hidden;
    border-radius: 22%;
    background:
      radial-gradient(120% 90% at 20% 10%, hsl(var(--h) 70% 72% / 0.55), transparent 60%),
      linear-gradient(145deg, hsl(var(--h) 45% 34%), hsl(calc(var(--h) + 40) 50% 20%));
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.08);
  }
  .round { border-radius: 50%; }
  img { width: 100%; height: 100%; object-fit: cover; }
  .ini { font-family: var(--display); font-weight: 700; color: hsl(var(--h) 60% 92%); letter-spacing: 0.02em; }
</style>
