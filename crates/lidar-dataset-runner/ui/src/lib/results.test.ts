import { describe, expect, it } from "vitest";
import {
  objectSummaryText,
  placedLessThanRequested,
  summariseObjects,
} from "./results";

const objects = {
  building: { requested: 4, placed: 4 },
  tree: { requested: 6, placed: 6 },
  bush: { requested: 10, placed: 7 },
  grass: { requested: 400, placed: 0 },
};

describe("summariseObjects", () => {
  it("names kinds in russian and keeps placement counts", () => {
    expect(summariseObjects(objects)).toEqual([
      { title: "Здания", placed: 4, requested: 4, short: false },
      { title: "Деревья", placed: 6, requested: 6, short: false },
      { title: "Кусты", placed: 7, requested: 10, short: true },
      { title: "Трава", placed: 0, requested: 400, short: true },
    ]);
  });

  it("skips kinds that were not requested and not placed", () => {
    expect(summariseObjects({ tree: { requested: 2, placed: 2 }, grass: { requested: 0, placed: 0 } }))
      .toHaveLength(1);
  });
});

describe("objectSummaryText", () => {
  it("reads as a sentence", () => {
    expect(objectSummaryText(objects)).toBe("Здания 4 из 4, деревья 6 из 6, кусты 7 из 10, трава 0 из 400");
  });
});

describe("placedLessThanRequested", () => {
  it("is true when a kind fell short", () => {
    expect(placedLessThanRequested(objects)).toBe(true);
    expect(placedLessThanRequested({ tree: { requested: 2, placed: 2 } })).toBe(false);
  });
});
