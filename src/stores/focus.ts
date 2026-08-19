import { listen } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { errorMessage, ipc } from "../lib/ipc";
import type { FocusSnapshot } from "../types";

const IDLE: FocusSnapshot = {
  status: "idle",
  sessionId: null,
  taskId: null,
  durationSeconds: 0,
  elapsedSeconds: 0,
  remainingSeconds: 0,
};

export const useFocusStore = defineStore("focus", () => {
  const snapshot = ref<FocusSnapshot>({ ...IDLE });
  const error = ref<string | null>(null);
  /** Set when a session runs out, until the user dismisses it. */
  const justFinishedTaskId = ref<number | null>(null);

  const isActive = computed(() => snapshot.value.status !== "idle");
  const isRunning = computed(() => snapshot.value.status === "running");
  const isPaused = computed(() => snapshot.value.status === "paused");

  function isFocused(taskId: number) {
    return isActive.value && snapshot.value.taskId === taskId;
  }

  function set(next: FocusSnapshot) {
    snapshot.value = next;
  }

  /**
   * The backend owns the clock, so the UI only mirrors what it emits. This
   * keeps the timer correct even while the popup is closed.
   */
  async function subscribe() {
    const stops = await Promise.all([
      listen<FocusSnapshot>("focus:tick", (event) => set(event.payload)),
      listen<FocusSnapshot>("focus:changed", (event) => set(event.payload)),
      listen<FocusSnapshot>("focus:finished", (event) => {
        set(event.payload);
        justFinishedTaskId.value = event.payload.taskId;
      }),
    ]);
    return () => stops.forEach((stop) => stop());
  }

  async function run(action: () => Promise<FocusSnapshot>) {
    try {
      error.value = null;
      set(await action());
    } catch (caught) {
      error.value = errorMessage(caught);
    }
  }

  const start = (taskId: number, durationMinutes?: number) =>
    run(() => ipc.startFocus(taskId, durationMinutes));
  const pause = () => run(ipc.pauseFocus);
  const resume = () => run(ipc.resumeFocus);

  async function stop() {
    justFinishedTaskId.value = null;
    await run(ipc.stopFocus);
  }

  return {
    snapshot,
    error,
    justFinishedTaskId,
    isActive,
    isRunning,
    isPaused,
    isFocused,
    set,
    subscribe,
    start,
    pause,
    resume,
    stop,
  };
});
