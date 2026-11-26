<script setup lang="ts">
import { onUnmounted, ref, watch } from "vue";

defineOptions({ name: "CancelButton" });

const props = defineProps<{ running: boolean }>();

const emit = defineEmits<{ (e: "cancel"): void }>();

const armed = ref(false);
let timer: ReturnType<typeof setTimeout> | null = null;

function disarm() {
  if (timer !== null) {
    clearTimeout(timer);
    timer = null;
  }
  armed.value = false;
}

function onClick() {
  if (!props.running) {
    return;
  }
  if (!armed.value) {
    armed.value = true;
    timer = setTimeout(() => {
      timer = null;
      armed.value = false;
    }, 4000);
    return;
  }
  disarm();
  emit("cancel");
}

watch(
  () => props.running,
  (running) => {
    if (!running) {
      disarm();
    }
  },
);

onUnmounted(disarm);
</script>

<template>
  <button type="button" class="btn btn-danger" :disabled="!running" @click="onClick">
    {{ armed ? "Точно остановить?" : "Остановить" }}
  </button>
</template>
