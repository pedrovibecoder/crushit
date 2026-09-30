<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import AppIcon from "../components/AppIcon.vue";
import CategoryPill from "../components/CategoryPill.vue";
import PanelHeader from "../components/PanelHeader.vue";
import TagChip from "../components/TagChip.vue";
import { copyText } from "../lib/clipboard";
import { buildContext } from "../lib/context";
import { errorMessage, ipc } from "../lib/ipc";
import { useAppStore } from "../stores/app";
import { useAgentStore } from "../stores/agent";
import { useCategoriesStore } from "../stores/categories";
import { useTagsStore } from "../stores/tags";
import { useTasksStore } from "../stores/tasks";
import { AGENT_LABELS, type BriefStep } from "../types";

const app = useAppStore();
const tasks = useTasksStore();
const categories = useCategoriesStore();
const tags = useTagsStore();
const agent = useAgentStore();

const agentName = computed(() => AGENT_LABELS[agent.selected]);

/** The tasks the popup picked, left behind for this window to pick up. */
const picked = ref<number[]>([]);
const chosen = computed(() =>
  picked.value
    .map((id) => tasks.byId.get(id))
    .filter((task): task is NonNullable<typeof task> => !!task),
);

const context = computed(() =>
  buildContext(chosen.value, {
    project: app.activeProject?.name ?? null,
    branch: app.activeProject?.branch ?? null,
    categoryLabel: (slug) => categories.labelFor(slug),
    tagLabel: (slug) => tags.labelFor(slug),
  }),
);

/**
 * The brief is what the agent makes of the tasks with the repository in front
 * of it. It starts the moment the screen opens: arriving here is the request.
 */
const brief = ref<string | null>(null);
const briefing = ref(false);
const briefError = ref<string | null>(null);

/**
 * What the agent is doing while it reads, in the order it did it. Finished
 * steps stay on screen: half the value of watching is seeing where it went.
 */
const steps = ref<BriefStep[]>([]);

/**
 * Started at once and awaited before asking, so the first thing the agent does
 * is not the one step nobody sees.
 */
const listening = (async () => {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<BriefStep>("brief:step", (event) => {
    const step = event.payload;
    const known = steps.value.find((other) => other.id === step.id);
    if (known) known.done = step.done;
    else if (step.label) steps.value.push({ ...step });
  });
})().catch(() => () => {});

onBeforeUnmount(async () => (await listening)());

async function writeBrief() {
  if (briefing.value || !app.activeProject || !chosen.value.length) return;
  briefing.value = true;
  briefError.value = null;
  steps.value = [];
  try {
    await listening;
    brief.value = await ipc.briefTasks(app.activeProject.id, context.value);
  } catch (caught) {
    briefError.value = errorMessage(caught);
  } finally {
    briefing.value = false;
  }
}

onMounted(async () => {
  picked.value = await ipc.contextSelection();
});

/**
 * Asked for once, as soon as there is something to ask about.
 *
 * This window may still be starting up when the screen opens — the tasks
 * arrive with the bootstrap, and until they do the picked ids resolve to
 * nothing. Waiting for them beats asking too early and sitting there silent.
 */
const asked = ref(false);
watch(
  () => [app.ready, app.activeProject?.id ?? null, chosen.value.length] as const,
  () => {
    if (asked.value || !app.ready || !app.activeProject || !chosen.value.length) return;
    asked.value = true;
    void writeBrief();
  },
  { immediate: true },
);

/** Which block was copied last, so the button can say so and settle again. */
const copiedWhat = ref<"brief" | "context" | "both" | null>(null);

async function copy(what: "brief" | "context" | "both") {
  const text =
    what === "brief"
      ? (brief.value ?? "")
      : what === "context"
        ? context.value
        : `${brief.value ? `${brief.value}\n\n---\n\n` : ""}${context.value}`;
  copiedWhat.value = (await copyText(text)) ? what : null;
  if (copiedWhat.value) window.setTimeout(() => { copiedWhat.value = null }, 2000);
}
</script>

