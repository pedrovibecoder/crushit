<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import AppIcon from "../components/AppIcon.vue";
import CategoryPill from "../components/CategoryPill.vue";
import PanelHeader from "../components/PanelHeader.vue";
import { formatDuration } from "../lib/format";
import { useAppStore } from "../stores/app";
import { useAgentStore } from "../stores/agent";
import { useTasksStore } from "../stores/tasks";
import { AGENT_LABELS } from "../types";

const app = useAppStore();
const codex = useAgentStore();
const agentName = computed(() => AGENT_LABELS[codex.selected]);
const tasks = useTasksStore();

const draft = ref("");
const busy = ref(false);

/** Which of the goal screen's states to render. */
const stage = computed(() => {
  if (codex.isAnalyzing) return "running";
  if (codex.plan) return "plan";
  if (codex.analysis.status === "failed" || codex.goal?.status === "failed") return "failed";
  if (codex.status && !codex.isReady) return "unavailable";
  return "compose";
});

const failure = computed(() => codex.analysis.error ?? codex.goal?.error ?? null);

onMounted(() => {
  if (!codex.status) void codex.checkStatus();
});

async function analyze() {
  if (!app.activeProject || !draft.value.trim()) return;
  busy.value = true;
  const started = await codex.analyze(app.activeProject.id, draft.value.trim());
  busy.value = false;
  if (started) draft.value = "";
}

async function accept() {
  const goalId = codex.goal?.id;
  if (goalId === undefined) return;
  busy.value = true;
  const added = await codex.accept(goalId);
  busy.value = false;
  if (added) {
    await tasks.load(app.activeProject?.id ?? null);
    app.back();
  }
}

async function discard() {
  const goalId = codex.goal?.id;
  if (goalId === undefined) {
    await codex.cancel(app.activeProject?.id ?? null);
    return;
  }
  busy.value = true;
  await codex.discard(goalId);
  busy.value = false;
}
</script>

