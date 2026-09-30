import { listen } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { errorMessage, ipc } from "../lib/ipc";
import type { Project, Settings, SettingsPatch } from "../types";
import { useAgentStore } from "./agent";
import { useCategoriesStore } from "./categories";
import { useGroupsStore } from "./groups";
import { useTagsStore } from "./tags";
import { useExecutionStore } from "./execution";
import { useFocusStore } from "./focus";
import { useRestStore } from "./rest";
import { useReviewStore } from "./review";
import { useTasksStore } from "./tasks";

export type View =
  | "today"
  | "plan"
  | "context"
  | "task"
  | "projects"
  | "goal"
  | "settings"
  | "changes"
  | "history"
  | "slack"
  | "onboarding";

const DEFAULT_SETTINGS: Settings = {
  focusMinutes: 25,
  showTimerInMenuBar: true,
  hidePopupOnBlur: true,
  activeProjectId: null,
  agent: "codex",
  codexPath: null,
  codexModel: null,
  claudePath: null,
  claudeModel: null,
  slackConnected: false,
  theme: "light",
  notifications: true,
  sounds: true,
  launchAtLogin: false,
  focusBlockEnabled: false,
  focusBlockSites: ["instagram.com", "facebook.com", "x.com", "twitter.com", "tiktok.com"],
  onboarded: false,
};

export const useAppStore = defineStore("app", () => {
  const ready = ref(false);
  const error = ref<string | null>(null);
  /** Set when startup itself failed, so the UI can offer a way back. */
  const failed = ref(false);
  const settings = ref<Settings>({ ...DEFAULT_SETTINGS });
  const projects = ref<Project[]>([]);
  const activeProject = ref<Project | null>(null);

  /** True on the desktop window, which has room for more than the popup. */
  const isDesktop = ref(false);
  const view = ref<View>("today");
  const selectedTaskId = ref<number | null>(null);

  const hasProject = computed(() => activeProject.value !== null);

  function go(next: View, taskId: number | null = null) {
    view.value = next;
    selectedTaskId.value = taskId;
  }

  function openTask(taskId: number) {
    go("task", taskId);
  }

  function back() {
    go("today");
  }

  async function bootstrap() {
    const tasks = useTasksStore();
    const focus = useFocusStore();
    try {
      error.value = null;
      failed.value = false;
      const data = await ipc.bootstrap();
      settings.value = data.settings;
      projects.value = data.projects;
      activeProject.value = data.activeProject;
      tasks.set(data.tasks);
      useCategoriesStore().set(data.categories);
      useTagsStore().set(data.tags);
      useGroupsStore().set(data.taskGroups);
      focus.set(data.focus);
      useRestStore().set(data.rest);
      const codex = useAgentStore();
      codex.setAnalysis(data.analysis);
      codex.setGoal(data.goal);
      useExecutionStore().set(data.execution);
      useReviewStore().set(data.verification);
      // First run walks through setup; after that, straight to the work.
      view.value = !data.settings.onboarded
        ? "onboarding"
        : data.activeProject
          ? "today"
          : "projects";
    } catch (caught) {
      error.value = errorMessage(caught);
      failed.value = true;
    } finally {
      ready.value = true;
    }
  }

  /** Backend mutations broadcast here so every open view stays in step. */
  async function subscribe() {
    const stopTasks = await listen("tasks:changed", () => {
      const tasks = useTasksStore();
      void tasks.load(activeProject.value?.id ?? null);
    });
    const focus = useFocusStore();
    const stopFocus = await focus.subscribe();
    const codex = useAgentStore();
    const stopCodex = await codex.subscribe(() => {
      void codex.refreshGoal(activeProject.value?.id ?? null);
    });
    const stopExecution = await useExecutionStore().subscribe();
    const stopReview = await useReviewStore().subscribe();
    const stopCategories = await useCategoriesStore().subscribe();
    const stopTags = await useTagsStore().subscribe();
    const stopRest = await useRestStore().subscribe();
    return () => {
      stopTasks();
      stopCategories();
      stopTags();
      stopRest();
      stopFocus();
      stopCodex();
      stopExecution();
      stopReview();
    };
  }

  async function selectProject(projectId: number | null) {
    const tasks = useTasksStore();
    try {
      error.value = null;
      settings.value = await ipc.setActiveProject(projectId);
      activeProject.value =
        projects.value.find((project) => project.id === projectId) ?? null;
      await tasks.load(projectId);
      await useGroupsStore().load(projectId);
      await useAgentStore().refreshGoal(projectId);
      view.value = projectId === null ? "projects" : "today";
    } catch (caught) {
      error.value = errorMessage(caught);
    }
  }

  async function addProject(path: string) {
    const tasks = useTasksStore();
    try {
      error.value = null;
      const project = await ipc.addProject(path);
      projects.value = await ipc.listProjects();
      activeProject.value = project;
      settings.value = { ...settings.value, activeProjectId: project.id };
      await tasks.load(project.id);
      await useAgentStore().refreshGoal(project.id);
      view.value = "today";
      return project;
    } catch (caught) {
      error.value = errorMessage(caught);
      return null;
    }
  }

  async function removeProject(projectId: number) {
    const tasks = useTasksStore();
    try {
      error.value = null;
      projects.value = await ipc.removeProject(projectId);
      settings.value = await ipc.getSettings();
      activeProject.value =
        projects.value.find(
          (project) => project.id === settings.value.activeProjectId,
        ) ?? null;
      await tasks.load(activeProject.value?.id ?? null);
      if (!activeProject.value) view.value = "projects";
    } catch (caught) {
      error.value = errorMessage(caught);
    }
  }

  /**
   * Picks up a branch switch made outside the app. This shells out to git
   * twice, and the popup can be opened many times a minute, so it is throttled
   * rather than run on every focus.
   */
  const REFRESH_EVERY_MS = 30_000;
  let lastRefresh = 0;

  async function refreshActiveProject(force = false) {
    if (!activeProject.value) return;
    if (!force && Date.now() - lastRefresh < REFRESH_EVERY_MS) return;
    lastRefresh = Date.now();
    try {
      const refreshed = await ipc.refreshProject(activeProject.value.id);
      activeProject.value = refreshed;
      projects.value = projects.value.map((project) =>
        project.id === refreshed.id ? refreshed : project,
      );
    } catch {
      // A project whose directory moved should not break the popup.
    }
  }

  /**
   * Re-reads what the backend says is running. These snapshots normally arrive
   * as events; a window that was not listening when one was emitted has no
   * other way to catch up, and would show nothing while a run is still going.
   */
  async function refreshRuns() {
    const codex = useAgentStore();
    const execution = useExecutionStore();
    const review = useReviewStore();
    try {
      const [analysis, run, verification] = await Promise.all([
        ipc.analysisSnapshot(),
        ipc.executionSnapshot(),
        ipc.verificationSnapshot(),
      ]);
      codex.setAnalysis(analysis);
      execution.set(run);
      review.set(verification);
    } catch (caught) {
      error.value = errorMessage(caught);
    }
  }

  async function updateSettings(patch: SettingsPatch) {
    try {
      error.value = null;
      settings.value = await ipc.updateSettings(patch);
    } catch (caught) {
      error.value = errorMessage(caught);
    }
  }

  return {
    ready,
    error,
    failed,
    isDesktop,
    settings,
    projects,
    activeProject,
    view,
    selectedTaskId,
    hasProject,
    go,
    openTask,
    back,
    bootstrap,
    subscribe,
    selectProject,
    addProject,
    removeProject,
    refreshActiveProject,
    refreshRuns,
    updateSettings,
  };
});
