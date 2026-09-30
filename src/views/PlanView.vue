<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import AppIcon from "../components/AppIcon.vue";
import CategoryPill from "../components/CategoryPill.vue";
import DayPicker from "../components/DayPicker.vue";
import TagChip from "../components/TagChip.vue";
import TagFilter from "../components/TagFilter.vue";
import PanelHeader from "../components/PanelHeader.vue";
import SelectMenu, { type SelectOption } from "../components/SelectMenu.vue";
import { byTitle, dayName, localDay } from "../lib/format";
import { useAppStore } from "../stores/app";
import { useGroupsStore } from "../stores/groups";
import { useTasksStore } from "../stores/tasks";
import type { Task } from "../types";

const app = useAppStore();
const tasks = useTasksStore();
const groups = useGroupsStore();

const today = computed(() => localDay());

const projectId = computed(() => app.activeProject?.id ?? null);
onMounted(() => groups.load(projectId.value));
watch(projectId, (id) => groups.load(id));

/**
 * What this screen is responsible for: work that is not happening today.
 * A task belongs here while it has no day at all, once it is planned for a
 * later day, or once it has been filed under a group. Finished work belongs
 * to History, not to a plan.
 */
/** One tag at a time, or none: see `TagFilter`. */
const tagFilter = ref<string | null>(null);

const planned = computed(() =>
  tasks.tasks.filter(
    (task) =>
      task.status !== "completed" &&
      (tagFilter.value === null || task.tags.includes(tagFilter.value)) &&
      (task.plannedFor === null || task.plannedFor > today.value || task.groupId !== null),
  ),
);

interface Bucket {
  /** `null` is the bucket for work that has not been filed under anything. */
  id: number | null;
  name: string;
  tasks: Task[];
  points: number;
}

/**
 * Every group is shown even when it is empty — an empty group is a bucket you
 * have just made and are about to fill — while the loose pile only appears
 * when something is actually in it.
 */
const buckets = computed<Bucket[]>(() => {
  const out: Bucket[] = groups.groups.map((group) => ({
    id: group.id,
    name: group.name,
    tasks: [],
    points: 0,
  }));
  out.push({ id: null, name: "Not in a group", tasks: [], points: 0 });

  const byId = new Map(out.map((bucket) => [bucket.id, bucket]));
  const loose = byId.get(null)!;
  for (const task of planned.value) {
    // A task whose group has since been deleted falls back to the loose pile
    // rather than vanishing from the screen.
    (byId.get(task.groupId) ?? loose).tasks.push(task);
  }
  for (const bucket of out) {
    // Scheduled work first, soonest first, and the undated pool after it:
    // what has a day is nearer to happening than what does not. Within a day,
    // by title, so a project's tasks stay together.
    bucket.tasks.sort(
      (a, b) =>
        Number(a.plannedFor === null) - Number(b.plannedFor === null) ||
        (a.plannedFor ?? "").localeCompare(b.plannedFor ?? "") ||
        byTitle(a, b),
    );
    bucket.points = bucket.tasks.reduce((sum, task) => sum + (task.storyPoints ?? 0), 0);
  }
  return out.filter((bucket) => bucket.id !== null || bucket.tasks.length > 0);
});

const isEmpty = computed(() => planned.value.length === 0 && groups.groups.length === 0);

/** Collapsed by group id, so a long group can be folded out of the way. */
const collapsed = ref(new Set<number | null>());
function toggle(id: number | null) {
  const next = new Set(collapsed.value);
  if (!next.delete(id)) next.add(id);
  collapsed.value = next;
}

// ------------------------------------------------------------- adding work

/**
 * A task planned here starts with no day: the point of the plan is to write
 * work down before deciding when to do it. A day can be chosen now, and it can
 * just as well be chosen weeks later from the row itself.
 */
const draftTitle = ref("");
const draftDay = ref<string | null>(null);
const draftGroup = ref("");
const draftDayOpen = ref(false);
const adding = ref(false);

const groupOptions = computed<SelectOption[]>(() => [
  { value: "", label: "No group" },
  ...groups.groups.map((group) => ({ value: String(group.id), label: group.name })),
]);

async function add() {
  const title = draftTitle.value.trim();
  if (!title || !app.activeProject || adding.value) return;
  adding.value = true;
  try {
    await tasks.create({
      projectId: app.activeProject.id,
      title,
      plannedFor: draftDay.value,
      groupId: draftGroup.value === "" ? null : Number(draftGroup.value),
    });
    // The day and the group stay put: adding three tasks to one group on one
    // day is the common case, and retyping them each time is the annoying one.
    draftTitle.value = "";
    draftDayOpen.value = false;
  } finally {
    adding.value = false;
  }
}

