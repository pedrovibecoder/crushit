<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import AppIcon from "../components/AppIcon.vue";
import DayPicker from "../components/DayPicker.vue";
import NewTaskForm from "../components/NewTaskForm.vue";
import PanelHeader from "../components/PanelHeader.vue";
import TagFilter from "../components/TagFilter.vue";
import ProjectMenu from "../components/ProjectMenu.vue";
import SleepingFigure from "../components/SleepingFigure.vue";
import ProgressBar from "../components/ProgressBar.vue";
import StatsCards from "../components/StatsCards.vue";
import TaskRow from "../components/TaskRow.vue";
import { byTitle, dayName, formatClock, localDay, shiftDay } from "../lib/format";
import { errorMessage, ipc } from "../lib/ipc";
import { useAppStore } from "../stores/app";
import { useAgentStore } from "../stores/agent";
import { useExecutionStore } from "../stores/execution";
import { useRestStore } from "../stores/rest";
import { useSlackStore } from "../stores/slack";
import { useReviewStore } from "../stores/review";
import { useFocusStore } from "../stores/focus";
import { useTasksStore } from "../stores/tasks";
import {
  AGENT_LABELS,
  IMPORTABLE_EXTENSIONS,
  REST_CHOICES,
  isImportable,
  type NewTask,
  type Task,
} from "../types";

const app = useAppStore();
const tasks = useTasksStore();
const focus = useFocusStore();
const codex = useAgentStore();
const execution = useExecutionStore();
const review = useReviewStore();
const rest = useRestStore();
const slack = useSlackStore();
const agentName = computed(() => AGENT_LABELS[codex.selected]);

/**
 * Which day the list is showing. Work that was not finished on an earlier day
 * is still open, so it appears on every later day until it is done — the list
 * carries it forward rather than leaving it behind on a page nobody revisits.
 */
const day = ref(localDay());
const today = computed(() => localDay());
const isToday = computed(() => day.value === today.value);

function goDay(by: number) {
  day.value = shiftDay(day.value, by);
}

/**
 * The calendar, for jumping somewhere the arrows would take all day to reach —
 * checking what was on last Tuesday, or what is already planned for Friday.
 */
const pickerOpen = ref(false);

/**
 * Switching project from the header, since which project you are in is read
 * off this line anyway and moving between them is not a setting.
 */
const projectsOpen = ref(false);

/** One panel at a time: they open over the same corner of the screen. */
watch(pickerOpen, (open) => { if (open) projectsOpen.value = false });
watch(projectsOpen, (open) => { if (open) pickerOpen.value = false });

const panelOpen = computed(() => pickerOpen.value || projectsOpen.value);

async function switchProject(projectId: number) {
  projectsOpen.value = false;
  await app.selectProject(projectId);
  // The new project's work is its own; start it where the list opens.
  day.value = today.value;
}

/** Spelled out, since the title only ever says Today or Yesterday. */
const fullDate = computed(() =>
  new Date(`${day.value}T12:00:00`).toLocaleDateString(undefined, {
    weekday: "long",
    day: "numeric",
    month: "long",
  }),
);

/**
 * A task is carried over when its own day is behind the one being read. Work
 * that has no day at all is not carried over — it is waiting in the plan, and
 * it appears here only once it has been given a date.
 */
function isCarriedOver(task: Task) {
  return (
    task.plannedFor !== null &&
    task.plannedFor < day.value &&
    task.status !== "completed"
  );
}

/** Everything that belongs on the day being shown. */
const onThisDay = computed(() =>
  tasks.tasks.filter((task) => {
    if (task.plannedFor === day.value) return true;
    if (isCarriedOver(task)) return true;
    // Finished on this day, even if it was planned for another one.
    return task.completedAt !== null && localDay(new Date(task.completedAt * 1000)) === day.value;
  }),
);

/** Progress for the day on screen, rather than for the project as a whole. */
const dayProgress = computed(() => {
  const total = onThisDay.value.length;
  const done = onThisDay.value.filter((task) => task.status === "completed").length;
  return { done, total, ratio: total === 0 ? 0 : done / total };
});

const dayPoints = computed(() => {
  let done = 0;
  let total = 0;
  for (const task of onThisDay.value) {
    const points = task.storyPoints ?? 0;
    total += points;
    if (task.status === "completed") done += points;
  }
  return { done, total };
});

