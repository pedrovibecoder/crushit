<script setup lang="ts">
/**
 * Someone asleep, for the break screen. Drawn rather than an emoji so it takes
 * the theme's own colours, and so the breathing and the Zs can be animated
 * apart from one another.
 */
defineProps<{ size?: number }>();
</script>

<template>
  <svg
    :width="size ?? 140"
    :height="(size ?? 140) * 0.78"
    viewBox="0 0 140 110"
    fill="none"
    aria-hidden="true"
  >
    <!-- Drawn first so they drift out from behind the sleeper. -->
    <g class="text-accent" fill="currentColor">
      <text class="snore" x="52" y="52" font-size="15" font-weight="700">z</text>
      <text
        class="snore"
        x="63"
        y="42"
        font-size="19"
        font-weight="700"
        style="animation-delay: 1s"
      >
        z
      </text>
      <text
        class="snore"
        x="76"
        y="31"
        font-size="23"
        font-weight="700"
        style="animation-delay: 2s"
      >
        z
      </text>
    </g>

    <g class="sleeper">
      <!-- Bed. -->
      <rect x="8" y="80" width="124" height="20" rx="9" class="text-line-soft" fill="currentColor" />
      <!-- Pillow, tucked behind the head. -->
      <rect
        x="14"
        y="64"
        width="42"
        height="20"
        rx="9"
        class="text-line"
        fill="currentColor"
        opacity="0.55"
      />

      <!-- The blanket over a body, rising where the shoulder is. -->
      <path
        d="M50 84c0-13 10-22 24-22h34c8 0 14 6 14 13v9z"
        class="text-accent"
        fill="currentColor"
        opacity="0.4"
      />
      <path
        d="M50 84c0-13 10-22 24-22h34c8 0 14 6 14 13v9"
        class="text-accent"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linejoin="round"
        opacity="0.7"
        fill="none"
      />

      <!-- Head, resting on the pillow. -->
      <circle cx="42" cy="70" r="14" class="text-card" fill="currentColor" />
      <circle
        cx="42"
        cy="70"
        r="14"
        class="text-line"
        stroke="currentColor"
        stroke-width="1.5"
        fill="none"
      />
      <!-- Closed eyes, and a mouth just open enough to be snoring. -->
      <g class="text-ink-2" stroke="currentColor" stroke-width="1.9" stroke-linecap="round">
        <path d="M35 68.5q2.6 2.6 5.2 0" />
        <path d="M44.5 68.5q2.6 2.6 5.2 0" />
      </g>
      <ellipse cx="42.5" cy="77" rx="2.6" ry="2" class="text-ink-3" fill="currentColor" />
    </g>
  </svg>
</template>