// ----------------------------------------------------------------- groups

const newGroupName = ref("");
const namingGroup = ref(false);
/**
 * Undefined rather than null while nothing is being renamed: `null` is the id
 * of the bucket for ungrouped work, and comparing the two would put a rename
 * box where that bucket's name should be.
 */
const renamingId = ref<number | undefined>();
const renameDraft = ref("");

async function createGroup() {
  const name = newGroupName.value.trim();
  if (!name || !app.activeProject) return;
  await groups.create(app.activeProject.id, name);
  newGroupName.value = "";
  namingGroup.value = false;
}

function startRename(id: number, name: string) {
  renamingId.value = id;
  renameDraft.value = name;
}

async function commitRename() {
  const id = renamingId.value;
  const name = renameDraft.value.trim();
  renamingId.value = undefined;
  if (id === undefined || !name) return;
  await groups.rename(id, name);
}

/** The work inside comes out of the group rather than going with it. */
async function removeGroup(id: number) {
  await groups.remove(id);
  await tasks.load(projectId.value);
}

// ------------------------------------------------------------ rescheduling

/** Which task's calendar is open, so only one hangs below the list at a time. */
const reschedulingId = ref<number | null>(null);

async function reschedule(task: Task, day: string | null) {
  reschedulingId.value = null;
  if (day === task.plannedFor) return;
  await tasks.update(task.id, { plannedFor: day });
}

/** `Tomorrow`, `Mon 25 Aug` — or the invitation to give the work a day. */
const dayLabel = (day: string | null) => (day === null ? "No date" : dayName(day, today.value));
</script>

