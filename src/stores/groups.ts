import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { errorMessage, ipc } from "../lib/ipc";
import type { TaskGroup } from "../types";

/**
 * Named buckets for work that is not happening today. A group holds no dates
 * of its own — the tasks in it keep the days they sit on — so this store is
 * only ever the list of names and the order they are shown in.
 */
export const useGroupsStore = defineStore("groups", () => {
  const groups = ref<TaskGroup[]>([]);
  const error = ref<string | null>(null);

  const byId = computed(() => new Map(groups.value.map((group) => [group.id, group])));

  /** A task filed under a group that has since gone shows as ungrouped. */
  function nameFor(groupId: number | null) {
    return groupId === null ? null : (byId.value.get(groupId)?.name ?? null);
  }

  function set(next: TaskGroup[]) {
    groups.value = next;
  }

  async function run(action: () => Promise<TaskGroup[]>) {
    try {
      error.value = null;
      groups.value = await action();
      return true;
    } catch (caught) {
      error.value = errorMessage(caught);
      return false;
    }
  }

  async function load(projectId: number | null) {
    if (projectId === null) {
      groups.value = [];
      return true;
    }
    return run(() => ipc.listTaskGroups(projectId));
  }

  const create = (projectId: number, name: string) =>
    run(() => ipc.createTaskGroup(projectId, name));
  const rename = (groupId: number, name: string) =>
    run(() => ipc.renameTaskGroup(groupId, name));
  const remove = (groupId: number) => run(() => ipc.deleteTaskGroup(groupId));
  const reorder = (projectId: number, orderedIds: number[]) =>
    run(() => ipc.reorderTaskGroups(projectId, orderedIds));

  return { groups, error, byId, nameFor, set, load, create, rename, remove, reorder };
});
