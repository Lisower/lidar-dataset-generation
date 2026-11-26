import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface PresetDto {
  id: string;
  kind: string;
  name: string;
  description: string;
  plantType: string | null;
}

export interface Catalog {
  landscapes: PresetDto[];
  buildings: PresetDto[];
  plants: PresetDto[];
}

export interface CatalogInfo {
  presetsRoot: string | null;
  catalog: Catalog;
}

export interface ClassDto {
  code: number;
  name: string;
  label: string;
  color: string;
}

export type ElementKind = "building" | "tree" | "bush" | "grass";

export interface ElementSpec {
  kind: ElementKind;
  presetId: string;
  count: number;
}

export interface Recipe {
  scenes: number;
  seed: number;
  landscape: string;
  maxSlopeDeg: number;
  elements: ElementSpec[];
}

export interface ValidationIssue {
  field: string;
  message: string;
}

export interface ObjectCounts {
  requested: number;
  placed: number;
}

export interface BBox {
  min: [number, number, number];
  max: [number, number, number];
}

export interface SceneStats {
  index: number;
  seed: number;
  file: string;
  bytes: number;
  points: number;
  byClass: Record<string, number>;
  objects: Record<string, ObjectCounts>;
  bbox: BBox;
  elapsedMs: number;
  frame: string;
}

export interface ProgressEvent {
  jobId: string;
  scenesDone: number;
  scenesTotal: number;
  pointsTotal: number;
  elapsedMs: number;
  last: SceneStats;
}

export interface Totals {
  scenes: number;
  points: number;
  byClass: Record<string, number>;
  bytes: number;
  elapsedMs: number;
}

export interface SceneError {
  index: number;
  message: string;
}

export interface RunReport {
  jobId: string;
  outputDir: string;
  recipe: Recipe;
  resolved: Record<string, string>;
  scenes: SceneStats[];
  totals: Totals;
  errors: SceneError[];
  cancelled: boolean;
}

export interface ScenePreview {
  index: number;
  seed: number;
  frame: string;
  bbox: BBox;
  points: number[];
  classes: number[];
  pointsTotal: number;
  pointsShown: number;
  stats: SceneStats;
}

export const getCatalog = () => invoke<CatalogInfo>("catalog");
export const getPointClasses = () => invoke<ClassDto[]>("point_classes");
export const getDefaultOutputDir = () => invoke<string>("default_output_dir");

export const previewGeneration = (recipe: Recipe, index: number) =>
  invoke<ScenePreview>("preview_generation", { recipe, index });

export const startGeneration = (recipe: Recipe, outputDir: string) =>
  invoke<string>("start_generation", { recipe, outputDir });

export const cancelGeneration = (jobId: string) =>
  invoke<boolean>("cancel_generation", { jobId });

export const onProgress = (cb: (event: ProgressEvent) => void) =>
  listen<ProgressEvent>("generation://progress", (e) => cb(e.payload));

export const onFinished = (cb: (report: RunReport) => void) =>
  listen<RunReport>("generation://finished", (e) => cb(e.payload));

export const onFailed = (cb: (message: string) => void) =>
  listen<string>("generation://failed", (e) => cb(e.payload));

export type { UnlistenFn };
