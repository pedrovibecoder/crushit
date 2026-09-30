<script setup lang="ts">
import { computed, ref } from "vue";
import AppIcon from "./AppIcon.vue";
import { onPressOutside } from "../lib/dismiss";

export interface SelectOption {
  value: string;
  label: string;
  hint?: string;
}

const props = defineProps<{
  modelValue: string;
  options: SelectOption[];
  label: string;
  placeholder?: string;
}>();

const emit = defineEmits<{ (event: "update:modelValue", value: string): void }>();

const open = ref(false);

/** The whole control, so pressing its own button is not "outside". */
const root = ref<HTMLElement | null>(null);
onPressOutside(root, () => { open.value = false });

const current = computed(
  () => props.options.find((option) => option.value === props.modelValue),
);

function pick(value: string) {
  emit("update:modelValue", value);
  open.value = false;
}
</script>

<template>
  <!--
    The list expands inline rather than floating: the popup window is sized to
    its content and clips overflow, so an absolute overlay would be cut off.
  -->
  <div ref="root">
    <button
      type="button"
      class="field flex w-full items-center gap-2 px-2 py-1.5 text-left"
      :aria-label="label"
      :aria-expanded="open"
      @click="open = !open"
      @keydown.esc="open = false"
    >
      <span
        class="min-w-0 flex-1 truncate text-[11.5px] font-medium"
        :class="!current && 'text-ink-3'"
      >
        {{ current?.label ?? placeholder ?? "Choose…" }}
      </span>
      <AppIcon
        name="chevron"
        :size="12"
        class="shrink-0 text-ink-3 transition-transform"
        :class="open && 'rotate-180'"
      />
    </button>

    <ul v-if="open" class="card mt-1 max-h-[164px] overflow-y-auto p-1">
      <li v-for="option in options" :key="option.value">
        <button
          type="button"
          class="flex w-full items-center gap-2 rounded-[8px] px-2 py-1.5 text-left transition-colors hover:bg-line-soft"
          :class="option.value === modelValue && 'bg-line-soft'"
          @click="pick(option.value)"
        >
          <span class="min-w-0 flex-1">
            <span class="block truncate text-[11.5px] font-medium">{{ option.label }}</span>
            <span v-if="option.hint" class="block truncate text-[10.5px] text-ink-2">
              {{ option.hint }}
            </span>
          </span>
          <AppIcon
            v-if="option.value === modelValue"
            name="check"
            :size="12"
            :weight="2.2"
            class="shrink-0 text-accent"
          />
        </button>
      </li>
      <li v-if="!options.length" class="px-2 py-2 text-[11px] text-ink-3">
        Nothing to choose from yet.
      </li>
    </ul>
  </div>
</template>
