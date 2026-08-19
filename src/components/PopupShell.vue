<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { ipc } from "../lib/ipc";
import { useAppStore } from "../stores/app";
import CelebrationOverlay from "./CelebrationOverlay.vue";
import ErrorBanner from "./ErrorBanner.vue";

defineProps<{ current: unknown; ready: boolean }>();

const app = useAppStore();

const shell = ref<HTMLElement | null>(null);
let observer: ResizeObserver | undefined;

onMounted(() => {
  if (!shell.value) return;
  // Keep the OS window exactly as tall as the rendered popup. Only the height
  // last requested is skipped: remembering more refuses legitimate repeats,
  // such as returning to a screen that was already this tall.
  let requested = 0;
  observer = new ResizeObserver(([entry]) => {
    const height = Math.ceil(
      entry.borderBoxSize?.[0]?.blockSize ?? (entry.target as HTMLElement).offsetHeight,
    );
    if (height === 0 || height === requested) return;
    requested = height;
    void ipc.resizePopup(height);
  });
  observer.observe(shell.value);
});

onBeforeUnmount(() => observer?.disconnect());
</script>

<template>
  <div ref="shell" class="w-full">
    <div
      class="relative flex min-h-[228px] w-full flex-col overflow-hidden rounded-[14px] border border-line bg-page"
    >
      <ErrorBanner />
      <div
        v-if="!ready"
        class="flex h-[228px] items-center justify-center text-[12px] text-ink-3"
      >
        Loading…
      </div>
      <!-- Startup itself failed; offer the one action that can help. -->
      <div v-else-if="app.failed" class="flex flex-col items-center gap-2 px-6 py-10 text-center">
        <p class="text-[12.5px] font-semibold">Blitzit could not start up</p>
        <p class="text-[11.5px] leading-relaxed text-ink-2">{{ app.error }}</p>
        <button class="btn btn-dark mt-1 px-4 py-2" @click="app.bootstrap()">Try again</button>
      </div>
      <Transition v-else name="screen" mode="out-in">
        <component :is="current" :key="app.view" />
      </Transition>
      <CelebrationOverlay />
    </div>
  </div>
</template>
