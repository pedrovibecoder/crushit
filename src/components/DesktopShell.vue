<script setup lang="ts">
import { computed } from "vue";
import { formatClock } from "../lib/format";
import { ipc } from "../lib/ipc";
import { useAgentStore } from "../stores/agent";
import { useAppStore, type View } from "../stores/app";
import { useExecutionStore } from "../stores/execution";
import { useFocusStore } from "../stores/focus";
import { useTasksStore } from "../stores/tasks";
import { AGENT_LABELS } from "../types";
import AppIcon from "./AppIcon.vue";
import CelebrationOverlay from "./CelebrationOverlay.vue";
import ErrorBanner from "./ErrorBanner.vue";

defineProps<{ current: unknown; ready: boolean }>();

const app = useAppStore();
const tasks = useTasksStore();
const focus = useFocusStore();
const agents = useAgentStore();
const execution = useExecutionStore();

const agentName = computed(() => AGENT_LABELS[agents.selected]);

/** The screens the sidebar navigates between, with a count where one helps. */
const NAV: Array<{ id: View; label: string; icon: string }> = [
  { id: "today", label: "Today", icon: "check" },
  { id: "goal", label: "Goal", icon: "sparkle" },
  { id: "changes", label: "Changes", icon: "branch" },
  { id: "history", label: "History", icon: "calendar" },
  { id: "slack", label: "Slack", icon: "message" },
  { id: "settings", label: "Settings", icon: "gear" },
];

const openCount = computed(() => tasks.openTasks.length);

/** What the agent is doing, in the words the menu bar would use. */
const agentState = computed(() => {
  if (execution.isAwaitingApproval) return { label: "Needs approval", tone: "text-warn" };
  if (execution.isActive) return { label: `${agentName.value} is working`, tone: "text-accent" };
  if (agents.isAnalyzing) return { label: "Reading the project", tone: "text-accent" };
  return null;
});
</script>

<template>
  <div class="relative flex h-screen w-full overflow-hidden bg-page">
    <!-- Left: where you are, and what is happening. -->
    <aside class="flex w-[196px] shrink-0 flex-col border-r border-line bg-card/40">
      <button
        class="flex items-center gap-2 px-3 py-3 text-left transition-colors hover:bg-line-soft/60"
        @click="app.go('projects')"
      >
        <span class="icon-btn h-7 w-7 shrink-0">
          <AppIcon name="folder" :size="13" />
        </span>
        <span class="min-w-0 flex-1">
          <span class="block truncate text-[12.5px] font-semibold">
            {{ app.activeProject?.name ?? "Choose a project" }}
          </span>
          <span v-if="app.activeProject?.branch" class="block truncate text-[10.5px] text-ink-2">
            {{ app.activeProject.branch }}
          </span>
        </span>
        <AppIcon name="chevron" :size="12" class="shrink-0 text-ink-3" />
      </button>

      <nav class="flex-1 px-2 pt-1">
        <button
          v-for="item in NAV"
          :key="item.id"
          class="mb-0.5 flex w-full items-center gap-2 rounded-[9px] px-2 py-1.5 text-left transition-colors"
          :class="
            app.view === item.id || (item.id === 'today' && app.view === 'task')
              ? 'bg-solid text-on-solid'
              : 'text-ink-2 hover:bg-line-soft hover:text-ink'
          "
          @click="app.go(item.id)"
        >
          <AppIcon :name="item.icon" :size="13" class="shrink-0" />
          <span class="min-w-0 flex-1 truncate text-[12.5px] font-semibold">{{ item.label }}</span>
          <span v-if="item.id === 'today' && openCount" class="tnum text-[11px] opacity-70">
            {{ openCount }}
          </span>
        </button>
      </nav>

      <!-- What is running sits above the bar, so the bar itself is the same
           height as the screen's own footer and the two line up across the
           window rather than stepping. -->
      <div class="px-3 pb-2">
        <p v-if="focus.isActive" class="tnum text-[13px] font-semibold">
          {{ formatClock(focus.snapshot.remainingSeconds) }}
          <span class="text-[11px] font-medium text-ink-2">
            {{ focus.isPaused ? "paused" : "focusing" }}
          </span>
        </p>
        <p v-if="agentState" class="mt-1 truncate text-[11px] font-semibold" :class="agentState.tone">
          {{ agentState.label }}
        </p>
        <p v-if="!focus.isActive && !agentState" class="text-[11px] text-ink-3">Nothing running</p>
      </div>

      <div class="border-t border-line px-3 py-2.5">
        <button
          class="btn btn-ghost w-full py-2 text-[12px]"
          @click="ipc.hideDesktopWindow()"
        >
          Hide to menu bar
        </button>
      </div>
    </aside>

    <!-- Right: the selected screen, at full width. -->
    <main class="flex min-w-0 flex-1 flex-col" style="--panel-max: none">
      <ErrorBanner />
      <div v-if="!ready" class="flex flex-1 items-center justify-center text-[12px] text-ink-3">
        Loading…
      </div>
      <div v-else-if="app.failed" class="flex flex-1 flex-col items-center justify-center gap-2 px-6 text-center">
        <p class="text-[12.5px] font-semibold">Crushit could not start up</p>
        <p class="text-[11.5px] leading-relaxed text-ink-2">{{ app.error }}</p>
        <button class="btn btn-dark mt-1 px-4 py-2" @click="app.bootstrap()">Try again</button>
      </div>
      <!-- The screen itself fills this column, so its footer sits on the
           bottom edge of the window instead of trailing the content. -->
      <div
        v-else
        class="mx-auto flex w-full max-w-[720px] min-h-0 flex-1 flex-col [&>*]:min-h-0 [&>*]:flex-1"
      >
        <Transition name="screen" mode="out-in">
          <component :is="current" :key="app.view" />
        </Transition>
      </div>
    </main>

    <CelebrationOverlay />
  </div>
</template>
