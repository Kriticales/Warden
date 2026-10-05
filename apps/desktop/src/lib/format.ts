/**
 * Números, tamanhos e horários em pt-BR com `Intl` (QUALITY §8.1; HANDOFF §8): "1.204",
 * "1,2 MB", "14:01", "01/10/2026".
 */
const LOCALE = 'pt-BR';

const integer = new Intl.NumberFormat(LOCALE, { maximumFractionDigits: 0 });
const time = new Intl.DateTimeFormat(LOCALE, { hour: '2-digit', minute: '2-digit' });
const date = new Intl.DateTimeFormat(LOCALE, { day: '2-digit', month: '2-digit', year: 'numeric' });

/** Número inteiro com separador de milhar: 1204 → "1.204". */
export function formatInteger(value: number): string {
  return integer.format(value);
}

const BYTE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB'] as const;

/**
 * Tamanho em bytes com base 1024 e uma casa decimal a partir de KB: 1_258_291 → "1,2 MB".
 * Valores abaixo de 1 KB ficam inteiros ("512 B").
 */
export function formatBytes(bytes: number): string {
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= 1024 && unit < BYTE_UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit === 0 ? 0 : 1;
  const text = new Intl.NumberFormat(LOCALE, {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(value);
  return `${text} ${BYTE_UNITS[unit] ?? 'B'}`;
}

/** Hora e minuto: "14:01". */
export function formatTime(ms: number): string {
  return time.format(ms);
}

/** Data: "01/10/2026". */
export function formatDate(ms: number): string {
  return date.format(ms);
}

/** Quando algo aconteceu, em relação a `now`: hoje, ontem ou outro dia. */
export type DayRelation = 'today' | 'yesterday' | 'other';

function startOfDay(ms: number): number {
  const day = new Date(ms);
  day.setHours(0, 0, 0, 0);
  return day.getTime();
}

/** Se `ms` é hoje, ontem ou outro dia (pelo relógio local). */
export function dayRelation(ms: number, now: number): DayRelation {
  const today = startOfDay(now);
  const day = startOfDay(ms);
  if (day === today) {
    return 'today';
  }
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);
  return day === yesterday.getTime() ? 'yesterday' : 'other';
}
