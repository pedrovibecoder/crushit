<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import AppIcon from "../components/AppIcon.vue";
import PanelHeader from "../components/PanelHeader.vue";
import SelectMenu, { type SelectOption } from "../components/SelectMenu.vue";
import { formatClock, formatSpan } from "../lib/format";
import { errorMessage, ipc } from "../lib/ipc";
import { useAppStore } from "../stores/app";
import { useFocusStore } from "../stores/focus";
import { useTasksStore } from "../stores/tasks";
import { useAgentStore } from "../stores/agent";
import { useCategoriesStore } from "../stores/categories";
import { useGroupsStore } from "../stores/groups";
import { useTagsStore } from "../stores/tags";
import { STORY_POINTS, STATUS_LABELS, type Task, type TaskCategory } from "../types";

const app = useAppStore();
const categories = useCategoriesStore();
const groups = useGroupsStore();
const tags = useTagsStore();
const agents = useAgentStore();

/** Tags are a set: picking one that is already on the task takes it off. */
function toggleTag(task: Task, slug: string) {
  const next = task.tags.includes(slug)
    ? task.tags.filter((other) => other !== slug)
    : [...task.tags, slug];
  return tasks.update(task.id, { tags: next });
}

const groupOptions = computed<SelectOption[]>(() => [
  { value: "", label: "No group" },
  ...groups.groups.map((group) => ({ value: String(group.id), label: group.name })),
]);

const categoryOptions = computed(() =>
  categories.categories.map((category) => ({
    value: category.slug,
    label: category.label,
  })),
);
const tasks = useTasksStore();
const focus = useFocusStore();

const task = computed(() =>
  app.selectedTaskId === null ? undefined : tasks.byId.get(app.selectedTaskId),
);

const taskOrNull = computed(() => task.value ?? null);
const isFocused = computed(() => !!task.value && focus.isFocused(task.value.id));
const blockers = computed(() => (task.value ? tasks.blockedBy(task.value) : []));
const finishedHere = computed(
  () => !!task.value && focus.justFinishedTaskId === task.value.id,
);

/** The clock shows the live session, or the planned duration before one starts. */
const clock = computed(() => {
  if (isFocused.value) return formatClock(focus.snapshot.remainingSeconds);
  return formatClock(app.settings.focusMinutes * 60);
});

/** How much has already gone into this task, across every session. */
const spent = computed(() => {
  const task = taskOrNull.value;
  if (!task || task.focusSessions === 0) return null;
  const sessions = `${task.focusSessions} session${task.focusSessions === 1 ? "" : "s"}`;
  return `${sessions} · ${formatSpan(task.focusSeconds)} on this task`;
});

const sessionNote = computed(() => {
  if (finishedHere.value) return "Session complete — stop to close it out.";
  if (isFocused.value && focus.isPaused) {
    return `Paused · ${formatClock(focus.snapshot.elapsedSeconds)} focused`;
  }
  if (isFocused.value) return `${formatClock(focus.snapshot.elapsedSeconds)} focused so far`;
  return `${app.settings.focusMinutes} minute session`;
});

const title = ref("");
const newCriterion = ref("");
const confirmingDelete = ref(false);
/** Finishing is one click away from undoing an afternoon, so it is asked for. */
const confirmingDone = ref(false);
/**
 * The description is held here rather than read straight off the task. The
 * screen redraws every second while a session runs, and a field bound to the
 * stored value can have half-typed text written back over it; a draft of your
 * own cannot.
 */
const description = ref("");
const editingDescription = ref(false);

/**
 * A task typed in a hurry rarely says when it is finished. The agent proposes
 * the checkable statements that would settle it, and they are shown for review
 * rather than written straight onto the task.
 */
const suggesting = ref(false);
const suggested = ref<string[]>([]);
const rejected = ref<Set<string>>(new Set());
const suggestError = ref<string | null>(null);

const accepted = computed(() =>
  suggested.value.filter((text) => !rejected.value.has(text)),
);

async function suggestCriteria() {
  if (!task.value || suggesting.value) return;
  suggesting.value = true;
  suggestError.value = null;
  suggested.value = [];
  rejected.value = new Set();
  try {
    const proposed = await ipc.suggestCriteria(task.value.id);
    suggested.value = proposed;
    if (!proposed.length) {
      suggestError.value = `${agents.selected === "codex" ? "Codex" : "Claude Code"} had nothing to add.`;
    }
  } catch (caught) {
    suggestError.value = errorMessage(caught);
  } finally {
    suggesting.value = false;
  }
}

function toggleSuggestion(text: string) {
  const next = new Set(rejected.value);
  if (next.has(text)) next.delete(text);
  else next.add(text);
  rejected.value = next;
}

