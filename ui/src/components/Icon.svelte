<script lang="ts">
  // A tiny hand-drawn icon set (24px grid, 1.8 stroke).
  let { name, size = 20 }: { name: string; size?: number } = $props();
  const paths: Record<string, string> = {
    library: "M4 5h4v14H4zM10 5h4v14h-4zM16.5 5.5l3.6 1-3.6 13-3.6-1z",
    roblox: "M6.5 3.5l14 3.8-3.8 14-14-3.8zM10.6 9.6l3.8 1-1 3.8-3.8-1z",
    proxy: "M4 7h11M11 3l4 4-4 4M20 17H9M13 13l-4 4 4 4",
    inspector: "M10.5 17a6.5 6.5 0 1 0 0-13 6.5 6.5 0 0 0 0 13zM15.5 15.5L21 21",
    settings: "M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7zM12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1L7 17M17 7l2.1-2.1",
    plus: "M12 5v14M5 12h14",
    refresh: "M20 11a8 8 0 0 0-14.6-4.5L4 8M4 4v4h4M4 13a8 8 0 0 0 14.6 4.5L20 16M20 20v-4h-4",
    folder: "M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2.5h8.5A1.5 1.5 0 0 1 21 9v9.5a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5z",
    trash: "M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13",
    play: "M7 4.5v15l12-7.5z",
    search: "M10.5 17a6.5 6.5 0 1 0 0-13 6.5 6.5 0 0 0 0 13zM15.5 15.5L20 20",
    close: "M6 6l12 12M18 6L6 18",
    upload: "M12 16V4M7 9l5-5 5 5M4 20h16",
    download: "M12 4v12M7 11l5 5 5-5M4 20h16",
    brush: "M14.5 4.5l5 5-8 8-5-5zM6.5 12.5c-2 0-3.5 1.5-3.5 3.5 0 1.2-.5 2.3-1 3 3 .5 6.5-.5 6.5-4",
    sparkle: "M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8zM19 16l.8 2.2L22 19l-2.2.8L19 22l-.8-2.2L16 19l2.2-.8z",
    command: "M9 6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3z",
    minimize: "M6 12h12",
    maximize: "M6 6h12v12H6z",
    gamepad: "M7 8h10a4 4 0 0 1 4 4v1a3.5 3.5 0 0 1-6.3 2.1L14 14h-4l-.7 1.1A3.5 3.5 0 0 1 3 13v-1a4 4 0 0 1 4-4zM8 10.5v3M6.5 12h3M15.5 11.5h.01M17.5 13.5h.01",
    image: "M4 5h16v14H4zM4 15l4.5-4.5 4 4 2.5-2.5L20 17M15.5 9.5h.01",
    cube: "M12 3l8 4.5v9L12 21l-8-4.5v-9zM4 7.5l8 4.5 8-4.5M12 12v9",
    shield: "M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6z",
  };
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <path d={paths[name] ?? ""} />
</svg>
