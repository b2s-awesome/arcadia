<script lang="ts" module>
  // Texture rectangles for each part of the player (64×64 skin layout).
  type Box = { u: number; v: number; w: number; h: number; d: number };
  const faces = (b: Box) => ({
    top: [b.u + b.d, b.v, b.w, b.d],
    bottom: [b.u + b.d + b.w, b.v, b.w, b.d],
    right: [b.u, b.v + b.d, b.d, b.h],
    front: [b.u + b.d, b.v + b.d, b.w, b.h],
    left: [b.u + b.d + b.w, b.v + b.d, b.d, b.h],
    back: [b.u + b.d + b.w + b.d, b.v + b.d, b.w, b.h],
  });
</script>

<script lang="ts">
  // A small 3D player built from CSS boxes: drag to turn it, it sways when
  // left alone, and it can walk. No WebGL, so it stays light.
  let {
    src,
    variant = "classic",
    cape = null,
    scale = 8,
    outer = true,
    walk = false,
    spin = true,
  }: { src: string | null; variant?: "classic" | "slim"; cape?: string | null; scale?: number; outer?: boolean; walk?: boolean; spin?: boolean } = $props();

  // Old 64×32 skins have no separate left limbs or outer layers.
  let legacy = $state(false);
  $effect(() => {
    legacy = false;
    if (!src) return;
    const img = new Image();
    img.onload = () => (legacy = img.naturalHeight === 32);
    img.src = src;
  });

  const aw = $derived(variant === "slim" ? 3 : 4);
  const parts = $derived.by(() => {
    const L = legacy;
    return [
      { id: "head", x: 0, y: -10, base: { u: 0, v: 0, w: 8, h: 8, d: 8 }, over: { u: 32, v: 0, w: 8, h: 8, d: 8 }, grow: 1.125, pivot: false },
      { id: "body", x: 0, y: 0, base: { u: 16, v: 16, w: 8, h: 12, d: 4 }, over: L ? null : { u: 16, v: 32, w: 8, h: 12, d: 4 }, grow: 1.06, pivot: false },
      { id: "arm-r", x: -(4 + aw / 2), y: 0, base: { u: 40, v: 16, w: aw, h: 12, d: 4 }, over: L ? null : { u: 40, v: 32, w: aw, h: 12, d: 4 }, grow: 1.08, pivot: true },
      { id: "arm-l", x: 4 + aw / 2, y: 0, base: L ? { u: 40, v: 16, w: aw, h: 12, d: 4 } : { u: 32, v: 48, w: aw, h: 12, d: 4 }, over: L ? null : { u: 48, v: 48, w: aw, h: 12, d: 4 }, grow: 1.08, pivot: true },
      { id: "leg-r", x: -2, y: 12, base: { u: 0, v: 16, w: 4, h: 12, d: 4 }, over: L ? null : { u: 0, v: 32, w: 4, h: 12, d: 4 }, grow: 1.08, pivot: true },
      { id: "leg-l", x: 2, y: 12, base: L ? { u: 0, v: 16, w: 4, h: 12, d: 4 } : { u: 16, v: 48, w: 4, h: 12, d: 4 }, over: L ? null : { u: 0, v: 48, w: 4, h: 12, d: 4 }, grow: 1.08, pivot: true },
    ];
  });

  // Turning: drag, or a slow sway when idle.
  let yaw = $state(-28);
  let pitch = $state(-8);
  let dragging = $state(false);
  let last = { x: 0, y: 0 };
  function down(e: PointerEvent) {
    dragging = true;
    last = { x: e.clientX, y: e.clientY };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function move(e: PointerEvent) {
    if (!dragging) return;
    yaw += (e.clientX - last.x) * 0.6;
    pitch = Math.max(-40, Math.min(40, pitch - (e.clientY - last.y) * 0.4));
    last = { x: e.clientX, y: e.clientY };
  }
  const H = $derived(32 * scale);
  const tex = (r: number[]) => `background-position:${-r[0] * scale}px ${-r[1] * scale}px; width:${r[2] * scale}px; height:${r[3] * scale}px;`;
</script>

{#snippet box(b: Box, img: string, grow: number, cls: string)}
  {@const f = faces(b)}
  {@const W = b.w * scale * grow}
  {@const Hh = b.h * scale * grow}
  {@const D = b.d * scale * grow}
  <div class="box {cls}" style="--img:url('{img}'); --bs:{64 * scale}px; transform: scale3d({grow},{grow},{grow});">
    <i class="f" style="{tex(f.front)} transform: translate(-50%,-50%) translateZ({D / 2 / grow}px);"></i>
    <i class="f" style="{tex(f.back)} transform: translate(-50%,-50%) rotateY(180deg) translateZ({D / 2 / grow}px);"></i>
    <i class="f" style="{tex(f.right)} transform: translate(-50%,-50%) rotateY(-90deg) translateZ({W / 2 / grow}px);"></i>
    <i class="f" style="{tex(f.left)} transform: translate(-50%,-50%) rotateY(90deg) translateZ({W / 2 / grow}px);"></i>
    <i class="f" style="{tex(f.top)} transform: translate(-50%,-50%) rotateX(90deg) translateZ({Hh / 2 / grow}px);"></i>
    <i class="f" style="{tex(f.bottom)} transform: translate(-50%,-50%) rotateX(-90deg) translateZ({Hh / 2 / grow}px);"></i>
  </div>
{/snippet}

<div
  class="stage"
  class:dragging
  style:width="{20 * scale}px"
  style:height="{H + 4 * scale}px"
  onpointerdown={down}
  onpointermove={move}
  onpointerup={() => (dragging = false)}
  onpointercancel={() => (dragging = false)}
  role="img"
  aria-label="3D preview of the skin. Drag to turn it."
>
  <div class="shadow" style:width="{12 * scale}px" style:height="{4 * scale}px"></div>
  <div class="turn" class:sway={spin && !dragging} style="transform: rotateX({pitch}deg) rotateY({yaw}deg);">
    <div class="model" class:walk style="transform: translateY({2 * scale}px);">
      {#if src}
        {#each parts as p (p.id)}
          <div class="part {p.id}" class:pivot={p.pivot} style="transform: translate3d({p.x * scale}px, {p.y * scale}px, 0); --top:{-p.base.h * scale / 2}px;">
            <div class="limb">
              {@render box(p.base, src, 1, "base")}
              {#if outer && p.over}{@render box(p.over, src, p.grow, "over")}{/if}
            </div>
          </div>
        {/each}
        {#if cape}
          <div class="part cape" style="transform: translate3d(0, {-6 * scale}px, {-2.6 * scale}px);">
            <div class="limb capeswing">
              <div class="box" style="--img:url('{cape}'); --bs:{64 * scale}px; transform: translateY({8 * scale}px);">
                <i class="f" style="{tex([12, 1, 10, 16])} transform: translate(-50%,-50%) translateZ({scale / 2}px) rotateY(180deg);"></i>
                <i class="f" style="{tex([1, 1, 10, 16])} transform: translate(-50%,-50%) translateZ({-scale / 2}px);"></i>
              </div>
            </div>
          </div>
        {/if}
      {/if}
    </div>
  </div>
</div>

<style>
  .stage { position: relative; perspective: 900px; cursor: grab; touch-action: none; user-select: none; display: grid; place-items: center; }
  .stage.dragging { cursor: grabbing; }
  .shadow { position: absolute; bottom: 2%; left: 50%; transform: translateX(-50%); border-radius: 50%; background: radial-gradient(closest-side, rgb(0 0 0 / 0.35), transparent); }
  .turn, .model, .part, .limb, .box { position: absolute; left: 50%; top: 50%; transform-style: preserve-3d; }
  .turn.sway { animation: sway 7s ease-in-out infinite alternate; }
  @keyframes sway { from { rotate: y -14deg; } to { rotate: y 14deg; } }
  .f {
    position: absolute; left: 0; top: 0; display: block;
    background-image: var(--img); background-size: var(--bs) var(--bs); background-repeat: no-repeat;
    image-rendering: pixelated; backface-visibility: hidden;
  }
  .over .f { backface-visibility: visible; }
  /* Limbs swing from the shoulder or hip. */
  .pivot .limb { transform-origin: 0 var(--top); }
  .walk .arm-r .limb, .walk .leg-l .limb { animation: swing 0.9s ease-in-out infinite alternate; }
  .walk .arm-l .limb, .walk .leg-r .limb { animation: swing 0.9s ease-in-out infinite alternate-reverse; }
  @keyframes swing { from { rotate: x 32deg; } to { rotate: x -32deg; } }
  .capeswing { transform-origin: 0 0; rotate: x 8deg; }
  .walk .capeswing { animation: cape 0.9s ease-in-out infinite alternate; }
  @keyframes cape { from { rotate: x 10deg; } to { rotate: x 22deg; } }
  /* Holding still is handled app-wide (data-still). */
</style>
