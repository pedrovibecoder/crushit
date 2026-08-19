<script setup lang="ts">
import { CATEGORY_DOTS, CATEGORY_LABELS, type TaskCategory } from "../types";

defineProps<{ category: TaskCategory }>();
</script>

<template>
  <span class="chip text-ink-2">
    <span
      class="h-[6px] w-[6px] shrink-0 rounded-full"
      :style="{ backgroundColor: CATEGORY_DOTS[category] }"
    />
    {{ CATEGORY_LABELS[category] }}
  </span>
</template>
