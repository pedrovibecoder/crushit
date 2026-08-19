<script setup lang="ts">
import { computed, ref } from "vue";
import AppIcon from "../components/AppIcon.vue";
import NewTaskForm from "../components/NewTaskForm.vue";
import PanelHeader from "../components/PanelHeader.vue";
import ProgressBar from "../components/ProgressBar.vue";
import TaskRow from "../components/TaskRow.vue";
import { useAppStore } from "../stores/app";
import { useAgentStore } from "../stores/agent";
import { useFocusStore } from "../stores/focus";
import { useTasksStore } from "../stores/tasks";
import { AGENT_LABELS, type NewTask, type Task } from "../types";

const app = useAppStore();
const tasks = useTasksStore();
const focus = useFocusStore();
const codex = useAgentStore();
const agentName = computed(() => AGENT_LABELS[codex.selected]);

type Filter = "all" | "open" | "done";
const FILTERS: Array<{ id: Filter; label: string }> = [
  { id: "all", label: "All tasks" },
  { id: "open", label: "Open" },
  { id: "done", label: "Done" },
];
const filter = ref<Filter>("all");

/** Focused task first, then open work, then anything already finished. */
const ordered = computed<Task[]>(() => {
  const rank = (task: Task) => {
    if (focus.isFocused(task.id)) return 0;
    if (task.status === "completed") return 2;
    return 1;
  };
  return [...tasks.tasks].sort(
    (a, b) => rank(a) - rank(b) || a.position - b.position,
  );
});

const visible = computed(() =>
  ordered.value.filter((task) => {
    if (filter.value === "open") return task.status !== "completed";
    if (filter.value === "done") return task.status === "completed";
    return true;
  }),
);

const percent = computed(() => Math.round(tasks.progress.ratio * 100));

async function add(input: NewTask) {
  await tasks.create(input);
}
</script>

<template>
  <div class="flex flex-col">
    <PanelHeader :eyebrow="app.activeProject?.name" title="Today" accent>
      <template #actions>
        <button
          class="icon-btn h-7 w-7"
          aria-label="Settings"
          title="Settings"
          @click="app.go('settings')"
        >
          <AppIcon name="gear" :size="14" />
        </button>
      </template>
    </PanelHeader>

    <div class="px-3.5 pb-2.5">
      <div class="card px-3 py-2.5">
        <div class="flex items-baseline justify-between gap-2">
          <span class="text-[12.5px] font-semibold">Progress</span>
          <span class="tnum text-[12.5px] font-semibold text-ink-2">{{ percent }}%</span>
        </div>
        <div class="mt-2">
          <ProgressBar :ratio="tasks.progress.ratio" />
        </div>
        <div class="mt-2.5 flex items-center gap-1.5">
          <span v-if="app.activeProject?.branch" class="chip text-ink-2">
            <AppIcon name="branch" :size="10" />{{ app.activeProject.branch }}
          </span>
          <span class="chip tnum text-ink-2">
            {{ tasks.progress.done }} of {{ tasks.progress.total }} done
          </span>
        </div>
      </div>
    </div>

    <!-- A run started here keeps going with the popup closed, so Today has to
         be able to lead back to it. -->
    <button
      v-if="codex.isAnalyzing || codex.plan"
      class="card mx-3.5 mb-2.5 flex items-center gap-2.5 border-accent/35 bg-accent-soft/50 px-3 py-2 text-left transition-colors hover:bg-accent-soft"
      @click="app.go('goal')"
    >
      <AppIcon name="sparkle" :size="13" filled class="shrink-0 text-accent" />
      <span class="min-w-0 flex-1">
        <span class="block truncate text-[12px] font-semibold">
          {{ codex.isAnalyzing ? `${agentName} is reading your project` : "A plan is ready" }}
        </span>
        <span class="block truncate text-[11px] text-ink-2">
          {{
            codex.isAnalyzing
              ? codex.analysis.goalTitle
              : `${codex.plan?.tasks.length ?? 0} tasks to review`
          }}
        </span>
      </span>
      <AppIcon name="forward" :size="13" class="shrink-0 text-ink-3" />
    </button>

    <div class="flex items-center gap-1 px-3.5 pb-2.5">
      <button
        v-for="option in FILTERS"
        :key="option.id"
        class="rounded-[8px] px-2.5 py-1 text-[11.5px] font-semibold transition-colors"
        :class="
          filter === option.id
            ? 'bg-ink text-white'
            : 'text-ink-2 hover:bg-line-soft'
        "
        @click="filter = option.id"
      >
        {{ option.label }}
      </button>
    </div>

    <div class="max-h-[360px] space-y-1.5 overflow-y-auto px-3.5 pb-3">
      <NewTaskForm
        v-if="app.activeProject"
        :project-id="app.activeProject.id"
        @submit="add"
      />

      <TaskRow
        v-for="task in visible"
        :key="task.id"
        :task="task"
        :focused="focus.isFocused(task.id)"
        :remaining-seconds="focus.snapshot.remainingSeconds"
        :blocked-by="tasks.blockedBy(task)"
        @open="app.openTask(task.id)"
        @toggle="tasks.toggleComplete(task)"
      />

      <p
        v-if="visible.length === 0"
        class="px-2 py-5 text-center text-[12px] leading-relaxed text-ink-3"
      >
        {{
          filter === "done"
            ? "Nothing finished yet."
            : "No tasks yet — add one above."
        }}
      </p>
    </div>

    <footer class="border-t border-line px-3.5 py-2.5">
      <button
        class="btn btn-ghost w-full py-2 text-[12px]"
        @click="app.go('goal')"
      >
        <AppIcon name="sparkle" :size="13" filled />New goal
      </button>
    </footer>
  </div>
</template>
