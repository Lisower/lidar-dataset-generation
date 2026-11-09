import { describe, expect, it } from "vitest";
import {
  etaMs,
  formatBytes,
  formatDuration,
  formatPoints,
  groupDigits,
  percent,
  pluralRu,
} from "./format";

const NBSP = "\u00a0";

describe("groupDigits", () => {
  it("groups thousands with a non-breaking space", () => {
    expect(groupDigits(1234567)).toBe(`1${NBSP}234${NBSP}567`);
    expect(groupDigits(999)).toBe("999");
    expect(groupDigits(0)).toBe("0");
    expect(groupDigits(-12500)).toBe(`-12${NBSP}500`);
  });
});

describe("formatPoints", () => {
  it("rounds and groups", () => {
    expect(formatPoints(12345.6)).toBe(`12${NBSP}346`);
  });
});

describe("formatBytes", () => {
  it("uses byte units with a russian decimal comma", () => {
    expect(formatBytes(512)).toBe("512 Б");
    expect(formatBytes(1536)).toBe("1,5 КБ");
    expect(formatBytes(1024 * 1024 * 12.44)).toBe("12,4 МБ");
    expect(formatBytes(1024 * 1024 * 1024 * 2.5)).toBe("2,5 ГБ");
  });

  it("drops a zero fraction", () => {
    expect(formatBytes(2048)).toBe("2 КБ");
  });
});

describe("formatDuration", () => {
  it("shows fractions of a second, then minutes, then hours", () => {
    expect(formatDuration(840)).toBe("0,8 с");
    expect(formatDuration(65_000)).toBe("1 мин 05 с");
    expect(formatDuration(3_725_000)).toBe("1 ч 02 мин");
  });

  it("drops a zero fraction of a second", () => {
    expect(formatDuration(2000)).toBe("2 с");
    expect(formatDuration(8000)).toBe("8 с");
  });
});

describe("percent", () => {
  it("clamps and rounds", () => {
    expect(percent(1, 3)).toBe(33);
    expect(percent(5, 5)).toBe(100);
    expect(percent(7, 5)).toBe(100);
    expect(percent(0, 0)).toBe(0);
  });
});

describe("etaMs", () => {
  it("projects remaining time from the average per scene", () => {
    expect(etaMs(2000, 2, 10)).toBe(8000);
  });

  it("returns null before the first scene is done", () => {
    expect(etaMs(0, 0, 10)).toBeNull();
    expect(etaMs(500, 0, 10)).toBeNull();
    expect(etaMs(500, 1, 0)).toBeNull();
  });
});

describe("pluralRu", () => {
  const scenes = ["сцена", "сцены", "сцен"] as [string, string, string];

  it("picks the russian plural form", () => {
    expect(pluralRu(1, scenes)).toBe("сцена");
    expect(pluralRu(3, scenes)).toBe("сцены");
    expect(pluralRu(5, scenes)).toBe("сцен");
    expect(pluralRu(11, scenes)).toBe("сцен");
    expect(pluralRu(21, scenes)).toBe("сцена");
    expect(pluralRu(0, scenes)).toBe("сцен");
  });
});
