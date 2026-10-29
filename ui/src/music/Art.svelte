<script lang="ts">
  // A cover, or a coloured tile with the first letter when there isn't one.
  import { artUrl } from "./store.svelte";
  let { src, name = "", color = null, size = null, radius = "var(--r-md)", round = false }: { src: string | null; name?: string; color?: string | null; size?: number | null; radius?: string; round?: boolean } = $props();
  let broken = $state(false);
  const url = $derived(broken ? null : artUrl(src));
  $effect(() => {
    void src;
    broken = false;
  });
</script>

<div class="art" class:round style:width={size ? `${size}px` : undefined} style:height={size ? `${size}px` : undefined} style:border-radius={round ? "50%" : radius} style:--c={color ?? "var(--raised)"}>
  {#if url}
    <img src={url} alt="" loading="lazy" decoding="async" draggable="false" onerror={() => (broken = true)} />
  {:else}
    <span aria-hidden="true">{(name.trim()[0] ?? "♪").toUpperCase()}</span>
  {/if}
</div>

<style>
  .art {
    position: relative; overflow: hidden; flex-shrink: 0;
    aspect-ratio: 1; width: 100%;
    background: linear-gradient(140deg, color-mix(in srgb, var(--c) 70%, black), var(--c));
    display: grid; place-items: center;
  }
  img { width: 100%; height: 100%; object-fit: cover; display: block; }
  span { font-family: var(--display); font-weight: 800; color: rgba(255, 255, 255, 0.85); font-size: 2.2em; }
</style>
