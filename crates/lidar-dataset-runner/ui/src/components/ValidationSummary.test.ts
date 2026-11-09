import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import ValidationSummary from "./ValidationSummary.vue";

describe("ValidationSummary", () => {
  it("renders nothing when there are no issues", () => {
    const wrapper = mount(ValidationSummary, { props: { issues: [] } });
    expect(wrapper.find(".validation-summary").exists()).toBe(false);
  });

  it("counts issues and lists messages", () => {
    const wrapper = mount(ValidationSummary, {
      props: {
        issues: [
          { field: "scenes", message: "число сцен должно быть от 1 до 1000" },
          { field: "elements[2].count", message: "«bush»: не больше 2000 объектов одного вида" },
        ],
      },
    });
    expect(wrapper.text()).toContain("2 ошибки в форме");
    expect(wrapper.text()).toContain("число сцен");
  });

  it("asks the form to focus the clicked field", async () => {
    const wrapper = mount(ValidationSummary, {
      props: { issues: [{ field: "scenes", message: "нет сцен" }] },
    });
    await wrapper.find("button").trigger("click");
    expect(wrapper.emitted("focus-field")?.[0]).toEqual(["scenes"]);
  });
});
