<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{ ratio: number }>();

const percent = computed(() =>
  Math.round(Math.min(1, Math.max(0, props.ratio)) * 100),
);
</script>

<template>
  <div class="h-1.5 w-full overflow-hidden rounded-full bg-line-soft">
    <div
      class="h-full rounded-full bg-linear-to-r from-[#f2c744] to-[#7ed957] transition-[width] duration-300"
      :style="{ width: `${percent}%` }"
    />
  </div>
</template>
