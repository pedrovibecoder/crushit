<script setup lang="ts">
import { useTagsStore } from "../stores/tags";

defineProps<{ slug: string }>();

const tags = useTagsStore();
</script>

<template>
  <!-- Tinted rather than dotted, so a row of tags reads apart from the single
       category pill sitting next to it. -->
  <span
    class="chip font-semibold"
    :style="{
      color: tags.colorFor(slug),
      borderColor: `color-mix(in oklab, ${tags.colorFor(slug)} 35%, transparent)`,
      backgroundColor: `color-mix(in oklab, ${tags.colorFor(slug)} 12%, transparent)`,
    }"
  >
    {{ tags.labelFor(slug) }}
  </span>
</template>
