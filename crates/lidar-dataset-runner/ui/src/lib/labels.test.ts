import { describe, expect, it } from "vitest";
import { ELEMENT_TITLE, FIELD_HINT, PLACEMENT_NOTE, RUN_STATE_LABEL, stateLabel } from "./labels";

describe("stateLabel", () => {
  it("speaks russian for every run state", () => {
    expect(stateLabel("idle")).toBe("Готов к запуску");
    expect(stateLabel("running")).toBe("Идёт генерация");
    expect(stateLabel("finished")).toBe("Готово");
    expect(stateLabel("failed")).toBe("Ошибка");
    expect(stateLabel("cancelled")).toBe("Отменено");
  });

  it("has no english fallthrough", () => {
    expect(Object.values(RUN_STATE_LABEL).some((v) => /[a-z]/i.test(v.replace(/[ёЁ]/g, "")))).toBe(false);
  });
});

describe("element titles", () => {
  it("names all four element kinds", () => {
    expect(ELEMENT_TITLE.building).toBe("Здания");
    expect(ELEMENT_TITLE.tree).toBe("Деревья");
    expect(ELEMENT_TITLE.bush).toBe("Кусты");
    expect(ELEMENT_TITLE.grass).toBe("Трава");
  });
});

describe("field hints", () => {
  it("explains every form field without jargon", () => {
    expect(FIELD_HINT.scenes).toContain("файл");
    expect(FIELD_HINT.seed).toContain("одинаков");
    expect(FIELD_HINT.maxSlopeDeg).toContain("уклон");
    expect(FIELD_HINT.landscape).toContain("рельеф");
    expect(FIELD_HINT.outputDir).toContain("папк");
    expect(PLACEMENT_NOTE).toContain("объект");
  });
});
