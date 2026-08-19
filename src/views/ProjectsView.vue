<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { ref } from "vue";
import AppIcon from "../components/AppIcon.vue";
import PanelHeader from "../components/PanelHeader.vue";
import { errorMessage, ipc } from "../lib/ipc";
import { shortenPath } from "../lib/format";
import { useAppStore } from "../stores/app";
import type { ProjectInspection } from "../types";

const app = useAppStore();

const candidate = ref<ProjectInspection | null>(null);
const busy = ref(false);
const localError = ref<string | null>(null);

async function choose() {
  localError.value = null;
  const picked = await open({ directory: true, multiple: false, title: "Select project" });
  if (typeof picked !== "string") return;
  busy.value = true;
  try {
    candidate.value = await ipc.inspectDirectory(picked);
  } catch (caught) {
    localError.value = errorMessage(caught);
  } finally {
    busy.value = false;
  }
}

async function useCandidate() {
  if (!candidate.value) return;
  busy.value = true;
  await app.addProject(candidate.value.path);
  candidate.value = null;
  busy.value = false;
}
</script>

<template>
  <div class="flex flex-col">
    <PanelHeader
      eyebrow="One local repository at a time"
      title="Project"
      :back="app.hasProject"
      @back="app.back()"
    />

    <div class="panel-scroll px-3.5 pb-3.5">
      <!-- Confirmation step for a freshly picked directory -->
      <div v-if="candidate" class="card px-3 py-3">
        <div class="flex items-start gap-2.5">
          <span class="icon-btn h-8 w-8 shrink-0">
            <AppIcon name="folder" :size="15" />
          </span>
          <div class="min-w-0 flex-1">
            <p class="truncate text-[13.5px] font-semibold">{{ candidate.name }}</p>
            <p class="truncate text-[11px] text-ink-2">{{ shortenPath(candidate.path) }}</p>
          </div>
        </div>

        <p class="eyebrow mt-3">Detected</p>
        <ul class="mt-1.5 flex flex-wrap gap-1">
          <li
            class="chip"
            :class="candidate.isGit ? 'border-success/30 bg-success-soft text-success' : 'text-ink-3'"
          >
            <AppIcon :name="candidate.isGit ? 'check' : 'close'" :size="10" :weight="2.2" />
            {{ candidate.isGit ? "Git repository" : "No Git repository" }}
          </li>
          <li v-if="candidate.branch" class="chip border-success/30 bg-success-soft text-success">
            <AppIcon name="branch" :size="10" />{{ candidate.branch }}
          </li>
          <li
            v-for="manifest in candidate.manifests"
            :key="manifest"
            class="chip border-success/30 bg-success-soft text-success"
          >
            <AppIcon name="check" :size="10" :weight="2.2" />{{ manifest }}
          </li>
        </ul>
        <p v-if="!candidate.exists" class="mt-1.5 text-[11px] text-danger">
          That folder no longer exists.
        </p>

        <div class="mt-3 flex gap-2">
          <button class="btn btn-ghost flex-1 py-2" @click="candidate = null">Cancel</button>
          <button
            class="btn btn-dark flex-1 py-2"
            :disabled="!candidate.exists || busy"
            @click="useCandidate"
          >
            {{ candidate.alreadyAdded ? "Switch to it" : "Use project" }}
          </button>
        </div>
      </div>

      <template v-else>
        <button
          class="card flex w-full items-center gap-2.5 border-dashed px-3 py-3 transition-colors hover:bg-line-soft/70"
          :disabled="busy"
          @click="choose"
        >
          <span class="icon-btn h-8 w-8 shrink-0 border-dashed">
            <AppIcon name="folder" :size="15" />
          </span>
          <span class="min-w-0 flex-1 text-left">
            <span class="block text-[13px] font-semibold">Choose a folder</span>
            <span class="block text-[11px] text-ink-2">Pick a local Git repository</span>
          </span>
          <AppIcon name="forward" :size="14" class="text-ink-3" />
        </button>

        <p v-if="app.projects.length" class="eyebrow mt-4">Recent</p>
        <ul class="mt-1.5 space-y-1.5">
          <li
            v-for="project in app.projects"
            :key="project.id"
            class="card group flex items-center gap-2.5 px-2.5 py-2.5 transition-colors"
            :class="
              project.id === app.activeProject?.id
                ? 'border-accent/45 bg-accent-soft/50'
                : 'hover:bg-line-soft/70'
            "
          >
            <button
              class="flex min-w-0 flex-1 items-center gap-2.5 text-left"
              @click="app.selectProject(project.id)"
            >
              <span class="icon-btn h-8 w-8 shrink-0">
                <AppIcon name="folder" :size="14" />
              </span>
              <span class="min-w-0 flex-1">
                <span class="block truncate text-[13px] font-semibold">{{ project.name }}</span>
                <span class="block truncate text-[11px] text-ink-2">
                  {{ shortenPath(project.path) }}
                  <template v-if="project.branch"> · {{ project.branch }}</template>
                </span>
              </span>
            </button>
            <button
              class="icon-btn h-7 w-7 shrink-0 text-ink-3 opacity-0 transition group-hover:opacity-100 hover:text-danger"
              :aria-label="`Remove ${project.name}`"
              title="Remove project and its tasks"
              @click="app.removeProject(project.id)"
            >
              <AppIcon name="trash" :size="13" />
            </button>
          </li>
        </ul>
      </template>

      <p
        v-if="localError"
        class="mt-2 rounded-[10px] border border-danger/25 bg-danger-soft px-2.5 py-1.5 text-[11px] text-danger"
      >
        {{ localError }}
      </p>
    </div>
  </div>
</template>
