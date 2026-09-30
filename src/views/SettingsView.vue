<script setup lang="ts">
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
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
import { useTagsStore } from "../stores/tags";
import { useSlackStore } from "../stores/slack";
import {
  AGENTS,
  AGENT_LABELS,
  CATEGORY_COLORS,
  THEMES,
  THEME_LABELS,
  type Agent,
  type BlockPermission,
  type ShortcutInfo,
  type Theme,
} from "../types";

const app = useAppStore();
const agents = useAgentStore();
const categories = useCategoriesStore();
const tags = useTagsStore();
const slack = useSlackStore();

const FOCUS_PRESETS = [15, 25, 45, 60];

/**
 * The blocklist is edited as text rather than as rows: it is a list of
 * domains, and typing one per line is faster than any control that could be
 * built around it. It is written back on blur, so a half-typed domain is
 * never saved out from under the cursor.
 */
const sitesDraft = ref("");
const blockPermission = ref<BlockPermission | null>(null);
const checkingBlock = ref(false);

watch(
  () => app.settings.focusBlockSites,
  (sites) => { sitesDraft.value = (sites ?? []).join("\n") },
  { immediate: true },
);

async function saveSites() {
  if (sitesDraft.value === (app.settings.focusBlockSites ?? []).join("\n")) return;
  await app.updateSettings({ focusBlockSites: sitesDraft.value });
}

/**
 * macOS decides whether one app may drive another, and refuses silently. The
 * only honest way to report that is to try it and say what came back.
 */
async function checkBlockPermission() {
  checkingBlock.value = true;
  try {
    blockPermission.value = await ipc.checkFocusBlock();
  } catch {
    blockPermission.value = "denied";
  } finally {
    checkingBlock.value = false;
  }
}

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

const newTag = ref("");
/** Which tag row has its palette open. */
const recolouringTag = ref<number | null>(null);

const nextTagColor = computed(
  () => CATEGORY_COLORS[tags.tags.length % CATEGORY_COLORS.length],
);

async function addTag() {
  const label = newTag.value.trim();
  if (!label) return;
  if (await tags.create(label, nextTagColor.value)) newTag.value = "";
}

async function renameTag(id: number, event: Event) {
  const input = event.target as HTMLInputElement;
  const label = input.value.trim();
  const current = tags.tags.find((tag) => tag.id === id);
  if (!current) return;
  if (!label || label === current.label) {
    input.value = current.label;
    return;
  }
  await tags.update(id, { label });
}

