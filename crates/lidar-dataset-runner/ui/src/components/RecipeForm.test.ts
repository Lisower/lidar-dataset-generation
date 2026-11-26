import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import type { Catalog, Recipe } from "../api";
import RecipeForm from "./RecipeForm.vue";

const catalog: Catalog = {
  landscapes: [{ id: "plain", kind: "plain", name: "равнина", description: "", plantType: null }],
  buildings: [
    { id: "rectangular", kind: "rectangular", name: "Прямоугольный", description: "", plantType: null },
  ],
  plants: [
    { id: "bush", kind: "bush", name: "куст", description: "", plantType: "bush" },
  ],
};

const recipe: Recipe = {
  scenes: 2,
  seed: 42,
  landscape: "plain",
  maxSlopeDeg: 25,
  elements: [{ kind: "building", presetId: "rectangular", count: 0 }],
};

describe("RecipeForm", () => {
  it("sends a whole-form error to the first count field, not the preset list", async () => {
    const wrapper = mount(RecipeForm, {
      attachTo: document.body,
      props: {
        catalog,
        presetsRoot: "/presets",
        recipe,
        outputDir: "/tmp/out",
        issues: [{ field: "elements", message: "во всех элементах нулевое количество" }],
        running: false,
      },
    });
    await wrapper.find(".validation-summary button").trigger("click");
    expect(document.activeElement?.id).toBe("field-element-building-count");
    wrapper.unmount();
  });
});
