import type { ElementKind, ObjectCounts } from "../api";
import { ELEMENT_TITLE } from "./labels";

export type ObjectSummaryRow = {
  title: string;
  placed: number;
  requested: number;
  short: boolean;
};

const ORDER: ElementKind[] = ["building", "tree", "bush", "grass"];

export function summariseObjects(objects: Record<string, ObjectCounts>): ObjectSummaryRow[] {
  const rows: ObjectSummaryRow[] = [];
  for (const kind of ORDER) {
    const counts = objects[kind];
    if (!counts) {
      continue;
    }
    if (counts.requested === 0 && counts.placed === 0) {
      continue;
    }
    rows.push({
      title: ELEMENT_TITLE[kind],
      placed: counts.placed,
      requested: counts.requested,
      short: counts.placed < counts.requested,
    });
  }
  return rows;
}

export function objectSummaryText(objects: Record<string, ObjectCounts>): string {
  return summariseObjects(objects)
    .map((row, index) => {
      const title = index === 0 ? row.title : row.title.toLowerCase();
      return `${title} ${row.placed} из ${row.requested}`;
    })
    .join(", ");
}

export function placedLessThanRequested(record: Record<string, ObjectCounts>): boolean {
  return Object.values(record).some((counts) => counts.placed < counts.requested);
}

export async function copyText(text: string): Promise<boolean> {
  try {
    if (!navigator.clipboard?.writeText) {
      return false;
    }
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}
