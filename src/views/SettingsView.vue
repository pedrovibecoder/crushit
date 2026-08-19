<script setup lang="ts">
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { computed, onMounted, ref, watch } from "vue";
import AppIcon from "../components/AppIcon.vue";
import PanelHeader from "../components/PanelHeader.vue";
import SelectMenu, { type SelectOption } from "../components/SelectMenu.vue";
import ToggleSwitch from "../components/ToggleSwitch.vue";
import { ipc } from "../lib/ipc";
import { SHORTCUTS } from "../lib/shortcuts";
import { shortenPath } from "../lib/format";
import { useAgentStore } from "../stores/agent";
import { useAppStore } from "../stores/app";
import { useCategoriesStore } from "../stores/categories";
import {
  AGENTS,
  AGENT_LABELS,
  CATEGORY_COLORS,
  THEMES,
  THEME_LABELS,
  type Agent,
  type ShortcutInfo,
  type Theme,
} from "../types";

const app = useAppStore();
const agents = useAgentStore();
const categories = useCategoriesStore();

const FOCUS_PRESETS = [15, 25, 45, 60];

const pathDraft = ref("");
const editingPath = ref(false);
const shortcut = ref<ShortcutInfo | null>(null);

const selected = computed<Agent>(() => app.settings.agent);
const configuredPath = computed(() =>
  selected.value === "codex" ? app.settings.codexPath : app.settings.claudePath,
);
const configuredModel = computed(
  () => (selected.value === "codex" ? app.settings.codexModel : app.settings.claudeModel) ?? "",
);

const modelOptions = computed<SelectOption[]>(() => [
  { value: "", label: `Use my ${AGENT_LABELS[selected.value]} config` },
  ...agents.currentModels.map((model) => ({
    value: model.id,
    label: model.isDefault ? `${model.displayName} (default)` : model.displayName,
    hint: model.description ?? undefined,
  })),
]);

onMounted(async () => {
  shortcut.value = await ipc.popupShortcut();
  await refreshAgent();
});

// Switching agents re-checks the new one rather than showing stale state.
watch(selected, () => {
  pathDraft.value = configuredPath.value ?? "";
  editingPath.value = false;
  void refreshAgent();
});

async function refreshAgent() {
  pathDraft.value = configuredPath.value ?? "";
  if (!agents.statuses[selected.value]) await agents.checkStatus(selected.value);
  if (agents.isReady && !agents.currentModels.length) void agents.loadModels(selected.value);
}

async function chooseAgent(agent: Agent) {
  if (agent === selected.value) return;
  await app.updateSettings({ agent });
}

async function savePath() {
  const value = pathDraft.value.trim() || null;
  await app.updateSettings(
    selected.value === "codex" ? { codexPath: value } : { claudePath: value },
  );
  editingPath.value = false;
  await agents.checkStatus(selected.value, true);
}

async function chooseModel(value: string) {
  const model = value || null;
  await app.updateSettings(
    selected.value === "codex" ? { codexModel: model } : { claudeModel: model },
  );
}

const newCategory = ref("");
/** Which row has its palette open; only one at a time keeps the list short. */
const recolouring = ref<number | null>(null);

/** The next colour a new category gets, so a fresh list is not all one shade. */
const nextColor = computed(
  () => CATEGORY_COLORS[categories.categories.length % CATEGORY_COLORS.length],
);

async function addCategory() {
  const label = newCategory.value.trim();
  if (!label) return;
  if (await categories.create(label, nextColor.value)) newCategory.value = "";
}

async function renameCategory(id: number, event: Event) {
  const input = event.target as HTMLInputElement;
  const label = input.value.trim();
  const current = categories.categories.find((category) => category.id === id);
  if (!current) return;
  if (!label || label === current.label) {
    input.value = current.label;
    return;
  }
  await categories.update(id, { label });
}

async function recolour(id: number, color: string) {
  recolouring.value = null;
  await categories.update(id, { color });
}

async function reveal() {
  if (!app.activeProject) return;
  try {
    await revealItemInDir(app.activeProject.path);
  } catch {
    // Finder being unavailable is not worth interrupting the user over.
  }
}
</script>

