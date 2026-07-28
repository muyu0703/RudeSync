import { test } from "node:test";
import assert from "node:assert/strict";
import {
  completionsByDay,
  billingByMonth,
  invoiceCurrencies,
  isEmptySeries,
} from "../../src/lib/features/dashboard/chartSeries.ts";
import type { Task } from "../../src/lib/types.ts";
import type { Invoice } from "../../src/lib/features/money/types.ts";

function task(partial: Partial<Task> & { id: string }): Task {
  return {
    title: "Task",
    notes: null,
    plannedDate: null,
    dueDate: null,
    reminderAt: null,
    priority: "none",
    category: null,
    projectId: null,
    recurrence: "none",
    subtasks: [],
    status: "open",
    completedAt: null,
    createdAt: "2026-07-01T00:00:00.000Z",
    updatedAt: "2026-07-01T00:00:00.000Z",
    ...partial,
  } as Task;
}

function invoice(partial: Partial<Invoice> & { id: string }): Invoice {
  return {
    number: "INV-2026-07-01-0001",
    projectId: "p1",
    projectName: "Project",
    clientName: "Client",
    billToEmail: null,
    billToAddress: null,
    sellerName: "Seller",
    sellerEmail: null,
    sellerAddress: null,
    sellerLogoPath: null,
    milestoneId: null,
    milestoneLabel: null,
    milestoneKind: "custom",
    milestonePercentBasisPoints: null,
    issueDate: "2026-07-01",
    dueDate: "2026-07-15",
    termKind: "14-days",
    currency: "USD",
    lineItems: [
      { id: "l1", description: "Work", quantity: "1", unitPriceMinor: 100000 },
    ],
    discount: null,
    taxPercentage: null,
    notes: null,
    paymentInstructions: null,
    status: "issued",
    payments: [],
    createdAt: "2026-07-01T00:00:00.000Z",
    updatedAt: "2026-07-01T00:00:00.000Z",
    ...partial,
  } as Invoice;
}

/* ---------------- completionsByDay ---------------- */

test("completionsByDay returns one point per day, ending on endDate", () => {
  const points = completionsByDay([], "2026-07-28", 14);
  assert.equal(points.length, 14);
  assert.equal(points[0].key, "2026-07-15");
  assert.equal(points[13].key, "2026-07-28");
});

test("completionsByDay counts completions on their completion date", () => {
  const tasks = [
    task({ id: "1", status: "completed", completedAt: "2026-07-28T09:00:00.000Z" }),
    task({ id: "2", status: "completed", completedAt: "2026-07-28T22:00:00.000Z" }),
    task({ id: "3", status: "completed", completedAt: "2026-07-27T10:00:00.000Z" }),
  ];
  const points = completionsByDay(tasks, "2026-07-28", 7);
  assert.equal(points.at(-1)?.value, 2);
  assert.equal(points.at(-2)?.value, 1);
});

test("completionsByDay ignores open tasks and completions outside the window", () => {
  const tasks = [
    task({ id: "1" }),
    task({ id: "2", status: "completed", completedAt: null }),
    task({ id: "3", status: "completed", completedAt: "2026-06-01T09:00:00.000Z" }),
  ];
  const points = completionsByDay(tasks, "2026-07-28", 7);
  assert.deepEqual(points.map((point) => point.value), [0, 0, 0, 0, 0, 0, 0]);
});

test("completionsByDay keeps empty days rather than dropping them", () => {
  const tasks = [
    task({ id: "1", status: "completed", completedAt: "2026-07-22T09:00:00.000Z" }),
  ];
  const points = completionsByDay(tasks, "2026-07-28", 7);
  assert.equal(points.length, 7);
  assert.equal(points.filter((point) => point.value === 0).length, 6);
});

test("completionsByDay crosses a month boundary correctly", () => {
  const points = completionsByDay([], "2026-08-02", 4);
  assert.deepEqual(points.map((point) => point.key), [
    "2026-07-30",
    "2026-07-31",
    "2026-08-01",
    "2026-08-02",
  ]);
});

