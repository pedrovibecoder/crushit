import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { errorMessage, ipc } from "../lib/ipc";
import type { SlackAccount, WaitingConversation } from "../types";

/**
 * Slack, as far as this app is concerned: who is waiting on you, and a reply
 * drafted for you to look at. Nothing is ever sent without `send` being called
 * from a button.
 */
export const useSlackStore = defineStore("slack", () => {
  const account = ref<SlackAccount | null>(null);
  const waiting = ref<WaitingConversation[]>([]);
  const loading = ref(false);
  const drafting = ref(false);
  const sending = ref(false);
  const error = ref<string | null>(null);

  /** Which conversation is open, and the reply being worked on for it. */
  const openId = ref<string | null>(null);
  const draft = ref("");

  const count = computed(() => waiting.value.length);
  const open = computed(
    () => waiting.value.find((conversation) => conversation.id === openId.value) ?? null,
  );

  async function run<T>(action: () => Promise<T>): Promise<T | null> {
    try {
      error.value = null;
      return await action();
    } catch (caught) {
      error.value = errorMessage(caught);
      return null;
    }
  }

  async function connect(token: string | null) {
    const connected = await run(() => ipc.connectSlack(token));
    account.value = connected ?? null;
    if (!connected) waiting.value = [];
    return connected !== null;
  }

  async function load() {
    loading.value = true;
    const found = await run(ipc.slackWaiting);
    if (found) waiting.value = found;
    loading.value = false;
  }

  async function refreshAccount() {
    account.value = (await run(ipc.slackAccount)) ?? null;
  }

  function openConversation(id: string | null) {
    openId.value = id;
    draft.value = "";
  }

  async function writeDraft() {
    if (!openId.value) return;
    drafting.value = true;
    const written = await run(() => ipc.draftSlackReply(openId.value as string));
    if (written) draft.value = written;
    drafting.value = false;
  }

  /** The only path that puts anything into Slack. */
  async function send() {
    if (!openId.value || !draft.value.trim()) return false;
    sending.value = true;
    const sent = await run(() => ipc.sendSlackReply(openId.value as string, draft.value));
    sending.value = false;
    if (sent === null) return false;
    openConversation(null);
    await load();
    return true;
  }

  return {
    account,
    waiting,
    loading,
    drafting,
    sending,
    error,
    openId,
    draft,
    count,
    open,
    connect,
    load,
    refreshAccount,
    openConversation,
    writeDraft,
    send,
  };
});
