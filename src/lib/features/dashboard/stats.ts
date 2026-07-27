import { calculateInvoiceTotals } from "../../domain/money.ts";
import type { Task } from "../../types.ts";
import type { Invoice } from "../money/types.ts";

export interface CurrencyAmount {
  currency: string;
  amountMinor: number;
}

/** Invoices that no longer represent money owed. */
function isInactive(invoice: Invoice): boolean {
  return invoice.status === "draft" || invoice.status === "void";
}

/**
 * `calculateInvoiceTotals` throws on an invoice with no line items, which a
 * dashboard must never do. Treat that as nothing owed.
 */
function balanceDueMinor(invoice: Invoice): number {
  if (invoice.lineItems.length === 0) return 0;
  return calculateInvoiceTotals({
    lineItems: invoice.lineItems,
    discount: invoice.discount,
    taxPercentage: invoice.taxPercentage,
    paymentsMinor: invoice.payments.map((payment) => payment.amountMinor),
  }).balanceDueMinor;
}

function sortByCurrency(totals: Map<string, number>): CurrencyAmount[] {
  return [...totals.entries()]
    .filter(([, amountMinor]) => amountMinor > 0)
    .map(([currency, amountMinor]) => ({ currency, amountMinor }))
    .sort((a, b) => a.currency.localeCompare(b.currency));
}

export function openTaskCount(tasks: Task[]): number {
  return tasks.filter((task) => task.status !== "completed").length;
}

export function completedTodayCount(tasks: Task[], today: string): number {
  return tasks.filter(
    (task) =>
      task.status === "completed" &&
      (task.completedAt ?? "").slice(0, 10) === today,
  ).length;
}

export function overdueTaskCount(tasks: Task[], today: string): number {
  return tasks.filter(
    (task) =>
      task.status !== "completed" &&
      task.dueDate !== null &&
      task.dueDate < today,
  ).length;
}

export function overdueInvoiceCount(invoices: Invoice[], today: string): number {
  return invoices.filter(
    (invoice) =>
      !isInactive(invoice) &&
      invoice.dueDate < today &&
      balanceDueMinor(invoice) > 0,
  ).length;
}

export function outstandingByCurrency(invoices: Invoice[]): CurrencyAmount[] {
  const totals = new Map<string, number>();
  for (const invoice of invoices) {
    if (isInactive(invoice)) continue;
    const balance = balanceDueMinor(invoice);
    if (balance <= 0) continue;
    totals.set(invoice.currency, (totals.get(invoice.currency) ?? 0) + balance);
  }
  return sortByCurrency(totals);
}

export function receivedInMonth(
  invoices: Invoice[],
  today: string,
): CurrencyAmount[] {
  const month = today.slice(0, 7);
  const totals = new Map<string, number>();
  for (const invoice of invoices) {
    if (invoice.status === "void") continue;
    for (const payment of invoice.payments) {
      if (payment.receivedDate.slice(0, 7) !== month) continue;
      totals.set(
        invoice.currency,
        (totals.get(invoice.currency) ?? 0) + payment.amountMinor,
      );
    }
  }
  return sortByCurrency(totals);
}
