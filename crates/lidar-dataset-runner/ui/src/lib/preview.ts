import type { ClassDto, ScenePreview } from "../api";

export type Camera = { yaw: number; pitch: number; zoom: number };

export type Bounds = {
  center: [number, number, number];
  size: number;
  height: number;
};

export type ProjectedPoint = { x: number; y: number; depth: number };

const EMPTY: Bounds = { center: [0, 0, 0], size: 0, height: 0 };

const DEFAULT_PITCH = 0.62;
const DEFAULT_YAW = 0.7;

export const DEFAULT_CAMERA: Camera = { yaw: DEFAULT_YAW, pitch: DEFAULT_PITCH, zoom: 1 };

export function boundsOf(points: Float32Array): Bounds {
  if (points.length < 4) {
    return EMPTY;
  }
  let minX = Number.POSITIVE_INFINITY;
  let minY = Number.POSITIVE_INFINITY;
  let minZ = Number.POSITIVE_INFINITY;
  let maxX = Number.NEGATIVE_INFINITY;
  let maxY = Number.NEGATIVE_INFINITY;
  let maxZ = Number.NEGATIVE_INFINITY;

  for (let index = 0; index < points.length; index += 4) {
    const x = points[index];
    const y = points[index + 1];
    const z = points[index + 2];
    if (x < minX) minX = x;
    if (y < minY) minY = y;
    if (z < minZ) minZ = z;
    if (x > maxX) maxX = x;
    if (y > maxY) maxY = y;
    if (z > maxZ) maxZ = z;
  }

  const size = Math.max(maxX - minX, maxY - minY, 0.001);
  return {
    center: [(minX + maxX) / 2, (minY + maxY) / 2, (minZ + maxZ) / 2],
    size,
    height: maxZ - minZ,
  };
}

export function sceneExtent(points: Float32Array, bounds: Bounds): number[] {
  const extent: number[] = [];
  const centre = bounds.center;
  const scale = 1 / (bounds.size || 1);
  for (let index = 0; index < points.length; index += 4) {
    extent.push((points[index] - centre[0]) * scale);
    extent.push((points[index + 1] - centre[1]) * scale);
    extent.push((points[index + 2] - centre[2]) * scale);
  }
  return extent;
}

export function projectPoint(
  point: [number, number, number],
  camera: Camera,
  bounds: Bounds,
  width: number,
  height: number,
): ProjectedPoint {
  const scale = 1 / (bounds.size || 1);
  const x = (point[0] - bounds.center[0]) * scale;
  const y = (point[1] - bounds.center[1]) * scale;
  const z = (point[2] - bounds.center[2]) * scale;

  const cosYaw = Math.cos(camera.yaw);
  const sinYaw = Math.sin(camera.yaw);
  const yawedX = x * cosYaw - y * sinYaw;
  const yawedY = x * sinYaw + y * cosYaw;

  const cosPitch = Math.cos(camera.pitch);
  const sinPitch = Math.sin(camera.pitch);
  const screenX = yawedX;
  const screenY = yawedY * sinPitch + z * cosPitch;
  const depth = yawedY * cosPitch - z * sinPitch;

  const fit = Math.min(width, height) * 0.82 * camera.zoom;
  return {
    x: width / 2 + screenX * fit,
    y: height / 2 - screenY * fit,
    depth,
  };
}

export function classColor(code: number, classes: ClassDto[]): string {
  return classes.find((item) => item.code === code)?.color ?? "var(--muted)";
}

export function previewPoints(preview: ScenePreview): Float32Array {
  return Float32Array.from(preview.points);
}
