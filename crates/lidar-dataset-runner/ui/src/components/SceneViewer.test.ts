import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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

type FakeContext = { setTransform: () => void; clearRect: () => void; fillRect: () => void };
let context: FakeContext;
let getContext: ReturnType<typeof vi.fn>;

beforeEach(() => {
  context = { setTransform: vi.fn(), clearRect: vi.fn(), fillRect: vi.fn() };
  getContext = vi.fn(() => context);
  HTMLCanvasElement.prototype.getContext = getContext as unknown as typeof HTMLCanvasElement.prototype.getContext;
});

afterEach(() => {
  vi.restoreAllMocks();
});

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

  it("paints the scene as soon as it arrives, without any pointer event", async () => {
    const wrapper = mount(SceneViewer, {
      props: { ...baseProps, preview: null },
    });
    expect(getContext).not.toHaveBeenCalled();

    await wrapper.setProps({ preview });
    await flushPromises();

    expect(getContext).toHaveBeenCalled();
    expect(context.fillRect).toHaveBeenCalled();
  });

  it("paints a scene that was already present when the view was created", async () => {
    mount(SceneViewer, { props: baseProps });
    await flushPromises();
    expect(context.fillRect).toHaveBeenCalled();
  });

  it("still paints when the environment has no ResizeObserver", async () => {
    const original = globalThis.ResizeObserver;
    // @ts-expect-error окружение без ResizeObserver
    delete globalThis.ResizeObserver;
    try {
      mount(SceneViewer, { props: baseProps });
      await flushPromises();
      expect(context.fillRect).toHaveBeenCalled();
    } finally {
      globalThis.ResizeObserver = original;
    }
  });

  it("switches the scene index", async () => {
    const wrapper = mount(SceneViewer, { props: { ...baseProps, scenesTotal: 5 } });
    await wrapper.find("#preview-scene").setValue("3");
    expect(wrapper.emitted("update:index")?.[0]).toEqual([3]);
  });
});
