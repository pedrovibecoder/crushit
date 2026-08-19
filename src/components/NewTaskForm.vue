<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import { useCategoriesStore } from "../stores/categories";
import type { NewTask, TaskCategory } from "../types";

const props = defineProps<{ projectId: number }>();
const emit = defineEmits<{ (event: "submit", input: NewTask): void }>();

const categories = useCategoriesStore();

const title = ref("");
/** Empty until the developer picks: the store's first category is the default. */
const chosen = ref<TaskCategory | null>(null);
const input = ref<HTMLInputElement | null>(null);

const category = computed(() => chosen.value ?? categories.fallback);

/**
 * The category picker only appears once there is something to categorise. A
 * dropdown here was cramped enough to clip its own labels, and the row reads
 * better as a plain input until it has to do more.
 */
const picking = computed(() => title.value.trim().length > 0);

function submit() {
  const trimmed = title.value.trim();
  if (!trimmed) return;
  emit("submit", {
    projectId: props.projectId,
    title: trimmed,
    category: category.value,
  });
  title.value = "";
  chosen.value = null;
  void nextTick(() => input.value?.focus());
}

defineExpose({ focus: () => input.value?.focus() });
</script>

<template>
  <form class="card border-dashed px-2.5 py-2" @submit.prevent="submit">
    <div class="flex items-center gap-2">
      <span class="h-[18px] w-[18px] shrink-0 rounded-[6px] border border-dashed border-line" />
      <input
        ref="input"
        v-model="title"
        type="text"
        placeholder="Type to add a new task…"
        aria-label="New task title"
        class="min-w-0 flex-1 bg-transparent text-[13px] font-medium outline-none placeholder:text-ink-3"
      />
      <button
        type="submit"
        class="btn btn-dark shrink-0 px-2.5 py-1 text-[11px]"
        :disabled="!title.trim()"
      >
        Add
      </button>
    </div>

    <div v-if="picking" class="mt-2 flex flex-wrap gap-1 pl-[26px]">
      <button
        v-for="option in categories.categories"
        :key="option.id"
        type="button"
        class="chip transition-colors"
        :class="
          category === option.slug
            ? 'border-solid bg-solid text-on-solid'
            : 'text-ink-2 hover:bg-line-soft'
        "
        :aria-pressed="category === option.slug"
        @click="chosen = option.slug"
      >
        <span
          class="h-[6px] w-[6px] shrink-0 rounded-full"
          :style="{ backgroundColor: option.color }"
        />
        {{ option.label }}
      </button>
    </div>
  </form>
</template>
