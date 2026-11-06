<script setup lang="ts">
import { computed } from "vue";
import type {
  Catalog,
  ElementKind,
  ElementSpec,
  Recipe,
  ValidationIssue,
} from "../api";

const props = defineProps<{
  catalog: Catalog;
  presetsRoot: string | null;
  recipe: Recipe;
  outputDir: string;
  issues: ValidationIssue[];
  running: boolean;
}>();

const emit = defineEmits<{
  (e: "update:recipe", value: Recipe): void;
  (e: "update:outputDir", value: string): void;
  (e: "pick-output"): void;
  (e: "start"): void;
  (e: "cancel"): void;
}>();

const kinds: { kind: ElementKind; title: string }[] = [
  { kind: "building", title: "Здания" },
  { kind: "tree", title: "Деревья" },
  { kind: "bush", title: "Кусты" },
  { kind: "grass", title: "Трава" },
];

const plantTypeByKind: Record<ElementKind, string | null> = {
  building: null,
  tree: "tree",
  bush: "bush",
  grass: "grass",
};

const recipes = computed(() => props.recipe);

function issueFor(field: string): string | undefined {
  return props.issues.find((issue) => issue.field === field)?.message;
}

function patch(recipe: Recipe) {
  emit("update:recipe", recipe);
}

function setScenes(event: Event) {
  const value = Number((event.target as HTMLInputElement).value);
  patch({ ...recipes.value, scenes: Number.isFinite(value) ? value : 0 });
}

function setSeed(event: Event) {
  const value = Number((event.target as HTMLInputElement).value);
  patch({ ...recipes.value, seed: Number.isFinite(value) ? value : 0 });
}

function setSlope(event: Event) {
  const value = Number((event.target as HTMLInputElement).value);
  patch({ ...recipes.value, maxSlopeDeg: Number.isFinite(value) ? value : 0 });
}

function setLandscape(event: Event) {
  patch({ ...recipes.value, landscape: (event.target as HTMLSelectElement).value });
}

function presetsFor(kind: ElementKind) {
  if (kind === "building") {
    return props.catalog.buildings;
  }
  const wanted = plantTypeByKind[kind];
  return props.catalog.plants.filter((preset) => preset.plantType === wanted);
}

function elementIndex(kind: ElementKind): number {
  return recipes.value.elements.findIndex((element) => element.kind === kind);
}

function element(kind: ElementKind): ElementSpec {
  const index = elementIndex(kind);
  if (index >= 0) {
    return recipes.value.elements[index];
  }
  const options = presetsFor(kind);
  return { kind, presetId: options[0]?.id ?? "", count: 0 };
}

function updateElement(kind: ElementKind, changes: Partial<ElementSpec>) {
  const elements = [...recipes.value.elements];
  const index = elementIndex(kind);
  const next: ElementSpec = { ...element(kind), ...changes };
  if (index >= 0) {
    elements[index] = next;
  } else {
    elements.push(next);
  }
  patch({ ...recipes.value, elements });
}

function setPreset(kind: ElementKind, event: Event) {
  updateElement(kind, { presetId: (event.target as HTMLSelectElement).value });
}

function setCount(kind: ElementKind, event: Event) {
  const value = Number((event.target as HTMLInputElement).value);
  updateElement(kind, { count: Number.isFinite(value) ? value : 0 });
}
</script>

