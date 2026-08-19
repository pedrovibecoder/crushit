import { listen } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { errorMessage, ipc } from "../lib/ipc";
import { UNKNOWN_CATEGORY_COLOR, type Category, type CategoryPatch } from "../types";

export const useCategoriesStore = defineStore("categories", () => {
  const categories = ref<Category[]>([]);
  const error = ref<string | null>(null);

  const bySlug = computed(
    () => new Map(categories.value.map((category) => [category.slug, category])),
  );

  /** What a new task is filed under, and where a deleted category's tasks land. */
  const fallback = computed(() => categories.value[0]?.slug ?? "task");

  function labelFor(slug: string) {
    // A slug with no row behind it can only come from a stale render; showing
    // the slug beats showing nothing.
    return bySlug.value.get(slug)?.label ?? slug;
  }

  function colorFor(slug: string) {
    return bySlug.value.get(slug)?.color ?? UNKNOWN_CATEGORY_COLOR;
  }

  function set(next: Category[]) {
    categories.value = next;
  }

  async function run(action: () => Promise<Category[]>) {
    try {
      error.value = null;
      categories.value = await action();
      return true;
    } catch (caught) {
      error.value = errorMessage(caught);
      return false;
    }
  }

  const load = () => run(ipc.listCategories);
  const create = (label: string, color: string) =>
    run(() => ipc.createCategory(label, color));
  const update = (categoryId: number, patch: CategoryPatch) =>
    run(() => ipc.updateCategory(categoryId, patch));
  const remove = (categoryId: number) => run(() => ipc.deleteCategory(categoryId));

  /** Another window may have added or renamed one. */
  async function subscribe() {
    return listen("categories:changed", () => void load());
  }

  return {
    categories,
    error,
    bySlug,
    fallback,
    labelFor,
    colorFor,
    set,
    load,
    create,
    update,
    remove,
    subscribe,
  };
});
