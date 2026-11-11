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
    gauge: "M4.5 17a8.5 8.5 0 1 1 15 0M12 13.5l4-5M12 15.2a1.7 1.7 0 1 0 0-3.4 1.7 1.7 0 0 0 0 3.4z",
    chip: "M7 7h10v10H7zM10 3v4M14 3v4M10 17v4M14 17v4M3 10h4M3 14h4M17 10h4M17 14h4",
    record: "M12 18a6 6 0 1 0 0-12 6 6 0 0 0 0 12z",
    stop: "M7 7h10v10H7z",
    undo: "M8 5L4 9l4 4M4 9h10a5 5 0 0 1 0 10h-3",
    star: "M12 4l2.4 5 5.6.8-4 3.9.9 5.5-4.9-2.6-4.9 2.6.9-5.5-4-3.9 5.6-.8z",
    info: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 11v6M12 7.5h.01",
    warning: "M12 4l9 16H3zM12 10v4.5M12 17.5h.01",
    check: "M5 12.5l4.5 4.5L19 7.5",
    sliders: "M5 6h8M17 6h2M5 12h2M11 12h8M5 18h10M19 18h0M15 4v4M9 10v4M17 16v4",
  };
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <path d={paths[name] ?? ""} />
</svg>
