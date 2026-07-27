import { test } from "node:test";
import assert from "node:assert/strict";
import {
  openTaskCount,
  completedTodayCount,
  overdueTaskCount,
  overdueInvoiceCount,
  outstandingByCurrency,
  receivedInMonth,
} from "../../src/lib/features/dashboard/stats.ts";
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

test("openTaskCount counts only open tasks", () => {
  assert.equal(
    openTaskCount([
      task({ id: "1" }),
      task({ id: "2", status: "completed" }),
      task({ id: "3" }),
    ]),
    2,
  );
});

test("completedTodayCount matches the completion date only", () => {
  const tasks = [
    task({ id: "1", status: "completed", completedAt: "2026-07-28T09:00:00.000Z" }),
    task({ id: "2", status: "completed", completedAt: "2026-07-27T23:00:00.000Z" }),
    task({ id: "3" }),
  ];
  assert.equal(completedTodayCount(tasks, "2026-07-28"), 1);
});

test("overdueTaskCount ignores completed and undated tasks", () => {
  const tasks = [
    task({ id: "1", dueDate: "2026-07-20" }),
    task({ id: "2", dueDate: "2026-07-20", status: "completed" }),
    task({ id: "3", dueDate: "2026-07-28" }),
    task({ id: "4" }),
  ];
  assert.equal(overdueTaskCount(tasks, "2026-07-28"), 1);
});

test("overdueInvoiceCount ignores paid, draft and void invoices", () => {
  const invoices = [
    invoice({ id: "a", dueDate: "2026-07-20" }),
    invoice({ id: "b", dueDate: "2026-07-20", status: "draft" }),
    invoice({ id: "c", dueDate: "2026-07-20", status: "void" }),
    invoice({
      id: "d",
      dueDate: "2026-07-20",
      payments: [
        { id: "p", amountMinor: 100000, receivedDate: "2026-07-10", note: null, createdAt: "" },
      ],
    }),
    invoice({ id: "e", dueDate: "2026-07-30" }),
  ];
  assert.equal(overdueInvoiceCount(invoices, "2026-07-28"), 1);
});

test("outstandingByCurrency groups per currency and never merges them", () => {
  const result = outstandingByCurrency([
    invoice({ id: "a" }),
    invoice({ id: "b", currency: "EUR" }),
    invoice({ id: "c", status: "void" }),
  ]);
  assert.deepEqual(result, [
    { currency: "EUR", amountMinor: 100000 },
    { currency: "USD", amountMinor: 100000 },
  ]);
});

test("receivedInMonth sums payments inside the current month only", () => {
  const result = receivedInMonth(
    [
      invoice({
        id: "a",
        payments: [
          { id: "p1", amountMinor: 30000, receivedDate: "2026-07-05", note: null, createdAt: "" },
          { id: "p2", amountMinor: 20000, receivedDate: "2026-06-30", note: null, createdAt: "" },
        ],
      }),
    ],
    "2026-07-28",
  );
  assert.deepEqual(result, [{ currency: "USD", amountMinor: 30000 }]);
});

test("empty input returns zeroes, not errors", () => {
  assert.equal(openTaskCount([]), 0);
  assert.deepEqual(outstandingByCurrency([]), []);
  assert.deepEqual(receivedInMonth([], "2026-07-28"), []);
});
