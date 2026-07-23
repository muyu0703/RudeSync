/**
 * A calendar date without a time or time zone.
 *
 * The template-literal type helps callers, while every public operation still
 * validates the value at runtime.
 */
export type IsoDate = `${number}-${number}-${number}`;

export interface CalendarDate {
  readonly year: number;
  readonly month: number;
  readonly day: number;
}

const ISO_DATE_PATTERN = /^(\d{4})-(\d{2})-(\d{2})$/;
const MIN_YEAR = 1;
const MAX_YEAR = 9999;

export function isLeapYear(year: number): boolean {
  assertIntegerInRange(year, MIN_YEAR, MAX_YEAR, "year");
  return year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
}

export function daysInMonth(year: number, month: number): number {
  assertIntegerInRange(year, MIN_YEAR, MAX_YEAR, "year");
  assertIntegerInRange(month, 1, 12, "month");

  switch (month) {
    case 2:
      return isLeapYear(year) ? 29 : 28;
    case 4:
    case 6:
    case 9:
    case 11:
      return 30;
    default:
      return 31;
  }
}

export function parseIsoDate(value: string): CalendarDate {
  const match = ISO_DATE_PATTERN.exec(value);
  if (!match) {
    throw new RangeError(`Invalid ISO date "${value}". Expected YYYY-MM-DD.`);
  }

  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);

  assertIntegerInRange(year, MIN_YEAR, MAX_YEAR, "year");
  assertIntegerInRange(month, 1, 12, "month");
  assertIntegerInRange(day, 1, daysInMonth(year, month), "day");

  return { year, month, day };
}

export function toIsoDate(date: CalendarDate): IsoDate {
  const { year, month, day } = date;
  assertIntegerInRange(year, MIN_YEAR, MAX_YEAR, "year");
  assertIntegerInRange(month, 1, 12, "month");
  assertIntegerInRange(day, 1, daysInMonth(year, month), "day");

  return `${String(year).padStart(4, "0")}-${String(month).padStart(
    2,
    "0",
  )}-${String(day).padStart(2, "0")}` as IsoDate;
}

export function normalizeIsoDate(value: string): IsoDate {
  return toIsoDate(parseIsoDate(value));
}

export function compareIsoDates(left: string, right: string): -1 | 0 | 1 {
  const normalizedLeft = normalizeIsoDate(left);
  const normalizedRight = normalizeIsoDate(right);

  if (normalizedLeft < normalizedRight) return -1;
  if (normalizedLeft > normalizedRight) return 1;
  return 0;
}

/**
 * Adds whole calendar days without converting through the host time zone.
 */
export function addDays(date: string, offsetDays: number): IsoDate {
  assertSafeInteger(offsetDays, "offsetDays");

  let { year, month, day } = parseIsoDate(date);
  let remaining = offsetDays;

  while (remaining > 0) {
    const daysRemainingInMonth = daysInMonth(year, month) - day;
    if (remaining <= daysRemainingInMonth) {
      day += remaining;
      remaining = 0;
      continue;
    }

    remaining -= daysRemainingInMonth + 1;
    ({ year, month } = adjacentMonth(year, month, 1));
    day = 1;
  }

  while (remaining < 0) {
    if (day + remaining >= 1) {
      day += remaining;
      remaining = 0;
      continue;
    }

    remaining += day;
    ({ year, month } = adjacentMonth(year, month, -1));
    day = daysInMonth(year, month);
  }

  return toIsoDate({ year, month, day });
}

/**
 * Adds calendar months and clamps to the target month's final day.
 *
 * `preferredDay` exists so recurring schedules can retain their original
 * anchor. For example, adding successive months to January 31 yields
 * February 28/29 and then March 31, rather than permanently moving to the 28th.
 */
export function addMonthsClamped(
  date: string,
  offsetMonths: number,
  preferredDay?: number,
): IsoDate {
  assertSafeInteger(offsetMonths, "offsetMonths");
  const parsed = parseIsoDate(date);
  const anchorDay = preferredDay ?? parsed.day;
  assertIntegerInRange(anchorDay, 1, 31, "preferredDay");

  const zeroBasedMonth =
    (parsed.year - 1) * 12 + (parsed.month - 1) + offsetMonths;
  if (zeroBasedMonth < 0 || zeroBasedMonth >= MAX_YEAR * 12) {
    throw new RangeError("Date calculation falls outside years 0001-9999.");
  }

  const year = Math.floor(zeroBasedMonth / 12) + 1;
  const month = (zeroBasedMonth % 12) + 1;
  const day = Math.min(anchorDay, daysInMonth(year, month));
  return toIsoDate({ year, month, day });
}

export function dateForDayOfMonth(
  year: number,
  month: number,
  preferredDay: number,
): IsoDate {
  assertIntegerInRange(preferredDay, 1, 31, "preferredDay");
  return toIsoDate({
    year,
    month,
    day: Math.min(preferredDay, daysInMonth(year, month)),
  });
}

function adjacentMonth(
  year: number,
  month: number,
  direction: -1 | 1,
): Pick<CalendarDate, "year" | "month"> {
  const zeroBasedMonth = (year - 1) * 12 + (month - 1) + direction;
  if (zeroBasedMonth < 0 || zeroBasedMonth >= MAX_YEAR * 12) {
    throw new RangeError("Date calculation falls outside years 0001-9999.");
  }

  return {
    year: Math.floor(zeroBasedMonth / 12) + 1,
    month: (zeroBasedMonth % 12) + 1,
  };
}

function assertIntegerInRange(
  value: number,
  minimum: number,
  maximum: number,
  label: string,
): void {
  assertSafeInteger(value, label);
  if (value < minimum || value > maximum) {
    throw new RangeError(
      `${label} must be between ${minimum} and ${maximum}; received ${value}.`,
    );
  }
}

function assertSafeInteger(value: number, label: string): void {
  if (!Number.isSafeInteger(value)) {
    throw new TypeError(`${label} must be a safe integer; received ${value}.`);
  }
}
