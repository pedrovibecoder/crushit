<script setup lang="ts">
defineProps<{ modelValue: boolean; label: string; hint?: string }>();
const emit = defineEmits<{ (event: "update:modelValue", value: boolean): void }>();
</script>

<template>
  <label class="flex cursor-pointer items-center gap-3 py-2">
    <span class="min-w-0 flex-1">
      <span class="block text-[12.5px] font-semibold">{{ label }}</span>
      <span v-if="hint" class="mt-px block text-[11px] text-ink-2">{{ hint }}</span>
    </span>
    <button
      type="button"
      role="switch"
      :aria-checked="modelValue"
      :aria-label="label"
      class="relative h-[20px] w-[34px] shrink-0 rounded-full transition-colors duration-150"
      :class="modelValue ? 'bg-accent' : 'bg-line'"
      @click="emit('update:modelValue', !modelValue)"
    >
      <span
        class="absolute top-[2px] h-4 w-4 rounded-full bg-white shadow-sm transition-[left] duration-150"
        :class="modelValue ? 'left-[16px]' : 'left-[2px]'"
      />
    </button>
  </label>
</template>
