<script setup lang="ts">
import { getCurrentWindow } from "@tauri-apps/api/window";
import { computed, onBeforeUnmount, onMounted, watch } from "vue";
import DesktopShell from "./components/DesktopShell.vue";
import PopupShell from "./components/PopupShell.vue";
import { ipc } from "./lib/ipc";
import { isTyping, match } from "./lib/shortcuts";
import { chime } from "./lib/sound";
import { useAppStore, type View } from "./stores/app";
import { useTasksStore } from "./stores/tasks";
import ChangesView from "./views/ChangesView.vue";
import HistoryView from "./views/HistoryView.vue";
import SlackView from "./views/SlackView.vue";
import GoalView from "./views/GoalView.vue";
import OnboardingView from "./views/OnboardingView.vue";
import ProjectsView from "./views/ProjectsView.vue";
import SettingsView from "./views/SettingsView.vue";
import ContextView from "./views/ContextView.vue";
import PlanView from "./views/PlanView.vue";
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
  plan: PlanView,
  context: ContextView,
  task: TaskView,
  projects: ProjectsView,
  goal: GoalView,
  settings: SettingsView,
  changes: ChangesView,
  history: HistoryView,
  slack: SlackView,
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

/**
 * Both windows run this app, so a sound has to come from one of them and the
 * hidden surface stays quiet. This is only for the small in-app sounds: the
 * alarm at the end of a session is played by the backend, because that is
 * exactly when every window is hidden and a suspended webview plays nothing.
 */
async function sound(play: () => void) {
  if (!app.settings.sounds) return;
  try {
    if (await getCurrentWindow().isVisible()) play();
  } catch {
    // Outside Tauri there is only one surface, so just play it.
    play();
  }
}

// The celebration clears itself; the confetti is over well before then.
const CELEBRATION_MS = 3200;
let celebrationTimer: ReturnType<typeof setTimeout> | undefined;

watch(
  () => tasks.celebration?.at,
  (at) => {
    clearTimeout(celebrationTimer);
    if (at === undefined) return;
    void sound(chime);
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
  // Listening starts before the data does: a window opened at a particular
  // screen is told where to go the moment it appears, which is well before
  // there is anything to show there.
  try {
    const { listen } = await import("@tauri-apps/api/event");
    teardown.push(
      await listen<string>("desktop:navigate", (event) => {
        if (event.payload in VIEWS) app.go(event.payload as View);
      }),
    );
  } catch {
    // Only the desktop window is ever sent anywhere.
  }

  await app.bootstrap();
  teardown.push(await app.subscribe());

  window.addEventListener("keydown", onKeydown);
  teardown.push(() => window.removeEventListener("keydown", onKeydown));

  // A window that was closed starts its webview only when it is shown, so the
  // event above can have been sent before anything existed to hear it — and
  // bootstrap has just decided which screen to open on regardless. Asking
  // settles both.
  if (isDesktop.value) {
    try {
      const pending = await ipc.takePendingView();
      if (pending && pending in VIEWS) app.go(pending as View);
    } catch {
      // Nowhere to be sent; stay where bootstrap left us.
    }
  }

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