<template>
  <div class="flex flex-col">
    <PanelHeader eyebrow="Crushit" title="Settings" back @back="app.back()" />

    <div class="panel-scroll px-3.5 pb-3.5">
      <section>
        <h2 class="eyebrow">Project</h2>
        <div v-if="app.activeProject" class="card mt-1.5 px-3 py-3">
          <div class="flex items-start gap-2.5">
            <span class="icon-btn h-8 w-8 shrink-0">
              <AppIcon name="folder" :size="14" />
            </span>
            <div class="min-w-0 flex-1">
              <p class="truncate text-[13px] font-semibold">{{ app.activeProject.name }}</p>
              <p class="truncate text-[11px] text-ink-2">
                {{ shortenPath(app.activeProject.path) }}
              </p>
            </div>
          </div>
          <div class="mt-2 flex flex-wrap gap-1">
            <span class="chip text-ink-2">
              <AppIcon name="branch" :size="10" />
              {{
                app.activeProject.branch ??
                (app.activeProject.isGit ? "detached HEAD" : "no Git repository")
              }}
            </span>
          </div>
          <div class="mt-2.5 flex gap-1.5">
            <button class="btn btn-ghost flex-1 py-1.5 text-[11.5px]" @click="app.go('projects')">
              Change
            </button>
            <button
              class="btn btn-ghost flex-1 py-1.5 text-[11.5px]"
              @click="app.refreshActiveProject(true)"
            >
              Refresh
            </button>
            <button class="btn btn-ghost flex-1 py-1.5 text-[11.5px]" @click="reveal">
              Reveal
            </button>
          </div>
        </div>
        <button v-else class="btn btn-ghost mt-1.5 w-full py-2" @click="app.go('projects')">
          Choose a project
        </button>
      </section>

      <section class="mt-4">
        <h2 class="eyebrow">Categories</h2>
        <p class="mt-1 text-[11px] leading-relaxed text-ink-2">
          What tasks are filed under, here and in the plans your agent writes.
        </p>

        <ul class="mt-1.5 space-y-1">
          <li
            v-for="category in categories.categories"
            :key="category.id"
            class="card px-2 py-1.5"
          >
            <div class="flex items-center gap-2">
              <button
                class="h-[18px] w-[18px] shrink-0 rounded-full border border-line"
                :style="{ backgroundColor: category.color }"
                :aria-label="`Change the colour of ${category.label}`"
                :title="`Change the colour of ${category.label}`"
                @click="recolouring = recolouring === category.id ? null : category.id"
              />
              <input
                :value="category.label"
                class="min-w-0 flex-1 rounded-[6px] border border-transparent bg-transparent px-1 py-0.5 text-[12.5px] font-semibold outline-none hover:border-line focus:border-ink-3"
                :aria-label="`Rename ${category.label}`"
                @blur="renameCategory(category.id, $event)"
                @keydown.enter.prevent="($event.target as HTMLInputElement).blur()"
              />
              <button
                v-if="categories.categories.length > 1"
                class="shrink-0 text-ink-3 transition-colors hover:text-danger"
                :aria-label="`Delete ${category.label}`"
                :title="`Delete ${category.label} — its tasks move to ${categories.categories[0].label}`"
                @click="categories.remove(category.id)"
              >
                <AppIcon name="close" :size="12" />
              </button>
            </div>

            <div v-if="recolouring === category.id" class="mt-1.5 flex flex-wrap gap-1 pl-[26px]">
              <button
                v-for="color in CATEGORY_COLORS"
                :key="color"
                class="h-[18px] w-[18px] rounded-full border transition-transform hover:scale-110"
                :class="color === category.color ? 'border-ink' : 'border-line'"
                :style="{ backgroundColor: color }"
                :aria-label="`Use this colour for ${category.label}`"
                @click="recolour(category.id, color)"
              />
            </div>
          </li>
        </ul>

        <div class="mt-1.5 flex gap-1.5">
          <span
            class="mt-[7px] h-[18px] w-[18px] shrink-0 rounded-full border border-line"
            :style="{ backgroundColor: nextColor }"
          />
          <input
            v-model="newCategory"
            type="text"
            placeholder="Add a category…"
            aria-label="New category name"
            class="field min-w-0 flex-1 px-2 py-1.5 text-[11.5px]"
            @keydown.enter.prevent="addCategory"
          />
          <button
            class="btn btn-dark shrink-0 px-2.5 py-1.5 text-[11.5px]"
            :disabled="!newCategory.trim()"
            @click="addCategory"
          >
            Add
          </button>
        </div>
        <p v-if="categories.error" class="mt-1.5 text-[11px] text-danger">
          {{ categories.error }}
        </p>
      </section>

      <section class="mt-4">
        <h2 class="eyebrow">Coding agent</h2>

        <!-- Segmented switch: one agent plans and, later, writes the code. -->
        <div class="mt-1.5 flex gap-1 rounded-[11px] border border-line bg-line-soft/60 p-1">
          <button
            v-for="option in AGENTS"
            :key="option"
            class="flex-1 rounded-[8px] py-1.5 text-[11.5px] font-semibold transition-colors"
            :class="
              selected === option
                ? 'bg-solid text-on-solid'
                : 'text-ink-2 hover:text-ink'
            "
            @click="chooseAgent(option)"
          >
            {{ AGENT_LABELS[option] }}
          </button>
        </div>

        <div class="card mt-1.5 px-3 py-3">
          <div class="flex items-start gap-2.5">
            <span
              class="mt-[3px] flex h-4 w-4 shrink-0 items-center justify-center rounded-full"
              :class="agents.isReady ? 'bg-success text-white' : 'bg-warn-soft text-warn'"
            >
              <AppIcon :name="agents.isReady ? 'check' : 'close'" :size="10" :weight="2.6" />
            </span>
            <div class="min-w-0 flex-1">
              <p class="text-[12.5px] font-semibold">
                {{
                  !agents.status
                    ? "Checking…"
                    : !agents.status.installed
                      ? `${AGENT_LABELS[selected]} not found`
                      : agents.status.signedIn
                        ? `Signed in with ${agents.status.authMode}`
                        : `${AGENT_LABELS[selected]} is not signed in`
                }}
              </p>
              <p v-if="agents.status?.version" class="text-[11px] text-ink-2">
                v{{ agents.status.version }}
              </p>
              <p v-if="agents.status?.path" class="truncate text-[10.5px] text-ink-3">
                {{ shortenPath(agents.status.path) }}
              </p>
              <p v-if="agents.status?.problem" class="mt-1 text-[11px] text-ink-3">
                {{ agents.status.problem }}
              </p>
              <p
                v-else-if="agents.status?.installed && !agents.status.signedIn"
                class="mt-1 text-[11px] text-ink-3"
              >
                Run
                <span class="font-semibold text-ink-2">
                  {{ selected === "codex" ? "codex login" : "claude auth login" }}
                </span>
                in a terminal.
              </p>
            </div>
          </div>

          <div class="mt-2.5 flex gap-1.5">
            <button
              class="btn btn-ghost flex-1 py-1.5 text-[11.5px]"
              :disabled="agents.checking"
              @click="agents.checkStatus(selected, true)"
            >
              {{ agents.checking ? "Checking…" : "Check again" }}
            </button>
            <button
              class="btn btn-ghost flex-1 py-1.5 text-[11.5px]"
              @click="editingPath = !editingPath"
            >
              Set path
            </button>
          </div>

          <div v-if="editingPath" class="mt-2 flex gap-1.5">
            <input
              v-model="pathDraft"
              type="text"
              :placeholder="selected === 'codex' ? '/usr/local/bin/codex' : '/usr/local/bin/claude'"
              aria-label="Agent binary path"
              class="field min-w-0 flex-1 px-2 py-1.5 text-[11.5px]"
              @keydown.enter.prevent="savePath"
            />
            <button class="btn btn-dark shrink-0 px-2.5 py-1.5 text-[11.5px]" @click="savePath">
              Save
            </button>
          </div>

          <div v-if="agents.isReady" class="mt-2.5">
            <p class="eyebrow">Model</p>
            <div class="mt-1">
              <SelectMenu
                :model-value="configuredModel"
                :options="modelOptions"
                label="Model"
                @update:model-value="chooseModel"
              />
            </div>
            <button
              v-if="!agents.currentModels.length"
              class="mt-1 text-[11px] text-ink-3 underline"
              @click="agents.loadModels(selected)"
            >
              Load available models
            </button>
          </div>
        </div>
      </section>

      <section class="mt-4">
        <h2 class="eyebrow">Focus length</h2>
        <div class="mt-1.5 flex items-center gap-1.5">
          <button
            v-for="minutes in FOCUS_PRESETS"
            :key="minutes"
            class="tnum flex-1 rounded-[9px] border py-1.5 text-[11.5px] font-semibold transition-colors"
            :class="
              app.settings.focusMinutes === minutes
                ? 'border-solid bg-solid text-on-solid'
                : 'border-line bg-card text-ink-2 hover:bg-line-soft'
            "
            @click="app.updateSettings({ focusMinutes: minutes })"
          >
            {{ minutes }}m
          </button>
          <input
            :value="app.settings.focusMinutes"
            type="number"
            min="1"
            max="240"
            aria-label="Custom focus length in minutes"
            class="field tnum w-14 px-2 py-1.5 text-[11.5px]"
            @change="app.updateSettings({ focusMinutes: Number(($event.target as HTMLInputElement).value) })"
          />
        </div>
        <p class="mt-1.5 text-[11px] text-ink-2">
          Sessions keep running while the popup is closed.
        </p>
      </section>

      <section class="mt-4">
        <h2 class="eyebrow">Appearance</h2>
        <div class="mt-1.5 flex gap-1 rounded-[11px] border border-line bg-line-soft/60 p-1">
          <button
            v-for="option in THEMES"
            :key="option"
            class="flex-1 rounded-[8px] py-1.5 text-[11.5px] font-semibold transition-colors"
            :class="
              app.settings.theme === option
                ? 'bg-solid text-on-solid'
                : 'text-ink-2 hover:text-ink'
            "
            @click="app.updateSettings({ theme: option as Theme })"
          >
            {{ THEME_LABELS[option] }}
          </button>
        </div>
      </section>

      <section class="mt-4">
        <h2 class="eyebrow">Menu bar</h2>
        <div class="card mt-1.5 divide-y divide-line px-3">
          <ToggleSwitch
            :model-value="app.settings.showTimerInMenuBar"
            label="Show the countdown"
            hint="Otherwise only the icon is shown"
            @update:model-value="app.updateSettings({ showTimerInMenuBar: $event })"
          />
          <ToggleSwitch
            :model-value="app.settings.hidePopupOnBlur"
            label="Close when it loses focus"
            hint="Behaves like other menu-bar extras"
            @update:model-value="app.updateSettings({ hidePopupOnBlur: $event })"
          />
        </div>

        <div class="card mt-1.5 divide-y divide-line px-3">
          <ToggleSwitch
            :model-value="app.settings.notifications"
            label="Notify me"
            hint="When a session ends, a run finishes, or an agent needs you"
            @update:model-value="app.updateSettings({ notifications: $event })"
          />
          <ToggleSwitch
            :model-value="app.settings.sounds"
            label="Sound the alarm"
            hint="When a session runs out, a run finishes, or a task is done"
            @update:model-value="app.updateSettings({ sounds: $event })"
          />
          <ToggleSwitch
            :model-value="app.settings.launchAtLogin"
            label="Launch at login"
            hint="Crushit starts in the menu bar when you log in"
            @update:model-value="app.updateSettings({ launchAtLogin: $event })"
          />
        </div>

        <!-- macOS hides menu-bar items it cannot fit, so the hotkey is the
             reliable way in on a busy menu bar. -->
        <div class="card mt-1.5 flex items-center gap-2.5 px-3 py-2.5">
          <div class="min-w-0 flex-1">
            <p class="text-[12.5px] font-semibold">Open with a shortcut</p>
            <p class="mt-px text-[11px] leading-snug text-ink-2">
              {{
                shortcut?.combo
                  ? "Works even when the menu bar has no room for the icon."
                  : (shortcut?.problem ?? "Checking…")
              }}
            </p>
          </div>
          <span v-if="shortcut?.combo" class="chip shrink-0 font-semibold">
            {{ shortcut.combo }}
          </span>
        </div>
      </section>

      <section class="mt-4">
        <h2 class="eyebrow">Shortcuts</h2>
        <div class="card mt-1.5 divide-y divide-line px-3">
          <div
            v-for="shortcut in SHORTCUTS"
            :key="shortcut.combo"
            class="flex items-center justify-between gap-2 py-1.5"
          >
            <span class="text-[12px]">{{ shortcut.label }}</span>
            <span class="chip shrink-0 font-semibold">{{ shortcut.combo }}</span>
          </div>
          <div class="flex items-center justify-between gap-2 py-1.5">
            <span class="text-[12px]">Back, or close</span>
            <span class="chip shrink-0 font-semibold">Esc</span>
          </div>
        </div>
      </section>

      <section class="mt-4 border-t border-line pt-3">
        <p class="text-[11px] leading-relaxed text-ink-3">
          Running tasks with an agent arrives in the next phase.
        </p>
        <button
          class="btn btn-ghost mt-2 w-full py-2 text-ink-2 hover:text-danger"
          @click="ipc.quitApp()"
        >
          Quit Crushit
        </button>
      </section>
    </div>
  </div>
</template>
