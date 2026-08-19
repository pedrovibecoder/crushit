<script setup lang="ts">
import { useCategoriesStore } from "../stores/categories";
import type { TaskCategory } from "../types";

defineProps<{ category: TaskCategory }>();

const categories = useCategoriesStore();
</script>

<template>
  <span class="chip text-ink-2">
    <span
      class="h-[6px] w-[6px] shrink-0 rounded-full"
      :style="{ backgroundColor: categories.colorFor(category) }"
    />
    {{ categories.labelFor(category) }}
  </span>
</template>
