<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { Eye, RotateCcw } from "lucide-vue-next";
import type { ClassDto, ScenePreview } from "../api";
import { formatPoints } from "../lib/format";
import { DEFAULT_CAMERA, boundsOf, classColor, projectPoint, type Camera } from "../lib/preview";

const props = withDefaults(
  defineProps<{
    preview: ScenePreview | null;
    classes: ClassDto[];
    loading: boolean;
    error: string;
    available: boolean;
    canPreview: boolean;
    scenesTotal?: number;
    index?: number;
  }>(),
  { scenesTotal: 1, index: 0 },
);

const emit = defineEmits<{
  (e: "preview"): void;
  (e: "update:index", value: number): void;
}>();

const canvas = ref<HTMLCanvasElement | null>(null);
const frame = ref<HTMLElement | null>(null);
const camera = ref<Camera>({ ...DEFAULT_CAMERA });
const size = ref({ width: 0, height: 0 });

const points = computed(() => (props.preview ? Float32Array.from(props.preview.points) : null));
const bounds = computed(() => boundsOf(points.value ?? new Float32Array()));

const classRows = computed(() => {
  const byClass = props.preview?.stats.byClass ?? {};
  return Object.entries(byClass)
    .map(([code, count]) => ({
      code: Number(code),
      count,
      label: props.classes.find((item) => item.code === Number(code))?.label ?? `класс ${code}`,
      color: classColor(Number(code), props.classes),
    }))
    .sort((a, b) => b.count - a.count);
});

const shownText = computed(() => {
  if (!props.preview) {
    return "";
  }
  const total = props.preview.pointsTotal;
  const shown = props.preview.pointsShown;
  return shown < total
    ? `${formatPoints(shown)} из ${formatPoints(total)} точек`
    : `${formatPoints(total)} точек`;
});

function draw() {
  const element = canvas.value;
  if (!element) {
    return;
  }
  let context: CanvasRenderingContext2D | null = null;
  try {
    context = element.getContext("2d");
  } catch {
    // jsdom без пакета canvas не умеет рисовать: в тестах проверяется
    // разметка, а не пиксели.
    return;
  }
  if (!context) {
    return;
  }

  const ratio = window.devicePixelRatio || 1;
  const width = Math.max(1, element.clientWidth);
  const height = Math.max(1, element.clientHeight);
  element.width = Math.round(width * ratio);
  element.height = Math.round(height * ratio);
  context.setTransform(ratio, 0, 0, ratio, 0, 0);
  context.clearRect(0, 0, width, height);

  const cloud = points.value;
  if (!cloud || cloud.length < 4) {
    return;
  }

  const view = camera.value;
  const box = bounds.value;
  const projected: { x: number; y: number; depth: number; color: string }[] = [];
  const palette = new Map<number, string>();

  for (let index = 0; index < cloud.length; index += 4) {
    const code = cloud[index + 3];
    let color = palette.get(code);
    if (color === undefined) {
      color = classColor(code, props.classes);
      palette.set(code, color);
    }
    const point = projectPoint(
      [cloud[index], cloud[index + 1], cloud[index + 2]],
      view,
      box,
      width,
      height,
    );
    projected.push({ ...point, color });
  }

  // Дальние точки рисуются первыми: без этого здание заслоняло бы землю,
  // которая к камере ближе.
  projected.sort((a, b) => b.depth - a.depth);

  const radius = width > 900 ? 1.3 : 1;
  for (const point of projected) {
    context.fillStyle = point.color;
    context.fillRect(point.x, point.y, radius, radius);
  }
}

let resizeObserver: ResizeObserver | null = null;

onMounted(() => {
  draw();
  if (typeof ResizeObserver !== "undefined" && frame.value) {
    resizeObserver = new ResizeObserver(() => draw());
    resizeObserver.observe(frame.value);
  }
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  resizeObserver = null;
});

watch(() => [props.preview, props.classes], () => draw(), { deep: false });

let dragging = false;
let lastX = 0;
let lastY = 0;

function onPointerDown(event: PointerEvent) {
  dragging = true;
  lastX = event.clientX;
  lastY = event.clientY;
  (event.target as HTMLElement).setPointerCapture?.(event.pointerId);
}

