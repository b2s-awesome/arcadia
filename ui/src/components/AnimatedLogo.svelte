<script lang="ts">
  // The Monogram building itself: the ring draws, the stem rises, the dot
  // drops in with a bounce, then the name slides in.
  let { size = 120, name = "Arcadia" }: { size?: number; name?: string } = $props();
</script>

<div class="lockup" style:--s="{size}px">
  <svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true">
    <circle class="ring" cx="28" cy="38" r="13" fill="none" stroke="var(--peach)" stroke-width="8" pathLength="100" />
    <rect class="stem" x="37" y="21" width="8" height="34" rx="1" fill="var(--peach)" />
    <circle class="dot" cx="52" cy="15" r="5" fill="var(--mint)" />
  </svg>
  <span class="name">{name}</span>
</div>

<style>
  .lockup { display: flex; align-items: center; gap: calc(var(--s) * 0.18); }
  .name {
    font-family: var(--display);
    font-weight: 800;
    font-size: calc(var(--s) * 0.56);
    letter-spacing: -0.03em;
    color: var(--text);
    line-height: 1;
    opacity: 0;
    animation: name 0.45s cubic-bezier(0.2, 0.8, 0.2, 1) 0.85s forwards;
  }

  /* Starts where the ring meets the stem and draws round. */
  .ring {
    stroke-dasharray: 100;
    stroke-dashoffset: 100;
    transform: rotate(-20deg);
    transform-box: fill-box;
    transform-origin: center;
    animation: draw 0.55s cubic-bezier(0.6, 0, 0.3, 1) forwards;
  }
  .stem {
    transform: scaleY(0);
    transform-box: fill-box;
    transform-origin: center bottom;
    animation: rise 0.35s cubic-bezier(0.3, 1.4, 0.5, 1) 0.32s forwards;
  }
  .dot {
    opacity: 0;
    transform-box: fill-box;
    transform-origin: center;
    animation: drop 0.6s linear 0.55s forwards;
  }

  @keyframes draw { to { stroke-dashoffset: 0; } }
  @keyframes rise { to { transform: scaleY(1); } }
  @keyframes drop {
    0% { opacity: 0; transform: translateY(-26px) scale(0.6); animation-timing-function: cubic-bezier(0.5, 0, 1, 0.6); }
    45% { opacity: 1; transform: translateY(0) scale(1.15, 0.8); animation-timing-function: cubic-bezier(0, 0.5, 0.5, 1); }
    70% { transform: translateY(-6px) scale(0.95, 1.05); animation-timing-function: cubic-bezier(0.5, 0, 1, 0.6); }
    85% { transform: translateY(0) scale(1.05, 0.95); }
    100% { opacity: 1; transform: translateY(0) scale(1); }
  }
  @keyframes name {
    from { opacity: 0; transform: translateX(-10px); }
    to { opacity: 1; transform: none; }
  }

  @media (prefers-reduced-motion: reduce) {
    .ring, .stem, .dot, .name { animation: none !important; stroke-dashoffset: 0; transform: none; opacity: 1; }
  }
</style>
