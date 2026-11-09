import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import StatusPill from "./StatusPill.vue";

describe("StatusPill", () => {
  it("shows the russian label for the state", async () => {
    const wrapper = mount(StatusPill, { props: { state: "running" } });
    expect(wrapper.text()).toBe("Идёт генерация");
    await wrapper.setProps({ state: "cancelled" });
    expect(wrapper.text()).toBe("Отменено");
  });

  it("announces changes to screen readers", () => {
    const wrapper = mount(StatusPill, { props: { state: "failed" } });
    expect(wrapper.attributes("role")).toBe("status");
    expect(wrapper.attributes("aria-live")).toBe("polite");
    expect(wrapper.attributes("data-state")).toBe("failed");
  });
});
