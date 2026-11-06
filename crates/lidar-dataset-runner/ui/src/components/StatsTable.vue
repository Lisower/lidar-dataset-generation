<script setup lang="ts">
import { computed } from "vue";
import type { ClassDto, SceneStats, RunReport } from "../api";

const props = defineProps<{
  report: RunReport | null;
  classes: ClassDto[];
}>();

const scenes = computed<SceneStats[]>(() => props.report?.scenes ?? []);

const classCodes = computed(() => {
  const codes = new Set<number>();
  for (const scene of scenes.value) {
    for (const code of Object.keys(scene.byClass)) {
      codes.add(Number(code));
    }
  }
  return [...codes].sort((a, b) => a - b);
});

function colorFor(code: number): string {
  return props.classes.find((item) => item.code === code)?.color ?? "#888888";
}

function labelFor(code: number): string {
  return props.classes.find((item) => item.code === code)?.label ?? `класс ${code}`;
}

function objects(row: SceneStats): string {
  return Object.entries(row.objects)
    .filter(([, counts]) => counts.requested > 0 || counts.placed > 0)
    .map(([key, counts]) => `${key} ${counts.placed}/${counts.requested}`)
    .join(", ");
}

function kilobytes(bytes: number): string {
  return (bytes / 1024).toFixed(0) + " КБ";
}
</script>

<template>
  <section class="panel">
    <h2>Статистика</h2>

    <p v-if="!report" class="hint">запустите генерацию, чтобы увидеть таблицу</p>

    <template v-else>
      <table>
        <thead>
          <tr>
            <th>#</th>
            <th>seed</th>
            <th>точек</th>
            <th v-for="code in classCodes" :key="code">
              <span class="dot" :style="{ background: colorFor(code) }" />
              {{ labelFor(code) }}
            </th>
            <th>объекты (поставлено/запрошено)</th>
            <th>файл</th>
            <th>мс</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="row in scenes" :key="row.index">
            <td>{{ row.index }}</td>
            <td>{{ row.seed }}</td>
            <td>{{ row.points.toLocaleString("ru-RU") }}</td>
            <td v-for="code in classCodes" :key="code">
              {{ (row.byClass[String(code)] ?? 0).toLocaleString("ru-RU") }}
            </td>
            <td class="objects">{{ objects(row) }}</td>
            <td class="file">{{ row.file }} ({{ kilobytes(row.bytes) }})</td>
            <td>{{ row.elapsedMs }}</td>
          </tr>
          <tr class="totals">
            <td colspan="2">итого: {{ report.totals.scenes }} сцен</td>
            <td>{{ report.totals.points.toLocaleString("ru-RU") }}</td>
            <td v-for="code in classCodes" :key="code">
              {{ (report.totals.byClass[String(code)] ?? 0).toLocaleString("ru-RU") }}
            </td>
            <td>{{ kilobytes(report.totals.bytes) }}</td>
            <td colspan="2">{{ report.totals.elapsedMs }} мс</td>
          </tr>
        </tbody>
      </table>

      <p v-if="report.cancelled" class="hint">запуск отменён: записаны только готовые сцены</p>

      <div v-if="report.errors.length" class="errors">
        <h3>Ошибки сцен</h3>
        <ul>
          <li v-for="error in report.errors" :key="error.index">#{{ error.index }}: {{ error.message }}</li>
        </ul>
      </div>
    </template>
  </section>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}

.hint {
  margin: 0;
  font-size: 0.82rem;
  color: #8a8a94;
}

table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.78rem;
  font-variant-numeric: tabular-nums;
}

th {
  text-align: left;
  color: #9a9aa4;
  font-weight: 500;
  padding: 0.25rem 0.4rem 0.25rem 0;
  white-space: nowrap;
}

td {
  padding: 0.2rem 0.4rem 0.2rem 0;
  border-top: 1px solid #26262e;
}

.dot {
  display: inline-block;
  width: 0.6rem;
  height: 0.6rem;
  border-radius: 50%;
  margin-right: 0.25rem;
}

.objects,
.file {
  color: #b9b9c2;
}

.totals td {
  font-weight: 600;
  border-top: 1px solid #3a3a45;
}

.errors {
  border: 1px solid #5c2d2d;
  border-radius: 6px;
  padding: 0.5rem 0.75rem;
  background: #2a1a1a;
}

.errors h3 {
  margin: 0 0 0.3rem;
  font-size: 0.85rem;
  font-weight: 600;
}

.errors ul {
  margin: 0;
  padding-left: 1.1rem;
  font-size: 0.8rem;
  color: #ffb0b0;
}
</style>
