<script setup lang="ts">
/** The small glyph set the popup uses, kept inline so nothing loads remotely. */
const PATHS: Record<string, string> = {
  play: "M8 5.5v13l11-6.5z",
  pause: "M9 5h2.5v14H9zm5.5 0H17v14h-2.5z",
  stop: "M7 7h10v10H7z",
  check: "M5 12.5l4.5 4.5L19 7.5",
  plus: "M12 5v14M5 12h14",
  back: "M14.5 5.5L8 12l6.5 6.5",
  forward: "M9.5 5.5L16 12l-6.5 6.5",
  gear: "M12 15.2a3.2 3.2 0 100-6.4 3.2 3.2 0 000 6.4zM19.4 12a7.4 7.4 0 00-.1-1.2l2-1.5-2-3.4-2.3 1a7.4 7.4 0 00-2.1-1.2L14.5 3h-4l-.4 2.7a7.4 7.4 0 00-2.1 1.2l-2.3-1-2 3.4 2 1.5a7.4 7.4 0 000 2.4l-2 1.5 2 3.4 2.3-1a7.4 7.4 0 002.1 1.2l.4 2.7h4l.4-2.7a7.4 7.4 0 002.1-1.2l2.3 1 2-3.4-2-1.5c.06-.4.1-.8.1-1.2z",
  folder: "M3.5 6.5A1.5 1.5 0 015 5h3.8l1.7 2H19a1.5 1.5 0 011.5 1.5v9A1.5 1.5 0 0119 19H5a1.5 1.5 0 01-1.5-1.5z",
  trash: "M5 7h14M10 7V5h4v2m-8 0l.8 12h10.4L18 7M10.5 10.5v5m3-5v5",
  sparkle: "M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9z",
  branch: "M7 4v10m0 0a3 3 0 003 3h4a3 3 0 003-3V8M7 14a2 2 0 100 4 2 2 0 000-4zm0-12a2 2 0 110 4 2 2 0 010-4zm10 2a2 2 0 110 4 2 2 0 010-4z",
  close: "M6 6l12 12M18 6L6 18",
  chevron: "M6 9.5l6 6 6-6",
};

defineProps<{
  name: keyof typeof PATHS | string;
  filled?: boolean;
  size?: number;
  weight?: number;
}>();
</script>

<template>
  <svg
    :width="size ?? 14"
    :height="size ?? 14"
    viewBox="0 0 24 24"
    :fill="filled ? 'currentColor' : 'none'"
    :stroke="filled ? 'none' : 'currentColor'"
    :stroke-width="weight ?? 1.75"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    class="shrink-0"
  >
    <path :d="PATHS[name] ?? ''" />
  </svg>
</template>