<template>
  <div class="flex flex-col">
    <PanelHeader
      :eyebrow="`${agentName} planning`"
      :title="stage === 'plan' ? (codex.goal?.title ?? 'Plan') : 'New goal'"
      accent
      back
      @back="app.back()"
    />

    <div class="panel-scroll px-3.5 pb-3.5">
      <!-- Codex missing or signed out -->
      <template v-if="stage === 'unavailable'">
        <div class="card px-3 py-3">
          <p class="text-[12.5px] font-semibold">
            {{ codex.status?.installed ? `${agentName} is not signed in` : `${agentName} not found` }}
          </p>
          <p class="mt-1 text-[11.5px] leading-relaxed text-ink-2">
            {{
              codex.status?.installed
                ? `Sign in to ${agentName} in a terminal, then check again.`
                : `Install ${agentName}, or set its path in Settings.`
            }}
          </p>
          <p v-if="codex.status?.problem" class="mt-1.5 text-[11px] text-ink-3">
            {{ codex.status.problem }}
          </p>
          <div class="mt-2.5 flex gap-2">
            <button class="btn btn-ghost flex-1 py-1.5" @click="app.go('settings')">
              Settings
            </button>
            <button
              class="btn btn-dark flex-1 py-1.5"
              :disabled="codex.checking"
              @click="codex.checkStatus(undefined, true)"
            >
              {{ codex.checking ? "Checking…" : "Check again" }}
            </button>
          </div>
        </div>
      </template>

      <!-- Compose a goal -->
      <template v-else-if="stage === 'compose'">
        <label class="eyebrow" for="goal-text">What do you want to build?</label>
        <textarea
          id="goal-text"
          v-model="draft"
          rows="4"
          placeholder="Allow customers to download invoices as PDF."
          class="field mt-1.5 w-full resize-none px-2.5 py-2 text-[12.5px] leading-relaxed"
          @keydown.meta.enter.prevent="analyze"
        />
        <div class="mt-2 flex items-center gap-1.5">
          <span class="chip text-ink-2">
            <AppIcon name="folder" :size="10" />{{ app.activeProject?.name ?? "no project" }}
          </span>
          <span class="chip text-ink-2">
            <AppIcon name="check" :size="10" />Read-only
          </span>
        </div>
        <p class="mt-2 text-[11px] leading-relaxed text-ink-3">
          {{ agentName }} reads the repository to work out what already exists
          and what is missing. It cannot change any files during planning.
        </p>
        <button
          class="btn btn-dark mt-3 w-full py-2"
          :disabled="!draft.trim() || busy || !app.activeProject"
          @click="analyze"
        >
          <AppIcon name="sparkle" :size="13" filled />
          {{ busy ? "Starting…" : "Analyze project" }}
        </button>
      </template>

      <!-- Analysis in flight -->
      <template v-else-if="stage === 'running'">
        <div class="card px-3 py-3">
          <p class="text-[12.5px] font-semibold">{{ codex.analysis.goalTitle }}</p>
          <p class="mt-0.5 text-[11px] text-ink-2">Reading {{ app.activeProject?.name }}…</p>

          <ul class="mt-2.5 space-y-1.5">
            <li
              v-for="step in codex.analysis.steps"
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
            <li v-if="!codex.analysis.steps.length" class="text-[11.5px] text-ink-2">
              Starting Codex…
            </li>
          </ul>
        </div>
        <p class="mt-2 text-[11px] text-ink-3">
          This keeps running if you close the popup.
        </p>
        <button class="btn btn-ghost mt-2.5 w-full py-2" @click="codex.cancel(app.activeProject?.id ?? null)">
          Stop analysis
        </button>
      </template>

      <!-- Analysis failed -->
      <template v-else-if="stage === 'failed'">
        <div class="card border-danger/25 bg-danger-soft/60 px-3 py-3">
          <p class="text-[12.5px] font-semibold text-danger">Analysis failed</p>
          <p class="mt-1 text-[11.5px] leading-relaxed text-ink-2">{{ failure }}</p>
        </div>
        <div class="mt-2.5 flex gap-2">
          <button class="btn btn-ghost flex-1 py-2" :disabled="busy" @click="discard">
            Discard
          </button>
          <button class="btn btn-dark flex-1 py-2" @click="codex.setGoal(null)">
            Try again
          </button>
        </div>
      </template>

      <!-- Review the generated plan -->
      <template v-else-if="stage === 'plan' && codex.plan">
        <p v-if="codex.plan.summary" class="text-[12.5px] leading-relaxed text-ink-2">
          {{ codex.plan.summary }}
        </p>

        <div v-if="codex.plan.existing.length" class="mt-3">
          <h2 class="eyebrow">Already there</h2>
          <ul class="mt-1.5 space-y-1">
            <li
              v-for="item in codex.plan.existing"
              :key="item"
              class="flex items-start gap-2 text-[11.5px] leading-snug text-ink-2"
            >
              <AppIcon name="check" :size="11" class="mt-[3px] shrink-0 text-success" />
              <span class="min-w-0 flex-1">{{ item }}</span>
            </li>
          </ul>
        </div>

        <div v-if="codex.plan.missing.length" class="mt-3">
          <h2 class="eyebrow">Missing</h2>
          <ul class="mt-1.5 flex flex-wrap gap-1">
            <li v-for="item in codex.plan.missing" :key="item" class="chip text-ink-2">
              {{ item }}
            </li>
          </ul>
        </div>

        <h2 class="eyebrow mt-4">
          {{ codex.plan.tasks.length }} task{{ codex.plan.tasks.length === 1 ? "" : "s" }}
        </h2>
        <ol class="mt-1.5 space-y-1.5">
          <li
            v-for="(task, index) in codex.plan.tasks"
            :key="index"
            class="card px-2.5 py-2.5"
          >
            <div class="flex items-baseline gap-2">
              <span class="tnum shrink-0 text-[11px] font-semibold text-ink-3">
                {{ index + 1 }}
              </span>
              <span class="min-w-0 flex-1 text-[12.5px] leading-snug font-semibold">
                {{ task.title }}
              </span>
            </div>
            <p v-if="task.description" class="mt-1 pl-5 text-[11.5px] leading-snug text-ink-2">
              {{ task.description }}
            </p>
            <div class="mt-1.5 flex flex-wrap items-center gap-1 pl-5">
              <CategoryPill :category="task.category" />
              <span v-if="task.estimateMinutes > 0" class="chip text-ink-2">
                {{ formatDuration(task.estimateMinutes) }}
              </span>
              <span v-if="task.acceptanceCriteria.length" class="chip tnum text-ink-2">
                {{ task.acceptanceCriteria.length }} criteria
              </span>
              <span v-if="task.dependsOn.length" class="chip text-ink-2">
                After {{ task.dependsOn.map((n) => `#${n}`).join(", ") }}
              </span>
            </div>
          </li>
        </ol>
      </template>
    </div>

    <footer
      v-if="stage === 'plan' && codex.plan"
      class="flex items-center gap-2 border-t border-line px-3.5 py-2.5"
    >
      <button class="btn btn-ghost px-3 py-2" :disabled="busy" @click="discard">Discard</button>
      <button class="btn btn-dark flex-1 py-2" :disabled="busy" @click="accept">
        <AppIcon name="plus" :size="13" />
        Add {{ codex.plan.tasks.length }} task{{ codex.plan.tasks.length === 1 ? "" : "s" }}
      </button>
    </footer>

    <p
      v-if="codex.error"
      class="border-t border-danger/20 bg-danger-soft px-3.5 py-2 text-[11px] text-danger"
    >
      {{ codex.error }}
    </p>
  </div>
</template>
