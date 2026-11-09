import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import type { ClassDto, ScenePreview } from "../api";
import SceneViewer from "./SceneViewer.vue";

const classes: ClassDto[] = [
  { code: 2, name: "ground", label: "Земля", color: "#6b6b4a" },
  { code: 6, name: "building", label: "Здания", color: "#b0624a" },
];

const preview: ScenePreview = {
  index: 0,
  seed: 42,
  frame: "z-up",
  bbox: { min: [0, 0, 0], max: [100, 100, 10] },
  points: [0, 0, 0, 2, 100, 0, 0, 2, 100, 100, 5, 6, 0, 100, 0, 2],
  classes: [2, 6],
  pointsTotal: 4,
  pointsShown: 4,
  stats: {
    index: 0,
    seed: 42,
    file: "",
    bytes: 0,
    points: 4,
    byClass: { "2": 3, "6": 1 },
    objects: { building: { requested: 1, placed: 1 } },
    bbox: { min: [0, 0, 0], max: [100, 100, 10] },
    elapsedMs: 0,
    frame: "z-up",
  },
};

const baseProps = {
  preview,
  classes,
  loading: false,
  error: "",
  available: true,
  canPreview: true,
};

describe("SceneViewer", () => {
  it("invites the user to build a scene before anything exists", () => {
    const wrapper = mount(SceneViewer, {
      props: { ...baseProps, preview: null, available: true },
    });
    expect(wrapper.text()).toContain("Показать сцену");
    expect(wrapper.find("canvas").exists()).toBe(false);
  });

  it("says why the preview is unavailable without a preset catalogue", () => {
    const wrapper = mount(SceneViewer, {
      props: { ...baseProps, preview: null, available: false },
    });
    expect(wrapper.text()).toContain("Каталог пресетов не найден");
  });

  it("draws the cloud and lists the classes it contains", () => {
    const wrapper = mount(SceneViewer, { props: baseProps });
    expect(wrapper.find("canvas").exists()).toBe(true);
    expect(wrapper.text()).toContain("4");
    expect(wrapper.text()).toContain("Земля");
    expect(wrapper.text()).toContain("Здания");
  });

  it("asks the parent to build the preview", async () => {
    const wrapper = mount(SceneViewer, { props: baseProps });
    const buttons = wrapper.findAll("button");
    const refresh = buttons.find((button) => button.text().includes("Показать сцену"));
    await refresh?.trigger("click");
    expect(wrapper.emitted("preview")).toHaveLength(1);
  });

  it("shows a spinner state while the scene is being built", () => {
    const wrapper = mount(SceneViewer, {
      props: { ...baseProps, preview: null, loading: true },
    });
    expect(wrapper.text()).toContain("Собираем сцену");
    expect(wrapper.find("#preview-build").attributes("disabled")).toBeDefined();
  });

  it("shows the backend error instead of an empty canvas", () => {
    const wrapper = mount(SceneViewer, {
      props: { ...baseProps, preview: null, error: "ландшафт «river» не найден" },
    });
    expect(wrapper.text()).toContain("ландшафт «river» не найден");
  });

  it("switches the scene index", async () => {
    const wrapper = mount(SceneViewer, { props: { ...baseProps, scenesTotal: 5 } });
    await wrapper.find("#preview-scene").setValue("3");
    expect(wrapper.emitted("update:index")?.[0]).toEqual([3]);
  });
});