async function recolourTag(id: number, color: string) {
  recolouringTag.value = null;
  await tags.update(id, { color });
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

const slackToken = ref("");
const slackBusy = ref(false);
const slackCopied = ref(false);

/**
 * Slack can build the whole app from this, so setting it up is one paste
 * rather than hunting for the right scopes. Only user scopes: the point is to
 * post as you, which a bot token cannot do.
 */
const SLACK_MANIFEST = `display_information:
  name: Crushit
oauth_config:
  scopes:
    user:
      - im:history
      - users:read
      - chat:write
settings:
  org_deploy_enabled: false
  socket_mode_enabled: false
  token_rotation_enabled: false`;

async function copyManifest(event: Event) {
  const field = (event.currentTarget as HTMLElement)
    .closest("section")
    ?.querySelector("textarea");
  try {
    await navigator.clipboard.writeText(SLACK_MANIFEST);
    slackCopied.value = true;
    setTimeout(() => (slackCopied.value = false), 2000);
  } catch {
    // Selecting it is the fallback when the clipboard is not available.
    field?.select();
  }
}

async function openSlackApps() {
  try {
    await openUrl("https://api.slack.com/apps");
  } catch {
    // Nothing to do if no browser will open; the address is on screen anyway.
  }
}

async function connectSlack() {
  slackBusy.value = true;
  const connected = await slack.connect(slackToken.value.trim() || null);
  slackBusy.value = false;
  if (connected) {
    slackToken.value = "";
    await app.updateSettings({});
  }
}

async function disconnectSlack() {
  slackBusy.value = true;
  await slack.connect(null);
  slackBusy.value = false;
  await app.updateSettings({});
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
        <h2 class="eyebrow">Tags</h2>
        <p class="mt-1 text-[11px] leading-relaxed text-ink-2">
          Which part of the work a task touches. A task can carry several, or none.
        </p>

        <ul class="mt-1.5 space-y-1">
          <li v-for="tag in tags.tags" :key="tag.id" class="card px-2 py-1.5">
            <div class="flex items-center gap-2">
              <button
                class="h-[18px] w-[18px] shrink-0 rounded-full border border-line"
                :style="{ backgroundColor: tag.color }"
                :aria-label="`Change the colour of ${tag.label}`"
                :title="`Change the colour of ${tag.label}`"
                @click="recolouringTag = recolouringTag === tag.id ? null : tag.id"
              />
              <input
                :value="tag.label"
                class="min-w-0 flex-1 rounded-[6px] border border-transparent bg-transparent px-1 py-0.5 text-[12.5px] font-semibold outline-none hover:border-line focus:border-ink-3"
                :aria-label="`Rename ${tag.label}`"
                @blur="renameTag(tag.id, $event)"
                @keydown.enter.prevent="($event.target as HTMLInputElement).blur()"
              />
              <button
                class="shrink-0 text-ink-3 transition-colors hover:text-danger"
                :aria-label="`Delete ${tag.label}`"
                :title="`Delete ${tag.label} — the tasks that carry it keep everything else`"
                @click="tags.remove(tag.id)"
              >
                <AppIcon name="close" :size="12" />
              </button>
            </div>

            <div v-if="recolouringTag === tag.id" class="mt-1.5 flex flex-wrap gap-1 pl-[26px]">
              <button
                v-for="color in CATEGORY_COLORS"
                :key="color"
                class="h-[18px] w-[18px] rounded-full border transition-transform hover:scale-110"
                :class="color === tag.color ? 'border-ink' : 'border-line'"
                :style="{ backgroundColor: color }"
                :aria-label="`Use this colour for ${tag.label}`"
                @click="recolourTag(tag.id, color)"
              />
            </div>
          </li>
        </ul>

        <div class="mt-1.5 flex gap-1.5">
          <span
            class="mt-[7px] h-[18px] w-[18px] shrink-0 rounded-full border border-line"
            :style="{ backgroundColor: nextTagColor }"
          />
          <input
            v-model="newTag"
            type="text"
            placeholder="Add a tag…"
            aria-label="New tag name"
            class="field min-w-0 flex-1 px-2 py-1.5 text-[11.5px]"
            @keydown.enter.prevent="addTag"
          />
          <button
            class="btn btn-dark shrink-0 px-2.5 py-1.5 text-[11.5px]"
            :disabled="!newTag.trim()"
            @click="addTag"
          >
            Add
          </button>
        </div>
        <p v-if="tags.error" class="mt-1.5 text-[11px] text-danger">{{ tags.error }}</p>
      </section>

      <section class="mt-4">
        <h2 class="eyebrow">Coding agent</h2>

        <!-- Segmented switch: one agent plans and, later, writes the code. -->
        <div class="segmented mt-1.5 flex w-full">
          <button
            v-for="option in AGENTS"
            :key="option"
            class="segmented-item flex-1 py-1.5"
            :class="selected === option && 'is-selected'"
            :aria-pressed="selected === option"
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
        <h2 class="eyebrow">Slack</h2>
        <p class="mt-1 text-[11px] leading-relaxed text-ink-2">
          Shows the direct messages waiting on you, and drafts a reply from your
          task list and repository. Nothing is sent until you press Send, and it
          goes out under your own name.
        </p>

        <div class="card mt-1.5 px-3 py-3">
          <template v-if="app.settings.slackConnected">
            <p class="flex items-center gap-1.5 text-[12.5px] font-semibold">
              <AppIcon name="check" :size="12" :weight="2.4" class="text-success" />
              Connected{{ slack.account ? ` as ${slack.account.user}` : "" }}
            </p>
            <p v-if="slack.account" class="mt-0.5 text-[11px] text-ink-2">
              {{ slack.account.team }}
            </p>
            <button
              class="btn btn-ghost mt-2 w-full py-1.5 text-[11.5px] text-ink-2 hover:text-danger"
              :disabled="slackBusy"
              @click="disconnectSlack"
            >
              Disconnect
            </button>
          </template>

          <template v-else>
            <ol class="space-y-1 text-[11.5px] leading-relaxed text-ink-2">
              <li>
                <span class="font-semibold text-ink">1.</span> Copy this app manifest
                and create a Slack app from it — <em>Create New App → From a manifest</em>.
              </li>
              <li>
                <span class="font-semibold text-ink">2.</span> Install it to your
                workspace, then copy the <em>User OAuth Token</em>.
              </li>
              <li><span class="font-semibold text-ink">3.</span> Paste it below.</li>
            </ol>

            <textarea
              :value="SLACK_MANIFEST"
              readonly
              rows="5"
              aria-label="Slack app manifest"
              class="field mt-2 w-full resize-none px-2 py-1.5 font-mono text-[10.5px] leading-snug"
            />
            <div class="mt-1.5 flex gap-1.5">
              <button class="btn btn-ghost flex-1 py-1.5 text-[11.5px]" @click="copyManifest">
                {{ slackCopied ? "Copied" : "Copy manifest" }}
              </button>
              <button class="btn btn-ghost flex-1 py-1.5 text-[11.5px]" @click="openSlackApps">
                Open Slack apps
              </button>
            </div>
            <div class="mt-2 flex gap-1.5">
              <input
                v-model="slackToken"
                type="password"
                placeholder="xoxp-…"
                aria-label="Slack user token"
                class="field min-w-0 flex-1 px-2 py-1.5 text-[11.5px]"
                @keydown.enter.prevent="connectSlack"
              />
              <button
                class="btn btn-dark shrink-0 px-2.5 py-1.5 text-[11.5px]"
                :disabled="!slackToken.trim() || slackBusy"
                @click="connectSlack"
              >
                {{ slackBusy ? "Checking…" : "Connect" }}
              </button>
            </div>
          </template>

          <p v-if="slack.error" class="mt-1.5 text-[11px] text-danger">{{ slack.error }}</p>
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

      <!-- Sitting under Focus length because it only ever applies to a running
           session: nothing here changes what the browser does otherwise. -->
      <section class="mt-4">
        <h2 class="eyebrow">Distractions</h2>
        <div class="card mt-1.5 px-3">
          <ToggleSwitch
            :model-value="app.settings.focusBlockEnabled"
            label="Block these sites while focusing"
            hint="A tab on one of them is sent to a holding page until the session ends"
            @update:model-value="app.updateSettings({ focusBlockEnabled: $event })"
          />
        </div>

        <template v-if="app.settings.focusBlockEnabled">
          <textarea
            v-model="sitesDraft"
            rows="5"
            spellcheck="false"
            aria-label="Blocked sites"
            placeholder="instagram.com"
            class="field mt-1.5 w-full resize-none px-2.5 py-2 text-[11.5px] leading-relaxed"
            @blur="saveSites"
          />
          <p class="mt-1 text-[11px] text-ink-2">
            One site per line. Subdomains are included, so
            <span class="font-semibold text-ink">facebook.com</span> also covers
            m.facebook.com.
          </p>

          <!-- Blocking depends on a permission macOS grants per app, and a
               refusal is silent — so there is a way to ask outright. -->
          <div class="card mt-1.5 flex items-center gap-2.5 px-3 py-2.5">
            <div class="min-w-0 flex-1">
              <p class="text-[12.5px] font-semibold">Browser access</p>
              <p
                class="mt-px text-[11px] leading-snug"
                :class="blockPermission === 'denied' ? 'text-danger' : 'text-ink-2'"
              >
                <template v-if="blockPermission === 'granted'">
                  Working — your browsers answer when asked.
                </template>
                <template v-else-if="blockPermission === 'denied'">
                  macOS is refusing. Allow Crushit under Privacy &amp; Security →
                  Automation, then check again.
                </template>
                <template v-else-if="blockPermission === 'unknown'">
                  No browser was open to ask. Open one and check again.
                </template>
                <template v-else>
                  macOS asks for permission the first time a session blocks a site.
                </template>
              </p>
            </div>
            <button
              class="btn btn-ghost shrink-0 px-3 py-1.5 text-[11.5px]"
              :disabled="checkingBlock"
              @click="checkBlockPermission"
            >
              {{ checkingBlock ? "Checking…" : "Check" }}
            </button>
          </div>
        </template>
      </section>

      <section class="mt-4">
        <h2 class="eyebrow">Appearance</h2>
        <div class="segmented mt-1.5 flex w-full">
          <button
            v-for="option in THEMES"
            :key="option"
            class="segmented-item flex-1 py-1.5"
            :class="app.settings.theme === option && 'is-selected'"
            :aria-pressed="app.settings.theme === option"
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
