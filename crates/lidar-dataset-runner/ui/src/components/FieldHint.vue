<script setup lang="ts">
defineProps<{
  id?: string;
  text: string;
  error?: string;
}>();
</script>

<template>
  <p v-if="error" :id="id" class="hint hint-error" role="alert">{{ error }}</p>
  <p v-else :id="id" class="hint">{{ text }}</p>
</template>

<style scoped>
.hint {
  margin: 0;
  font-size: 12px;
  color: var(--muted);
}

.hint-error {
  color: var(--danger);
}
</style>
