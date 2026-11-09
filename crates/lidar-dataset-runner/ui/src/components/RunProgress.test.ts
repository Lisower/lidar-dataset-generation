import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import type { ProgressEvent } from "../api";
import RunProgress from "./RunProgress.vue";

const lastScene = {
  index: 1,
  seed: 43,
  file: "scene-0001.xyz",
  bytes: 2048,
  points: 1000,
  byClass: { "2": 700, "6": 300 },
  objects: { building: { requested: 4, placed: 4 } },
  bbox: {
    min: [0, 0, 0] as [number, number, number],
    max: [1, 1, 1] as [number, number, number],
  },
  elapsedMs: 900,
  frame: "z-up",
};

const progress: ProgressEvent = {
  jobId: "job-1",
  scenesDone: 2,
  scenesTotal: 10,
  pointsTotal: 2000,
  elapsedMs: 2000,
  last: lastScene,
};

const classes = [
  { code: 2, name: "ground", label: "Земля", color: "#6b6b4a" },
  { code: 6, name: "building", label: "Здания", color: "#b0624a" },
];

describe("RunProgress", () => {
  it("shows what is happening before the first scene", () => {
    const wrapper = mount(RunProgress, {
      props: { progress: null, scenesTotal: 10, classes, running: true },
    });
    expect(wrapper.text()).toContain("Готовим первую сцену");
    expect(wrapper.find("[role=progressbar]").attributes("aria-valuenow")).toBe("0");
  });

  it("estimates the remaining time from the scenes already written", () => {
    const wrapper = mount(RunProgress, {
      props: { progress, scenesTotal: 10, classes, running: true },
    });
    expect(wrapper.text()).toContain("2 из 10 сцен");
    expect(wrapper.text()).toContain("Осталось примерно 8 с");
  });

  it("reports a finished run instead of an estimate", () => {
    const wrapper = mount(RunProgress, {
      props: { progress, scenesTotal: 10, classes, running: false },
    });
    expect(wrapper.text()).toContain("Готово за 2 с");
    expect(wrapper.text()).not.toContain("Осталось");
    expect(wrapper.text()).not.toContain("Готовим первую сцену");
  });

  it("lets the user stop the run", async () => {
    const wrapper = mount(RunProgress, {
      props: { progress: null, scenesTotal: 10, classes, running: true },
    });
    const cancel = wrapper.findComponent({ name: "CancelButton" });
    await cancel.trigger("click");
    await cancel.trigger("click");
    expect(wrapper.emitted("cancel")).toHaveLength(1);
  });
});
