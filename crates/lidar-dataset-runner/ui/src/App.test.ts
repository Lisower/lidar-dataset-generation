import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

const state = vi.hoisted(() => ({
  presetsRoot: "/presets" as string | null,
  startMode: "error" as "error" | "validation",
  onFailed: (_message: string) => {},
}));

vi.mock("./api", () => ({
  getCatalog: () =>
    Promise.resolve({
      presetsRoot: state.presetsRoot,
      catalog: {
        landscapes: [{ id: "plain", kind: "plain", name: "равнина", description: "", plantType: null }],
        buildings: [
          { id: "rectangular", kind: "rectangular", name: "Прямоугольный", description: "", plantType: null },
        ],
        plants: [
          { id: "deciduous-tree", kind: "deciduous-tree", name: "дерево", description: "", plantType: "tree" },
          { id: "bush", kind: "bush", name: "куст", description: "", plantType: "bush" },
          { id: "sparse-grass", kind: "sparse-grass", name: "трава", description: "", plantType: "grass" },
        ],
      },
    }),
  getPointClasses: () => Promise.resolve([]),
  getDefaultOutputDir: () => Promise.resolve("/tmp/out"),
  startGeneration: () =>
    state.startMode === "validation"
      ? Promise.reject(
          JSON.stringify([{ field: "elements", message: "во всех элементах нулевое количество" }]),
        )
      : Promise.reject(new Error("нет каталога")),
  cancelGeneration: () => Promise.resolve(true),
  onProgress: () => Promise.resolve(() => {}),
  onFinished: () => Promise.resolve(() => {}),
  onFailed: (cb: (message: string) => void) => {
    state.onFailed = cb;
    return Promise.resolve(() => {});
  },
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: () => Promise.resolve(null) }));

import App from "./App.vue";

beforeEach(() => {
  localStorage.clear();
  state.presetsRoot = "/presets";
  state.startMode = "error";
  state.onFailed = () => {};
});

async function mountApp() {
  const wrapper = mount(App);
  await flushPromises();
  return wrapper;
}

describe("App shell", () => {
  it("explains a missing preset catalogue and blocks the run", async () => {
    state.presetsRoot = null;
    const wrapper = await mountApp();
    const banner = wrapper.find(".banner-error");
    expect(banner.exists()).toBe(true);
    expect(banner.text()).toContain("Каталог пресетов не найден");
    expect(wrapper.find("#start-run").attributes("disabled")).toBeDefined();
  });

  it("returns to an error state when the run fails", async () => {
    const wrapper = await mountApp();
    expect(wrapper.find("#start-run").attributes("disabled")).toBeUndefined();
    state.onFailed("генерация прервана паникой: index out of bounds");
    await flushPromises();
    expect(wrapper.text()).toContain("Запуск прерван");
    expect(wrapper.text()).toContain("генерация прервана паникой");
    expect(wrapper.find("#start-run").attributes("disabled")).toBeUndefined();
  });

  it("turns backend validation issues into a field summary instead of raw json", async () => {
    state.startMode = "validation";
    const wrapper = await mountApp();
    await wrapper.find("#start-run").trigger("click");
    await flushPromises();
    expect(wrapper.find(".validation-summary").exists()).toBe(true);
    expect(wrapper.text()).toContain("во всех элементах нулевое количество");
    expect(wrapper.text()).not.toContain("\"field\"");
    expect(wrapper.text()).not.toContain("Запуск прерван");
  });
});