<template>
  <div class="flex flex-col">
    <PanelHeader
      :eyebrow="app.activeProject?.name"
      title="Context"
      :back="true"
      accent
      @back="app.go('today')"
    />

    <div class="panel-scroll px-3.5 pb-3.5">
      <p v-if="!app.ready" class="mt-4 text-center text-[11.5px] text-ink-3">Loading…</p>
      <p v-else-if="!chosen.length" class="mt-4 text-center text-[11.5px] text-ink-2">
        Nothing was picked. Choose some tasks on Today and press Context.
      </p>

      <template v-else>
        <!-- What was picked, so it is obvious what the brief is about. -->
        <ul class="flex flex-wrap gap-1">
          <li
            v-for="task in chosen"
            :key="task.id"
            class="card flex items-center gap-1.5 px-2 py-1"
          >
            <span class="max-w-[280px] truncate text-[11.5px] font-semibold">
              {{ task.title }}
            </span>
            <CategoryPill :category="task.category" />
            <TagChip v-for="slug in task.tags" :key="slug" :slug="slug" />
          </li>
        </ul>

        <section class="mt-3">
          <div class="flex items-center gap-2">
            <h2 class="eyebrow flex-1">What this adds up to</h2>
            <button
              v-if="brief"
              class="btn btn-ghost shrink-0 px-2.5 py-1 text-[11px]"
              @click="copy('brief')"
            >
              {{ copiedWhat === "brief" ? "Copied" : "Copy" }}
            </button>
            <button
              v-if="!briefing"
              class="btn btn-ghost shrink-0 px-2.5 py-1 text-[11px]"
              :title="brief ? 'Ask again' : 'Ask the agent'"
              @click="writeBrief"
            >
              <AppIcon name="refresh" :size="11" />{{ brief ? "Again" : "Ask" }}
            </button>
          </div>

          <div class="card mt-1.5 px-3 py-3">
            <template v-if="briefing">
              <p class="flex items-center gap-2 text-[12px] text-ink-2">
                <AppIcon name="sparkle" :size="12" filled class="text-accent" />
                {{ agentName }} is reading {{ app.activeProject?.name }}…
              </p>
              <!-- The same list the goal screen shows while a plan is being
                   written, so a long read looks like work rather than a stall. -->
              <ul class="mt-2.5 space-y-1.5">
                <li
                  v-for="step in steps"
                  :key="step.id"
                  class="flex items-start gap-2 text-[11.5px]"
                  :class="step.done ? 'text-ink-2' : 'text-ink'"
                >
                  <span
                    class="mt-[3px] flex h-3 w-3 shrink-0 items-center justify-center rounded-full border"
                    :class="step.done ? 'border-success bg-success text-white' : 'border-accent'"
                  >
                    <AppIcon v-if="step.done" name="check" :size="8" :weight="3" />
                  </span>
                  <span class="min-w-0 flex-1 truncate">{{ step.label }}</span>
                </li>
                <li v-if="!steps.length" class="text-[11.5px] text-ink-3">
                  Starting {{ agentName }}…
                </li>
              </ul>
            </template>
            <p v-else-if="briefError" class="text-[12px] leading-relaxed text-danger">
              {{ briefError }}
            </p>
            <p
              v-else-if="brief"
              class="text-[12.5px] leading-relaxed whitespace-pre-wrap"
            >{{ brief }}</p>
            <p v-else class="text-[12px] text-ink-2">
              Nothing yet — ask {{ agentName }} to read the repository and say what
              these tasks amount to.
            </p>
          </div>
        </section>

        <section class="mt-3">
          <div class="flex items-center gap-2">
            <h2 class="eyebrow flex-1">The tasks, as recorded</h2>
            <button class="btn btn-ghost shrink-0 px-2.5 py-1 text-[11px]" @click="copy('context')">
              {{ copiedWhat === "context" ? "Copied" : "Copy" }}
            </button>
            <button class="btn btn-dark shrink-0 px-2.5 py-1 text-[11px]" @click="copy('both')">
              {{ copiedWhat === "both" ? "Copied" : "Copy both" }}
            </button>
          </div>
          <!-- Read-only and selectable: this is meant to be taken away. -->
          <textarea
            :value="context"
            readonly
            rows="18"
            aria-label="The picked tasks as text"
            class="field mt-1.5 w-full resize-y px-3 py-2.5 font-mono text-[11px] leading-relaxed"
            @focus="($event.target as HTMLTextAreaElement).select()"
          />
        </section>
      </template>
    </div>
  </div>
</template>