type Filter = "all" | "open" | "done";
const FILTERS: Array<{ id: Filter; label: string }> = [
  { id: "all", label: "All tasks" },
  { id: "open", label: "Open" },
  { id: "done", label: "Done" },
];
const filter = ref<Filter>("all");

/**
 * Focused task first, then open work, then anything already finished — and
 * within each of those, by title. Titles here carry their own structure
 * (`<Project> [FE] …`), so alphabetical keeps a project's work together
 * instead of scattering it in the order it happened to be typed.
 */
const ordered = computed<Task[]>(() => {
  const rank = (task: Task) => {
    if (focus.isFocused(task.id)) return 0;
    if (task.status === "completed") return 2;
    return 1;
  };
  return [...onThisDay.value].sort(
    (a, b) => rank(a) - rank(b) || byTitle(a, b),
  );
});

/** One tag at a time, or none: see `TagFilter`. */
const tagFilter = ref<string | null>(null);

/**
 * Picking several tasks out of the list to hand to an agent. It is a mode
 * rather than a permanent column of checkboxes: the list is read far more
 * often than it is gathered up, and a row of empty boxes on every task would
 * be there all day for the sake of a thing done now and then.
 */
const selecting = ref(false);
const selected = ref(new Set<number>());
const contextError = ref<string | null>(null);

function toggleSelected(taskId: number) {
  const next = new Set(selected.value);
  if (!next.delete(taskId)) next.add(taskId);
  selected.value = next;
}

function stopSelecting() {
  selecting.value = false;
  selected.value = new Set();
  contextError.value = null;
}

/** In the order the list is showing, not the order they were picked. */
const selectedTasks = computed(() =>
  visible.value.filter((task) => selected.value.has(task.id)),
);

/**
 * The brief opens in the desktop window rather than here. A menu-bar popup is
 * 360 points wide: enough to say what today is, not enough to read a page of
 * prose about what a handful of tasks amount to.
 */
async function openContext() {
  contextError.value = null;
  try {
    await ipc.openContextWindow(selectedTasks.value.map((task) => task.id));
    stopSelecting();
  } catch (caught) {
    contextError.value = errorMessage(caught);
  }
}

const visible = computed(() =>
  ordered.value.filter((task) => {
    if (tagFilter.value !== null && !task.tags.includes(tagFilter.value)) return false;
    if (filter.value === "open") return task.status !== "completed";
    if (filter.value === "done") return task.status === "completed";
    return true;
  }),
);

const percent = computed(() => Math.round(dayProgress.value.ratio * 100));
const points = computed(() => dayPoints.value);
const isComplete = computed(
  () => dayProgress.value.total > 0 && dayProgress.value.done === dayProgress.value.total,
);
const remaining = computed(() => dayProgress.value.total - dayProgress.value.done);

/** The card says where you are before it says how far along that is. */
const headline = computed(() => {
  if (dayProgress.value.total === 0) return "Nothing planned yet";
  if (isComplete.value) return "All done";
  return "Progress";
});

async function add(input: NewTask) {
  await tasks.create({ ...input, plannedFor: day.value });
}

/**
 * The break takes over the whole widget: the point of stopping is not to be
 * looking at the list. It opens by itself whenever a break is running, so
 * reopening the popup mid-break lands here rather than on the work.
 */
const restOpen = ref(false);
watch(() => rest.isActive, (active) => { if (active) restOpen.value = true }, { immediate: true });

const takeBreak = (minutes: number) => rest.start(minutes);

async function backToWork() {
  await rest.stop();
  restOpen.value = false;
}

/**
 * Everything on this screen is read once and then left alone: the figures come
 * from a query on mount, and the branch is only re-read on a throttle. One
 * button re-reads the lot, for when the work happened somewhere else.
 */
const stats = ref<InstanceType<typeof StatsCards> | null>(null);
const refreshing = ref(false);

async function refresh() {
  if (refreshing.value) return;
  refreshing.value = true;
  try {
    await Promise.all([
      tasks.load(app.activeProject?.id ?? null),
      app.refreshActiveProject(true),
      codex.refreshGoal(app.activeProject?.id ?? null),
      stats.value?.reload() ?? Promise.resolve(),
      // Re-read what the backend believes is running. A snapshot only arrives
      // by event, so a window that missed one can sit showing nothing while
      // the menu bar still reads "Coding…".
      app.refreshRuns(),
    ]);
  } finally {
    refreshing.value = false;
  }
}

