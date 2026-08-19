<script setup lang="ts">
import { computed, ref, watch } from "vue";
import AppIcon from "../components/AppIcon.vue";
import PanelHeader from "../components/PanelHeader.vue";
import SelectMenu from "../components/SelectMenu.vue";
import { formatClock } from "../lib/format";
import { useAppStore } from "../stores/app";
import { useFocusStore } from "../stores/focus";
import { useTasksStore } from "../stores/tasks";
import { CATEGORY_LABELS, STATUS_LABELS, TASK_CATEGORIES, type TaskCategory } from "../types";

const CATEGORY_OPTIONS = TASK_CATEGORIES.map((value) => ({
  value,
  label: CATEGORY_LABELS[value],
}));

const app = useAppStore();
const tasks = useTasksStore();
const focus = useFocusStore();

const task = computed(() =>
  app.selectedTaskId === null ? undefined : tasks.byId.get(app.selectedTaskId),
);

const isFocused = computed(() => !!task.value && focus.isFocused(task.value.id));
const blockers = computed(() => (task.value ? tasks.blockedBy(task.value) : []));
const finishedHere = computed(
  () => !!task.value && focus.justFinishedTaskId === task.value.id,
);

/** The clock shows the live session, or the planned duration before one starts. */
const clock = computed(() => {
  if (isFocused.value) return formatClock(focus.snapshot.remainingSeconds);
  return formatClock(app.settings.focusMinutes * 60);
});

const sessionNote = computed(() => {
  if (finishedHere.value) return "Session complete — stop to close it out.";
  if (isFocused.value && focus.isPaused) {
    return `Paused · ${formatClock(focus.snapshot.elapsedSeconds)} focused`;
  }
  if (isFocused.value) return `${formatClock(focus.snapshot.elapsedSeconds)} focused so far`;
  return `${app.settings.focusMinutes} minute session`;
});

const title = ref("");
const newCriterion = ref("");
const confirmingDelete = ref(false);

watch(
  task,
  (current) => {
    title.value = current?.title ?? "";
    confirmingDelete.value = false;
  },
  { immediate: true },
);

async function commitTitle() {
  const trimmed = title.value.trim();
  if (!task.value || !trimmed || trimmed === task.value.title) {
    title.value = task.value?.title ?? "";
    return;
  }
  await tasks.update(task.value.id, { title: trimmed });
}

async function addCriterion() {
  const trimmed = newCriterion.value.trim();
  if (!task.value || !trimmed) return;
  const texts = [...task.value.criteria.map((c) => c.text), trimmed];
  await tasks.update(task.value.id, { criteria: texts });
  newCriterion.value = "";
}

async function removeCriterion(index: number) {
  if (!task.value) return;
  const texts = task.value.criteria
    .map((c) => c.text)
    .filter((_, position) => position !== index);
  await tasks.update(task.value.id, { criteria: texts });
}

async function toggleDone() {
  if (!task.value) return;
  if (isFocused.value) await focus.stop();
  await tasks.toggleComplete(task.value);
  app.back();
}

async function destroy() {
  if (!task.value) return;
  await tasks.remove(task.value.id);
  app.back();
}
</script>

