import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import type { ClassDto, RunReport } from "../api";
import StatsTable from "./StatsTable.vue";

const classes: ClassDto[] = [
  { code: 2, name: "ground", label: "Земля", color: "#6b6b4a" },
  { code: 6, name: "building", label: "Здания", color: "#b0624a" },
];

const scene = (index: number) => ({
  index,
  seed: 42 + index,
  file: `scene-000${index}.xyz`,
  bytes: 2048,
  points: 1000,
  byClass: { "2": 700, "6": 300 },
  objects: { building: { requested: 2, placed: 2 } },
  bbox: {
    min: [0, 0, 0] as [number, number, number],
    max: [100, 100, 5] as [number, number, number],
  },
  elapsedMs: 900,
  frame: "z-up",
});

const report: RunReport = {
  jobId: "job-1",
  outputDir: "/tmp/out",
  recipe: {
    scenes: 2,
    seed: 42,
    landscape: "plain",
    maxSlopeDeg: 25,
    elements: [{ kind: "building", presetId: "rectangular", count: 2 }],
  },
  resolved: {},
  scenes: [scene(0), scene(1)],
  totals: { scenes: 2, points: 2000, byClass: { "2": 1400, "6": 600 }, bytes: 4096, elapsedMs: 1800 },
  errors: [],
  cancelled: false,
};

describe("StatsTable", () => {
  it("starts with an empty state and no table", () => {
    const wrapper = mount(StatsTable, { props: { report: null, classes } });
    expect(wrapper.find("table").exists()).toBe(false);
    expect(wrapper.text()).toContain("Здесь появятся сцены");
  });

  it("opens a scene in the viewer when its row is clicked", async () => {
    const wrapper = mount(StatsTable, { props: { report, classes } });
    const rows = wrapper.findAll("tbody tr.scene-row");
    expect(rows).toHaveLength(2);
    await rows[1].trigger("click");
    expect(wrapper.emitted("select-scene")?.[0]).toEqual([1]);
  });

  it("also opens a scene from the keyboard", async () => {
    const wrapper = mount(StatsTable, { props: { report, classes } });
    await wrapper.findAll("tbody tr.scene-row")[0].trigger("keydown.enter");
    expect(wrapper.emitted("select-scene")?.[0]).toEqual([0]);
  });

  it("keeps the totals row out of the click target", () => {
    const wrapper = mount(StatsTable, { props: { report, classes } });
    expect(wrapper.find("tbody tr.totals").attributes("role")).toBeUndefined();
  });
});
