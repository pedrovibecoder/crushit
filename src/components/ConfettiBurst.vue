<script setup lang="ts">
/**
 * A short confetti shower over the popup. Purely decorative, so it is hidden
 * from assistive tech and skipped entirely when the system asks for reduced
 * motion — the congratulation text carries the meaning on its own.
 */
const COUNT = 46;
const COLORS = ["#7b61ff", "#3fbf6a", "#f2c744", "#ff8ec2", "#4a9bf5", "#ffa45c"];

const pick = <T,>(items: T[]) => items[Math.floor(Math.random() * items.length)];
const between = (low: number, high: number) => low + Math.random() * (high - low);

const pieces = Array.from({ length: COUNT }, (_, index) => ({
  id: index,
  style: {
    "--x": `${between(-4, 104)}%`,
    "--dx": `${between(-40, 40)}px`,
    "--dy": `${between(320, 520)}px`,
    "--rot": `${between(-540, 540)}deg`,
    "--w": `${between(4, 8)}px`,
    "--h": `${between(7, 13)}px`,
    "--c": pick(COLORS),
    "--delay": `${between(0, 420)}ms`,
    "--duration": `${between(1100, 1900)}ms`,
  } as Record<string, string>,
}));
</script>

<template>
  <div class="pointer-events-none absolute inset-0 overflow-hidden" aria-hidden="true">
    <span v-for="piece in pieces" :key="piece.id" class="piece" :style="piece.style" />
  </div>
</template>

<style scoped>
.piece {
  position: absolute;
  top: -14px;
  left: var(--x);
  width: var(--w);
  height: var(--h);
  background: var(--c);
  border-radius: 1px;
  opacity: 0;
  animation: fall var(--duration) cubic-bezier(0.25, 0.6, 0.4, 1) var(--delay) forwards;
}

@keyframes fall {
  0% {
    opacity: 1;
    transform: translate3d(0, 0, 0) rotate(0deg);
  }
  85% {
    opacity: 1;
  }
  100% {
    opacity: 0;
    transform: translate3d(var(--dx), var(--dy), 0) rotate(var(--rot));
  }
}

@media (prefers-reduced-motion: reduce) {
  .piece {
    display: none;
  }
}
</style>