<template>
  <div v-if="!task" class="px-3.5 py-6 text-center">
    <p class="text-[12px] text-ink-2">That task is gone.</p>
    <button class="btn btn-ghost mt-3 w-full py-2" @click="app.back()">Back to Today</button>
  </div>

  <div v-else class="flex flex-col">
    <PanelHeader :eyebrow="STATUS_LABELS[task.status]" :title="task.title" back @back="app.back()">
      <template #title>
        <input
          v-model="title"
          class="-mx-1 w-full rounded-[6px] border border-transparent bg-transparent px-1 text-[19px] leading-tight font-bold tracking-[-0.015em] outline-none hover:border-line focus:border-ink-3"
          aria-label="Task title"
          @blur="commitTitle"
          @keydown.enter.prevent="($event.target as HTMLInputElement).blur()"
        />
      </template>
    </PanelHeader>

    <div class="max-h-[380px] overflow-y-auto px-3.5 pb-3">
      <!-- Timer -->
      <div class="card px-3 py-3">
        <div class="flex items-center gap-3">
          <div class="min-w-0 flex-1">
            <p class="tnum text-[32px] leading-none font-bold tracking-[-0.03em]">
              {{ clock }}
            </p>
            <p class="mt-1.5 truncate text-[11px] text-ink-2">{{ sessionNote }}</p>
          </div>
          <button
            v-if="!isFocused"
            class="btn btn-dark shrink-0 px-3.5 py-2"
            @click="focus.start(task.id)"
          >
            <AppIcon name="play" :size="12" filled />Start
          </button>
          <template v-else>
            <button
              class="icon-btn h-8 w-8 shrink-0"
              :aria-label="focus.isRunning ? 'Pause' : 'Resume'"
              :title="focus.isRunning ? 'Pause' : 'Resume'"
              @click="focus.isRunning ? focus.pause() : focus.resume()"
            >
              <AppIcon :name="focus.isRunning ? 'pause' : 'play'" :size="13" filled />
            </button>
            <button
              class="icon-btn h-8 w-8 shrink-0 text-danger"
              aria-label="Stop session"
              title="Stop session"
              @click="focus.stop()"
            >
              <AppIcon name="stop" :size="13" filled />
            </button>
          </template>
        </div>
      </div>

      <p
        v-if="blockers.length"
        class="mt-2 rounded-[10px] border border-warn/25 bg-warn-soft px-2.5 py-1.5 text-[11px] text-warn"
      >
        Blocked by {{ blockers.map((other) => other.title).join(", ") }}
      </p>

      <!-- Acceptance criteria -->
      <section class="mt-4">
        <div class="flex items-baseline justify-between">
          <h2 class="eyebrow">Acceptance criteria</h2>
          <span v-if="task.criteria.length" class="tnum text-[11px] text-ink-3">
            {{ task.criteria.filter((c) => c.isMet).length }}/{{ task.criteria.length }}
          </span>
        </div>
        <ul class="mt-1.5 space-y-1.5">
          <li
            v-for="(criterion, index) in task.criteria"
            :key="criterion.id"
            class="card group flex items-start gap-2.5 px-2.5 py-2"
          >
            <button
              class="mt-px flex h-[17px] w-[17px] shrink-0 items-center justify-center rounded-[6px] border transition-colors"
              :class="
                criterion.isMet
                  ? 'border-success bg-success text-white'
                  : 'border-line hover:border-ink-3'
              "
              :aria-label="criterion.isMet ? 'Mark unmet' : 'Mark met'"
              @click="tasks.setCriterion(criterion.id, !criterion.isMet)"
            >
              <AppIcon v-if="criterion.isMet" name="check" :size="10" :weight="2.4" />
            </button>
            <span
              class="min-w-0 flex-1 text-[12.5px] leading-snug font-medium"
              :class="criterion.isMet && 'text-ink-3 line-through'"
            >
              {{ criterion.text }}
            </span>
            <button
              class="shrink-0 text-ink-3 opacity-0 transition-opacity group-hover:opacity-100 hover:text-danger"
              aria-label="Remove criterion"
              @click="removeCriterion(index)"
            >
              <AppIcon name="close" :size="12" />
            </button>
          </li>
        </ul>
        <input
          v-model="newCriterion"
          type="text"
          placeholder="Add a criterion…"
          class="field mt-1.5 w-full px-2.5 py-2 text-[12.5px]"
          @keydown.enter.prevent="addCriterion"
        />
      </section>

      <!-- Details -->
      <section class="mt-4">
        <h2 class="eyebrow">Details</h2>
        <textarea
          :value="task.description ?? ''"
          rows="2"
          placeholder="What needs to change?"
          class="field mt-1.5 w-full resize-none px-2.5 py-2 text-[12.5px] leading-snug"
          @change="tasks.update(task.id, { description: ($event.target as HTMLTextAreaElement).value })"
        />
        <div class="mt-1.5 flex items-center gap-1.5">
          <div class="min-w-0 flex-1">
            <SelectMenu
              :model-value="task.category"
              :options="CATEGORY_OPTIONS"
              label="Category"
              @update:model-value="tasks.update(task.id, { category: $event as TaskCategory })"
            />
          </div>
          <input
            :value="task.estimateMinutes ?? ''"
            type="number"
            min="1"
            placeholder="min"
            aria-label="Estimate in minutes"
            class="field tnum w-16 px-2 py-1.5 text-[11.5px]"
            @change="tasks.update(task.id, { estimateMinutes: Number(($event.target as HTMLInputElement).value) })"
          />
        </div>
        <ul v-if="task.files.length" class="mt-1.5 space-y-1">
          <li
            v-for="file in task.files"
            :key="file"
            class="truncate rounded-[8px] bg-line-soft px-2 py-1 text-[11px] text-ink-2"
          >
            {{ file }}
          </li>
        </ul>
      </section>

      <!-- Codex slot, wired in a later phase -->
      <section class="card mt-4 border-accent/25 bg-accent-soft/50 px-3 py-2.5">
        <div class="flex items-center justify-between gap-2">
          <span class="flex items-center gap-1.5 text-[12px] font-semibold text-accent">
            <AppIcon name="sparkle" :size="12" filled />Codex
          </span>
          <span class="text-[11px] text-ink-2">Not started</span>
        </div>
        <button
          class="btn btn-ghost mt-2 w-full cursor-not-allowed py-1.5"
          disabled
          title="Codex execution arrives in a later phase"
        >
          Run with Codex
        </button>
      </section>
    </div>

    <footer class="flex items-center gap-2 border-t border-line px-3.5 py-2.5">
      <button class="btn btn-dark flex-1 py-2" @click="toggleDone">
        <AppIcon name="check" :size="13" />
        {{ task.status === "completed" ? "Reopen" : "Mark complete" }}
      </button>
      <button
        v-if="!confirmingDelete"
        class="icon-btn h-[34px] w-[34px] shrink-0 text-ink-2 hover:text-danger"
        aria-label="Delete task"
        title="Delete task"
        @click="confirmingDelete = true"
      >
        <AppIcon name="trash" :size="14" />
      </button>
      <button
        v-else
        class="btn shrink-0 border border-danger/30 bg-danger-soft px-2.5 py-2 text-danger"
        @click="destroy"
      >
        Delete
      </button>
    </footer>
  </div>
</template>
