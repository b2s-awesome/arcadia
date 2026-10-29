<script lang="ts">
  // The spectrum from the audio engine, drawn on a canvas. Asks for new
  // levels about 30 times a second and only while on screen; the engine
  // stops collecting samples when this goes away.
  import { onMount } from "svelte";
  import { music } from "../../lib/api";
  import { ms } from "../../music/store.svelte";

  let { bands = 48, style = "bars", color = "#fe8019", peak = "#fabd2f", gap = 4 }: {
    bands?: number; style?: "bars" | "blocks" | "mirror"; color?: string; peak?: string; gap?: number;
  } = $props();

  let canvas: HTMLCanvasElement | undefined = $state();
  let levels: number[] = [];
  let peaks: number[] = [];

  onMount(() => {
    let raf = 0;
    let busy = false;
    let last = 0;
    const frame = async (now: number) => {
      raf = requestAnimationFrame(frame);
      draw();
      if (busy || now - last < 33) return;
      last = now;
      busy = true;
      try {
        const v = ms.status?.playing ? await music.spectrum(bands) : new Array(bands).fill(0);
        // Rise at once, fall gently; peaks hang then drop.
        levels = v.map((x, i) => Math.max(x, (levels[i] ?? 0) * 0.82));
        peaks = levels.map((x, i) => Math.max(x, (peaks[i] ?? 0) - 0.012));
      } catch {
        // No engine (no sound card): stay flat.
      } finally {
        busy = false;
      }
    };
    raf = requestAnimationFrame(frame);
    return () => {
      cancelAnimationFrame(raf);
      music.spectrumStop().catch(() => {});
    };
  });

  function draw() {
    const c = canvas;
    if (!c) return;
    const dpr = devicePixelRatio || 1;
    const w = c.clientWidth;
    const h = c.clientHeight;
    if (c.width !== Math.round(w * dpr) || c.height !== Math.round(h * dpr)) {
      c.width = Math.round(w * dpr);
      c.height = Math.round(h * dpr);
    }
    const g = c.getContext("2d");
    if (!g) return;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, w, h);
    const n = Math.max(1, levels.length || bands);
    const bw = (w - gap * (n - 1)) / n;
    for (let i = 0; i < n; i++) {
      const v = levels[i] ?? 0;
      const x = i * (bw + gap);
      g.fillStyle = color;
      if (style === "mirror") {
        const bh = Math.max(2, v * h * 0.5);
        g.fillRect(x, h / 2 - bh, bw, bh * 2);
      } else if (style === "blocks") {
        const cell = 7;
        const lit = Math.round((v * h) / cell);
        for (let k = 0; k < lit; k++) g.fillRect(x, h - (k + 1) * cell, bw, cell - 2);
      } else {
        const bh = Math.max(2, v * h);
        g.fillRect(x, h - bh, bw, bh);
      }
      if (style !== "mirror") {
        g.fillStyle = peak;
        g.fillRect(x, h - Math.max(2, (peaks[i] ?? 0) * h) - 3, bw, 2);
      }
    }
  }
</script>

<canvas bind:this={canvas} aria-hidden="true"></canvas>

<style>
  canvas { width: 100%; height: 100%; display: block; }
</style>
