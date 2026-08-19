<script setup lang="ts">
import AppIcon from "./AppIcon.vue";

withDefaults(
  defineProps<{
    /** Small line above the title: where you are, or what this belongs to. */
    eyebrow?: string;
    title: string;
    back?: boolean;
    accent?: boolean;
  }>(),
  { accent: false },
);

const emit = defineEmits<{ (event: "back"): void }>();
</script>

<template>
  <header class="flex items-start gap-2.5 px-3.5 pt-3.5 pb-3">
    <button
      v-if="back"
      class="icon-btn mt-1 h-7 w-7 shrink-0"
      aria-label="Back"
      @click="emit('back')"
    >
      <AppIcon name="back" :size="14" />
    </button>
    <div class="min-w-0 flex-1">
      <p
        v-if="eyebrow"
        class="truncate text-[11px] leading-tight font-semibold"
        :class="accent ? 'text-accent' : 'text-ink-2'"
      >
        {{ eyebrow }}
      </p>
      <!-- Slotted so a screen can swap the heading for an editable field
           without losing the type scale. -->
      <slot name="title">
        <h1 class="truncate text-[19px] leading-tight font-bold tracking-[-0.015em]">
          {{ title }}
        </h1>
      </slot>
    </div>
    <div class="mt-0.5 flex shrink-0 items-center gap-1.5">
      <slot name="actions" />
    </div>
  </header>
</template>
