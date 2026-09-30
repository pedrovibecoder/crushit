import { listen } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { errorMessage, ipc } from "../lib/ipc";
import { UNKNOWN_CATEGORY_COLOR, type Tag, type TagPatch } from "../types";

/**
 * The other axis a task can be read along: which part of the work it touches.
 * Unlike a category, a task can carry several of these or none at all, so
 * there is nothing to fall back to when one is deleted.
 */
export const useTagsStore = defineStore("tags", () => {
  const tags = ref<Tag[]>([]);
  const error = ref<string | null>(null);

  const bySlug = computed(() => new Map(tags.value.map((tag) => [tag.slug, tag])));

  function labelFor(slug: string) {
    // A slug with no row behind it can only come from a stale render; showing
    // the slug beats showing nothing.
    return bySlug.value.get(slug)?.label ?? slug;
  }

  function colorFor(slug: string) {
    return bySlug.value.get(slug)?.color ?? UNKNOWN_CATEGORY_COLOR;
  }

  function set(next: Tag[]) {
    tags.value = next;
  }

  async function run(action: () => Promise<Tag[]>) {
    try {
      error.value = null;
      tags.value = await action();
      return true;
    } catch (caught) {
      error.value = errorMessage(caught);
      return false;
    }
  }

  const load = () => run(ipc.listTags);
  const create = (label: string, color: string) => run(() => ipc.createTag(label, color));
  const update = (tagId: number, patch: TagPatch) => run(() => ipc.updateTag(tagId, patch));
  const remove = (tagId: number) => run(() => ipc.deleteTag(tagId));
  const reorder = (orderedIds: number[]) => run(() => ipc.reorderTags(orderedIds));

  /** Another window may have added, renamed or removed one. */
  async function subscribe() {
    return listen("tags:changed", () => void load());
  }

  return {
    tags,
    error,
    bySlug,
    labelFor,
    colorFor,
    set,
    load,
    create,
    update,
    remove,
    reorder,
    subscribe,
  };
});
