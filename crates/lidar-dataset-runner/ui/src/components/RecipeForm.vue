<script setup lang="ts">
import { computed } from "vue";
import { FolderOpen } from "lucide-vue-next";
import FieldHint from "./FieldHint.vue";
import ValidationSummary from "./ValidationSummary.vue";
import PresetSelect from "./PresetSelect.vue";
import type {
  Catalog,
  ElementKind,
  ElementSpec,
  Recipe,
  ValidationIssue,
} from "../api";
import { ELEMENT_HINT, ELEMENT_TITLE, FIELD_HINT, PLACEMENT_NOTE } from "../lib/labels";

const props = defineProps<{
  catalog: Catalog;
  recipe: Recipe;
  outputDir: string;
  issues: ValidationIssue[];
  running: boolean;
}>();

const emit = defineEmits<{
  (e: "update:recipe", value: Recipe): void;
  (e: "update:outputDir", value: string): void;
  (e: "pick-output"): void;
}>();

const kinds: ElementKind[] = ["building", "tree", "bush", "grass"];

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

function setLandscape(value: string) {
  patch({ ...recipes.value, landscape: value });
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

function setPreset(kind: ElementKind, value: string) {
  updateElement(kind, { presetId: value });
}

function setCount(kind: ElementKind, event: Event) {
  const value = Number((event.target as HTMLInputElement).value);
  updateElement(kind, { count: Number.isFinite(value) ? value : 0 });
}

function elementIssue(kind: ElementKind, suffix: "presetId" | "count"): string | undefined {
  const index = elementIndex(kind);
  if (index < 0) {
    return undefined;
  }
  return issueFor(`elements[${index}].${suffix}`);
}

function setOutput(event: Event) {
  emit("update:outputDir", (event.target as HTMLInputElement).value);
}

function fieldIdFor(field: string): string {
  if (field === "scenes" || field === "seed" || field === "maxSlopeDeg" || field === "landscape") {
    const map: Record<string, string> = {
      scenes: "field-scenes",
      seed: "field-seed",
      maxSlopeDeg: "field-slope",
      landscape: "field-landscape",
    };
    return map[field];
  }
  if (field === "outputDir") {
    return "field-output";
  }
  if (field === "elements") {
    return `field-element-${firstElementKind()}-count`;
  }
  const match = /^elements\[(\d+)\](?:\.(\w+))?$/.exec(field);
  if (match) {
    const element = recipes.value.elements[Number(match[1])];
    if (element) {
      return match[2] === "count"
        ? `field-element-${element.kind}-count`
        : `field-element-${element.kind}`;
    }
  }
  return field;
}

function firstElementKind(): ElementKind {
  return recipes.value.elements[0]?.kind ?? "building";
}

function focusField(field: string) {
  document.getElementById(fieldIdFor(field))?.focus();
}
</script>

<template>
  <form class="form" @submit.prevent>
    <ValidationSummary :issues="issues" @focus-field="focusField" />

    <fieldset class="card group">
      <legend>Сцена</legend>

      <div class="row-3">
        <div class="field">
          <label for="field-scenes">Сцен</label>
          <input
            id="field-scenes"
            aria-describedby="hint-scenes"
            type="number"
            min="1"
            max="1000"
            :value="recipe.scenes"
            :disabled="running"
            @input="setScenes"
          />
          <FieldHint id="hint-scenes" :text="FIELD_HINT.scenes" :error="issueFor('scenes')" />
        </div>

        <div class="field">
          <label for="field-seed">Сидовое число</label>
          <input
            id="field-seed"
            aria-describedby="hint-seed"
            type="number"
            min="0"
            :value="recipe.seed"
            :disabled="running"
            @input="setSeed"
          />
          <FieldHint id="hint-seed" :text="FIELD_HINT.seed" :error="issueFor('seed')" />
        </div>

        <div class="field">
          <label for="field-slope">Максимальный уклон, °</label>
          <input
            id="field-slope"
            aria-describedby="hint-slope"
            type="number"
            min="1"
            max="45"
            step="0.5"
            :value="recipe.maxSlopeDeg"
            :disabled="running"
            @input="setSlope"
          />
          <FieldHint
            id="hint-slope"
            :text="FIELD_HINT.maxSlopeDeg"
            :error="issueFor('maxSlopeDeg')"
          />
        </div>
      </div>

      <div class="field">
        <label for="field-landscape">Рельеф</label>
        <PresetSelect
          id="field-landscape"
          :presets="catalog.landscapes"
          :model-value="recipe.landscape"
          :disabled="running"
          described-by="hint-landscape"
          empty-text="в каталоге нет рельефов"
          @update:model-value="setLandscape"
        />
        <FieldHint id="hint-landscape" :text="FIELD_HINT.landscape" :error="issueFor('landscape')" />
      </div>
    </fieldset>

    <fieldset class="card group">
      <legend>Что поставить на сцену</legend>

      <div v-for="kind in kinds" :key="kind" class="element">
        <div class="element-head">
          <span class="element-title">{{ ELEMENT_TITLE[kind] }}</span>
          <span class="element-hint muted">{{ ELEMENT_HINT[kind] }}</span>
        </div>
        <div class="element-controls">
          <PresetSelect
            :id="`field-element-${kind}`"
            :presets="presetsFor(kind)"
            :model-value="element(kind).presetId"
            :disabled="running"
            :described-by="`hint-element-${kind}`"
            empty-text="нет доступных пресетов"
            @update:model-value="setPreset(kind, $event)"
          />
          <input
            :id="`field-element-${kind}-count`"
            :aria-describedby="`hint-element-${kind}`"
            :aria-label="`Количество: ${ELEMENT_TITLE[kind]}`"
            class="count"
            type="number"
            min="0"
            max="2000"
            :value="element(kind).count"
            :disabled="running"
            @input="setCount(kind, $event)"
          />
        </div>
        <FieldHint
          :id="`hint-element-${kind}`"
          text="0 означает, что объекты этого вида не ставятся"
          :error="elementIssue(kind, 'presetId') ?? elementIssue(kind, 'count')"
        />
      </div>

      <FieldHint v-if="issueFor('elements')" id="hint-elements" text="" :error="issueFor('elements')" />

      <p class="note muted">{{ PLACEMENT_NOTE }}</p>
    </fieldset>

    <fieldset class="card group">
      <legend>Куда сохранять</legend>

      <div class="field">
        <label for="field-output">Папка вывода</label>
        <div class="output-row">
          <input
            id="field-output"
            aria-describedby="hint-output"
            type="text"
            :value="outputDir"
            :disabled="running"
            @input="setOutput"
          />
          <button type="button" class="btn" :disabled="running" @click="emit('pick-output')">
            <FolderOpen :size="16" aria-hidden="true" />
            Выбрать папку…
          </button>
        </div>
        <FieldHint id="hint-output" :text="FIELD_HINT.outputDir" :error="issueFor('outputDir')" />
      </div>
    </fieldset>

  </form>
</template>

<style scoped>
.form {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.group {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.row-3 {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(9rem, 1fr));
  gap: var(--space-3);
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

label {
  font-size: 13px;
  color: var(--muted);
}

.element {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding-bottom: var(--space-3);
  border-bottom: 1px solid var(--border);
}

.element:last-of-type {
  border-bottom: none;
  padding-bottom: 0;
}

.element-head {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--space-2);
}

.element-title {
  font-weight: 600;
}

.element-hint {
  font-size: 12px;
}

.element-controls {
  display: flex;
  gap: var(--space-2);
  align-items: flex-start;
}

.element-controls > :first-child {
  flex: 1;
  min-width: 0;
}

.count {
  width: 6rem;
}

.note {
  font-size: 12px;
}

.output-row {
  display: flex;
  gap: var(--space-2);
}

.output-row input {
  flex: 1;
  min-width: 0;
}
</style>
