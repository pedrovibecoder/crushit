<script setup lang="ts">
import { computed } from "vue";

const props = withDefaults(
  defineProps<{
    ratio: number;
    /**
     * Draw the bar as this many equal parts. With a handful of tasks a
     * continuous bar is hard to read against — one notch per task turns the
     * fill into something countable, and each one finished visibly lands.
     * Zero, or more parts than fit, leaves the bar solid.
     */
    segments?: number;
    /** Tailwind height utility, so a bar can be sized to what it sits beside. */
    height?: string;
    /** Roll the filled part, for a bar that is watched rather than glanced at. */
    wavy?: boolean;
  }>(),
  { segments: 0, height: "h-2", wavy: true },
);

/** More than this and the notches are thinner than the gaps between them. */
const MAX_SEGMENTS = 12;

const percent = computed(() =>
  Math.round(Math.min(1, Math.max(0, props.ratio)) * 100),
);

const notches = computed(() =>
  props.segments > 1 && props.segments <= MAX_SEGMENTS ? props.segments : 0,
);
</script>

<template>
  <div
    class="relative w-full overflow-hidden rounded-full bg-line-soft"
    :class="height"
    role="progressbar"
    :aria-valuenow="percent"
    aria-valuemin="0"
    aria-valuemax="100"
  >
    <div
      class="absolute inset-y-0 left-0 overflow-hidden rounded-full bg-linear-to-r from-progress-from to-progress-to transition-[width] duration-500 ease-out"
      :style="{ width: `${percent}%` }"
    >
      <!-- Nothing to roll in an empty bar, and a finished one has earned rest. -->
      <span v-if="wavy && percent > 0" class="progress-wave" aria-hidden="true" />
    </div>
    <!--
      The notches are drawn over the fill rather than the fill being split, so
      the gradient still runs the length of the bar rather than restarting in
      every part.
    -->
    <div v-if="notches" class="absolute inset-0 flex" aria-hidden="true">
      <span
        v-for="index in notches"
        :key="index"
        class="flex-1"
        :class="index < notches && 'border-r-2 border-card'"
      />
    </div>
  </div>
</template>
