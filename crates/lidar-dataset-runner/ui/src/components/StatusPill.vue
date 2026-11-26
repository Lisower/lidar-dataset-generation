<script setup lang="ts">
import { stateLabel, type RunState } from "../lib/labels";

defineProps<{ state: RunState }>();
</script>

<template>
  <span class="pill" role="status" aria-live="polite" :data-state="state">
    {{ stateLabel(state) }}
  </span>
</template>

<style scoped>
.pill {
  font-size: 12px;
  color: var(--muted);
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 2px 10px;
  white-space: nowrap;
}

.pill[data-state="running"] {
  color: var(--accent);
  border-color: var(--accent);
}

.pill[data-state="finished"] {
  color: var(--ok);
  border-color: var(--ok);
}

.pill[data-state="failed"],
.pill[data-state="cancelled"] {
  color: var(--danger);
  border-color: var(--danger);
}
</style>
