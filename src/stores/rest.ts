import { listen } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { errorMessage, ipc } from "../lib/ipc";
import type { RestSnapshot } from "../types";

const IDLE: RestSnapshot = {
  status: "idle",
  minutes: 0,
  durationSeconds: 0,
  elapsedSeconds: 0,
  remainingSeconds: 0,
};

/** Taking a break: the developer's own clock, attached to no task. */
export const useRestStore = defineStore("rest", () => {
  const snapshot = ref<RestSnapshot>({ ...IDLE });
  const error = ref<string | null>(null);

  const isResting = computed(() => snapshot.value.status === "running");
  const isOver = computed(() => snapshot.value.status === "finished");
  const isActive = computed(() => snapshot.value.status !== "idle");

  function set(next: RestSnapshot) {
    snapshot.value = next;
  }

  async function run(action: () => Promise<RestSnapshot>) {
    try {
      error.value = null;
      set(await action());
    } catch (caught) {
      error.value = errorMessage(caught);
    }
  }

  const start = (minutes: number) => run(() => ipc.startRest(minutes));
  const stop = () => run(ipc.stopRest);
  const load = () => run(ipc.restSnapshot);

  /** The backend owns the clock, so this only mirrors what it emits. */
  async function subscribe() {
    return listen<RestSnapshot>("rest:changed", (event) => set(event.payload));
  }

  return { snapshot, error, isResting, isOver, isActive, set, load, start, stop, subscribe };
});