/**
 * Dropping an existing task list — a screenshot of a board, a CSV, a
 * spreadsheet — hands it to the agent, which reads it and comes back with a
 * plan to review on the goal screen.
 */
const dragging = ref(false);
const dropError = ref<string | null>(null);
/** Said in the formats people have, rather than as a list of extensions. */
const IMPORT_KINDS = ["Screenshot", "CSV", "Excel", "Numbers", "Markdown"];

/** The panel the import button opens, for when there is nothing to drag. */
const importing = ref(false);
let stopDrops: (() => void) | undefined;

async function importFrom(path: string) {
  dropError.value = null;
  if (!app.activeProject) {
    dropError.value = "Choose a project first.";
    return;
  }
  if (!isImportable(path)) {
    dropError.value = "Drop a screenshot, a CSV, or a spreadsheet.";
    return;
  }
  if (await codex.importFrom(app.activeProject.id, path)) {
    importing.value = false;
    app.go("goal");
  } else {
    dropError.value = codex.error;
  }
}

/** The same import, for a file that is easier to pick than to drag. */
async function chooseFile() {
  dropError.value = null;
  try {
    const picked = await open({
      multiple: false,
      title: "Import a task list",
      filters: [{ name: "Task lists", extensions: IMPORTABLE_EXTENSIONS }],
    });
    if (typeof picked === "string") await importFrom(picked);
  } catch (caught) {
    dropError.value = caught instanceof Error ? caught.message : "Could not open that file.";
  }
}

onMounted(() => {
  // Only when it is set up: an unconnected Slack should cost nothing.
  if (app.settings.slackConnected) void slack.load();
});

onMounted(async () => {
  try {
    stopDrops = await getCurrentWindow().onDragDropEvent(({ payload }) => {
      if (payload.type === "over" || payload.type === "enter") {
        dragging.value = true;
        importing.value = true;
      } else if (payload.type === "drop") {
        dragging.value = false;
        // One list at a time: a second file would queue behind a run that is
        // already using the agent.
        const [first] = payload.paths;
        if (first) void importFrom(first);
      } else {
        dragging.value = false;
      }
    });
  } catch {
    // Outside Tauri there is nothing to drop onto.
  }
});

onBeforeUnmount(() => stopDrops?.());
</script>

