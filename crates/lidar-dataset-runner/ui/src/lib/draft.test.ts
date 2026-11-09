import { beforeEach, describe, expect, it } from "vitest";
import type { Catalog, Recipe } from "../api";
import { DRAFT_KEY, clearDraft, loadDraft, reconcileRecipe, saveDraft } from "./draft";

const recipe: Recipe = {
  scenes: 3,
  seed: 7,
  landscape: "plain",
  maxSlopeDeg: 20,
  elements: [
    { kind: "building", presetId: "rectangular", count: 2 },
    { kind: "tree", presetId: "spruce", count: 5 },
  ],
};

const catalog: Catalog = {
  landscapes: [{ id: "plain", kind: "plain", name: "равнина", description: "", plantType: null }],
  buildings: [
    { id: "rectangular", kind: "rectangular", name: "Прямоугольный", description: "", plantType: null },
  ],
  plants: [
    { id: "spruce", kind: "spruce", name: "ель", description: "", plantType: "tree" },
    { id: "bush", kind: "bush", name: "куст", description: "", plantType: "bush" },
  ],
};

beforeEach(() => {
  localStorage.clear();
});

describe("draft storage", () => {
  it("round-trips a draft", () => {
    saveDraft({ recipe, outputDir: "/tmp/out" });
    expect(loadDraft()).toEqual({ recipe, outputDir: "/tmp/out" });
  });

  it("ignores broken or foreign payloads", () => {
    localStorage.setItem(DRAFT_KEY, "{not json");
    expect(loadDraft()).toBeNull();
    localStorage.setItem(DRAFT_KEY, JSON.stringify({ hello: 1 }));
    expect(loadDraft()).toBeNull();
  });

  it("clears the draft", () => {
    saveDraft({ recipe, outputDir: "/tmp/out" });
    clearDraft();
    expect(loadDraft()).toBeNull();
  });
});

describe("reconcileRecipe", () => {
  it("keeps values that still exist", () => {
    expect(reconcileRecipe(recipe, catalog)).toEqual(recipe);
  });

  it("replaces missing presets with the first available one of the same kind", () => {
    const stale: Recipe = {
      ...recipe,
      landscape: "river",
      elements: [
        { kind: "building", presetId: "skyscraper", count: 2 },
        { kind: "tree", presetId: "palm", count: 5 },
        { kind: "bush", presetId: "juniper", count: 1 },
      ],
    };
    const fixed = reconcileRecipe(stale, catalog);
    expect(fixed.landscape).toBe("plain");
    expect(fixed.elements[0].presetId).toBe("rectangular");
    expect(fixed.elements[1].presetId).toBe("spruce");
    expect(fixed.elements[2].presetId).toBe("bush");
    expect(fixed.elements[2].count).toBe(1);
  });

  it("drops element kinds that have no presets at all", () => {
    const noGrass: Catalog = { ...catalog, plants: catalog.plants.filter((p) => p.plantType !== "grass") };
    const withGrass: Recipe = {
      ...recipe,
      elements: [...recipe.elements, { kind: "grass", presetId: "sparse-grass", count: 100 }],
    };
    expect(reconcileRecipe(withGrass, noGrass).elements).toHaveLength(2);
  });
});