test("completionsByDay buckets by the caller's day resolver", () => {
  // A 07:00 local completion east of UTC carries a previous-day UTC stamp.
  // With the view's local-day resolver it must land on the local day, so the
  // chart agrees with the progress meter beside it.
  const tasks = [
    task({ id: "1", status: "completed", completedAt: "2026-07-27T23:00:00.000Z" }),
  ];
  const localDay = (timestamp: string): string =>
    timestamp === "2026-07-27T23:00:00.000Z" ? "2026-07-28" : timestamp.slice(0, 10);

  assert.equal(completionsByDay(tasks, "2026-07-28", 2).at(-1)?.value, 0);
  assert.equal(completionsByDay(tasks, "2026-07-28", 2, localDay).at(-1)?.value, 1);
});

test("completionsByDay labels a day unambiguously", () => {
  const points = completionsByDay([], "2026-07-28", 1);
  assert.equal(points[0].label, "28");
  assert.equal(points[0].fullLabel, "Tue 28 Jul");
});

/* ---------------- billingByMonth ---------------- */

test("billingByMonth buckets by issue month, not payment date", () => {
  // Billed in June, banked in July. The June bar must own both numbers, or the
  // two months would be comparing different populations.
  const invoices = [
    invoice({
      id: "i1",
      issueDate: "2026-06-10",
      payments: [
        { id: "p1", amountMinor: 40000, receivedDate: "2026-07-05", note: null, createdAt: "" },
      ],
    }),
  ];
  const points = billingByMonth(invoices, "USD", "2026-07-28", 3);
  const june = points.find((point) => point.key === "2026-06");
  const july = points.find((point) => point.key === "2026-07");
  assert.equal(june?.total, 100000);
  assert.equal(june?.value, 40000);
  assert.equal(july?.total, 0);
  assert.equal(july?.value, 0);
});

test("billingByMonth never mixes currencies", () => {
  const invoices = [
    invoice({ id: "i1", currency: "USD", issueDate: "2026-07-02" }),
    invoice({ id: "i2", currency: "EUR", issueDate: "2026-07-03" }),
  ];
  const usd = billingByMonth(invoices, "USD", "2026-07-28", 1);
  assert.equal(usd[0].total, 100000);
});

test("billingByMonth excludes draft and void invoices", () => {
  const invoices = [
    invoice({ id: "i1", issueDate: "2026-07-02", status: "draft" }),
    invoice({ id: "i2", issueDate: "2026-07-03", status: "void" }),
  ];
  const points = billingByMonth(invoices, "USD", "2026-07-28", 1);
  assert.equal(points[0].total, 0);
});

test("billingByMonth treats an invoice with no line items as nothing billed", () => {
  const invoices = [invoice({ id: "i1", issueDate: "2026-07-02", lineItems: [] })];
  assert.doesNotThrow(() => billingByMonth(invoices, "USD", "2026-07-28", 1));
  assert.equal(billingByMonth(invoices, "USD", "2026-07-28", 1)[0].total, 0);
});

test("billingByMonth returns a contiguous month range ending on endDate", () => {
  const points = billingByMonth([], "USD", "2026-01-15", 3);
  assert.deepEqual(points.map((point) => point.key), ["2025-11", "2025-12", "2026-01"]);
});

test("billingByMonth sums several invoices in one month", () => {
  const invoices = [
    invoice({
      id: "i1",
      issueDate: "2026-07-02",
      payments: [{ id: "p1", amountMinor: 100000, receivedDate: "2026-07-04", note: null, createdAt: "" }],
    }),
    invoice({ id: "i2", issueDate: "2026-07-20" }),
  ];
  const points = billingByMonth(invoices, "USD", "2026-07-28", 1);
  assert.equal(points[0].total, 200000);
  assert.equal(points[0].value, 100000);
});

/* ---------------- helpers ---------------- */

test("invoiceCurrencies lists billable currencies in a stable order", () => {
  const invoices = [
    invoice({ id: "i1", currency: "USD" }),
    invoice({ id: "i2", currency: "EUR" }),
    invoice({ id: "i3", currency: "USD" }),
    invoice({ id: "i4", currency: "GBP", status: "draft" }),
  ];
  assert.deepEqual(invoiceCurrencies(invoices), ["EUR", "USD"]);
});

test("isEmptySeries distinguishes an all-zero series from a real one", () => {
  assert.equal(isEmptySeries(completionsByDay([], "2026-07-28", 7)), true);
  const tasks = [
    task({ id: "1", status: "completed", completedAt: "2026-07-28T09:00:00.000Z" }),
  ];
  assert.equal(isEmptySeries(completionsByDay(tasks, "2026-07-28", 7)), false);
});
