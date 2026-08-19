<script setup lang="ts">
import ConfettiBurst from "./ConfettiBurst.vue";
import { useTasksStore } from "../stores/tasks";

const tasks = useTasksStore();
</script>

<template>
  <!-- Sits above the panel and never affects its height. -->
  <Transition
    enter-active-class="transition duration-200 ease-out"
    enter-from-class="opacity-0 -translate-y-1"
    leave-active-class="transition duration-300 ease-in"
    leave-to-class="opacity-0"
  >
    <div
      v-if="tasks.celebration"
      class="pointer-events-none absolute inset-0 z-10"
      role="status"
      aria-live="polite"
    >
      <ConfettiBurst />
      <div class="flex justify-center px-4 pt-[52px]">
        <div
          class="card flex max-w-full items-center gap-2 border-success/35 bg-success-soft px-3 py-2 shadow-sm"
        >
          <span class="text-[15px] leading-none">🎉</span>
          <span class="min-w-0">
            <span class="block text-[12.5px] font-semibold">Congratulations — task done</span>
            <span class="block truncate text-[11px] text-ink-2">
              {{ tasks.celebration.title }}
            </span>
          </span>
        </div>
      </div>
    </div>
  </Transition>
</template>
