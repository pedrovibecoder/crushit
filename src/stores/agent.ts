import { listen } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { errorMessage, ipc } from "../lib/ipc";
import type { Agent, AgentModel, AgentStatus, AnalysisSnapshot, Goal } from "../types";
import { useAppStore } from "./app";

const IDLE: AnalysisSnapshot = {
  status: "idle",
  goalId: null,
  projectId: null,
  goalTitle: "",
  steps: [],
  error: null,
  startedAt: 0,
};

export const useAgentStore = defineStore("agent", () => {
  /** Status per agent; null until checked, since detection shells out. */
  const statuses = ref<Partial<Record<Agent, AgentStatus>>>({});
  const checking = ref(false);
  const models = ref<Partial<Record<Agent, AgentModel[]>>>({});
  const analysis = ref<AnalysisSnapshot>({ ...IDLE });
  const goal = ref<Goal | null>(null);
  const error = ref<string | null>(null);

  /** The agent the user has selected. */
  const selected = computed<Agent>(() => useAppStore().settings.agent);
  const status = computed(() => statuses.value[selected.value] ?? null);
  const currentModels = computed(() => models.value[selected.value] ?? []);

  const isReady = computed(
    () => !!status.value?.installed && !!status.value?.signedIn,
  );
  const isAnalyzing = computed(() => analysis.value.status === "running");
  const plan = computed(() =>
    goal.value?.status === "ready" ? (goal.value.plan ?? null) : null,
  );

  function setAnalysis(next: AnalysisSnapshot) {
    analysis.value = next;
  }

  function setGoal(next: Goal | null) {
    goal.value = next;
  }

  /** `force` re-probes; without it a recent result is reused. */
  async function checkStatus(agent: Agent = selected.value, force = false) {
    checking.value = true;
    try {
      error.value = null;
      statuses.value = { ...statuses.value, [agent]: await ipc.agentStatus(agent, force) };
    } catch (caught) {
      error.value = errorMessage(caught);
    } finally {
      checking.value = false;
    }
  }

  async function loadModels(agent: Agent = selected.value) {
    try {
      models.value = { ...models.value, [agent]: await ipc.agentModels(agent) };
    } catch (caught) {
      // A model list is a nicety; failing to fetch it must not block planning.
      error.value = errorMessage(caught);
    }
  }

  async function refreshGoal(projectId: number | null) {
    if (projectId === null) {
      goal.value = null;
      return;
    }
    try {
      goal.value = await ipc.latestGoal(projectId);
    } catch (caught) {
      error.value = errorMessage(caught);
    }
  }

  async function analyze(projectId: number, prompt: string) {
    try {
      error.value = null;
      goal.value = await ipc.startAnalysis(projectId, prompt);
      return true;
    } catch (caught) {
      error.value = errorMessage(caught);
      return false;
    }
  }

  async function cancel(projectId: number | null) {
    try {
      analysis.value = await ipc.cancelAnalysis();
      await refreshGoal(projectId);
    } catch (caught) {
      error.value = errorMessage(caught);
    }
  }

  async function accept(goalId: number) {
    try {
      error.value = null;
      await ipc.acceptPlan(goalId);
      goal.value = null;
      analysis.value = { ...IDLE };
      return true;
    } catch (caught) {
      error.value = errorMessage(caught);
      return false;
    }
  }

  async function discard(goalId: number) {
    try {
      error.value = null;
      await ipc.discardPlan(goalId);
      goal.value = null;
      analysis.value = { ...IDLE };
    } catch (caught) {
      error.value = errorMessage(caught);
    }
  }

  /** The backend owns the run, so the UI only mirrors what it emits. */
  async function subscribe(onGoalsChanged: () => void) {
    const stops = await Promise.all([
      listen<AnalysisSnapshot>("codex:analysis", (event) => setAnalysis(event.payload)),
      listen("goals:changed", () => onGoalsChanged()),
    ]);
    return () => stops.forEach((stop) => stop());
  }

  return {
    statuses,
    status,
    selected,
    checking,
    models,
    currentModels,
    analysis,
    goal,
    error,
    isReady,
    isAnalyzing,
    plan,
    setAnalysis,
    setGoal,
    checkStatus,
    loadModels,
    refreshGoal,
    analyze,
    cancel,
    accept,
    discard,
    subscribe,
  };
});
