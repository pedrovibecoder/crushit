<script setup lang="ts">
import { computed } from "vue";
import { useAgentStore } from "../stores/agent";
import { useAppStore } from "../stores/app";
import { useExecutionStore } from "../stores/execution";
import { useFocusStore } from "../stores/focus";
import { useReviewStore } from "../stores/review";
import { useTasksStore } from "../stores/tasks";

const app = useAppStore();
const tasks = useTasksStore();
const focus = useFocusStore();
const agents = useAgentStore();
const execution = useExecutionStore();
const review = useReviewStore();

/** Any store may have something worth showing; the first one wins. */
const message = computed(
  () =>
    app.error ??
    tasks.error ??
    focus.error ??
    agents.error ??
    execution.error ??
    review.error,
);

function dismiss() {
  app.error = null;
  tasks.error = null;
  focus.error = null;
  agents.error = null;
  execution.error = null;
  review.error = null;
}
</script>

<template>
  <p
    v-if="message"
    class="flex items-start gap-2 border-b border-danger/25 bg-danger-soft px-3.5 py-2 text-[11.5px] text-danger"
  >
    <span class="min-w-0 flex-1">{{ message }}</span>
    <button class="shrink-0 font-semibold underline" @click="dismiss">Dismiss</button>
  </p>
</template>