function dismissSuggestions() {
  suggested.value = [];
  rejected.value = new Set();
  suggestError.value = null;
}

async function acceptSuggestions() {
  if (!task.value || !accepted.value.length) return;
  const texts = [...task.value.criteria.map((c) => c.text), ...accepted.value];
  await tasks.update(task.value.id, { criteria: texts });
  dismissSuggestions();
}

watch(
  task,
  (current) => {
    title.value = current?.title ?? "";
    // Only while it is not being typed into: a change arriving from an agent
    // run should show up, but never mid-sentence.
    if (!editingDescription.value) description.value = current?.description ?? "";
    confirmingDelete.value = false;
    confirmingDone.value = false;
    dismissSuggestions();
  },
  { immediate: true },
);

async function commitDescription() {
  editingDescription.value = false;
  const value = description.value.trim();
  if (!task.value || value === (task.value.description ?? "")) return;
  await tasks.update(task.value.id, { description: value });
}

async function commitTitle() {
  const trimmed = title.value.trim();
  if (!task.value || !trimmed || trimmed === task.value.title) {
    title.value = task.value?.title ?? "";
    return;
  }
  await tasks.update(task.value.id, { title: trimmed });
}

async function addCriterion() {
  const trimmed = newCriterion.value.trim();
  if (!task.value || !trimmed) return;
  const texts = [...task.value.criteria.map((c) => c.text), trimmed];
  await tasks.update(task.value.id, { criteria: texts });
  newCriterion.value = "";
}

async function removeCriterion(index: number) {
  if (!task.value) return;
  const texts = task.value.criteria
    .map((c) => c.text)
    .filter((_, position) => position !== index);
  await tasks.update(task.value.id, { criteria: texts });
}

async function toggleDone() {
  if (!task.value) return;
  // Reopening something is harmless and undoes itself; finishing it stops a
  // session and takes you off the screen, so only that is confirmed.
  if (task.value.status !== "completed" && !confirmingDone.value) {
    confirmingDone.value = true;
    return;
  }
  confirmingDone.value = false;
  if (isFocused.value) await focus.stop();
  await tasks.toggleComplete(task.value);
  app.back();
}

async function destroy() {
  const doomed = task.value;
  if (!doomed) return;
  confirmingDelete.value = false;
  // Back to the list first, then delete: the row is still on screen, so it can
  // be seen leaving instead of having vanished behind a screen change.
  app.back();
  await nextTick();
  await tasks.remove(doomed.id);
}
</script>

