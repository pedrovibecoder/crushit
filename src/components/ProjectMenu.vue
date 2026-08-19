<script setup lang="ts">
import { onMounted, ref } from "vue";
import AppIcon from "./AppIcon.vue";
import { shortenPath } from "../lib/format";
import type { Project } from "../types";

defineProps<{
  projects: Project[];
  /** The project being read, so the list can say which one you are in. */
  activeId: number | null;
}>();

const emit = defineEmits<{
  (event: "pick", projectId: number): void;
  (event: "manage"): void;
  (event: "close"): void;
}>();

/** Takes focus on open so Escape closes it without a click first. */
const root = ref<HTMLElement | null>(null);
onMounted(() => root.value?.focus());
</script>

<template>
  <div
    ref="root"
    class="card p-1 outline-none"
    tabindex="-1"
    role="dialog"
    aria-label="Switch project"
    @keydown.esc.stop="emit('close')"
  >
    <ul class="max-h-[168px] overflow-y-auto">
      <li v-for="project in projects" :key="project.id">
        <button
          type="button"
          class="flex w-full items-center gap-2 rounded-[8px] px-2 py-1.5 text-left transition-colors hover:bg-line-soft"
          :class="project.id === activeId && 'bg-line-soft'"
          @click="project.id === activeId ? emit('close') : emit('pick', project.id)"
        >
          <AppIcon name="folder" :size="13" class="shrink-0 text-ink-3" />
          <span class="min-w-0 flex-1">
            <span class="block truncate text-[11.5px] font-semibold">{{ project.name }}</span>
            <span class="block truncate text-[10.5px] text-ink-2">
              {{ shortenPath(project.path) }}
              <template v-if="project.branch"> · {{ project.branch }}</template>
            </span>
          </span>
          <AppIcon
            v-if="project.id === activeId"
            name="check"
            :size="12"
            :weight="2.2"
            class="shrink-0 text-accent"
          />
        </button>
      </li>
      <li v-if="!projects.length" class="px-2 py-2 text-[11px] text-ink-3">
        No projects yet.
      </li>
    </ul>

    <!-- Adding and removing stay on their own screen; this is only for moving
         between the projects that are already there. -->
    <button
      type="button"
      class="mt-1 flex w-full items-center gap-2 border-t border-line-soft px-2 pt-1.5 pb-1 text-left text-[11px] font-semibold text-ink-2 transition-colors hover:text-ink"
      @click="emit('manage')"
    >
      <AppIcon name="plus" :size="11" :weight="2.2" class="shrink-0" />
      Add or remove projects
    </button>
  </div>
</template>
