import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import type { PresetDto } from "../api";
import PresetSelect from "./PresetSelect.vue";

const presets: PresetDto[] = [
  { id: "spruce", kind: "spruce", name: "ель", description: "Хвойное дерево", plantType: "tree" },
  { id: "pine", kind: "pine", name: "сосна", description: "", plantType: "tree" },
];

describe("PresetSelect", () => {
  it("lists presets by russian name and describes the chosen one", () => {
    const wrapper = mount(PresetSelect, {
      props: { id: "field-element-tree", presets, modelValue: "spruce" },
    });
    const options = wrapper.findAll("option");
    expect(options).toHaveLength(2);
    expect(options[0].text()).toBe("ель");
    expect(wrapper.text()).toContain("Хвойное дерево");
  });

  it("emits the new id on change", async () => {
    const wrapper = mount(PresetSelect, { props: { id: "x", presets, modelValue: "spruce" } });
    await wrapper.find("select").setValue("pine");
    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual(["pine"]);
  });

  it("says what is missing when the catalogue is empty", () => {
    const wrapper = mount(PresetSelect, {
      props: { id: "x", presets: [], modelValue: "", emptyText: "в каталоге нет деревьев" },
    });
    expect(wrapper.find("select").exists()).toBe(false);
    expect(wrapper.text()).toContain("в каталоге нет деревьев");
  });
});
