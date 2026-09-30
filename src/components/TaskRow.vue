<script setup lang="ts">
import { computed } from "vue";
import { formatClock, formatDuration } from "../lib/format";
import type { Task } from "../types";
import AppIcon from "./AppIcon.vue";
import CategoryPill from "./CategoryPill.vue";
import TagChip from "./TagChip.vue";

const props = defineProps<{
  task: Task;
  focused: boolean;
  remainingSeconds: number;
  blockedBy: Task[];
  /** True while this row is being deleted, so it can play its way out. */
  deleting?: boolean;
  /** True when the task was planned for an earlier day and is still open. */
  carriedOver?: boolean;
  /** While the list is being picked over, a row selects instead of opening. */
  selecting?: boolean;
  selected?: boolean;
}>();

const emit = defineEmits<{
  (event: "open"): void;
  (event: "toggle"): void;
  (event: "select"): void;
}>();

/** One click, two meanings, depending on what the list is being used for. */
function press() {
  if (props.selecting) emit("select");
  else emit("open");
}

const isDone = computed(() => props.task.status === "completed");
const metCriteria = computed(
  () => props.task.criteria.filter((criterion) => criterion.isMet).length,
);

/** The right-hand slot: live clock when focused, otherwise the estimate. */
const trailing = computed(() => {
  if (props.focused) return formatClock(props.remainingSeconds);
  if (props.task.estimateMinutes) return formatDuration(props.task.estimateMinutes);
  return "";
});
</script>

<template>
  <div
    class="card flex cursor-pointer items-start gap-2.5 px-2.5 py-2.5 transition-colors"
    :class="[
      selected
        ? 'border-accent/60 bg-accent-soft/60'
        : focused
          ? 'border-accent/45 bg-accent-soft/50'
          : 'hover:bg-line-soft/70',
      deleting && 'row-deleting',
    ]"
    role="button"
    tabindex="0"
    :aria-pressed="selecting ? selected : undefined"
    @click="press"
    @keydown.enter.prevent="press"
    @keydown.space.prevent="press"
  >
    <!-- The same corner does both jobs: finishing a task, or picking it out.
         Round while selecting, so it does not read as "mark complete". -->
    <button
      class="mt-[1px] flex h-[18px] w-[18px] shrink-0 items-center justify-center border transition-colors"
      :class="[
        selecting ? 'rounded-full' : 'rounded-[6px]',
        selecting
          ? selected
            ? 'border-accent bg-accent text-white'
            : 'border-line bg-card hover:border-accent'
          : isDone
            ? 'border-success bg-success text-white'
            : 'border-line bg-card hover:border-ink-3',
      ]"
      :title="selecting ? (selected ? 'Leave it out' : 'Pick it out') : isDone ? 'Mark as not done' : 'Mark complete'"
      :aria-label="selecting ? (selected ? 'Leave it out' : 'Pick it out') : isDone ? 'Mark as not done' : 'Mark complete'"
      @click.stop="selecting ? emit('select') : emit('toggle')"
    >
      <AppIcon v-if="selecting ? selected : isDone" name="check" :size="11" :weight="2.4" />
    </button>

    <div class="min-w-0 flex-1">
      <p
        class="line-clamp-2 text-[13px] leading-snug font-semibold break-words"
        :class="isDone && 'text-ink-3 line-through'"
      >
        {{ task.title }}
      </p>
      <div class="mt-1.5 flex flex-wrap items-center gap-1">
        <CategoryPill :category="task.category" />
        <TagChip v-for="slug in task.tags" :key="slug" :slug="slug" />
        <span v-if="task.storyPoints" class="chip tnum text-ink-2" title="Story points">
          {{ task.storyPoints }} SP
        </span>
        <span v-if="task.criteria.length" class="chip tnum text-ink-2">
          {{ metCriteria }}/{{ task.criteria.length }}
        </span>
        <span v-if="task.isAiGenerated" class="chip border-accent/35 bg-accent-soft text-accent">
          <AppIcon name="sparkle" :size="9" filled />AI
        </span>
        <span v-if="blockedBy.length" class="chip border-warn/30 bg-warn-soft text-warn">
          Blocked
        </span>
        <span v-if="carriedOver" class="chip text-ink-3" title="Planned for an earlier day">
          Carried over
        </span>
      </div>
    </div>

    <span
      v-if="trailing"
      class="chip tnum mt-[1px] shrink-0"
      :class="focused ? 'border-accent/40 bg-card text-accent' : 'text-ink-2'"
    >
      {{ trailing }}
    </span>
  </div>
</template>
