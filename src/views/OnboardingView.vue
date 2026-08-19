<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { computed, onMounted, ref } from "vue";
import AppIcon from "../components/AppIcon.vue";
import PanelHeader from "../components/PanelHeader.vue";
import { errorMessage, ipc } from "../lib/ipc";
import { shortenPath } from "../lib/format";
import { useAgentStore } from "../stores/agent";
import { useAppStore } from "../stores/app";
import { AGENT_LABELS } from "../types";

const app = useAppStore();
const agents = useAgentStore();

/** Two steps: point at a repository, then check the agent behind it. */
const step = ref<"project" | "agent">("project");
const busy = ref(false);
const localError = ref<string | null>(null);

const agentName = computed(() => AGENT_LABELS[agents.selected]);

onMounted(() => {
  // Someone re-running setup may already have a project.
  if (app.activeProject) step.value = "agent";
});

async function choose() {
  localError.value = null;
  const picked = await open({ directory: true, multiple: false, title: "Select project" });
  if (typeof picked !== "string") return;
  busy.value = true;
  try {
    const found = await ipc.inspectDirectory(picked);
    if (!found.exists) {
      localError.value = "That folder does not exist.";
      return;
    }
    await app.addProject(found.path);
    step.value = "agent";
    void agents.checkStatus();
  } catch (caught) {
    localError.value = errorMessage(caught);
  } finally {
    busy.value = false;
  }
}

async function finish() {
  await app.updateSettings({ onboarded: true });
  app.go(app.activeProject ? "today" : "projects");
}
</script>

<template>
  <div class="flex flex-col">
    <PanelHeader eyebrow="Welcome" title="Set up Blitzit" />

    <div class="panel-scroll px-3.5 pb-3.5">
      <!-- Step one: the repository everything else hangs off. -->
      <template v-if="step === 'project'">
        <p class="text-[12.5px] leading-relaxed text-ink-2">
          Blitzit works against one local repository at a time. Pick the project
          you want to plan and build in — you can change it later.
        </p>
        <button
          class="card mt-3 flex w-full items-center gap-2.5 border-dashed px-3 py-3 transition-colors hover:bg-line-soft/70"
          :disabled="busy"
          @click="choose"
        >
          <span class="icon-btn h-8 w-8 shrink-0 border-dashed">
            <AppIcon name="folder" :size="15" />
          </span>
          <span class="min-w-0 flex-1 text-left">
            <span class="block text-[13px] font-semibold">Choose a folder</span>
            <span class="block text-[11px] text-ink-2">A local Git repository</span>
          </span>
          <AppIcon name="forward" :size="14" class="text-ink-3" />
        </button>
      </template>

      <!-- Step two: is the coding agent actually usable? -->
      <template v-else>
        <div v-if="app.activeProject" class="card flex items-center gap-2.5 px-3 py-2.5">
          <AppIcon name="check" :size="14" :weight="2.4" class="shrink-0 text-success" />
          <span class="min-w-0 flex-1">
            <span class="block truncate text-[12.5px] font-semibold">
              {{ app.activeProject.name }}
            </span>
            <span class="block truncate text-[11px] text-ink-2">
              {{ shortenPath(app.activeProject.path) }}
            </span>
          </span>
        </div>

        <p class="eyebrow mt-3">{{ agentName }}</p>
        <div class="card mt-1.5 px-3 py-2.5">
          <p v-if="!agents.status" class="text-[12px] text-ink-2">Checking {{ agentName }}…</p>
          <template v-else>
            <p class="flex items-center gap-1.5 text-[12.5px] font-semibold">
              <AppIcon
                :name="agents.status.installed ? 'check' : 'close'"
                :size="12"
                :weight="2.4"
                :class="agents.status.installed ? 'text-success' : 'text-warn'"
              />
              {{ agents.status.installed ? `${agentName} installed` : `${agentName} not found` }}
            </p>
            <p class="mt-1 flex items-center gap-1.5 text-[12.5px] font-semibold">
              <AppIcon
                :name="agents.status.signedIn ? 'check' : 'close'"
                :size="12"
                :weight="2.4"
                :class="agents.status.signedIn ? 'text-success' : 'text-warn'"
              />
              {{ agents.status.signedIn ? `Signed in with ${agents.status.authMode}` : "Not signed in" }}
            </p>
            <p v-if="!agents.isReady" class="mt-1.5 text-[11px] leading-relaxed text-ink-2">
              Planning and running tasks need this. You can finish setup now and
              sort it out later in Settings — the timer and task list work either way.
            </p>
          </template>

          <button
            class="btn btn-ghost mt-2 w-full py-1.5 text-[11.5px]"
            :disabled="agents.checking"
            @click="agents.checkStatus(undefined, true)"
          >
            {{ agents.checking ? "Checking…" : "Check again" }}
          </button>
        </div>
      </template>

      <p v-if="localError" class="mt-2 text-[11px] text-danger">{{ localError }}</p>
    </div>

    <footer class="flex items-center gap-2 border-t border-line px-3.5 py-2.5">
      <button class="btn btn-ghost px-3 py-2" @click="finish">Skip</button>
      <button
        class="btn btn-dark flex-1 py-2"
        :disabled="step === 'project' && !app.activeProject"
        @click="step === 'project' ? choose() : finish()"
      >
        {{ step === "project" ? "Choose a project" : "Start working" }}
      </button>
    </footer>
  </div>
</template>
