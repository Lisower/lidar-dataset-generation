import { mount } from "@vue/test-utils";
import { afterEach, describe, expect, it, vi } from "vitest";
import CancelButton from "./CancelButton.vue";

afterEach(() => {
  vi.useRealTimers();
});

describe("CancelButton", () => {
  it("asks for confirmation before cancelling", async () => {
    const wrapper = mount(CancelButton, { props: { running: true } });
    expect(wrapper.text()).toBe("Остановить");
    await wrapper.trigger("click");
    expect(wrapper.text()).toBe("Точно остановить?");
    expect(wrapper.emitted("cancel")).toBeUndefined();
    await wrapper.trigger("click");
    expect(wrapper.emitted("cancel")).toHaveLength(1);
  });

  it("forgets the confirmation after four seconds", async () => {
    vi.useFakeTimers();
    const wrapper = mount(CancelButton, { props: { running: true } });
    await wrapper.trigger("click");
    vi.advanceTimersByTime(4000);
    await wrapper.vm.$nextTick();
    expect(wrapper.text()).toBe("Остановить");
  });

  it("is disabled while nothing is running", () => {
    const wrapper = mount(CancelButton, { props: { running: false } });
    expect(wrapper.find("button").attributes("disabled")).toBeDefined();
    expect(wrapper.text()).toBe("Остановить");
  });
});
