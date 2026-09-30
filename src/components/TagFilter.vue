<script setup lang="ts">
import { useTagsStore } from "../stores/tags";

defineProps<{
  /** The tag being looked at on its own, or `null` for everything. */
  modelValue: string | null;
}>();

const emit = defineEmits<{ (event: "update:modelValue", slug: string | null): void }>();

const tags = useTagsStore();
</script>

<template>
  <!-- One tag at a time: narrowing to "front-end work" is the question people
       actually ask, and a set of checkboxes would cost more room than the
       popup has. Pressing the tag that is already on clears it. -->
  <div v-if="tags.tags.length" class="flex flex-wrap items-center gap-1">
    <button
      v-for="tag in tags.tags"
      :key="tag.id"
      class="chip font-semibold transition-colors"
      :style="
        modelValue === tag.slug
          ? {
              color: tag.color,
              borderColor: `color-mix(in oklab, ${tag.color} 45%, transparent)`,
              backgroundColor: `color-mix(in oklab, ${tag.color} 14%, transparent)`,
            }
          : {}
      "
      :class="modelValue !== tag.slug && 'text-ink-3 hover:bg-line-soft'"
      :aria-pressed="modelValue === tag.slug"
      :title="modelValue === tag.slug ? `Show every tag again` : `Show only ${tag.label}`"
      @click="emit('update:modelValue', modelValue === tag.slug ? null : tag.slug)"
    >
      {{ tag.label }}
    </button>
  </div>
</template>
