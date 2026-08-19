<script setup lang="ts">
import { computed, onMounted } from "vue";
import AppIcon from "../components/AppIcon.vue";
import PanelHeader from "../components/PanelHeader.vue";
import { useAppStore } from "../stores/app";
import { useReviewStore } from "../stores/review";

const app = useAppStore();
const review = useReviewStore();

onMounted(() => review.loadChanges());

const summary = computed(() => {
  const count = review.changes.files.length;
  return `${count} file${count === 1 ? "" : "s"} changed`;
});

/** A dot colour per state, so the list reads without labels. */
const STATE_TONE: Record<string, string> = {
  added: "text-success",
  modified: "text-accent",
  deleted: "text-danger",
  renamed: "text-warn",
  untracked: "text-ink-3",
};

/** Colour each diff line by whether it adds, removes, or just gives context. */
function lineTone(line: string): string {
  if (line.startsWith("+++") || line.startsWith("---")) return "text-ink-3";
  if (line.startsWith("@@")) return "text-accent";
  if (line.startsWith("+")) return "text-success";
  if (line.startsWith("-")) return "text-danger";
  return "text-ink-2";
}
</script>

<template>
  <div class="flex flex-col">
    <PanelHeader
      :eyebrow="app.activeProject?.name"
      :title="review.diff ? 'Diff' : 'Changes'"
      back
      @back="review.diff ? review.closeDiff() : app.back()"
    >
      <template #actions>
        <button
          v-if="!review.diff"
          class="icon-btn h-7 w-7"
          aria-label="Refresh"
          title="Refresh"
          @click="review.loadChanges()"
        >
          <AppIcon name="branch" :size="13" />
        </button>
      </template>
    </PanelHeader>

    <div class="panel-scroll px-3.5 pb-3.5">
      <!-- One file's diff. -->
      <template v-if="review.diff">
        <p class="truncate text-[11.5px] font-semibold">{{ review.diff.path }}</p>
        <pre
          class="mt-1.5 overflow-x-auto rounded-[10px] border border-line bg-card p-2 font-mono text-[10.5px] leading-[1.5]"
        ><code><span
            v-for="(line, index) in review.diff.text.split('\n')"
            :key="index"
            class="block whitespace-pre"
            :class="lineTone(line)"
          >{{ line || " " }}</span></code></pre>
      </template>

      <template v-else-if="!review.changes.isGit">
        <p class="text-[12px] text-ink-2">
          This project is not a Git repository, so there is nothing to compare against.
        </p>
      </template>

      <template v-else-if="!review.changes.files.length">
        <p class="text-[12px] text-ink-2">The working tree is clean.</p>
      </template>

      <template v-else>
        <div class="card flex items-center gap-2 px-3 py-2.5">
          <span class="min-w-0 flex-1 text-[12.5px] font-semibold">{{ summary }}</span>
          <span class="tnum shrink-0 text-[11.5px] font-semibold text-success">
            +{{ review.changes.insertions }}
          </span>
          <span class="tnum shrink-0 text-[11.5px] font-semibold text-danger">
            −{{ review.changes.deletions }}
          </span>
        </div>

        <ul class="mt-2 space-y-1.5">
          <li v-for="file in review.changes.files" :key="file.path">
            <button
              class="card flex w-full items-center gap-2 px-2.5 py-2 text-left transition-colors hover:bg-line-soft/70"
              @click="review.openDiff(file.path)"
            >
              <span
                class="shrink-0 text-[16px] leading-none"
                :class="STATE_TONE[file.state] ?? 'text-ink-3'"
                :title="file.state"
                >•</span
              >
              <span class="min-w-0 flex-1 truncate text-[12px] font-medium">{{ file.path }}</span>
              <span v-if="file.insertions" class="tnum shrink-0 text-[10.5px] text-success">
                +{{ file.insertions }}
              </span>
              <span v-if="file.deletions" class="tnum shrink-0 text-[10.5px] text-danger">
                −{{ file.deletions }}
              </span>
              <AppIcon name="forward" :size="12" class="shrink-0 text-ink-3" />
            </button>
          </li>
        </ul>
      </template>

      <p v-if="review.error" class="mt-2 text-[11px] text-danger">{{ review.error }}</p>
    </div>
  </div>
</template>
