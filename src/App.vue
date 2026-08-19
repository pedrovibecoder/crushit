<script setup lang="ts">
import { getCurrentWindow } from "@tauri-apps/api/window";
import { computed, onBeforeUnmount, onMounted, watch } from "vue";
import DesktopShell from "./components/DesktopShell.vue";
import PopupShell from "./components/PopupShell.vue";
import { ipc } from "./lib/ipc";
import { isTyping, match } from "./lib/shortcuts";
import { useAppStore } from "./stores/app";
import { useTasksStore } from "./stores/tasks";
import ChangesView from "./views/ChangesView.vue";
import GoalView from "./views/GoalView.vue";
import OnboardingView from "./views/OnboardingView.vue";
import ProjectsView from "./views/ProjectsView.vue";
import SettingsView from "./views/SettingsView.vue";
import TaskView from "./views/TaskView.vue";
import TodayView from "./views/TodayView.vue";

const app = useAppStore();
const tasks = useTasksStore();

const teardown: Array<() => void> = [];

/**
 * Both surfaces run the same app; the window label says which one this is.
 * Outside Tauri (the preview harness) it falls back to the popup.
 */
const surface = (() => {
  try {
    return getCurrentWindow().label;
  } catch {
    return "popup";
  }
})();
const isDesktop = computed(() => surface === "main");
app.isDesktop = isDesktop.value;

const VIEWS = {
  today: TodayView,
  task: TaskView,
  projects: ProjectsView,
  goal: GoalView,
  settings: SettingsView,
  changes: ChangesView,
  onboarding: OnboardingView,
};

const current = computed(() => VIEWS[app.view]);

// The palette lives on the root element; every token resolves from there.
watch(
  () => app.settings.theme,
  (theme) => document.documentElement.setAttribute("data-theme", theme),
  { immediate: true },
);

function hideSurface() {
  if (isDesktop.value) void ipc.hideDesktopWindow();
  else void ipc.hidePopup();
}

function onKeydown(event: KeyboardEvent) {
  // Escape works even mid-typing: it is how you back out of a field.
  if (event.key === "Escape") {
    if (isTyping(event.target)) return;
    if (app.view !== "today" && app.hasProject) app.back();
    else hideSurface();
    return;
  }

  const shortcut = match(event);
  if (!shortcut || isTyping(event.target)) return;
  event.preventDefault();

  if (shortcut.action === "hide") hideSurface();
  else if (shortcut.action === "newTask") {
    app.go("today");
    // Let Today render before reaching for its input.
    requestAnimationFrame(() =>
      document.querySelector<HTMLInputElement>('input[aria-label="New task title"]')?.focus(),
    );
  } else app.go(shortcut.action);
}

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

// Leaving a task that no longer exists would render an empty shell.
watch(
  () => app.selectedTaskId,
  (id) => {
    if (app.view === "task" && id !== null && !tasks.byId.has(id)) app.back();
  },
);

onMounted(async () => {
  await app.bootstrap();
  teardown.push(await app.subscribe());

  window.addEventListener("keydown", onKeydown);
  teardown.push(() => window.removeEventListener("keydown", onKeydown));

  try {
    const unlisten = await getCurrentWindow().onFocusChanged(({ payload }) => {
      // A branch may have changed on disk while this surface was hidden.
      if (payload) void app.refreshActiveProject();
    });
    teardown.push(unlisten);
  } catch {
    // Focus tracking is a nicety; both surfaces work without it.
  }
});

onBeforeUnmount(() => {
  clearTimeout(celebrationTimer);
  teardown.forEach((stop) => stop());
});
</script>

<template>
  <DesktopShell v-if="isDesktop" :current="current" :ready="app.ready" />
  <PopupShell v-else :current="current" :ready="app.ready" />
</template>
