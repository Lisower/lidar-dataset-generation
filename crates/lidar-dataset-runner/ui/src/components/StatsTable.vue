<script setup lang="ts">
import { computed, ref } from "vue";
import { Copy } from "lucide-vue-next";
import type { ClassDto, RunReport, SceneStats } from "../api";
import { formatBytes, formatDuration, formatPoints } from "../lib/format";
import { copyText, objectSummaryText, placedLessThanRequested } from "../lib/results";

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
  return props.classes.find((item) => item.code === code)?.color ?? "var(--muted)";
}

function labelFor(code: number): string {
  return props.classes.find((item) => item.code === code)?.label ?? `класс ${code}`;
}

function objects(row: SceneStats): string {
  return objectSummaryText(row.objects);
}

const copyFailed = ref(false);

async function copyPath() {
  if (!props.report) {
    return;
  }
  copyFailed.value = !(await copyText(props.report.outputDir));
}

const visibleErrors = computed(() => props.report?.errors.slice(0, 5) ?? []);
const hiddenErrors = computed(() => props.report?.errors.slice(5) ?? []);
</script>

<template>
  <section class="card panel">
    <h2>Результаты</h2>

    <p v-if="!report" class="muted empty">
      Здесь появятся сцены после запуска: число точек, классы, поставленные объекты.
    </p>

    <template v-else>
      <div class="output">
        <code class="path">{{ report.outputDir }}</code>
        <button type="button" class="btn btn-ghost" @click="copyPath">
          <Copy :size="16" aria-hidden="true" />
          Скопировать путь
        </button>
      </div>
      <p class="muted note">
        {{
          copyFailed
            ? "Не удалось скопировать, выделите путь вручную"
            : "Рецепт запуска и итоги лежат в run.json в этой папке"
        }}
      </p>

      <div class="table-scroll">
        <table>
          <thead>
            <tr>
              <th class="sticky">#</th>
              <th>Сидовое число</th>
              <th class="numeric">Точек</th>
              <th v-for="code in classCodes" :key="code">
                <span class="dot" :style="{ background: colorFor(code) }" />
                {{ labelFor(code) }}
              </th>
              <th>Объекты</th>
              <th>Файл</th>
              <th>Время</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in scenes" :key="row.index">
              <td class="sticky">{{ row.index }}</td>
              <td class="numeric">{{ row.seed }}</td>
              <td class="numeric">{{ formatPoints(row.points) }}</td>
              <td v-for="code in classCodes" :key="code" class="numeric">
                {{ formatPoints(row.byClass[String(code)] ?? 0) }}
              </td>
              <td
                class="objects"
                :class="{ short: placedLessThanRequested(row.objects) }"
                :title="objects(row)"
              >
                {{ objects(row) }}
              </td>
              <td class="file">
                {{ row.file }} <span class="muted">({{ formatBytes(row.bytes) }})</span>
              </td>
              <td class="numeric">{{ formatDuration(row.elapsedMs) }}</td>
            </tr>
            <tr class="totals">
              <td class="sticky">Итого</td>
              <td></td>
              <td class="numeric">{{ formatPoints(report.totals.points) }}</td>
              <td v-for="code in classCodes" :key="code" class="numeric">
                {{ formatPoints(report.totals.byClass[String(code)] ?? 0) }}
              </td>
              <td></td>
              <td class="file">{{ formatBytes(report.totals.bytes) }}</td>
              <td class="numeric">{{ formatDuration(report.totals.elapsedMs) }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <p v-if="report.cancelled" class="cancelled">
        Запуск остановлен: сохранены только готовые сцены
      </p>

      <div v-if="report.errors.length" class="errors">
        <h3>Ошибки сцен</h3>
        <ul>
          <li v-for="error in visibleErrors" :key="error.index">
            Сцена #{{ error.index }}: {{ error.message }}
          </li>
        </ul>
        <details v-if="hiddenErrors.length">
          <summary>показать все</summary>
          <ul>
            <li v-for="error in hiddenErrors" :key="error.index">
              Сцена #{{ error.index }}: {{ error.message }}
            </li>
          </ul>
        </details>
      </div>
    </template>
  </section>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.empty {
  font-size: 13px;
}

.output {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
}

.path {
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
  background: var(--surface-2);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  padding: var(--space-1) var(--space-2);
}

.note {
  font-size: 12px;
}

.table-scroll {
  overflow-x: auto;
}

table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

th {
  text-align: left;
  color: var(--muted);
  font-weight: 500;
  padding: var(--space-1) var(--space-2) var(--space-1) 0;
  white-space: nowrap;
}

td {
  padding: 2px var(--space-2) 2px 0;
  border-top: 1px solid var(--border);
}

.sticky {
  position: sticky;
  left: 0;
  background: var(--surface);
}

.numeric {
  text-align: right;
}

.dot {
  display: inline-block;
  width: 9px;
  height: 9px;
  border-radius: 50%;
  margin-right: var(--space-1);
}

.objects,
.file {
  color: var(--muted);
}

.objects.short {
  color: var(--warn);
}

.totals td {
  font-weight: 600;
  border-top: 1px solid var(--accent);
}

.cancelled {
  color: var(--warn);
  font-size: 13px;
}

.errors {
  border: 1px solid var(--danger);
  border-radius: var(--radius-sm);
  padding: var(--space-2) var(--space-3);
  background: var(--danger-surface);
}

.errors h3 {
  margin-bottom: var(--space-1);
}

.errors ul {
  margin: 0;
  padding-left: var(--space-4);
  font-size: 12px;
  color: var(--danger);
}

details summary {
  cursor: pointer;
  font-size: 12px;
  color: var(--muted);
}
</style>
