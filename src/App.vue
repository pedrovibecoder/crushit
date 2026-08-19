<script setup lang="ts">
import { getCurrentWindow } from "@tauri-apps/api/window";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import ConfettiBurst from "./components/ConfettiBurst.vue";
import { ipc } from "./lib/ipc";
import { useAppStore } from "./stores/app";
import { useFocusStore } from "./stores/focus";
import { useTasksStore } from "./stores/tasks";
import GoalView from "./views/GoalView.vue";
import ProjectsView from "./views/ProjectsView.vue";
import SettingsView from "./views/SettingsView.vue";
import TaskView from "./views/TaskView.vue";
import TodayView from "./views/TodayView.vue";

const app = useAppStore();
const tasks = useTasksStore();
const focus = useFocusStore();

const shell = ref<HTMLElement | null>(null);
const teardown: Array<() => void> = [];

const VIEWS = {
  today: TodayView,
  task: TaskView,
  projects: ProjectsView,
  goal: GoalView,
  settings: SettingsView,
};

const current = computed(() => VIEWS[app.view]);

/** Any of the three stores may have something worth showing. */
const banner = computed(() => app.error ?? tasks.error ?? focus.error);

function dismissBanner() {
  app.error = null;
  tasks.error = null;
  focus.error = null;
}

function onKeydown(event: KeyboardEvent) {
  if (event.key !== "Escape") return;
  // Escape backs out of a subview first, then closes the popup.
  if (app.view !== "today" && app.hasProject) app.back();
  else void ipc.hidePopup();
}

onMounted(async () => {
  await app.bootstrap();
  teardown.push(await app.subscribe());

  window.addEventListener("keydown", onKeydown);
  teardown.push(() => window.removeEventListener("keydown", onKeydown));

  // Keep the OS window exactly as tall as the rendered popup.
  if (shell.value) {
    // Only the height we last asked for is skipped. Remembering more than that
    // refuses legitimate repeats — going Today → Task → Today reports Today's
    // original height again, and suppressing it strands the window at the
    // task screen's size.
    let requested = 0;
    const observer = new ResizeObserver(([entry]) => {
      const height = Math.ceil(
        entry.borderBoxSize?.[0]?.blockSize ?? (entry.target as HTMLElement).offsetHeight,
      );
      if (height === 0 || height === requested) return;
      requested = height;
      void ipc.resizePopup(height);
    });
    observer.observe(shell.value);
    teardown.push(() => observer.disconnect());
  }

  try {
    const unlisten = await getCurrentWindow().onFocusChanged(({ payload }) => {
      // A branch may have changed on disk while the popup was hidden.
      if (payload) void app.refreshActiveProject();
    });
    teardown.push(unlisten);
  } catch {
    // Focus tracking is a nicety; the popup works without it.
  }
});

onBeforeUnmount(() => teardown.forEach((stop) => stop()));

// The celebration clears itself; the confetti is over well before then.
const CELEBRATION_MS = 3200;
let celebrationTimer: ReturnType<typeof setTimeout> | undefined;

watch(
  () => tasks.celebration?.at,
  (at) => {
    clearTimeout(celebrationTimer);
    if (at === undefined) return;
    celebrationTimer = setTimeout(() => tasks.clearCelebration(), CELEBRATION_MS);
  },
);

onBeforeUnmount(() => clearTimeout(celebrationTimer));

// Leaving a task that no longer exists would render an empty shell.
watch(
  () => app.selectedTaskId,
  (id) => {
    if (app.view === "task" && id !== null && !tasks.byId.has(id)) app.back();
  },
);
</script>

<template>
  <div ref="shell" class="w-full">
    <div
      class="relative flex min-h-[228px] w-full flex-col overflow-hidden rounded-[14px] border border-line bg-page"
    >
      <p
        v-if="banner"
        class="flex items-start gap-2 border-b border-danger/25 bg-danger-soft px-3.5 py-2 text-[11.5px] text-danger"
      >
        <span class="min-w-0 flex-1">{{ banner }}</span>
        <button class="shrink-0 font-semibold underline" @click="dismissBanner">
          Dismiss
        </button>
      </p>

      <div
        v-if="!app.ready"
        class="flex h-[228px] items-center justify-center text-[12px] text-ink-3"
      >
        Loading…
      </div>
      <component :is="current" v-else />

      <!-- Celebration sits above the panel and never affects its height. -->
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
    </div>
  </div>
</template>
