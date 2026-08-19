import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { errorMessage, ipc } from "../lib/ipc";
import type { NewTask, Task, TaskPatch, TaskStatus } from "../types";

const OPEN_STATUSES: TaskStatus[] = [
  "ready",
  "in-progress",
  "needs-review",
  "blocked",
  "backlog",
];

export const useTasksStore = defineStore("tasks", () => {
  const tasks = ref<Task[]>([]);
  const error = ref<string | null>(null);
  /** Set when a task has just been finished, so the UI can celebrate it. */
  const celebration = ref<{ title: string; at: number } | null>(null);

  const byId = computed(
    () => new Map(tasks.value.map((task) => [task.id, task])),
  );

  const openTasks = computed(() =>
    tasks.value.filter((task) => OPEN_STATUSES.includes(task.status)),
  );
  const completedTasks = computed(() =>
    tasks.value.filter((task) => task.status === "completed"),
  );

  const progress = computed(() => {
    const total = tasks.value.length;
    const done = completedTasks.value.length;
    return { done, total, ratio: total === 0 ? 0 : done / total };
  });

  /** A task is blocked while any task it depends on is still open. */
  function blockedBy(task: Task): Task[] {
    return task.dependsOn
      .map((id) => byId.value.get(id))
      .filter((other): other is Task => !!other && other.status !== "completed");
  }

  function replace(updated: Task) {
    const index = tasks.value.findIndex((task) => task.id === updated.id);
    if (index >= 0) tasks.value[index] = updated;
    else tasks.value.push(updated);
  }

  async function run<T>(action: () => Promise<T>): Promise<T | null> {
    try {
      error.value = null;
      return await action();
    } catch (caught) {
      error.value = errorMessage(caught);
      return null;
    }
  }

  function set(next: Task[]) {
    tasks.value = next;
  }

  async function load(projectId: number | null) {
    if (projectId === null) {
      tasks.value = [];
      return;
    }
    const loaded = await run(() => ipc.listTasks(projectId));
    if (loaded) tasks.value = loaded;
  }

  async function create(input: NewTask) {
    const created = await run(() => ipc.createTask(input));
    if (created) tasks.value.push(created);
    return created;
  }

  async function update(taskId: number, patch: TaskPatch) {
    const before = byId.value.get(taskId)?.status;
    const updated = await run(() => ipc.updateTask(taskId, patch));
    if (!updated) return updated;
    replace(updated);
    // Celebrate the moment work crosses the line, however it was crossed.
    if (updated.status === "completed" && before !== "completed") {
      celebration.value = { title: updated.title, at: Date.now() };
    }
    return updated;
  }

  function clearCelebration() {
    celebration.value = null;
  }

  async function setStatus(taskId: number, status: TaskStatus) {
    return update(taskId, { status });
  }

  async function toggleComplete(task: Task) {
    return setStatus(task.id, task.status === "completed" ? "ready" : "completed");
  }

  async function remove(taskId: number) {
    const done = await run(() => ipc.deleteTask(taskId));
    if (done !== null) tasks.value = tasks.value.filter((t) => t.id !== taskId);
  }

  async function reorder(projectId: number, orderedIds: number[]) {
    const reordered = await run(() => ipc.reorderTasks(projectId, orderedIds));
    if (reordered) tasks.value = reordered;
  }

  async function setCriterion(criterionId: number, isMet: boolean) {
    const updated = await run(() => ipc.setCriterionMet(criterionId, isMet));
    if (updated) replace(updated);
  }

  return {
    tasks,
    error,
    celebration,
    clearCelebration,
    byId,
    openTasks,
    completedTasks,
    progress,
    blockedBy,
    set,
    load,
    create,
    update,
    setStatus,
    toggleComplete,
    remove,
    reorder,
    setCriterion,
  };
});
