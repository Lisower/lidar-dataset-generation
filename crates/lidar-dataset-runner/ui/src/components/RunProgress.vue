<script setup lang="ts">
import { computed } from "vue";
import type { ClassDto, ProgressEvent } from "../api";
import type { RunState } from "../lib/labels";
import { etaMs, formatDuration, formatPoints, percent, pluralRu } from "../lib/format";
import CancelButton from "./CancelButton.vue";

const props = defineProps<{
  progress: ProgressEvent | null;
  scenesTotal: number;
  classes: ClassDto[];
  running: boolean;
  state?: RunState;
}>();

const emit = defineEmits<{ (e: "cancel"): void }>();

const done = computed(() => props.progress?.scenesDone ?? 0);
const total = computed(() => props.progress?.scenesTotal ?? props.scenesTotal);
const elapsedMs = computed(() => props.progress?.elapsedMs ?? 0);

const percentDone = computed(() => percent(done.value, total.value));

const pointsPerSecond = computed(() => {
  if (!props.progress || props.progress.elapsedMs <= 0) {
    return 0;
  }
  return Math.round((props.progress.pointsTotal / props.progress.elapsedMs) * 1000);
});

const eta = computed(() => etaMs(elapsedMs.value, done.value, total.value));

const scenesWord = computed(() => pluralRu(total.value, ["сцена", "сцены", "сцен"]));

function colorFor(code: number): string {
  return props.classes.find((item) => item.code === code)?.color ?? "var(--muted)";
}

function labelFor(code: number): string {
  const found = props.classes.find((item) => String(item.code) === String(code));
  return found ? found.label : `класс ${code}`;
}

const classRows = computed(() => {
  const byClass = props.progress?.last.byClass ?? {};
  return Object.entries(byClass)
    .map(([code, count]) => ({ code: Number(code), count }))
    .sort((a, b) => b.count - a.count);
});
</script>

<template>
  <section class="card panel">
    <h2>Прогресс</h2>

    <div
      class="bar"
      role="progressbar"
      aria-label="Записано сцен"
      aria-valuemin="0"
      aria-valuemax="100"
      :aria-valuenow="percentDone"
    >
      <div class="fill" :style="{ width: percentDone + '%' }" />
    </div>

    <div class="facts numeric">
      <span>{{ done }} из {{ total }} {{ scenesWord }}</span>
      <span>{{ formatPoints(progress?.pointsTotal ?? 0) }} точек</span>
      <span>{{ formatPoints(pointsPerSecond) }} точек в секунду</span>
      <span>прошло {{ formatDuration(elapsedMs) }}</span>
    </div>

    <p v-if="eta !== null && running" class="estimate">
      Осталось примерно {{ formatDuration(eta) }}
    </p>
    <p v-else-if="running" class="estimate muted">Считаем первую сцену…</p>
    <p v-else-if="progress && state === 'cancelled'" class="estimate">Остановлено</p>
    <p v-else-if="progress && state === 'failed'" class="estimate">Запуск прерван</p>
    <p v-else-if="progress" class="estimate">Готово за {{ formatDuration(elapsedMs) }}</p>

    <div v-if="running && !progress" class="first-scene">
      <p>Готовим первую сцену: читаем пресеты и считаем рельеф</p>
      <p class="muted">Обычно это занимает несколько секунд</p>
    </div>

    <ul v-if="classRows.length" class="classes">
      <li v-for="row in classRows" :key="row.code">
        <span class="dot" :style="{ background: colorFor(row.code) }" />
        <span class="name">{{ labelFor(row.code) }}</span>
        <span class="count numeric">{{ formatPoints(row.count) }}</span>
      </li>
    </ul>

    <div class="actions">
      <CancelButton :running="running" @cancel="emit('cancel')" />
    </div>
  </section>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.bar {
  height: 8px;
  background: var(--surface-2);
  border-radius: 999px;
  overflow: hidden;
}

.fill {
  height: 100%;
  background: var(--accent);
  transition: width 0.2s ease;
}

.facts {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-3);
  font-size: 13px;
  color: var(--muted);
}

.estimate {
  font-size: 13px;
}

.first-scene {
  font-size: 13px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.first-scene .muted {
  font-size: 12px;
}

.classes {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  font-size: 13px;
}

.classes li {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
}

.name {
  flex: 1;
  color: var(--muted);
}

.actions {
  display: flex;
  justify-content: flex-end;
}
</style>
