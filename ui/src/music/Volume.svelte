<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import { ms, setVolume } from "./store.svelte";
  let last = 0.8;
  const v = $derived(ms.status?.volume ?? 0.8);
  function mute() {
    if (v > 0) {
      last = v;
      setVolume(0);
    } else setVolume(last || 0.8);
  }
  function wheel(e: WheelEvent) {
    e.preventDefault();
    setVolume(Math.min(1, Math.max(0, v + (e.deltaY < 0 ? 0.05 : -0.05))));
  }
</script>

<div class="vol" onwheel={wheel}>
  <button class="ic" onclick={mute} aria-label={v > 0 ? "Mute" : "Unmute"}><Icon name={v > 0 ? "volume" : "mute"} size={17} /></button>
  <input type="range" min="0" max="1" step="0.01" value={v} style:--pct="{v * 100}%" aria-label="Volume" oninput={(e) => setVolume(Number((e.currentTarget as HTMLInputElement).value))} />
</div>

<style>
  .vol { display: flex; align-items: center; gap: 0.3rem; }
  .ic { border: 0; background: transparent; padding: 0.3rem; color: var(--muted); display: grid; }
  .ic:hover { color: var(--text); background: transparent; }
  input { width: 96px; appearance: none; -webkit-appearance: none; height: 16px; background: transparent; padding: 0; border: 0; cursor: pointer; }
  input::-webkit-slider-runnable-track { height: 4px; border-radius: 9px; background: linear-gradient(90deg, var(--text) var(--pct), color-mix(in srgb, var(--text) 16%, transparent) var(--pct)); }
  input::-webkit-slider-thumb { -webkit-appearance: none; width: 12px; height: 12px; border-radius: 50%; margin-top: -4px; background: var(--text); }
</style>
