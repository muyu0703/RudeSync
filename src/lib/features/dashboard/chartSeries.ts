import { calculateInvoiceTotals } from "../../domain/money.ts";
import type { Task } from "../../types.ts";
import type { Invoice } from "../money/types.ts";

/**
 * One bar. `total` is what makes the two chart forms one component: `null`
 * draws a plain bar (a count), a number draws a part-to-whole meter where
 * `value` is the filled portion of `total`.
 */
export interface ChartPoint {
  /** Machine key: `YYYY-MM-DD` for days, `YYYY-MM` for months. */
  key: string;
  /** Short axis tick. Thinned by the component; never all shown at once. */
  label: string;
  /** Spoken and tooltip form. Always unambiguous on its own. */
  fullLabel: string;
  value: number;
  total: number | null;
}

const MONTHS = [
  "Jan", "Feb", "Mar", "Apr", "May", "Jun",
  "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const MONTHS_FULL = [
  "January", "February", "March", "April", "May", "June",
  "July", "August", "September", "October", "November", "December",
];
const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/**
 * Date maths runs in UTC so a day never gains or loses an hour to daylight
 * saving. The keys this produces are compared against the `YYYY-MM-DD` prefix
 * of stored timestamps, which is the same convention `stats.ts` uses — the
 * chart's last bar and the "Completed today" stat must never disagree.
 */
function addDays(isoDate: string, delta: number): string {
  const [year, month, day] = isoDate.split("-").map(Number);
  return new Date(Date.UTC(year, month - 1, day + delta))
    .toISOString()
    .slice(0, 10);
}

function addMonths(monthKey: string, delta: number): string {
  const [year, month] = monthKey.split("-").map(Number);
  return new Date(Date.UTC(year, month - 1 + delta, 1))
    .toISOString()
    .slice(0, 7);
}

function dayLabels(isoDate: string): { label: string; fullLabel: string } {
  const [year, month, day] = isoDate.split("-").map(Number);
  const at = new Date(Date.UTC(year, month - 1, day));
  return {
    label: String(day),
    fullLabel: `${WEEKDAYS[at.getUTCDay()]} ${day} ${MONTHS[month - 1]}`,
  };
}

function monthLabels(monthKey: string): { label: string; fullLabel: string } {
  const [year, month] = monthKey.split("-").map(Number);
  return {
    label: MONTHS[month - 1],
    fullLabel: `${MONTHS_FULL[month - 1]} ${year}`,
  };
}

/** Invoices that never represented money owed. Mirrors `stats.ts`. */
function isInactive(invoice: Invoice): boolean {
  return invoice.status === "draft" || invoice.status === "void";
}

/**
 * `calculateInvoiceTotals` throws on an invoice with no line items, which a
 * chart must never do. Treat that as nothing billed and nothing paid.
 */
function billedAndPaidMinor(invoice: Invoice): {
  totalMinor: number;
  paidMinor: number;
} {
  if (invoice.lineItems.length === 0) return { totalMinor: 0, paidMinor: 0 };
  const totals = calculateInvoiceTotals({
    lineItems: invoice.lineItems,
    discount: invoice.discount,
    taxPercentage: invoice.taxPercentage,
    paymentsMinor: invoice.payments.map((payment) => payment.amountMinor),
  });
  return { totalMinor: totals.totalMinor, paidMinor: totals.paidMinor };
}

/** Default day resolver: the UTC date already embedded in the timestamp. */
const utcDay = (timestamp: string): string => timestamp.slice(0, 10);

/**
 * Tasks completed on each of the `days` days ending on `endDate` (inclusive).
 * Always returns exactly `days` points, zeros included — a gap in the bars is
 * information, and dropping empty days would silently rescale the axis.
 *
 * `dayOf` decides which calendar day a completion timestamp belongs to. The
 * caller must pass the *same* resolver the surrounding view uses: Today and
 * Review bucket by local day, so east-of-UTC a task finished at 07:00 local
 * carries a previous-day UTC stamp, and the default resolver would drop it
 * into the neighbouring bar — the chart would then contradict the progress
 * meter sitting directly beside it. It is injected rather than read from the
 * environment so these series stay deterministic under test.
 */
export function completionsByDay(
  tasks: Task[],
  endDate: string,
  days: number,
  dayOf: (timestamp: string) => string = utcDay,
): ChartPoint[] {
  const counts = new Map<string, number>();
  for (const task of tasks) {
    if (task.status !== "completed") continue;
    if (!task.completedAt) continue;
    const day = dayOf(task.completedAt);
    if (!day) continue;
    counts.set(day, (counts.get(day) ?? 0) + 1);
  }

  const points: ChartPoint[] = [];
  for (let offset = days - 1; offset >= 0; offset -= 1) {
    const key = addDays(endDate, -offset);
    points.push({
      key,
      ...dayLabels(key),
      value: counts.get(key) ?? 0,
      total: null,
    });
  }
  return points;
}

/**
 * Billed against collected, bucketed by the month the invoice was *issued*.
 *
 * The cohort matters: a payment banked in July may settle a June invoice, so
 * bucketing payments by their own received date and drawing them inside July's
 * billed total would compare two different populations and overstate some
 * months while starving others. Every bar here answers one question — "of what
 * I billed that month, how much has come in?" — which is a true part-to-whole.
 *
 * Money is per-currency and is never summed across currencies, so the caller
 * picks one.
 */
export function billingByMonth(
  invoices: Invoice[],
  currency: string,
  endDate: string,
  months: number,
): ChartPoint[] {
  const billed = new Map<string, number>();
  const collected = new Map<string, number>();

  for (const invoice of invoices) {
    if (isInactive(invoice)) continue;
    if (invoice.currency !== currency) continue;
    const month = invoice.issueDate.slice(0, 7);
    if (!month) continue;
    const { totalMinor, paidMinor } = billedAndPaidMinor(invoice);
    billed.set(month, (billed.get(month) ?? 0) + totalMinor);
    collected.set(month, (collected.get(month) ?? 0) + paidMinor);
  }

  const endMonth = endDate.slice(0, 7);
  const points: ChartPoint[] = [];
  for (let offset = months - 1; offset >= 0; offset -= 1) {
    const key = addMonths(endMonth, -offset);
    points.push({
      key,
      ...monthLabels(key),
      value: collected.get(key) ?? 0,
      total: billed.get(key) ?? 0,
    });
  }
  return points;
}

/**
 * Currencies that actually have billable invoices, for the chart's currency
 * picker. Sorted so the control does not reorder itself between loads.
 */
export function invoiceCurrencies(invoices: Invoice[]): string[] {
  const seen = new Set<string>();
  for (const invoice of invoices) {
    if (isInactive(invoice)) continue;
    seen.add(invoice.currency);
  }
  return [...seen].sort((a, b) => a.localeCompare(b));
}

/** True when every bar is zero, so a view can show an empty state instead. */
export function isEmptySeries(points: ChartPoint[]): boolean {
  return points.every((point) => point.value === 0 && !point.total);
}
