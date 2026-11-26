<script setup lang="ts">
import { computed } from "vue";
import type { PresetDto } from "../api";

const props = withDefaults(
  defineProps<{
    id: string;
    presets: PresetDto[];
    modelValue: string;
    disabled?: boolean;
    emptyText?: string;
    describedBy?: string;
  }>(),
  { disabled: false, emptyText: "нет доступных пресетов", describedBy: undefined },
);

const emit = defineEmits<{ (e: "update:modelValue", value: string): void }>();

const description = computed(
  () => props.presets.find((preset) => preset.id === props.modelValue)?.description ?? "",
);

function onChange(event: Event) {
  emit("update:modelValue", (event.target as HTMLSelectElement).value);
}
</script>

<template>
  <div class="preset">
    <p v-if="presets.length === 0" :id="id" class="preset-empty muted" tabindex="-1">
      {{ emptyText }}
    </p>
    <template v-else>
      <select
        :id="id"
        :disabled="disabled"
        :aria-describedby="describedBy"
        :value="modelValue"
        @change="onChange"
      >
        <option v-for="preset in presets" :key="preset.id" :value="preset.id">
          {{ preset.name }}
        </option>
      </select>
      <p v-if="description" class="preset-description">{{ description }}</p>
    </template>
  </div>
</template>

<style scoped>
.preset {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

select {
  width: 100%;
}

.preset-description {
  font-size: 12px;
  color: var(--muted);
}

.preset-empty {
  font-size: 12px;
  margin: 0;
}
</style>
