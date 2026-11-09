import type { ElementKind } from "../api";

export type RunState = "idle" | "running" | "finished" | "failed" | "cancelled";

export const RUN_STATE_LABEL: Record<RunState, string> = {
  idle: "Готов к запуску",
  running: "Идёт генерация",
  finished: "Готово",
  failed: "Ошибка",
  cancelled: "Отменено",
};

export const ELEMENT_TITLE: Record<ElementKind, string> = {
  building: "Здания",
  tree: "Деревья",
  bush: "Кусты",
  grass: "Трава",
};

export const ELEMENT_HINT: Record<ElementKind, string> = {
  building: "Дома и многоэтажки, ставятся только на ровных участках",
  tree: "Деревья разного вида, растут группами",
  bush: "Кусты и кустарники, заполняют промежутки",
  grass: "Пучки травы, задают плотность сцены",
};

export const FIELD_HINT = {
  scenes: "Сколько файлов облака точек получится. Один файл это одна сцена 100 на 100 метров.",
  seed: "Число запуска. Одинаковое число даёт одинаковый датасет, другое число даёт новый.",
  maxSlopeDeg: "Здания не ставятся на участки с уклоном круче этого значения, в градусах.",
  landscape: "Тип рельефа, на который ставятся объекты: равнина, холмы, река, обрыв.",
  outputDir: "Куда писать датасет: папка для файлов облака точек и отчёта о запуске.",
};

export const PLACEMENT_NOTE =
  "Если объект не поместился, он не ставится: проверяются уклон, вода, края карты и занятая площадь.";

export const stateLabel = (state: RunState): string => RUN_STATE_LABEL[state];