<template>
  <div class="relative flex flex-col" :class="panelOpen && 'min-h-[420px]'">
    <!--
      Nothing may go above that element, comments included: a screen is swapped
      inside a `<Transition mode="out-in">`, and a comment makes this component's
      root a fragment rather than an element. The transition then has nothing to
      animate, the leave never completes, and the screen you asked for never
      mounts — the popup just goes empty.

      A panel floats over the list rather than pushing it down, so opening one
      does not move the work you were reading. The popup window is only as tall
      as its content and clips the rest, so while a panel is open the screen
      holds a floor tall enough to show it.
    -->
    <!--
      Import: opened from the footer, and also whenever something is dragged
      over the window, so the drop target is always somewhere visible rather
      than being a gesture you have to already know about.
    -->
    <div
      v-if="importing"
      class="absolute inset-0 z-20 flex flex-col bg-page"
      role="dialog"
      aria-label="Import tasks"
    >
      <div class="flex items-center justify-between gap-2 px-3.5 pt-3.5 pb-2">
        <div class="min-w-0">
          <p class="eyebrow">Import</p>
          <p class="text-[15px] leading-tight font-bold tracking-[-0.015em]">
            Bring in a task list
          </p>
        </div>
        <button
          class="icon-btn h-7 w-7 shrink-0"
          aria-label="Close import"
          @click="importing = false"
        >
          <AppIcon name="close" :size="13" />
        </button>
      </div>

      <div class="flex flex-1 flex-col justify-center px-3.5 pb-3.5">
        <button
          class="flex max-h-[340px] flex-1 flex-col items-center justify-center gap-1.5 rounded-[12px] border-2 border-dashed px-6 py-8 text-center transition-colors"
          :class="
            dragging
              ? 'border-accent bg-accent-soft/60'
              : 'border-line bg-card/40 hover:border-ink-3 hover:bg-line-soft/50'
          "
          @click="chooseFile"
        >
          <span
            class="icon-btn h-9 w-9 border-dashed transition-colors"
            :class="dragging && 'border-accent text-accent'"
          >
            <AppIcon name="import" :size="17" />
          </span>
          <span class="mt-0.5 text-[12.5px] font-semibold">
            {{ dragging ? "Drop it here" : "Drop a file, or click to choose one" }}
          </span>
          <span class="text-[11px] leading-snug text-ink-2">
            A screenshot of your board, a CSV, or a spreadsheet.
          </span>
          <span class="mt-1 text-[10.5px] leading-snug text-ink-3">
            {{ agentName }} reads it, writes one task per item and sizes each in
            story points. You review the list before anything is added.
          </span>
        </button>

        <div class="mt-2 flex flex-wrap items-center justify-center gap-1">
          <span v-for="kind in IMPORT_KINDS" :key="kind" class="chip text-ink-2">
            {{ kind }}
          </span>
        </div>
      </div>
    </div>

    <!-- A break, full on the widget. -->
    <div
      v-if="restOpen"
      class="absolute inset-0 z-20 flex flex-col bg-page"
      role="dialog"
      aria-label="Break"
    >
      <div class="flex items-start justify-between gap-2 px-3.5 pt-3.5">
        <div class="min-w-0">
          <p class="eyebrow">Break</p>
          <p class="text-[15px] leading-tight font-bold tracking-[-0.015em]">
            {{ rest.isOver ? "Break over" : rest.isResting ? "Resting" : "Take a break" }}
          </p>
        </div>
        <button class="icon-btn h-7 w-7 shrink-0" aria-label="Close break" @click="restOpen = false">
          <AppIcon name="close" :size="13" />
        </button>
      </div>

      <div class="flex flex-1 flex-col items-center justify-center px-6 pb-4 text-center">
        <SleepingFigure :size="140" />

        <template v-if="rest.isActive">
          <p class="tnum mt-1 text-[34px] leading-none font-bold tracking-[-0.03em]">
            {{ formatClock(rest.snapshot.remainingSeconds) }}
          </p>
          <p class="mt-1.5 text-[11.5px] leading-snug text-ink-2">
            {{
              rest.isOver
                ? `That was ${rest.snapshot.minutes} minutes. Nothing was counted against your task.`
                : `of ${rest.snapshot.minutes} minutes. The timer stays paused until you are back.`
            }}
          </p>
        </template>
        <template v-else>
          <p class="mt-1 text-[12.5px] font-semibold">How long do you need?</p>
          <p class="mt-1 text-[11.5px] leading-snug text-ink-2">
            A running session is paused while you are away.
          </p>
        </template>
      </div>

      <footer class="border-t border-line px-3.5 py-2.5">
        <div v-if="!rest.isActive" class="flex items-center gap-1.5">
          <button
            v-for="minutes in REST_CHOICES"
            :key="minutes"
            class="tnum flex-1 rounded-[9px] border border-line bg-card py-2 text-[12px] font-semibold text-ink-2 transition-colors hover:bg-line-soft"
            @click="takeBreak(minutes)"
          >
            {{ minutes }}m
          </button>
        </div>
        <button v-else class="btn btn-dark w-full py-2" @click="backToWork">
          I'm fresh now
        </button>
      </footer>
    </div>

    <PanelHeader :title="dayName(day)" accent>
      <template #eyebrow>
        <button
          class="-mx-1 flex max-w-full items-center gap-1 rounded-[7px] px-1 py-0.5 transition-colors hover:bg-line-soft"
          :class="projectsOpen && 'bg-line-soft'"
          :aria-expanded="projectsOpen"
          title="Switch project"
          @pointerdown.stop
          @click="projectsOpen = !projectsOpen"
        >
          <span class="truncate text-[11px] leading-tight font-semibold text-accent">
            {{ app.activeProject?.name ?? "No project" }}
          </span>
          <AppIcon
            name="chevron"
            :size="10"
            :weight="2.4"
            class="shrink-0 text-accent transition-transform"
            :class="projectsOpen && 'rotate-180'"
          />
        </button>
      </template>

      <template #actions>
        <!-- Only once you have wandered off it: on today it would do nothing,
             and a button that does nothing still asks to be read. -->
        <button
          v-if="!isToday"
          class="icon-btn h-7 w-7 text-accent"
          aria-label="Back to today"
          title="Back to today"
          @click="day = today"
        >
          <AppIcon name="today" :size="14" />
        </button>
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

    <!-- Zero height in the flow: it only marks where the panel hangs from. -->
    <div v-if="projectsOpen" class="relative z-10">
      <ProjectMenu
        class="absolute inset-x-3.5 top-0 shadow-[var(--raised)]"
        :projects="app.projects"
        :active-id="app.activeProject?.id ?? null"
        @pick="switchProject"
        @manage="projectsOpen = false; app.go('projects')"
        @close="projectsOpen = false"
      />
    </div>

    <!-- Which day the list is showing. The date sits between the arrows, and
         doubles as the way back when you have wandered off today. -->
    <div class="flex items-center gap-2 px-3.5 pb-2.5">
      <button
        class="icon-btn h-7 w-7 shrink-0"
        aria-label="Previous day"
        title="Previous day"
        @click="goDay(-1)"
      >
        <AppIcon name="back" :size="13" />
      </button>
      <button
        class="flex min-w-0 flex-1 items-center justify-center gap-1.5 rounded-[9px] px-2 py-1 transition-colors hover:bg-line-soft"
        :class="pickerOpen && 'bg-line-soft'"
        :aria-expanded="pickerOpen"
        title="Pick a day"
        @pointerdown.stop
          @click="pickerOpen = !pickerOpen"
      >
        <AppIcon
          name="calendar"
          :size="12"
          class="shrink-0"
          :class="isToday ? 'text-ink-3' : 'text-accent'"
        />
        <span
          class="truncate text-[12px] font-semibold tracking-[0.01em]"
          :class="isToday ? 'text-ink-2' : 'text-accent'"
        >
          {{ fullDate }}
        </span>
      </button>
      <button
        class="icon-btn h-7 w-7 shrink-0"
        aria-label="Next day"
        title="Next day"
        @click="goDay(1)"
      >
        <AppIcon name="forward" :size="13" />
      </button>
    </div>

    <div v-if="pickerOpen" class="relative z-10">
      <DayPicker
        v-model="day"
        class="absolute inset-x-3.5 top-0 shadow-[var(--raised)]"
        :today="today"
        :marks="tasks.dayMarks"
        @close="pickerOpen = false"
      />
    </div>

    <div v-if="app.isDesktop" class="px-3.5 pb-2.5">
      <StatsCards ref="stats" />
    </div>

    <div class="px-3.5 pb-2.5">
      <div class="card progress-card px-3 py-3">
        <div class="flex items-baseline justify-between gap-2">
          <p
            class="min-w-0 truncate text-[13px] font-semibold"
            :class="isComplete && 'text-success'"
          >
            {{ headline }}
          </p>
          <p class="tnum shrink-0 text-[11px] font-medium text-ink-2">
            <span class="font-semibold text-ink">{{ dayProgress.done }}</span
            >/{{ dayProgress.total }} tasks
            <template v-if="points.total">
              · <span class="font-semibold text-ink">{{ points.done }}</span
              >/{{ points.total }} SP
            </template>
          </p>
        </div>

        <div class="mt-3 flex items-center gap-2.5">
          <div class="min-w-0 flex-1">
            <ProgressBar :ratio="dayProgress.ratio" :segments="dayProgress.total" />
          </div>
          <span
            class="tnum shrink-0 text-[17px] leading-none font-bold tracking-[-0.02em]"
            :class="isComplete ? 'text-success' : dayProgress.total ? '' : 'text-ink-3'"
          >
            <template v-if="dayProgress.total">{{ percent }}%</template>
            <template v-else>—</template>
          </span>
        </div>

        <div class="mt-2.5 flex items-start gap-2">
          <div class="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">
            <span v-if="app.activeProject?.branch" class="chip text-ink-2">
              <AppIcon name="branch" :size="10" />{{ app.activeProject.branch }}
            </span>
            <span
              v-if="tasks.inProgress"
              class="chip border-accent/35 bg-accent-soft text-accent"
            >
              <AppIcon name="sparkle" :size="9" filled />{{ tasks.inProgress }} in progress
            </span>
            <span v-if="remaining" class="chip tnum text-ink-2">{{ remaining }} to go</span>
          </div>

          <span class="flex shrink-0 items-center gap-1">
            <button
              class="icon-btn h-7 w-7"
              :class="rest.isActive && 'border-accent text-accent'"
              aria-label="Take a break"
              title="I'm tired — take a break"
              @click="restOpen = true"
            >
              <AppIcon name="moon" :size="13" />
            </button>
            <button
              class="icon-btn h-7 w-7"
              :disabled="refreshing"
              aria-label="Refresh"
              title="Refresh tasks, figures and branch"
              @click="refresh"
            >
              <AppIcon name="refresh" :size="13" :class="refreshing && 'animate-spin'" />
            </button>
          </span>
        </div>
      </div>
    </div>

    <!-- A run keeps going with the popup closed, so Today has to lead back to
         it — especially when it is blocked on a question. -->
    <button
      v-if="execution.isActive && execution.snapshot.taskId"
      class="card mx-3.5 mb-2.5 flex items-center gap-2.5 px-3 py-2 text-left transition-colors"
      :class="
        execution.isAwaitingApproval
          ? 'border-warn/40 bg-warn-soft/70 hover:bg-warn-soft'
          : 'border-accent/35 bg-accent-soft/50 hover:bg-accent-soft'
      "
      @click="app.openTask(execution.snapshot.taskId)"
    >
      <AppIcon
        :name="execution.isAwaitingApproval ? 'close' : 'sparkle'"
        :size="13"
        filled
        class="shrink-0"
        :class="execution.isAwaitingApproval ? 'text-warn' : 'text-accent'"
      />
      <span class="min-w-0 flex-1">
        <span class="block truncate text-[12px] font-semibold">
          {{
            execution.isAwaitingApproval
              ? `${agentName} needs approval`
              : `${agentName} is working`
          }}
        </span>
        <span class="block truncate text-[11px] text-ink-2">
          {{ execution.snapshot.taskTitle }}
        </span>
      </span>
      <AppIcon name="forward" :size="13" class="shrink-0 text-ink-3" />
    </button>

    <button
      v-else-if="slack.count"
      class="card mx-3.5 mb-2.5 flex items-center gap-2.5 border-accent/35 bg-accent-soft/50 px-3 py-2 text-left transition-colors hover:bg-accent-soft"
      @click="app.go('slack')"
    >
      <AppIcon name="message" :size="13" class="shrink-0 text-accent" />
      <span class="min-w-0 flex-1">
        <span class="block truncate text-[12px] font-semibold">
          {{ slack.count }} message{{ slack.count === 1 ? "" : "s" }} waiting on you
        </span>
        <span class="block truncate text-[11px] text-ink-2">
          {{ slack.waiting.map((conversation) => conversation.with).join(", ") }}
        </span>
      </span>
      <AppIcon name="forward" :size="13" class="shrink-0 text-ink-3" />
    </button>

    <button
      v-else-if="review.isVerifying"
      class="card mx-3.5 mb-2.5 flex items-center gap-2.5 border-accent/35 bg-accent-soft/50 px-3 py-2 text-left transition-colors hover:bg-accent-soft"
      @click="review.verification.taskId && app.openTask(review.verification.taskId)"
    >
      <AppIcon name="check" :size="13" class="shrink-0 text-accent" />
      <span class="min-w-0 flex-1">
        <span class="block truncate text-[12px] font-semibold">
          {{ agentName }} is reviewing a task
        </span>
        <span class="block truncate text-[11px] text-ink-2">
          {{ review.verification.taskTitle }}
        </span>
      </span>
      <span
        class="btn btn-ghost shrink-0 px-2 py-1 text-[11px]"
        role="button"
        @click.stop="review.cancel()"
      >
        Stop
      </span>
    </button>

    <button
      v-else-if="codex.isAnalyzing || codex.plan"
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

    <div class="px-3.5 pb-2.5">
      <div class="segmented">
        <button
          v-for="option in FILTERS"
          :key="option.id"
          class="segmented-item"
          :class="filter === option.id && 'is-selected'"
          :aria-pressed="filter === option.id"
          @click="filter = option.id"
        >
          {{ option.label }}
        </button>
      </div>
      <div class="mt-1.5 flex items-center gap-2">
        <TagFilter v-model="tagFilter" class="min-w-0 flex-1" />
        <button
          class="chip shrink-0 font-semibold transition-colors"
          :class="selecting ? 'border-accent/45 bg-accent-soft text-accent' : 'text-ink-2 hover:bg-line-soft'"
          :aria-pressed="selecting"
          :title="selecting ? 'Stop picking tasks out' : 'Pick tasks out to hand to an agent'"
          @click="selecting ? stopSelecting() : (selecting = true)"
        >
          <AppIcon name="sparkle" :size="10" :filled="selecting" />
          {{ selecting ? "Done" : "Select" }}
        </button>
      </div>
    </div>

    <div class="space-y-1.5 panel-scroll px-3.5 pb-3">
      <NewTaskForm
        v-if="app.activeProject"
        :project-id="app.activeProject.id"
        @submit="add"
      />

      <TransitionGroup name="row" tag="div" class="relative space-y-1.5">
        <TaskRow
          v-for="task in visible"
          :key="task.id"
          :task="task"
          :focused="focus.isFocused(task.id)"
          :remaining-seconds="focus.snapshot.remainingSeconds"
          :blocked-by="tasks.blockedBy(task)"
          :deleting="tasks.deletingId === task.id"
          :carried-over="isCarriedOver(task)"
          :selecting="selecting"
          :selected="selected.has(task.id)"
          @open="app.openTask(task.id)"
          @toggle="tasks.toggleComplete(task)"
          @select="toggleSelected(task.id)"
        />
      </TransitionGroup>

      <p
        v-if="visible.length === 0"
        class="px-2 py-5 text-center text-[12px] leading-relaxed text-ink-3"
      >
        {{
          filter === "done"
            ? "Nothing finished on this day."
            : isToday
              ? "No tasks yet — add one above, or drop in a screenshot or spreadsheet of a list you already have."
              : `Nothing on ${dayName(day).toLowerCase()}. Anything you add here is planned for that day.`
        }}
      </p>
    </div>

    <p
      v-if="dropError"
      class="border-t border-danger/20 bg-danger-soft px-3.5 py-2 text-[11px] text-danger"
    >
      {{ dropError }}
    </p>

    <!-- What was gathered up, and what can be done with it. Sits above the
         footer so the list keeps its own actions where they always are. -->
    <div v-if="selecting" class="border-t border-line px-3.5 py-2.5">
      <div class="flex items-center gap-2">
        <p class="min-w-0 flex-1 text-[11.5px] font-medium text-ink-2">
          <span class="tnum font-semibold text-ink">{{ selected.size }}</span>
          {{ selected.size === 1 ? "task" : "tasks" }} picked
        </p>
        <button
          class="btn btn-ghost shrink-0 px-2.5 py-1.5 text-[11.5px]"
          :disabled="!selected.size"
          @click="selected = new Set(visible.map((task) => task.id))"
        >
          All
        </button>
        <button
          class="btn btn-dark shrink-0 px-3 py-1.5 text-[11.5px]"
          :disabled="!selected.size"
          title="Open these in a window and ask the agent what they add up to"
          @click="openContext"
        >
          <AppIcon name="sparkle" :size="11" filled />Context
        </button>
      </div>
      <p v-if="contextError" class="mt-1.5 text-[11px] text-danger">{{ contextError }}</p>
    </div>

    <footer class="flex items-center gap-2 border-t border-line px-3.5 py-2.5">
      <button class="btn btn-ghost flex-1 py-2 text-[12px]" @click="app.go('goal')">
        <AppIcon name="sparkle" :size="13" filled />New goal
      </button>
      <button
        class="btn btn-ghost px-2.5 py-2"
        title="Plan work for later"
        aria-label="Plan work for later"
        @click="app.go('plan')"
      >
        <AppIcon name="stack" :size="13" />
      </button>
      <button
        class="btn btn-ghost px-2.5 py-2"
        title="History of finished work"
        aria-label="History of finished work"
        @click="app.isDesktop ? app.go('history') : ipc.openDesktopWindow('history')"
      >
        <AppIcon name="calendar" :size="13" />
      </button>
      <button
        class="btn btn-ghost px-2.5 py-2"
        title="Import a task list"
        aria-label="Import a task list"
        @click="importing = true"
      >
        <AppIcon name="import" :size="13" />
      </button>
      <button
        class="btn btn-ghost px-2.5 py-2"
        title="Review changes"
        aria-label="Review changes"
        @click="app.go('changes')"
      >
        <AppIcon name="branch" :size="13" />
      </button>
      <button
        class="btn btn-ghost px-2.5 py-2"
        title="Open in a window"
        aria-label="Open in a window"
        @click="ipc.openDesktopWindow()"
      >
        <AppIcon name="expand" :size="13" />
      </button>
    </footer>
  </div>
</template>
