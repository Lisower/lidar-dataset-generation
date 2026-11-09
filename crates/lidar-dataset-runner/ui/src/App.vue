<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { Play, RotateCcw } from "lucide-vue-next";
import RecipeForm from "./components/RecipeForm.vue";
import StatusPill from "./components/StatusPill.vue";
import CancelButton from "./components/CancelButton.vue";
import RunProgress from "./components/RunProgress.vue";
import StatsTable from "./components/StatsTable.vue";
import { type RunState } from "./lib/labels";
import {
  clearDraft,
  loadDraft,
  reconcileRecipe,
  saveDraft,
  type Draft,
} from "./lib/draft";
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

const DEFAULT_RECIPE: Recipe = {
  scenes: 2,
  seed: 42,
  landscape: "",
  maxSlopeDeg: 25,
  elements: [
    { kind: "building", presetId: "rectangular", count: 4 },
    { kind: "tree", presetId: "deciduous-tree", count: 6 },
    { kind: "bush", presetId: "bush", count: 10 },
    { kind: "grass", presetId: "sparse-grass", count: 400 },
  ],
};

const CATALOGUE_MISSING =
  "Каталог пресетов не найден. Приложению нужна папка presets рядом с исполняемым файлом или переменная окружения LIDAR_PRESETS_ROOT.";

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

const stored = loadDraft();

const recipe = ref<Recipe>(stored?.recipe ?? structuredClone(DEFAULT_RECIPE));

let unlisten: UnlistenFn[] = [];
let draftTimer: ReturnType<typeof setTimeout> | null = null;
let restoredOutputDir = stored?.outputDir ?? null;

const running = computed(() => state.value === "running");

const catalogEmpty = computed(
  () => presetsRoot.value !== null && catalog.value.landscapes.length === 0,
);

const catalogueProblem = computed(() => {
  if (presetsRoot.value === null) {
    return CATALOGUE_MISSING;
  }
  if (catalogEmpty.value) {
    return `Каталог пресетов пуст: ${presetsRoot.value}`;
  }
  return "";
});

const startDisabled = computed(
  () => running.value || presetsRoot.value === null || catalogEmpty.value,
);

const showEmptyState = computed(() => state.value === "idle" && report.value === null);

async function loadCatalog() {
  const info = await getCatalog();
  catalog.value = info.catalog;
  presetsRoot.value = info.presetsRoot;
  recipe.value = reconcileRecipe(recipe.value, info.catalog);
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
  if (startDisabled.value) {
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

function scheduleDraftSave() {
  if (state.value === "running") {
    return;
  }
  if (draftTimer !== null) {
    clearTimeout(draftTimer);
  }
  draftTimer = setTimeout(() => {
    draftTimer = null;
    const draft: Draft = { recipe: recipe.value, outputDir: outputDir.value };
    saveDraft(draft);
  }, 300);
}

watch([recipe, outputDir], scheduleDraftSave, { deep: true });

async function resetRecipe() {
  if (draftTimer !== null) {
    clearTimeout(draftTimer);
    draftTimer = null;
  }
  clearDraft();
  recipe.value = structuredClone(DEFAULT_RECIPE);
  issues.value = [];
  await loadCatalog();
  outputDir.value = await getDefaultOutputDir();
}

onMounted(async () => {
  classes.value = await getPointClasses();
  outputDir.value = restoredOutputDir ?? (await getDefaultOutputDir());
  restoredOutputDir = null;
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
  if (draftTimer !== null) {
    clearTimeout(draftTimer);
    draftTimer = null;
  }
  for (const stop of unlisten) {
    stop();
  }
  unlisten = [];
});
</script>

<template>
  <div class="app">
    <header class="app-header">
      <div class="titles">
        <h1>LiDAR Dataset Runner</h1>
        <p class="muted">Собирает сцены с рельефом, зданиями и растениями в облака точек</p>
      </div>
      <StatusPill :state="state" />
    </header>

    <div v-if="catalogueProblem" class="banner banner-error">
      <p class="banner-title">{{ catalogueProblem }}</p>
    </div>

    <div v-if="failure" class="banner banner-error">
      <p class="banner-title">Запуск прерван</p>
      <p class="banner-text">{{ failure }}</p>
      <button type="button" class="btn btn-ghost" @click="failure = ''">Скрыть</button>
    </div>

    <main class="app-main" :aria-busy="running">
      <div class="app-form">
        <RecipeForm
          :catalog="catalog"
          :presets-root="presetsRoot"
          :recipe="recipe"
          :output-dir="outputDir"
          :issues="issues"
          :running="running"
          @update:recipe="recipe = $event"
          @update:output-dir="outputDir = $event"
          @pick-output="pickOutput"
        />

        <div class="actions-bar">
          <button type="button" class="btn btn-ghost" :disabled="running" @click="resetRecipe">
            <RotateCcw :size="16" aria-hidden="true" />
            Сбросить настройки
          </button>
          <button
            id="start-run"
            type="button"
            class="btn btn-primary"
            :disabled="startDisabled"
            :title="catalogueProblem || undefined"
            @click="start"
          >
            <Play :size="16" aria-hidden="true" />
            Сгенерировать датасет
          </button>
          <CancelButton :running="running" @cancel="cancel" />
        </div>
      </div>

      <aside class="app-side">
        <div v-if="showEmptyState" class="card empty-state">
          <h2>Здесь появится прогресс</h2>
          <ol>
            <li>Выберите рельеф и объекты слева</li>
            <li>Нажмите Сгенерировать датасет</li>
            <li>Файлы появятся в выбранной папке</li>
          </ol>
        </div>
        <div v-else-if="state === 'failed'" class="card empty-state">
          <h2>Запуск не удался</h2>
          <button type="button" class="btn" @click="start">Попробовать снова</button>
        </div>

        <RunProgress
          :progress="progress"
          :scenes-total="recipe.scenes"
          :classes="classes"
          :running="running"
          @cancel="cancel"
        />
        <StatsTable :report="report" :classes="classes" />
      </aside>
    </main>
  </div>
</template>

<style scoped>
.app {
  height: 100vh;
  display: flex;
  flex-direction: column;
}

.app-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--border);
  background: var(--surface);
  min-height: 72px;
}

.titles {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.titles .muted {
  font-size: 13px;
}

.banner {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  margin: var(--space-3) var(--space-4) 0;
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--surface-2);
}

.banner-error {
  border-color: var(--danger);
  background: var(--danger-surface);
}

.banner-title {
  font-weight: 600;
  color: var(--danger);
}

.banner-text {
  white-space: pre-wrap;
  flex: 1;
  min-width: 0;
  font-size: 13px;
}

.app-main {
  flex: 1;
  height: calc(100vh - 72px);
  overflow: auto;
  display: grid;
  grid-template-columns: minmax(0, 30rem) minmax(0, 1fr);
  gap: var(--space-5);
  align-items: start;
  padding: var(--space-4);
}

.app-form {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-width: 0;
}

.actions-bar {
  position: sticky;
  bottom: 0;
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: var(--space-2);
  background: var(--surface);
  border-top: 1px solid var(--border);
  padding: var(--space-3) var(--space-4);
}

.app-side {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-width: 0;
}

.empty-state {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  font-size: 13px;
}

.empty-state ol {
  margin: 0;
  padding-left: var(--space-4);
  color: var(--muted);
}

@media (max-width: 960px) {
  .app-main {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