<template>
  <div v-if="!task" class="px-3.5 py-6 text-center">
    <p class="text-[12px] text-ink-2">That task is gone.</p>
    <button class="btn btn-ghost mt-3 w-full py-2" @click="app.back()">Back to Today</button>
  </div>

  <div v-else class="flex flex-col">
    <PanelHeader :eyebrow="STATUS_LABELS[task.status]" :title="task.title" back @back="app.back()">
      <template #title>
        <input
          v-model="title"
          class="-mx-1 w-full rounded-[6px] border border-transparent bg-transparent px-1 text-[19px] leading-tight font-bold tracking-[-0.015em] outline-none hover:border-line focus:border-ink-3"
          aria-label="Task title"
          @blur="commitTitle"
          @keydown.enter.prevent="($event.target as HTMLInputElement).blur()"
        />
      </template>
    </PanelHeader>

    <div class="panel-scroll px-3.5 pb-3">
      <!-- Timer -->
      <div class="card px-3 py-3">
        <div class="flex items-center gap-3">
          <div class="min-w-0 flex-1">
            <p class="tnum text-[32px] leading-none font-bold tracking-[-0.03em]">
              {{ clock }}
            </p>
            <p class="mt-1.5 truncate text-[11px] text-ink-2">{{ sessionNote }}</p>
            <p v-if="spent" class="mt-0.5 truncate text-[11px] text-ink-3">{{ spent }}</p>
          </div>
          <button
            v-if="!isFocused"
            class="btn btn-dark shrink-0 px-3.5 py-2"
            @click="focus.start(task.id)"
          >
            <AppIcon name="play" :size="12" filled />Start
          </button>
          <template v-else>
            <button
              class="icon-btn h-8 w-8 shrink-0"
              :aria-label="focus.isRunning ? 'Pause' : 'Resume'"
              :title="focus.isRunning ? 'Pause' : 'Resume'"
              @click="focus.isRunning ? focus.pause() : focus.resume()"
            >
              <AppIcon :name="focus.isRunning ? 'pause' : 'play'" :size="13" filled />
            </button>
            <button
              class="icon-btn h-8 w-8 shrink-0 text-danger"
              aria-label="Stop session"
              title="Stop session"
              @click="focus.stop()"
            >
              <AppIcon name="stop" :size="13" filled />
            </button>
          </template>
        </div>
      </div>

      <p
        v-if="blockers.length"
        class="mt-2 rounded-[10px] border border-warn/25 bg-warn-soft px-2.5 py-1.5 text-[11px] text-warn"
      >
        Blocked by {{ blockers.map((other) => other.title).join(", ") }}
      </p>

      <!-- Acceptance criteria -->
      <section class="mt-4">
        <div class="flex items-baseline justify-between">
          <h2 class="eyebrow">Acceptance criteria</h2>
          <span v-if="task.criteria.length" class="tnum text-[11px] text-ink-3">
            {{ task.criteria.filter((c) => c.isMet).length }}/{{ task.criteria.length }}
          </span>
        </div>
        <ul class="mt-1.5 space-y-1.5">
          <li
            v-for="(criterion, index) in task.criteria"
            :key="criterion.id"
            class="card group flex items-start gap-2.5 px-2.5 py-2"
          >
            <button
              class="mt-px flex h-[17px] w-[17px] shrink-0 items-center justify-center rounded-[6px] border transition-colors"
              :class="
                criterion.isMet
                  ? 'border-success bg-success text-white'
                  : 'border-line hover:border-ink-3'
              "
              :aria-label="criterion.isMet ? 'Mark unmet' : 'Mark met'"
              @click="tasks.setCriterion(criterion.id, !criterion.isMet)"
            >
              <AppIcon v-if="criterion.isMet" name="check" :size="10" :weight="2.4" />
            </button>
            <span
              class="min-w-0 flex-1 text-[12.5px] leading-snug font-medium"
              :class="criterion.isMet && 'text-ink-3 line-through'"
            >
              {{ criterion.text }}
            </span>
            <button
              class="shrink-0 text-ink-3 opacity-0 transition-opacity group-hover:opacity-100 hover:text-danger"
              aria-label="Remove criterion"
              @click="removeCriterion(index)"
            >
              <AppIcon name="close" :size="12" />
            </button>
          </li>
        </ul>
        <input
          v-model="newCriterion"
          type="text"
          placeholder="Add a criterion…"
          class="field mt-1.5 w-full px-2.5 py-2 text-[12.5px]"
          @keydown.enter.prevent="addCriterion"
        />

        <!-- What the agent proposes, before any of it is written down. -->
        <div v-if="suggested.length" class="card mt-1.5 border-accent/35 px-2.5 py-2">
          <p class="eyebrow text-accent">Suggested acceptance criteria</p>
          <ul class="mt-1.5 space-y-1">
            <li v-for="text in suggested" :key="text">
              <button
                class="flex w-full items-start gap-2 text-left"
                :aria-pressed="!rejected.has(text)"
                @click="toggleSuggestion(text)"
              >
                <span
                  class="mt-px flex h-[15px] w-[15px] shrink-0 items-center justify-center rounded-[5px] border transition-colors"
                  :class="
                    rejected.has(text)
                      ? 'border-line'
                      : 'border-accent bg-accent text-white'
                  "
                >
                  <AppIcon v-if="!rejected.has(text)" name="check" :size="9" :weight="2.6" />
                </span>
                <span
                  class="min-w-0 flex-1 text-[12px] leading-snug"
                  :class="rejected.has(text) ? 'text-ink-3 line-through' : 'text-ink-2'"
                >
                  {{ text }}
                </span>
              </button>
            </li>
          </ul>
          <div class="mt-2 flex gap-1.5">
            <button class="btn btn-ghost px-2.5 py-1.5 text-[11.5px]" @click="dismissSuggestions">
              Discard
            </button>
            <button
              class="btn btn-dark flex-1 py-1.5 text-[11.5px]"
              :disabled="!accepted.length"
              @click="acceptSuggestions"
            >
              Add {{ accepted.length }}
            </button>
          </div>
        </div>

        <button
          v-else-if="agents.isReady"
          class="btn btn-ghost mt-1.5 w-full py-1.5 text-[11.5px] text-ink-2"
          :disabled="suggesting"
          @click="suggestCriteria"
        >
          <AppIcon name="sparkle" :size="11" filled />
          {{ suggesting ? "Reading the project…" : "Generate acceptance criteria" }}
        </button>

        <p v-if="suggestError" class="mt-1.5 text-[11px] text-danger">{{ suggestError }}</p>
      </section>

      <!-- Details -->
      <section class="mt-4">
        <h2 class="eyebrow">Details</h2>
        <textarea
          v-model="description"
          rows="8"
          placeholder="What needs to change?"
          class="field mt-1.5 w-full resize-none px-2.5 py-2 text-[12.5px] leading-snug"
          @focus="editingDescription = true"
          @blur="commitDescription"
        />
        <div class="mt-1.5 flex items-center gap-1.5">
          <div class="min-w-0 flex-1">
            <SelectMenu
              :model-value="task.category"
              :options="categoryOptions"
              label="Category"
              @update:model-value="tasks.update(task.id, { category: $event as TaskCategory })"
            />
          </div>
          <input
            :value="task.estimateMinutes ?? ''"
            type="number"
            min="1"
            placeholder="min"
            aria-label="Estimate in minutes"
            class="field tnum w-16 px-2 py-1.5 text-[11.5px]"
            @change="tasks.update(task.id, { estimateMinutes: Number(($event.target as HTMLInputElement).value) })"
          />
        </div>
        <!-- Several at once, unlike the category: a task can be front-end
             work and devops work at the same time. -->
        <div v-if="tags.tags.length" class="mt-1.5 flex flex-wrap items-center gap-1">
          <button
            v-for="tag in tags.tags"
            :key="tag.id"
            class="chip font-semibold transition-colors"
            :style="
              task.tags.includes(tag.slug)
                ? {
                    color: tag.color,
                    borderColor: `color-mix(in oklab, ${tag.color} 45%, transparent)`,
                    backgroundColor: `color-mix(in oklab, ${tag.color} 14%, transparent)`,
                  }
                : {}
            "
            :class="!task.tags.includes(tag.slug) && 'text-ink-3 hover:bg-line-soft'"
            :aria-pressed="task.tags.includes(tag.slug)"
            @click="toggleTag(task, tag.slug)"
          >
            {{ tag.label }}
          </button>
        </div>

        <!-- Which bucket the task waits in. Its day is set where days are
             chosen — on Today, or on the plan screen. -->
        <div class="mt-1.5">
          <SelectMenu
            :model-value="task.groupId === null ? '' : String(task.groupId)"
            :options="groupOptions"
            label="Group"
            @update:model-value="
              tasks.update(task.id, { groupId: $event === '' ? null : Number($event) })
            "
          />
        </div>
        <div class="mt-1.5 flex items-center gap-1.5">
          <span class="eyebrow shrink-0">Points</span>
          <div class="flex flex-1 items-center gap-1">
            <button
              v-for="points in STORY_POINTS"
              :key="points"
              class="tnum flex-1 rounded-[8px] border py-1 text-[11px] font-semibold transition-colors"
              :class="
                task.storyPoints === points
                  ? 'border-solid bg-solid text-on-solid'
                  : 'border-line bg-card text-ink-2 hover:bg-line-soft'
              "
              :aria-pressed="task.storyPoints === points"
              @click="
                tasks.update(task.id, {
                  storyPoints: task.storyPoints === points ? 0 : points,
                })
              "
            >
              {{ points }}
            </button>
          </div>
        </div>

        <ul v-if="task.files.length" class="mt-1.5 space-y-1">
          <li
            v-for="file in task.files"
            :key="file"
            class="truncate rounded-[8px] bg-line-soft px-2 py-1 text-[11px] text-ink-2"
          >
            {{ file }}
          </li>
        </ul>
      </section>

    </div>

    <footer class="flex items-center gap-2 border-t border-line px-3.5 py-2.5">
      <!-- Deleting takes the criteria and the recorded time with it, so it is
           spelled out rather than hidden behind a second click on an icon. -->
      <template v-if="confirmingDelete">
        <span class="min-w-0 flex-1 text-[11.5px] leading-snug text-ink-2">
          Delete this task{{ task.criteria.length ? " and its criteria" : "" }}?
        </span>
        <button class="btn btn-ghost shrink-0 px-3 py-2" @click="confirmingDelete = false">
          Cancel
        </button>
        <button
          class="btn shrink-0 border border-danger/40 bg-danger-soft px-3 py-2 text-danger"
          @click="destroy"
        >
          <AppIcon name="trash" :size="13" />
          Delete
        </button>
      </template>

      <template v-else>
        <button
          v-if="confirmingDone"
          class="btn btn-ghost shrink-0 px-3 py-2"
          @click="confirmingDone = false"
        >
          Cancel
        </button>
        <button class="btn btn-dark flex-1 py-2" @click="toggleDone">
          <AppIcon name="check" :size="13" />
          {{
            task.status === "completed"
              ? "Reopen"
              : confirmingDone
                ? "Yes, it's done"
                : "Mark complete"
          }}
        </button>
        <button
          class="icon-btn h-[34px] w-[34px] shrink-0 text-ink-2 hover:text-danger"
          aria-label="Delete task"
          title="Delete task"
          @click="confirmingDelete = true; confirmingDone = false"
        >
          <AppIcon name="trash" :size="14" />
        </button>
      </template>
    </footer>
  </div>
</template>
