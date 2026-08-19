<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import AppIcon from "../components/AppIcon.vue";
import CategoryPill from "../components/CategoryPill.vue";
import ContributionGraph from "../components/ContributionGraph.vue";
import PanelHeader from "../components/PanelHeader.vue";
import { errorMessage, ipc } from "../lib/ipc";
import { useAppStore } from "../stores/app";
import { useTasksStore } from "../stores/tasks";
import type { CompletedTask } from "../types";

const app = useAppStore();
const tasks = useTasksStore();

const DAYS = 365;

const history = ref<CompletedTask[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);
/** Set by clicking a square, to read one day rather than the whole year. */
const picked = ref<string | null>(null);

/** Local date, matching how the graph buckets its squares. */
function dayOf(completedAt: number) {
  const date = new Date(completedAt * 1000);
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

async function load() {
  if (!app.activeProject) {
    history.value = [];
    return;
  }
  loading.value = true;
  try {
    error.value = null;
    history.value = await ipc.completedTasks(app.activeProject.id, DAYS);
  } catch (caught) {
    error.value = errorMessage(caught);
  } finally {
    loading.value = false;
  }
}

onMounted(load);
// Finishing something while this is open should show up in it.
watch(() => [app.activeProject?.id, tasks.progress.done], load);

const counts = computed(() => {
  const byDay: Record<string, number> = {};
  for (const task of history.value) {
    const day = dayOf(task.completedAt);
    byDay[day] = (byDay[day] ?? 0) + 1;
  }
  return byDay;
});

const points = computed(() =>
  history.value.reduce((sum, task) => sum + (task.storyPoints ?? 0), 0),
);

/** The longest run of days ending today or yesterday, in the reader's clock. */
const streak = computed(() => {
  const days = counts.value;
  const today = new Date();
  today.setHours(12, 0, 0, 0);
  let cursor = new Date(today);
  // A day that is not over yet should not break a streak.
  if (!days[dayOf(cursor.getTime() / 1000)]) cursor = new Date(cursor.getTime() - 86_400_000);
  let run = 0;
  while (days[dayOf(cursor.getTime() / 1000)]) {
    run += 1;
    cursor = new Date(cursor.getTime() - 86_400_000);
  }
  return run;
});

/** Newest first, grouped into the day each task was finished on. */
const grouped = computed(() => {
  const days = new Map<string, CompletedTask[]>();
  for (const task of history.value) {
    const day = dayOf(task.completedAt);
    if (picked.value && day !== picked.value) continue;
    const list = days.get(day) ?? [];
    list.push(task);
    days.set(day, list);
  }
  return [...days.entries()];
});

function readableDay(day: string) {
  const date = new Date(`${day}T12:00:00`);
  const today = new Date();
  today.setHours(12, 0, 0, 0);
  const days = Math.round((today.getTime() - date.getTime()) / 86_400_000);
  if (days === 0) return "Today";
  if (days === 1) return "Yesterday";
  return date.toLocaleDateString(undefined, {
    weekday: "long",
    day: "numeric",
    month: "long",
    year: date.getFullYear() === today.getFullYear() ? undefined : "numeric",
  });
}

function clockOf(completedAt: number) {
  return new Date(completedAt * 1000).toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });
}
</script>

<template>
  <div class="flex flex-col">
    <PanelHeader
      :eyebrow="app.activeProject?.name"
      title="History"
      :back="!app.isDesktop"
      @back="app.back()"
    />

    <div class="panel-scroll px-3.5 pb-3.5">
      <div class="card px-3 py-3">
        <div class="flex flex-wrap items-baseline justify-between gap-2">
          <p class="text-[12.5px] font-semibold">
            {{ history.length }} task{{ history.length === 1 ? "" : "s" }} finished
            <span class="font-medium text-ink-2">in the last year</span>
          </p>
          <p class="tnum flex items-center gap-2 text-[11px] text-ink-2">
            <span v-if="points">{{ points }} SP</span>
            <span v-if="streak" class="text-success">{{ streak }}-day streak</span>
          </p>
        </div>

        <div class="mt-2.5">
          <ContributionGraph :counts="counts" @pick="picked = picked === $event ? null : $event" />
        </div>
      </div>

      <p v-if="error" class="mt-2 text-[11px] text-danger">{{ error }}</p>

      <div v-if="picked" class="mt-3 flex items-center gap-2">
        <p class="eyebrow flex-1">Showing {{ readableDay(picked) }}</p>
        <button class="btn btn-ghost px-2 py-1 text-[11px]" @click="picked = null">
          Show everything
        </button>
      </div>

      <div v-for="[day, done] in grouped" :key="day" class="mt-3">
        <div class="flex items-baseline justify-between gap-2">
          <h2 class="eyebrow">{{ readableDay(day) }}</h2>
          <span class="tnum text-[10.5px] text-ink-3">{{ done.length }}</span>
        </div>
        <ul class="mt-1.5 space-y-1">
          <li
            v-for="task in done"
            :key="task.id"
            class="card flex items-start gap-2.5 px-2.5 py-2"
          >
            <span
              class="mt-px flex h-[17px] w-[17px] shrink-0 items-center justify-center rounded-[6px] bg-success text-white"
            >
              <AppIcon name="check" :size="10" :weight="2.4" />
            </span>
            <span class="min-w-0 flex-1">
              <span class="block text-[12.5px] leading-snug font-semibold break-words">
                {{ task.title }}
              </span>
              <span class="mt-1 flex flex-wrap items-center gap-1">
                <CategoryPill :category="task.category" />
                <span v-if="task.storyPoints" class="chip tnum text-ink-2">
                  {{ task.storyPoints }} SP
                </span>
              </span>
            </span>
            <span class="tnum shrink-0 text-[10.5px] text-ink-3">
              {{ clockOf(task.completedAt) }}
            </span>
          </li>
        </ul>
      </div>

      <p
        v-if="!loading && !grouped.length"
        class="px-2 py-6 text-center text-[12px] leading-relaxed text-ink-3"
      >
        {{
          picked
            ? "Nothing was finished on that day."
            : "Nothing finished yet — completed tasks land here with the date you finished them."
        }}
      </p>
    </div>
  </div>
</template>
