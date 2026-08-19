<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";

const props = withDefaults(
  defineProps<{
    /** How many tasks were finished, keyed by local `YYYY-MM-DD`. */
    counts: Record<string, number>;
    /** The last day shown, as a local date. Defaults to today. */
    until?: Date;
    weeks?: number;
  }>(),
  { weeks: 53 },
);

const emit = defineEmits<{ (event: "pick", day: string): void }>();

const DAY_MS = 86_400_000;
const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/** Local date, not ISO: a task finished at 11pm belongs to that day, not the next. */
function key(date: Date) {
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

/**
 * Columns of seven days, Sunday first, ending on the week that holds `until`.
 * The last column runs past today, and those days are rendered as gaps so the
 * grid keeps its shape without implying a week that has not happened.
 */
const columns = computed(() => {
  const until = props.until ? new Date(props.until) : new Date();
  until.setHours(12, 0, 0, 0);
  const end = new Date(until.getTime() + (6 - until.getDay()) * DAY_MS);
  const start = new Date(end.getTime() - (props.weeks * 7 - 1) * DAY_MS);

  const grid: Array<Array<{ day: string; count: number; future: boolean } | null>> = [];
  for (let week = 0; week < props.weeks; week += 1) {
    const column: Array<{ day: string; count: number; future: boolean } | null> = [];
    for (let weekday = 0; weekday < 7; weekday += 1) {
      const date = new Date(start.getTime() + (week * 7 + weekday) * DAY_MS);
      const day = key(date);
      column.push({
        day,
        count: props.counts[day] ?? 0,
        future: date.getTime() > until.getTime(),
      });
    }
    grid.push(column);
  }
  return grid;
});

/** A month label sits over the first column that week belongs to it. */
const months = computed(() =>
  columns.value.map((column, index) => {
    const first = column[0];
    if (!first) return "";
    const date = new Date(`${first.day}T12:00:00`);
    const previous = columns.value[index - 1]?.[0];
    const previousMonth = previous ? new Date(`${previous.day}T12:00:00`).getMonth() : -1;
    return date.getMonth() === previousMonth ? "" : MONTHS[date.getMonth()];
  }),
);

/**
 * Five steps, scaled to the busiest day rather than to fixed counts: someone
 * finishing two tasks a day should see the same range as someone finishing ten.
 */
const busiest = computed(() =>
  Math.max(1, ...Object.values(props.counts).filter((count) => Number.isFinite(count))),
);

function level(count: number) {
  if (count <= 0) return 0;
  return Math.min(4, Math.ceil((count / busiest.value) * 4));
}

const LEVELS = [
  "bg-line-soft",
  "bg-success/25",
  "bg-success/45",
  "bg-success/70",
  "bg-success",
];

/**
 * A year rarely fits the window. What matters is the recent end of it, so the
 * graph starts scrolled there and a reader goes back through the year rather
 * than having to scroll forward to reach this week.
 */
const scroller = ref<HTMLElement | null>(null);

function showRecent() {
  const element = scroller.value;
  if (element) element.scrollLeft = element.scrollWidth;
}

onMounted(() => void nextTick(showRecent));
watch(() => props.counts, () => void nextTick(showRecent));

function label(cell: { day: string; count: number }) {
  const date = new Date(`${cell.day}T12:00:00`);
  const when = date.toLocaleDateString(undefined, {
    weekday: "short",
    day: "numeric",
    month: "short",
    year: "numeric",
  });
  if (cell.count === 0) return `No tasks finished on ${when}`;
  return `${cell.count} task${cell.count === 1 ? "" : "s"} finished on ${when}`;
}
</script>

<template>
  <!-- The grid scrolls on its own so a narrow window shortens the graph rather
       than pushing the page sideways; the weekday labels stay out of it so they
       are still there once it has been scrolled. -->
  <div class="flex gap-1">
    <!-- Only three weekday labels, as there is room for three. -->
    <div class="flex shrink-0 flex-col gap-[3px] pt-[15px]">
      <span
        v-for="(name, index) in ['', 'Mon', '', 'Wed', '', 'Fri', '']"
        :key="index"
        class="h-[11px] text-[9px] leading-[11px] text-ink-3"
        >{{ name }}</span
      >
    </div>

    <div ref="scroller" class="min-w-0 flex-1 overflow-x-auto pb-1">
      <div class="flex min-w-max gap-[3px]">
        <div v-for="(column, week) in columns" :key="week" class="flex flex-col gap-[3px]">
          <span class="h-3 text-[9px] leading-3 text-ink-3">{{ months[week] }}</span>
          <template v-for="(cell, weekday) in column" :key="weekday">
            <span v-if="!cell || cell.future" class="h-[11px] w-[11px]" />
            <button
              v-else
              type="button"
              class="h-[11px] w-[11px] rounded-[2px] transition-transform hover:scale-125"
              :class="LEVELS[level(cell.count)]"
              :title="label(cell)"
              :aria-label="label(cell)"
              @click="emit('pick', cell.day)"
            />
          </template>
        </div>
      </div>
    </div>
  </div>

  <div class="mt-1.5 flex items-center justify-end gap-1 text-[10px] text-ink-3">
    <span>Less</span>
    <span
      v-for="step in [0, 1, 2, 3, 4]"
      :key="step"
      class="h-[10px] w-[10px] rounded-[2px]"
      :class="LEVELS[step]"
    />
    <span>More</span>
  </div>
</template>
