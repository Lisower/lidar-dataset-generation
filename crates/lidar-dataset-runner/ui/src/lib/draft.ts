import type { Catalog, ElementKind, ElementSpec, PresetDto, Recipe } from "../api";

export const DRAFT_KEY = "lidar-runner:draft";

export type Draft = {
  recipe: Recipe;
  outputDir: string;
};

const PLANT_TYPE_BY_KIND: Record<ElementKind, string | null> = {
  building: null,
  tree: "tree",
  bush: "bush",
  grass: "grass",
};

function isRecipe(value: unknown): value is Recipe {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as Partial<Recipe>;
  return (
    typeof candidate.scenes === "number" &&
    typeof candidate.seed === "number" &&
    typeof candidate.landscape === "string" &&
    typeof candidate.maxSlopeDeg === "number" &&
    Array.isArray(candidate.elements) &&
    candidate.elements.every(
      (element) =>
        element &&
        typeof element === "object" &&
        typeof (element as ElementSpec).kind === "string" &&
        typeof (element as ElementSpec).presetId === "string" &&
        typeof (element as ElementSpec).count === "number",
    )
  );
}

export function loadDraft(): Draft | null {
  try {
    const raw = localStorage.getItem(DRAFT_KEY);
    if (!raw) {
      return null;
    }
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") {
      return null;
    }
    const candidate = parsed as Partial<Draft>;
    if (!isRecipe(candidate.recipe) || typeof candidate.outputDir !== "string") {
      return null;
    }
    return { recipe: candidate.recipe, outputDir: candidate.outputDir };
  } catch {
    return null;
  }
}

export function saveDraft(draft: Draft): void {
  try {
    localStorage.setItem(DRAFT_KEY, JSON.stringify(draft));
  } catch {
    // Private mode or a full quota: the draft is a convenience, not data.
  }
}

export function clearDraft(): void {
  try {
    localStorage.removeItem(DRAFT_KEY);
  } catch {
    // Nothing to do when storage is unavailable.
  }
}

function presetsFor(kind: ElementKind, catalog: Catalog): PresetDto[] {
  if (kind === "building") {
    return catalog.buildings;
  }
  const wanted = PLANT_TYPE_BY_KIND[kind];
  return catalog.plants.filter((preset) => preset.plantType === wanted);
}

export function reconcileRecipe(recipe: Recipe, catalog: Catalog): Recipe {
  const landscapeExists = catalog.landscapes.some((preset) => preset.id === recipe.landscape);
  const landscape =
    !landscapeExists && catalog.landscapes.length > 0
      ? catalog.landscapes[0].id
      : recipe.landscape;

  const elements: ElementSpec[] = [];
  for (const element of recipe.elements) {
    const options = presetsFor(element.kind, catalog);
    if (options.length === 0) {
      continue;
    }
    const presetId = options.some((preset) => preset.id === element.presetId)
      ? element.presetId
      : options[0].id;
    elements.push({ ...element, presetId });
  }

  return { ...recipe, landscape, elements };
}
