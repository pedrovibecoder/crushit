import { listen } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { errorMessage, ipc } from "../lib/ipc";
import type { RepoChanges, VerificationSnapshot } from "../types";

const IDLE: VerificationSnapshot = {
  status: "idle",
  taskId: null,
  taskTitle: "",
  steps: [],
  result: null,
  satisfied: 0,
  total: 0,
  recommendation: "",
  error: null,
  startedAt: 0,
};

const NO_CHANGES: RepoChanges = {
  files: [],
  insertions: 0,
  deletions: 0,
  isGit: false,
};

/** Reviewing finished work: what changed in the repository, and whether it holds up. */
export const useReviewStore = defineStore("review", () => {
  const verification = ref<VerificationSnapshot>({ ...IDLE });
  const changes = ref<RepoChanges>({ ...NO_CHANGES });
  const diff = ref<{ path: string; text: string } | null>(null);
  const error = ref<string | null>(null);
  const loadingDiff = ref(false);

  const isVerifying = computed(() => verification.value.status === "running");
  const hasVerdict = computed(() => verification.value.status === "ready");

  function isFor(taskId: number) {
    return verification.value.taskId === taskId && verification.value.status !== "idle";
  }

  function set(next: VerificationSnapshot) {
    verification.value = next;
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

  async function loadChanges() {
    const next = await run(ipc.repoChanges);
    if (next) changes.value = next;
  }

  async function openDiff(path: string) {
    loadingDiff.value = true;
    const text = await run(() => ipc.fileDiff(path));
    loadingDiff.value = false;
    diff.value = text === null ? null : { path, text };
  }

  function closeDiff() {
    diff.value = null;
  }

  async function verify(taskId: number) {
    const next = await run(() => ipc.startVerification(taskId));
    if (next) set(next);
  }

  async function cancel() {
    const next = await run(ipc.cancelVerification);
    if (next) set(next);
  }

  async function subscribe() {
    return listen<VerificationSnapshot>("verification:changed", (event) => set(event.payload));
  }

  return {
    verification,
    changes,
    diff,
    error,
    loadingDiff,
    isVerifying,
    hasVerdict,
    isFor,
    set,
    loadChanges,
    openDiff,
    closeDiff,
    verify,
    cancel,
    subscribe,
  };
});