<template>
  <div class="flex flex-col">
    <PanelHeader
      :eyebrow="app.activeProject?.name"
      title="Plan"
      :back="!app.isDesktop"
      accent
      @back="app.back()"
    />

    <div class="panel-scroll px-3.5 pb-3.5">
      <!-- Adding is the first thing on the screen: the plan is a place you put
           work into more often than one you read. -->
      <div class="card px-2.5 py-2.5">
        <input
          v-model="draftTitle"
          placeholder="Plan a task…"
          aria-label="New task title"
          class="field w-full px-2.5 py-2 text-[12.5px]"
          @keydown.enter="add"
        />
        <div class="mt-1.5 flex items-center gap-1.5">
          <button
            class="field flex shrink-0 items-center gap-1.5 px-2 py-1.5 text-[11.5px] font-medium"
            :class="[draftDayOpen && 'bg-line-soft', draftDay === null && 'text-ink-3']"
            title="Give it a day now, or leave it for later"
            @pointerdown.stop
          @click="draftDayOpen = !draftDayOpen"
          >
            <AppIcon name="calendar" :size="12" class="text-ink-3" />
            {{ dayLabel(draftDay) }}
          </button>
          <div class="min-w-0 flex-1">
            <SelectMenu v-model="draftGroup" :options="groupOptions" label="Group" />
          </div>
          <button
            class="btn btn-dark shrink-0 px-3 py-1.5 text-[11.5px]"
            :disabled="!draftTitle.trim() || adding"
            @click="add"
          >
            Add
          </button>
        </div>
        <DayPicker
          v-if="draftDayOpen"
          v-model="draftDay"
          class="mt-1.5"
          :today="today"
          :marks="tasks.dayMarks"
          clearable
          @clear="draftDay = null"
          @close="draftDayOpen = false"
        />
      </div>

      <TagFilter v-model="tagFilter" class="mt-2.5" />

      <p v-if="isEmpty" class="mt-4 text-center text-[11.5px] leading-relaxed text-ink-2">
        Nothing planned yet.<br />
        Work added here waits without a day until you give it one, and a group
        keeps related work together in the meantime.
      </p>

      <section v-for="bucket in buckets" :key="bucket.id ?? 'loose'" class="mt-3">
        <div class="flex items-center gap-1.5">
          <button
            class="icon-btn h-6 w-6 shrink-0 border-0 bg-transparent text-ink-3"
            :aria-label="collapsed.has(bucket.id) ? `Expand ${bucket.name}` : `Collapse ${bucket.name}`"
            :aria-expanded="!collapsed.has(bucket.id)"
            @click="toggle(bucket.id)"
          >
            <AppIcon
              name="chevron"
              :size="11"
              :weight="2.2"
              class="transition-transform"
              :class="collapsed.has(bucket.id) && '-rotate-90'"
            />
          </button>

          <!-- Renaming happens in place, on the name itself: a group is only a
               name, so a screen of its own would be a screen with one field. -->
          <input
            v-if="renamingId === bucket.id"
            v-model="renameDraft"
            class="field min-w-0 flex-1 px-1.5 py-0.5 text-[12px] font-semibold"
            :aria-label="`Rename ${bucket.name}`"
            @keydown.enter="commitRename"
            @keydown.esc="renamingId = undefined"
            @blur="commitRename"
          />
          <button
            v-else-if="bucket.id !== null"
            class="min-w-0 flex-1 truncate rounded-[7px] px-1 py-0.5 text-left text-[12px] font-semibold transition-colors hover:bg-line-soft"
            :title="`Rename ${bucket.name}`"
            @click="startRename(bucket.id, bucket.name)"
          >
            {{ bucket.name }}
          </button>
          <span v-else class="min-w-0 flex-1 truncate px-1 text-[12px] font-semibold text-ink-2">
            {{ bucket.name }}
          </span>

          <span class="tnum shrink-0 text-[10.5px] font-medium text-ink-3">
            {{ bucket.tasks.length }}
            <template v-if="bucket.points"> · {{ bucket.points }} SP</template>
          </span>

          <button
            v-if="bucket.id !== null"
            class="icon-btn h-6 w-6 shrink-0 text-ink-3 hover:text-danger"
            :aria-label="`Delete ${bucket.name}`"
            title="Delete the group — the tasks in it stay, without a group"
            @click="removeGroup(bucket.id)"
          >
            <AppIcon name="trash" :size="11" />
          </button>
        </div>

        <ul v-if="!collapsed.has(bucket.id)" class="mt-1.5 space-y-1.5">
          <li v-for="task in bucket.tasks" :key="task.id">
            <div class="card flex items-center gap-2 px-2.5 py-2">
              <button
                class="min-w-0 flex-1 text-left"
                @click="app.openTask(task.id)"
              >
                <span class="block truncate text-[12.5px] font-semibold">{{ task.title }}</span>
                <span class="mt-0.5 flex items-center gap-1.5">
                  <CategoryPill :category="task.category" />
                  <TagChip v-for="slug in task.tags" :key="slug" :slug="slug" />
                  <span v-if="task.storyPoints" class="chip tnum text-ink-2">
                    {{ task.storyPoints }} SP
                  </span>
                </span>
              </button>
              <!-- The day is the one thing you change from here; everything
                   else about a task is edited on the task's own screen. -->
              <button
                class="chip shrink-0 font-semibold"
                :class="
                  reschedulingId === task.id
                    ? 'border-accent/45 bg-accent-soft text-accent'
                    : task.plannedFor === null
                      ? 'border-dashed text-ink-3 hover:bg-line-soft'
                      : task.plannedFor < today
                        ? 'border-warn/30 bg-warn-soft text-warn hover:bg-warn-soft/70'
                        : 'text-ink-2 hover:bg-line-soft'
                "
                :title="
                  task.plannedFor === null
                    ? 'Give this a day'
                    : `Planned for ${task.plannedFor}`
                "
                @pointerdown.stop
                @click="reschedulingId = reschedulingId === task.id ? null : task.id"
              >
                <AppIcon name="calendar" :size="10" />
                {{ dayLabel(task.plannedFor) }}
              </button>
            </div>

            <DayPicker
              v-if="reschedulingId === task.id"
              class="mt-1.5"
              :model-value="task.plannedFor"
              :today="today"
              :marks="tasks.dayMarks"
              :clearable="task.plannedFor !== null"
              @update:model-value="reschedule(task, $event)"
              @clear="reschedule(task, null)"
              @close="reschedulingId = null"
            />
          </li>

          <li
            v-if="!bucket.tasks.length"
            class="rounded-[10px] border border-dashed border-line px-2.5 py-2 text-[11px] text-ink-3"
          >
            Nothing in here yet.
          </li>
        </ul>
      </section>

      <!-- Making a group is rare next to filing into one, so it sits at the
           bottom as a line rather than as a field taking up room. -->
      <div class="mt-3">
        <div v-if="namingGroup" class="flex items-center gap-1.5">
          <input
            v-model="newGroupName"
            placeholder="Group name…"
            aria-label="New group name"
            class="field min-w-0 flex-1 px-2.5 py-1.5 text-[11.5px]"
            @keydown.enter="createGroup"
            @keydown.esc="namingGroup = false"
          />
          <button
            class="btn btn-dark shrink-0 px-3 py-1.5 text-[11.5px]"
            :disabled="!newGroupName.trim()"
            @click="createGroup"
          >
            Create
          </button>
        </div>
        <button
          v-else
          class="btn btn-ghost w-full py-2 text-[11.5px]"
          @click="namingGroup = true"
        >
          <AppIcon name="plus" :size="12" :weight="2.2" />New group
        </button>
      </div>

      <p v-if="groups.error" class="mt-2 text-[11px] text-danger">{{ groups.error }}</p>
    </div>
  </div>
</template>