<template>
  <section class="panel">
    <h2>Рецепт</h2>

    <p v-if="!presetsRoot" class="error">
      Каталог пресетов не найден: генерация недоступна. Положите каталог
      <code>presets/</code> рядом с приложением.
    </p>
    <p v-else-if="catalog.landscapes.length === 0" class="error">
      Каталог пресетов пуст: {{ presetsRoot }}
    </p>

    <div class="grid">
      <label>
        Сцен
        <input
          type="number"
          min="1"
          max="1000"
          :value="recipe.scenes"
          :disabled="running"
          @input="setScenes"
        />
        <small v-if="issueFor('scenes')" class="error">{{ issueFor("scenes") }}</small>
      </label>

      <label>
        Seed
        <input
          type="number"
          min="0"
          :value="recipe.seed"
          :disabled="running"
          @input="setSeed"
        />
      </label>

      <label>
        Макс. уклон, °
        <input
          type="number"
          min="1"
          max="45"
          step="0.5"
          :value="recipe.maxSlopeDeg"
          :disabled="running"
          @input="setSlope"
        />
        <small v-if="issueFor('maxSlopeDeg')" class="error">{{ issueFor("maxSlopeDeg") }}</small>
      </label>
    </div>

    <label class="wide">
      Ландшафт
      <select :value="recipe.landscape" :disabled="running" @change="setLandscape">
        <option v-for="preset in catalog.landscapes" :key="preset.id" :value="preset.id">
          {{ preset.name }}
        </option>
      </select>
      <small v-if="issueFor('landscape')" class="error">{{ issueFor("landscape") }}</small>
    </label>

    <table class="elements">
      <thead>
        <tr>
          <th>Элемент</th>
          <th>Пресет</th>
          <th>Количество</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="row in kinds" :key="row.kind">
          <td>{{ row.title }}</td>
          <td>
            <select
              :value="element(row.kind).presetId"
              :disabled="running"
              @change="setPreset(row.kind, $event)"
            >
              <option v-for="preset in presetsFor(row.kind)" :key="preset.id" :value="preset.id">
                {{ preset.name }}
              </option>
            </select>
          </td>
          <td>
            <input
              type="number"
              min="0"
              max="2000"
              :value="element(row.kind).count"
              :disabled="running"
              @input="setCount(row.kind, $event)"
            />
            <small v-if="issueFor(`elements[${elementIndex(row.kind)}].presetId`)" class="error">
              {{ issueFor(`elements[${elementIndex(row.kind)}].presetId`) }}
            </small>
            <small v-if="issueFor(`elements[${elementIndex(row.kind)}].count`)" class="error">
              {{ issueFor(`elements[${elementIndex(row.kind)}].count`) }}
            </small>
          </td>
        </tr>
      </tbody>
    </table>
    <small v-if="issueFor('elements')" class="error">{{ issueFor("elements") }}</small>

    <label class="wide">
      Каталог вывода
      <span class="row">
        <input
          type="text"
          :value="outputDir"
          :disabled="running"
          @input="emit('update:outputDir', ($event.target as HTMLInputElement).value)"
        />
        <button type="button" :disabled="running" @click="emit('pick-output')">Выбрать…</button>
      </span>
    </label>

    <div class="actions">
      <button type="button" class="primary" :disabled="running" @click="emit('start')">
        Сгенерировать
      </button>
      <button type="button" :disabled="!running" @click="emit('cancel')">Отменить</button>
    </div>
  </section>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 0.75rem;
}

label {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  font-size: 0.85rem;
  color: #b9b9c2;
}

input,
select {
  background: #101014;
  border: 1px solid #33333c;
  border-radius: 6px;
  color: #e8e8ea;
  padding: 0.4rem 0.5rem;
  font: inherit;
}

input:disabled,
select:disabled {
  opacity: 0.55;
}

.elements {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.85rem;
}

.elements th {
  text-align: left;
  color: #9a9aa4;
  font-weight: 500;
  padding-bottom: 0.3rem;
}

.elements td {
  padding: 0.2rem 0.4rem 0.2rem 0;
  vertical-align: top;
}

.elements select,
.elements input {
  width: 100%;
}

.row {
  display: flex;
  gap: 0.5rem;
}

.row input {
  flex: 1;
}

.actions {
  display: flex;
  gap: 0.5rem;
  margin-top: 0.25rem;
}

button {
  background: #26262e;
  border: 1px solid #3a3a45;
  border-radius: 6px;
  color: #e8e8ea;
  padding: 0.45rem 0.9rem;
  font: inherit;
  cursor: pointer;
}

button:disabled {
  opacity: 0.45;
  cursor: default;
}

button.primary {
  background: #2d5cbf;
  border-color: #3b6fd8;
}

.error {
  color: #ff8080;
  font-size: 0.78rem;
  margin: 0;
}
</style>
