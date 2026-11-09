<script setup lang="ts">
import { onMounted, ref } from "vue";
import type { ValidationIssue } from "../api";
import { pluralRu } from "../lib/format";

const props = defineProps<{ issues: ValidationIssue[] }>();

const emit = defineEmits<{ (e: "focus-field", field: string): void }>();

const root = ref<HTMLElement | null>(null);

onMounted(() => {
  root.value?.scrollIntoView?.({ behavior: "smooth", block: "nearest" });
});

function plural(n: number): string {
  return pluralRu(n, ["ошибка в форме", "ошибки в форме", "ошибок в форме"]);
}

function focus(field: string) {
  emit("focus-field", field);
}
</script>

<template>
  <div
    v-if="issues.length"
    ref="root"
    class="validation-summary"
    role="alert"
    aria-labelledby="validation-summary-title"
  >
    <p id="validation-summary-title" class="title">
      {{ issues.length }} {{ plural(issues.length) }}
    </p>
    <ul>
      <li v-for="(issue, index) in issues" :key="`${issue.field}-${index}`">
        <button type="button" @click="focus(issue.field)">{{ issue.message }}</button>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.validation-summary {
  background: var(--danger-surface);
  border: 1px solid var(--danger);
  border-radius: var(--radius);
  padding: var(--space-3) var(--space-4);
}

.title {
  font-weight: 600;
  color: var(--danger);
  margin-bottom: var(--space-1);
}

ul {
  margin: 0;
  padding-left: var(--space-4);
  display: flex;
  flex-direction: column;
  gap: 2px;
}

button {
  background: none;
  border: none;
  padding: 0;
  color: var(--text);
  text-align: left;
  cursor: pointer;
  text-decoration: underline;
  text-decoration-color: var(--danger);
}

button:hover {
  color: var(--danger);
}
</style>