function onPointerMove(event: PointerEvent) {
  if (!dragging) {
    return;
  }
  const dx = event.clientX - lastX;
  const dy = event.clientY - lastY;
  lastX = event.clientX;
  lastY = event.clientY;
  camera.value = {
    ...camera.value,
    yaw: camera.value.yaw - dx * 0.008,
    pitch: Math.min(1.45, Math.max(0.02, camera.value.pitch + dy * 0.006)),
  };
  draw();
}

function onPointerUp() {
  dragging = false;
}

function onWheel(event: WheelEvent) {
  event.preventDefault();
  const factor = event.deltaY < 0 ? 1.12 : 1 / 1.12;
  camera.value = {
    ...camera.value,
    zoom: Math.min(12, Math.max(0.25, camera.value.zoom * factor)),
  };
  draw();
}

function resetCamera() {
  camera.value = { ...DEFAULT_CAMERA };
  draw();
}

function build() {
  if (props.canPreview && !props.loading) {
    emit("preview");
  }
}

function setIndex(event: Event) {
  emit("update:index", Number((event.target as HTMLSelectElement).value));
}
</script>

<template>
  <section class="viewer card">
    <div class="viewer-head">
      <h2>Просмотр сцены</h2>
      <div class="viewer-controls">
        <label v-if="scenesTotal > 1" class="scene-picker">
          Сцена
          <select id="preview-scene" :value="index" @change="setIndex">
            <option v-for="n in scenesTotal" :key="n" :value="n - 1">{{ n }}</option>
          </select>
        </label>
        <button
          id="preview-build"
          type="button"
          class="btn"
          :disabled="!canPreview || loading"
          @click="build"
        >
          <Eye :size="16" aria-hidden="true" />
          Показать сцену
        </button>
        <button
          type="button"
          class="btn btn-ghost"
          :disabled="!preview"
          title="Вернуть камеру в исходное положение"
          @click="resetCamera"
        >
          <RotateCcw :size="16" aria-hidden="true" />
          Камера
        </button>
      </div>
    </div>

    <p v-if="!available" class="notice muted">
      Каталог пресетов не найден, показать сцену не из чего.
    </p>
    <p v-else-if="loading" class="notice muted">Собираем сцену: рельеф и объекты.</p>
    <p v-else-if="error" class="notice error">{{ error }}</p>
    <p v-else-if="!preview" class="notice muted">
      Нажмите «Показать сцену», чтобы увидеть рельеф и объекты из облака точек.
    </p>

    <div
      v-if="preview"
      ref="frame"
      class="canvas-frame"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @pointerleave="onPointerUp"
      @wheel="onWheel"
    >
      <canvas ref="canvas" aria-label="Облако точек сцены"></canvas>
    </div>
    <div v-else class="canvas-frame placeholder" aria-hidden="true"></div>

    <div v-if="preview" class="viewer-foot">
      <ul class="legend">
        <li v-for="row in classRows" :key="row.code">
          <span class="dot" :style="{ background: row.color }" />
          {{ row.label }}
          <span class="numeric muted">{{ formatPoints(row.count) }}</span>
        </li>
      </ul>
      <p class="muted numeric">{{ shownText }}, сцена с сидом {{ preview.seed }}</p>
    </div>
  </section>
</template>

<style scoped>
.viewer {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-height: 0;
  height: 100%;
}

.viewer-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.viewer-controls {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
}

.scene-picker {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  font-size: 13px;
  color: var(--muted);
}

.notice {
  font-size: 13px;
}

.notice.error {
  color: var(--danger);
}

.canvas-frame {
  flex: 1;
  min-height: 220px;
  border-radius: var(--radius-sm);
  background:
    linear-gradient(var(--surface-2), var(--surface-2)) padding-box,
    radial-gradient(circle at 50% 40%, color-mix(in srgb, var(--accent) 18%, transparent), transparent 70%);
  border: 1px solid var(--border);
  overflow: hidden;
  cursor: grab;
  touch-action: none;
}

.canvas-frame:active {
  cursor: grabbing;
}

.placeholder {
  cursor: default;
  background-image: var(--surface-2);
}

canvas {
  display: block;
  width: 100%;
  height: 100%;
}

.viewer-foot {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  font-size: 12px;
}

.legend {
  list-style: none;
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
}

.legend li {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  color: var(--muted);
}

.dot {
  width: 9px;
  height: 9px;
  border-radius: 50%;
}
</style>
