const NBSP = "\u00a0";

const BYTE_UNITS = ["Б", "КБ", "МБ", "ГБ"];

export function groupDigits(value: number): string {
  const negative = value < 0;
  const digits = String(Math.abs(Math.trunc(value)));
  let grouped = "";
  for (let index = 0; index < digits.length; index += 1) {
    if (index > 0 && (digits.length - index) % 3 === 0) {
      grouped += NBSP;
    }
    grouped += digits[index];
  }
  return negative ? `-${grouped}` : grouped;
}

export function formatPoints(value: number): string {
  return groupDigits(Math.round(value));
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${Math.round(bytes)} ${BYTE_UNITS[0]}`;
  }
  let unitIndex = 0;
  let scaled = bytes;
  while (scaled >= 1024 && unitIndex < BYTE_UNITS.length - 1) {
    scaled /= 1024;
    unitIndex += 1;
  }
  const rounded = Math.round(scaled * 10) / 10;
  const text = String(rounded).replace(".", ",");
  return `${text} ${BYTE_UNITS[unitIndex]}`;
}

export function formatDuration(ms: number): string {
  const safe = Math.max(0, ms);
  if (safe < 60_000) {
    const seconds = Math.round(safe / 100) / 10;
    return `${String(seconds).replace(".", ",")} с`;
  }
  if (safe < 3_600_000) {
    const totalSeconds = Math.round(safe / 1000);
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes} мин ${String(seconds).padStart(2, "0")} с`;
  }
  const totalMinutes = Math.round(safe / 60_000);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return `${hours} ч ${String(minutes).padStart(2, "0")} мин`;
}

export function percent(done: number, total: number): number {
  if (total <= 0) {
    return 0;
  }
  return Math.min(100, Math.max(0, Math.round((done / total) * 100)));
}

export function etaMs(elapsedMs: number, done: number, total: number): number | null {
  if (done <= 0 || total <= 0) {
    return null;
  }
  return Math.max(0, Math.round((elapsedMs / done) * (total - done)));
}

export function pluralRu(value: number, forms: [string, string, string]): string {
  const absolute = Math.abs(Math.trunc(value)) % 100;
  if (absolute >= 11 && absolute <= 14) {
    return forms[2];
  }
  const last = absolute % 10;
  if (last === 1) {
    return forms[0];
  }
  if (last >= 2 && last <= 4) {
    return forms[1];
  }
  return forms[2];
}
