<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import AppIcon from "./AppIcon.vue";
import { localDay, shiftDay } from "../lib/format";

/** What a day looks like from the list's point of view: how much, how far. */
export interface DayMark {
  total: number;
  done: number;
}

const props = withDefaults(
  defineProps<{
    /** The day being read, as `YYYY-MM-DD`. */
    modelValue: string;
    /** Which day counts as today, so the grid can point at it. */
    today?: string;
    /** Work per day, keyed by `YYYY-MM-DD`, used for the dots under a date. */
    marks?: Record<string, DayMark>;
  }>(),
  { marks: () => ({}) },
);

const emit = defineEmits<{
  (event: "update:modelValue", day: string): void;
  (event: "close"): void;
}>();

const WEEKDAYS = ["S", "M", "T", "W", "T", "F", "S"];

const today = computed(() => props.today ?? localDay());

/**
 * Which month the grid is showing. It follows the selection when that jumps
 * somewhere else, but paging through months on its own does not move the day:
 * looking ahead is not the same as going there.
 */
const cursor = ref(props.modelValue.slice(0, 7));
watch(
  () => props.modelValue,
  (day) => { cursor.value = day.slice(0, 7) },
);

/** Noon, so a shift across a daylight-saving boundary stays on its date. */
function at(month: string, day = 1) {
  return new Date(`${month}-${String(day).padStart(2, "0")}T12:00:00`);
}

const monthName = computed(() =>
  at(cursor.value).toLocaleDateString(undefined, { month: "long", year: "numeric" }),
);

function goMonth(by: number) {
  const date = at(cursor.value);
  date.setMonth(date.getMonth() + by);
  cursor.value = localDay(date).slice(0, 7);
}

interface Cell {
  day: string;
  date: number;
  mark: DayMark | undefined;
  future: boolean;
}

/**
 * The month laid out Sunday first, with the leading gap kept as blanks so the
 * first of the month lands under the weekday it actually falls on.
 */
const grid = computed(() => {
  const first = at(cursor.value);
  const lead = first.getDay();
  const days = new Date(first.getFullYear(), first.getMonth() + 1, 0).getDate();
  const cells: Array<Cell | null> = Array.from({ length: lead }, () => null);
  for (let date = 1; date <= days; date += 1) {
    const day = localDay(at(cursor.value, date));
    cells.push({
      day,
      date,
      mark: props.marks[day],
      future: day > today.value,
    });
  }
  return cells;
});

/** Nothing planned reads as an empty day; part-done and done read apart. */
function tone(mark: DayMark | undefined) {
  if (!mark || mark.total === 0) return "";
  return mark.done === mark.total ? "bg-success" : "bg-accent";
}

function pick(day: string) {
  emit("update:modelValue", day);
  emit("close");
}

/**
 * Arrows walk the calendar a day or a week at a time, which is how a date is
 * usually found once the grid is open. The day moves with the cursor rather
 * than waiting for a click, so the list behind updates as you go.
 */
const root = ref<HTMLElement | null>(null);

function onKey(event: KeyboardEvent) {
  const steps: Record<string, number> = {
    ArrowLeft: -1,
    ArrowRight: 1,
    ArrowUp: -7,
    ArrowDown: 7,
  };
  const by = steps[event.key];
  if (by === undefined) return;
  event.preventDefault();
  emit("update:modelValue", shiftDay(props.modelValue, by));
}

/** Opened by a click on the date, so the grid takes focus to hear the arrows. */
onMounted(() => root.value?.focus());
</script>

<template>
  <div
    ref="root"
    class="card p-2 outline-none"
    tabindex="-1"
    role="dialog"
    aria-label="Pick a day"
    @keydown.esc.stop="emit('close')"
    @keydown="onKey"
  >
    <div class="flex items-center gap-1 px-0.5 pb-1.5">
      <button
        class="icon-btn h-6 w-6 shrink-0"
        aria-label="Previous month"
        title="Previous month"
        @click="goMonth(-1)"
      >
        <AppIcon name="back" :size="12" />
      </button>
      <p class="min-w-0 flex-1 truncate text-center text-[11.5px] font-semibold">
        {{ monthName }}
      </p>
      <button
        class="icon-btn h-6 w-6 shrink-0"
        aria-label="Next month"
        title="Next month"
        @click="goMonth(1)"
      >
        <AppIcon name="forward" :size="12" />
      </button>
    </div>

    <div class="grid grid-cols-7 gap-0.5">
      <span
        v-for="(weekday, index) in WEEKDAYS"
        :key="index"
        class="pb-0.5 text-center text-[9.5px] font-semibold tracking-[0.04em] text-ink-3 uppercase"
      >
        {{ weekday }}
      </span>

      <template v-for="(cell, index) in grid">
        <span v-if="!cell" :key="`gap-${index}`" />
        <button
          v-else
          :key="cell.day"
          type="button"
          class="relative flex h-7 flex-col items-center justify-center rounded-[7px] text-[11.5px] font-medium transition-colors"
          :class="[
            cell.day === modelValue
              ? 'bg-solid text-on-solid'
              : 'hover:bg-line-soft',
            cell.day !== modelValue && cell.day === today && 'text-accent font-bold',
            cell.day !== modelValue && cell.future && 'text-ink-3',
          ]"
          :aria-label="cell.day"
          :aria-current="cell.day === today ? 'date' : undefined"
          :title="cell.mark ? `${cell.mark.done}/${cell.mark.total} done` : undefined"
          @click="pick(cell.day)"
        >
          <span class="tnum leading-none">{{ cell.date }}</span>
          <!-- A dot, not a count: the grid says which days have work on them,
               and the list itself says what that work is. -->
          <span
            class="mt-0.5 h-1 w-1 rounded-full"
            :class="cell.day === modelValue ? (cell.mark?.total ? 'bg-on-solid/70' : '') : tone(cell.mark)"
          />
        </button>
      </template>
    </div>

    <div class="mt-1.5 flex items-center gap-1.5 border-t border-line-soft pt-1.5">
      <button
        class="flex-1 rounded-[8px] py-1 text-[11px] font-semibold text-accent transition-colors hover:bg-line-soft"
        @click="pick(today)"
      >
        Today
      </button>
      <button
        class="flex-1 rounded-[8px] py-1 text-[11px] font-semibold text-ink-2 transition-colors hover:bg-line-soft"
        @click="emit('close')"
      >
        Close
      </button>
    </div>
  </div>
</template>
