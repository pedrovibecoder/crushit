import { listen } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { errorMessage, ipc } from "../lib/ipc";
import type { ExecutionSnapshot } from "../types";

const IDLE: ExecutionSnapshot = {
  status: "idle",
  taskId: null,
  taskTitle: "",
  agent: "codex",
  threadId: null,
  activity: [],
  changedFiles: [],
  approval: null,
  summary: null,
  error: null,
  startedAt: 0,
};

export const useExecutionStore = defineStore("execution", () => {
  const snapshot = ref<ExecutionSnapshot>({ ...IDLE });
  const error = ref<string | null>(null);

  const isActive = computed(
    () =>
      snapshot.value.status === "running" || snapshot.value.status === "awaiting-approval",
  );
  const isAwaitingApproval = computed(() => snapshot.value.status === "awaiting-approval");
  const isSettled = computed(
    () => snapshot.value.status === "finished" || snapshot.value.status === "failed",
  );

  /** True when the run on screen belongs to this task. */
  function isFor(taskId: number) {
    return snapshot.value.taskId === taskId && snapshot.value.status !== "idle";
  }

  function set(next: ExecutionSnapshot) {
    snapshot.value = next;
  }

  async function run<T>(action: () => Promise<T>) {
    try {
      error.value = null;
      return await action();
    } catch (caught) {
      error.value = errorMessage(caught);
      return null;
    }
  }

  const start = (taskId: number, resume = false) => run(() => ipc.startExecution(taskId, resume));

  async function stop() {
    const next = await run(ipc.stopExecution);
    if (next) set(next);
  }

  async function respond(approve: boolean) {
    const next = await run(() => ipc.respondToApproval(approve));
    if (next) set(next);
  }

  async function dismiss() {
    const next = await run(ipc.clearExecution);
    if (next) set(next);
  }

  /** The backend owns the run, so the UI only mirrors what it emits. */
  async function subscribe() {
    return listen<ExecutionSnapshot>("execution:changed", (event) => set(event.payload));
  }

  return {
    snapshot,
    error,
    isActive,
    isAwaitingApproval,
    isSettled,
    isFor,
    set,
    start,
    stop,
    respond,
    dismiss,
    subscribe,
  };
});
