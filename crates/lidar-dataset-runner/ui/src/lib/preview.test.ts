import { describe, expect, it } from "vitest";
import type { ClassDto } from "../api";
import { boundsOf, classColor, projectPoint, sceneExtent } from "./preview";

const classes: ClassDto[] = [
  { code: 2, name: "ground", label: "Земля", color: "#6b6b4a" },
  { code: 6, name: "building", label: "Здания", color: "#b0624a" },
];

const flat = new Float32Array([
  0, 0, 0, 2,
  100, 0, 0, 2,
  100, 100, 0, 2,
  0, 100, 0, 2,
]);

const view = { yaw: 0, pitch: 0, zoom: 1 };

describe("boundsOf", () => {
  it("measures the point cloud in world units", () => {
    const bounds = boundsOf(flat);
    expect(bounds.center).toEqual([50, 50, 0]);
    expect(bounds.size).toBeCloseTo(100);
    expect(bounds.height).toBe(0);
  });

  it("survives an empty cloud", () => {
    const bounds = boundsOf(new Float32Array());
    expect(bounds.size).toBe(0);
    expect(bounds.center).toEqual([0, 0, 0]);
  });
});

describe("sceneExtent", () => {
  it("scales the cloud down to a unit box", () => {
    const bounds = boundsOf(flat);
    const extent = sceneExtent(flat, bounds);
    expect(Math.min(...extent)).toBeGreaterThanOrEqual(-0.5);
    expect(Math.max(...extent)).toBeLessThanOrEqual(0.5);
  });
});

describe("projectPoint", () => {
  const bounds = boundsOf(flat);

  it("puts the centre of the cloud in the middle of the viewport", () => {
    const point = projectPoint([50, 50, 0], view, bounds, 400, 300);
    expect(point.x).toBeCloseTo(200, 5);
    expect(point.y).toBeCloseTo(150, 5);
  });

  it("grows with zoom", () => {
    const near = projectPoint([100, 50, 0], view, bounds, 400, 300);
    const zoomed = projectPoint([100, 50, 0], { ...view, zoom: 2 }, bounds, 400, 300);
    expect(zoomed.x - 200).toBeGreaterThan(near.x - 200);
  });

  it("lifts the far part of the ground toward the horizon when the camera tilts", () => {
    const far = projectPoint([50, 100, 0], { ...view, pitch: Math.PI / 4 }, bounds, 400, 300);
    const centre = projectPoint([50, 50, 0], { ...view, pitch: Math.PI / 4 }, bounds, 400, 300);
    expect(far.y).toBeLessThan(centre.y);
  });

  it("turns the cloud when the camera yaws", () => {
    const turned = projectPoint([50, 100, 0], { ...view, yaw: Math.PI / 2 }, bounds, 400, 300);
    expect(turned.x).toBeLessThan(200);
  });

  it("reports a depth that grows away from the camera", () => {
    const near = projectPoint([50, 0, 0], view, bounds, 400, 300);
    const far = projectPoint([50, 100, 0], view, bounds, 400, 300);
    expect(far.depth).toBeGreaterThan(near.depth);
  });
});

describe("classColor", () => {
  it("uses the palette from the backend", () => {
    expect(classColor(6, classes)).toBe("#b0624a");
  });

  it("falls back to a muted grey for unknown classes", () => {
    expect(classColor(99, classes)).toBe("var(--muted)");
  });
});
