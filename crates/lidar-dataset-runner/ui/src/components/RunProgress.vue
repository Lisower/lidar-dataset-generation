<script setup lang="ts">
import { computed } from "vue";
import type { ClassDto, ProgressEvent } from "../api";

const props = defineProps<{
  progress: ProgressEvent | null;
  scenesTotal: number;
  classes: ClassDto[];
  running: boolean;
}>();

const percent = computed(() => {
  const total = props.progress?.scenesTotal ?? props.scenesTotal;
  if (!total) {
    return 0;
  }
  const done = props.progress?.scenesDone ?? 0;
  return Math.min(100, Math.round((done / total) * 100));
});

const pointsPerSecond = computed(() => {
  if (!props.progress || props.progress.elapsedMs <= 0) {
    return 0;
  }
  return Math.round((props.progress.pointsTotal / props.progress.elapsedMs) * 1000);
});

const seconds = computed(() => ((props.progress?.elapsedMs ?? 0) / 1000).toFixed(1));

function colorFor(code: number): string {
  return props.classes.find((item) => item.code === code)?.color ?? "#888888";
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
  <section class="panel">
    <h2>Прогресс</h2>

    <div class="bar">
      <div class="fill" :style="{ width: percent + '%' }" />
    </div>

    <div class="facts">
      <span>{{ progress?.scenesDone ?? 0 }} / {{ progress?.scenesTotal ?? scenesTotal }} сцен</span>
      <span>{{ (progress?.pointsTotal ?? 0).toLocaleString("ru-RU") }} точек</span>
      <span>{{ pointsPerSecond.toLocaleString("ru-RU") }} точек/с</span>
      <span>{{ seconds }} с</span>
    </div>

    <p v-if="running && !progress" class="hint">первая сцена считается…</p>

    <ul v-if="classRows.length" class="classes">
      <li v-for="row in classRows" :key="row.code">
        <span class="dot" :style="{ background: colorFor(row.code) }" />
        <span class="name">{{ labelFor(row.code) }}</span>
        <span class="count">{{ row.count.toLocaleString("ru-RU") }}</span>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}

.bar {
  height: 8px;
  background: #22222a;
  border-radius: 999px;
  overflow: hidden;
}

.fill {
  height: 100%;
  background: #2d5cbf;
  transition: width 0.2s ease;
}

.facts {
  display: flex;
  flex-wrap: wrap;
  gap: 1rem;
  font-size: 0.85rem;
  color: #b9b9c2;
}

.hint {
  margin: 0;
  font-size: 0.8rem;
  color: #8a8a94;
}

.classes {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
  font-size: 0.82rem;
}

.classes li {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.dot {
  width: 0.65rem;
  height: 0.65rem;
  border-radius: 50%;
}

.name {
  flex: 1;
  color: #b9b9c2;
}

.count {
  font-variant-numeric: tabular-nums;
}
</style>
