<script setup lang="ts">
import { computed, onMounted } from "vue";
import AppIcon from "../components/AppIcon.vue";
import PanelHeader from "../components/PanelHeader.vue";
import { formatSpan } from "../lib/format";
import { useAppStore } from "../stores/app";
import { useSlackStore } from "../stores/slack";

const app = useAppStore();
const slack = useSlackStore();

onMounted(() => {
  if (app.settings.slackConnected) {
    if (!slack.account) void slack.refreshAccount();
    void slack.load();
  }
});

/** The last thing they said, which is what a reply has to answer. */
const asked = computed(() => {
  const messages = slack.open?.messages ?? [];
  return [...messages].reverse().find((message) => !message.isMine)?.text ?? "";
});
</script>

<template>
  <div class="flex flex-col">
    <PanelHeader
      eyebrow="Slack"
      :title="slack.open ? slack.open.with : 'Waiting on you'"
      :back="!app.isDesktop || !!slack.open"
      @back="slack.open ? slack.openConversation(null) : app.back()"
    >
      <template v-if="!slack.open" #actions>
        <button
          class="icon-btn h-7 w-7"
          :disabled="slack.loading"
          aria-label="Check Slack"
          title="Check Slack"
          @click="slack.load()"
        >
          <AppIcon name="refresh" :size="14" :class="slack.loading && 'animate-spin'" />
        </button>
      </template>
    </PanelHeader>

    <div class="panel-scroll px-3.5 pb-3.5">
      <!-- Not connected yet -->
      <template v-if="!app.settings.slackConnected">
        <div class="card px-3 py-3">
          <p class="text-[12.5px] font-semibold">Slack is not connected</p>
          <p class="mt-1 text-[11.5px] leading-relaxed text-ink-2">
            Connect it in Settings and Crushit will show the direct messages waiting
            on you, and draft a reply you can edit before it is sent.
          </p>
          <button class="btn btn-dark mt-2.5 w-full py-2" @click="app.go('settings')">
            Open Settings
          </button>
        </div>
      </template>

      <!-- One conversation, with the reply being written -->
      <template v-else-if="slack.open">
        <ul class="space-y-1.5">
          <li
            v-for="message in slack.open.messages"
            :key="message.ts"
            class="card px-2.5 py-2"
            :class="message.isMine && 'border-accent/30 bg-accent-soft/40'"
          >
            <p class="text-[10.5px] font-semibold text-ink-3">
              {{ message.isMine ? "You" : slack.open.with }}
            </p>
            <p class="mt-0.5 text-[12.5px] leading-snug break-words">{{ message.text }}</p>
          </li>
        </ul>

        <p class="eyebrow mt-3">Your reply</p>
        <textarea
          v-model="slack.draft"
          rows="5"
          placeholder="Write a reply, or have one drafted."
          class="field mt-1.5 w-full resize-none px-2.5 py-2 text-[12.5px] leading-snug"
        />

        <button
          class="btn btn-ghost mt-1.5 w-full py-1.5 text-[11.5px]"
          :disabled="slack.drafting"
          @click="slack.writeDraft()"
        >
          <AppIcon name="sparkle" :size="11" filled />
          {{ slack.drafting ? "Reading the project…" : slack.draft ? "Draft again" : "Draft a reply" }}
        </button>

        <p class="mt-2 text-[10.5px] leading-relaxed text-ink-3">
          This goes out from your account, under your name. Read it first — it is
          written from your task list and your repository, and it can be wrong.
        </p>
      </template>

      <!-- Everything waiting -->
      <template v-else>
        <p v-if="slack.account" class="text-[11px] text-ink-2">
          Posting as <span class="font-semibold text-ink">{{ slack.account.user }}</span>
          in {{ slack.account.team }}.
        </p>

        <ul class="mt-2 space-y-1.5">
          <li v-for="conversation in slack.waiting" :key="conversation.id">
            <button
              class="card flex w-full items-start gap-2.5 px-2.5 py-2.5 text-left transition-colors hover:bg-line-soft/70"
              @click="slack.openConversation(conversation.id)"
            >
              <span class="min-w-0 flex-1">
                <span class="flex items-baseline justify-between gap-2">
                  <span class="truncate text-[12.5px] font-semibold">{{ conversation.with }}</span>
                  <span class="tnum shrink-0 text-[10.5px] text-ink-3">
                    {{ formatSpan(conversation.waitingSeconds) }}
                  </span>
                </span>
                <span class="mt-0.5 line-clamp-2 text-[11.5px] leading-snug text-ink-2">
                  {{ conversation.messages[conversation.messages.length - 1]?.text }}
                </span>
              </span>
              <AppIcon name="forward" :size="13" class="mt-1 shrink-0 text-ink-3" />
            </button>
          </li>
        </ul>

        <p
          v-if="!slack.loading && !slack.waiting.length"
          class="px-2 py-6 text-center text-[12px] leading-relaxed text-ink-3"
        >
          Nothing waiting — every direct message has your answer as its last word.
        </p>
      </template>

      <p v-if="slack.error" class="mt-2 text-[11px] text-danger">{{ slack.error }}</p>
    </div>

    <footer v-if="slack.open" class="flex items-center gap-2 border-t border-line px-3.5 py-2.5">
      <button class="btn btn-ghost px-3 py-2" @click="slack.openConversation(null)">
        Cancel
      </button>
      <button
        class="btn btn-dark flex-1 py-2"
        :disabled="!slack.draft.trim() || slack.sending"
        :title="asked ? `Replying to: ${asked}` : undefined"
        @click="slack.send()"
      >
        <AppIcon name="forward" :size="13" />
        {{ slack.sending ? "Sending…" : "Send as me" }}
      </button>
    </footer>
  </div>
</template>
