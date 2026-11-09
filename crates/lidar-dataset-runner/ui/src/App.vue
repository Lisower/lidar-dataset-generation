<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import RecipeForm from "./components/RecipeForm.vue";
import StatusPill from "./components/StatusPill.vue";
import RunProgress from "./components/RunProgress.vue";
import StatsTable from "./components/StatsTable.vue";
import { type RunState } from "./lib/labels";
import {
  cancelGeneration,
  getCatalog,
  getDefaultOutputDir,
  getPointClasses,
  onFailed,
  onFinished,
  onProgress,
  startGeneration,
  type Catalog,
  type ClassDto,
  type ProgressEvent,
  type Recipe,
  type RunReport,
  type UnlistenFn,
  type ValidationIssue,
} from "./api";

const state = ref<RunState>("idle");
const catalog = ref<Catalog>({ landscapes: [], buildings: [], plants: [] });
const presetsRoot = ref<string | null>(null);
const classes = ref<ClassDto[]>([]);
const outputDir = ref("");
const issues = ref<ValidationIssue[]>([]);
const failure = ref("");
const jobId = ref<string | null>(null);
const progress = ref<ProgressEvent | null>(null);
const report = ref<RunReport | null>(null);

const recipe = ref<Recipe>({
  scenes: 2,
  seed: 42,
  landscape: "",
  maxSlopeDeg: 25,
  elements: [],
});

let unlisten: UnlistenFn[] = [];

async function loadCatalog() {
  const info = await getCatalog();
  catalog.value = info.catalog;
  presetsRoot.value = info.presetsRoot;
  if (!recipe.value.landscape && info.catalog.landscapes.length) {
    recipe.value = { ...recipe.value, landscape: info.catalog.landscapes[0].id };
  }
}

async function pickOutput() {
  const picked = await open({ directory: true, multiple: false });
  if (typeof picked === "string") {
    outputDir.value = picked;
  }
}

function validationIssues(raw: unknown): ValidationIssue[] | null {
  if (typeof raw !== "string") {
    return null;
  }
  try {
    const parsed = JSON.parse(raw);
    if (Array.isArray(parsed) && parsed.every((item) => "field" in item && "message" in item)) {
      return parsed as ValidationIssue[];
    }
  } catch {
    return null;
  }
  return null;
}

async function start() {
  if (state.value === "running") {
    return;
  }
  issues.value = [];
  failure.value = "";
  progress.value = null;
  report.value = null;
  state.value = "running";
  try {
    jobId.value = await startGeneration(recipe.value, outputDir.value);
  } catch (error) {
    const parsed = validationIssues(error);
    if (parsed) {
      issues.value = parsed;
      state.value = "idle";
      return;
    }
    failure.value = String(error);
    state.value = "failed";
  }
}

async function cancel() {
  if (!jobId.value) {
    return;
  }
  await cancelGeneration(jobId.value);
}

onMounted(async () => {
  classes.value = await getPointClasses();
  outputDir.value = await getDefaultOutputDir();
  await loadCatalog();

  unlisten.push(await onProgress((event) => (progress.value = event)));
  unlisten.push(
    await onFailed((message) => {
      failure.value = message;
      jobId.value = null;
      state.value = "failed";
    }),
  );
  unlisten.push(
    await onFinished((finished) => {
      report.value = finished;
      jobId.value = null;
      state.value = finished.cancelled ? "cancelled" : "finished";
    }),
  );
});

onUnmounted(() => {
  for (const stop of unlisten) {
    stop();
  }
  unlisten = [];
});
</script>

<template>
  <main>
    <header>
      <h1>LiDAR Dataset Runner</h1>
      <StatusPill :state="state" />
    </header>

    <p v-if="failure" class="failure">Ошибка запуска: {{ failure }}</p>

    <div class="layout">
      <RecipeForm
        :catalog="catalog"
        :presets-root="presetsRoot"
        :recipe="recipe"
        :output-dir="outputDir"
        :issues="issues"
        :running="state === 'running'"
        @update:recipe="recipe = $event"
        @update:output-dir="outputDir = $event"
        @pick-output="pickOutput"
        @start="start"
        @cancel="cancel"
      />

      <div class="right">
        <RunProgress
          :progress="progress"
          :scenes-total="recipe.scenes"
          :classes="classes"
          :running="state === 'running'"
        />
        <StatsTable :report="report" :classes="classes" />
      </div>
    </div>
  </main>
</template>

<style>
body {
  margin: 0;
  font-family: system-ui, sans-serif;
  background: #1a1a1e;
  color: #e8e8ea;
}

main {
  padding: 1.25rem 1.5rem;
}

header {
  display: flex;
  align-items: baseline;
  gap: 1rem;
  margin-bottom: 1rem;
}

h1 {
  font-size: 1.15rem;
  margin: 0;
}

h2 {
  font-size: 0.95rem;
  margin: 0;
  color: #d0d0d8;
}

.failure {
  margin: 0 0 0.75rem;
  color: #ff9a9a;
  font-size: 0.85rem;
}

.layout {
  display: grid;
  grid-template-columns: minmax(0, 26rem) minmax(0, 1fr);
  gap: 1.5rem;
  align-items: start;
}

.right {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
  min-width: 0;
}
</style>
