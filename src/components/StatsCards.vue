<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { formatSpan } from "../lib/format";
import { ipc } from "../lib/ipc";
import { useFocusStore } from "../stores/focus";
import { useTasksStore } from "../stores/tasks";
import type { Stats } from "../types";

const tasks = useTasksStore();
const focus = useFocusStore();
const stats = ref<Stats | null>(null);

async function load() {
  stats.value = await ipc.performanceStats().catch(() => null);
}

onMounted(load);

// Finishing a task or a session changes these numbers, so re-read them then
// rather than leaving stale figures on screen.
watch(() => [tasks.progress.done, focus.snapshot.status], load);

/** Four readings: what today looks like, and how the week has gone. */
const cards = computed(() => [
  {
    label: "Focused today",
    value: stats.value ? formatSpan(stats.value.focusTodaySeconds) : "—",
    hint: "since midnight",
  },
  {
    label: "Tasks done",
    value: stats.value ? String(stats.value.tasksDoneWeek) : "—",
    hint: "last 7 days",
  },
  {
    label: "Sessions",
    value: stats.value ? String(stats.value.sessionsWeek) : "—",
    hint: "last 7 days",
  },
  {
    label: "Average session",
    value: stats.value ? formatSpan(stats.value.averageSessionSeconds) : "—",
    hint: "last 7 days",
  },
]);
</script>

<template>
  <div class="grid grid-cols-2 gap-2 sm:grid-cols-4">
    <div v-for="card in cards" :key="card.label" class="card px-3 py-2.5">
      <p class="text-[11px] font-semibold text-ink-2">{{ card.label }}</p>
      <p class="tnum mt-0.5 text-[19px] leading-tight font-bold tracking-[-0.01em]">
        {{ card.value }}
      </p>
      <p class="text-[10.5px] text-ink-3">{{ card.hint }}</p>
    </div>
  </div>
</template>
