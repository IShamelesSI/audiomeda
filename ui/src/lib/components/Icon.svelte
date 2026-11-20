<script lang="ts">
  // Small stroke icon set, drawn inline so it inherits currentColor.
  const PATHS = {
    speaker: "M11 5 6 9H2v6h4l5 4zM15.5 8.5a5 5 0 0 1 0 7M19 5a10 10 0 0 1 0 14",
    "speaker-off": "M11 5 6 9H2v6h4l5 4zM22 9l-6 6M16 9l6 6",
    mic: "M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3zM19 10v2a7 7 0 0 1-14 0v-2M12 19v3",
    plus: "M12 5v14M5 12h14",
    x: "M18 6 6 18M6 6l12 12",
    reset: "M3 12a9 9 0 1 0 3-6.7L3 8M3 3v5h5",
    output: "M4 14v-2a8 8 0 0 1 16 0v2M4 14a2 2 0 0 1 2-2h1v7H6a2 2 0 0 1-2-2zM20 14a2 2 0 0 0-2-2h-1v7h1a2 2 0 0 0 2-2z",
    save: "M5 3h11l3 3v15H5zM8 3v5h7M8 21v-7h8v7",
    pin: "M12 17v5M9 3h6l-1 7 4 3v2H6v-2l4-3z",
    edit: "M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z",
  } as const;

  let { name, size = 16 }: { name: keyof typeof PATHS; size?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={PATHS[name]} />
</svg>
