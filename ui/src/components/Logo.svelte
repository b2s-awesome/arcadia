<script lang="ts">
  // Arcadia's mark: a geometric lowercase "a" with a dot. Colours follow the
  // current look (accent for the letter, the "on" colour for the dot).
  let { size = 20 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true">
  <circle cx="28" cy="38" r="13" fill="none" stroke="var(--peach)" stroke-width="8" />
  <rect x="37" y="21" width="8" height="34" rx="1" fill="var(--peach)" />
  <circle cx="52" cy="15" r="5" fill="var(--mint)" />
</svg>
