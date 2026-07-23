import assert from "node:assert/strict";
import test from "node:test";

import {
  calculateInvoiceDueDate,
  calculateInvoiceTotals,
  calculateInvoiceTotalsFromSubtotal,
  calculateLineItemTotal,
  formatInvoiceNumber,
  generateLoanDueDates,
  nextInvoiceNumber,
  parseInvoiceNumber,
} from "../../src/lib/domain/index.ts";
import {
  needsSubtaskCompletionConfirmation,
  shouldOfferCompletedWork,
  unfinishedSubtaskCount,
} from "../../src/lib/features/tasks/completion.ts";

test("invoice numbers use the requested date and four-digit sequence", () => {
  assert.equal(
    formatInvoiceNumber("2026-07-23", 1),
    "INV-2026-07-23-0001",
  );
  assert.deepEqual(parseInvoiceNumber("INV-2026-07-23-0042"), {
    issueDate: "2026-07-23",
    sequence: 42,
  });
  assert.equal(
    nextInvoiceNumber("2026-07-23", [
      "INV-2026-07-22-0099",
      "INV-2026-07-23-0001",
      "not-an-invoice-number",
      "INV-2026-07-23-0004",
    ]),
    "INV-2026-07-23-0005",
  );
});

test("invoice due terms add calendar days and support custom dates", () => {
  assert.equal(calculateInvoiceDueDate("2026-12-25", 7), "2027-01-01");
  assert.equal(calculateInvoiceDueDate("2026-07-23", 0), "2026-07-23");
  assert.equal(
    calculateInvoiceDueDate("2026-07-23", {
      kind: "custom",
      dueDate: "2026-08-19",
    }),
    "2026-08-19",
  );
});

test("money calculations use exact minor units and round exact halves up", () => {
  assert.equal(calculateLineItemTotal(1001, "2.5"), 2503);

  assert.deepEqual(
    calculateInvoiceTotalsFromSubtotal({
      subtotalMinor: 1,
      discount: { kind: "percentage", percentage: "50" },
      taxPercentage: "50",
    }),
    {
      subtotalMinor: 1,
      discountMinor: 1,
      discountedSubtotalMinor: 0,
      taxMinor: 0,
      totalMinor: 0,
      paidMinor: 0,
      balanceDueMinor: 0,
      creditMinor: 0,
    },
  );
});

test("invoice order is subtotal, discount, then tax, with payments separated", () => {
  assert.deepEqual(
    calculateInvoiceTotals({
      lineItems: [
        { unitPriceMinor: 10_000, quantity: "1" },
        { unitPriceMinor: 2_500, quantity: "2" },
      ],
      discount: { kind: "percentage", percentage: "10" },
      taxPercentage: "8.25",
      paymentsMinor: [5_000, 10_000],
    }),
    {
      subtotalMinor: 15_000,
      discountMinor: 1_500,
      discountedSubtotalMinor: 13_500,
      taxMinor: 1_114,
      totalMinor: 14_614,
      paidMinor: 15_000,
      balanceDueMinor: 0,
      creditMinor: 386,
    },
  );
});

test("monthly schedules retain the original day after month-end clamping", () => {
  assert.deepEqual(
    generateLoanDueDates({
      frequency: "monthly",
      firstPaymentDate: "2028-01-31",
      installmentCount: 4,
    }),
    ["2028-01-31", "2028-02-29", "2028-03-31", "2028-04-30"],
  );
});

test("weekly and every-two-weeks schedules use fixed day intervals", () => {
  assert.deepEqual(
    generateLoanDueDates({
      frequency: "weekly",
      firstPaymentDate: "2026-08-13",
      installmentCount: 3,
    }),
    ["2026-08-13", "2026-08-20", "2026-08-27"],
  );
  assert.deepEqual(
    generateLoanDueDates({
      frequency: "every-two-weeks",
      firstPaymentDate: "2026-08-13",
      installmentCount: 3,
    }),
    ["2026-08-13", "2026-08-27", "2026-09-10"],
  );
});

test("twice-monthly schedules start exactly at the explicit first payment", () => {
  assert.deepEqual(
    generateLoanDueDates({
      frequency: "twice-monthly",
      firstPaymentDate: "2026-08-28",
      installmentCount: 5,
      paymentDays: [13, 28],
    }),
    [
      "2026-08-28",
      "2026-09-13",
      "2026-09-28",
      "2026-10-13",
      "2026-10-28",
    ],
  );
});

test("twice-monthly selected days clamp to a short month's final day", () => {
  assert.deepEqual(
    generateLoanDueDates({
      frequency: "twice-monthly",
      firstPaymentDate: "2027-01-15",
      installmentCount: 4,
      paymentDays: [15, 31],
    }),
    ["2027-01-15", "2027-01-31", "2027-02-15", "2027-02-28"],
  );
});

test("twice-monthly schedules retain the selected first day after an initial clamp", () => {
  assert.deepEqual(
    generateLoanDueDates({
      frequency: "twice-monthly",
      firstPaymentDate: "2027-02-28",
      installmentCount: 5,
      paymentDays: [31, 15],
    }),
    [
      "2027-02-28",
      "2027-03-15",
      "2027-03-31",
      "2027-04-15",
      "2027-04-30",
    ],
  );
});

test("custom schedules preserve explicitly entered dates", () => {
  assert.deepEqual(
    generateLoanDueDates({
      frequency: "custom",
      firstPaymentDate: "2026-08-13",
      dueDates: ["2026-08-13", "2026-09-02", "2026-11-20"],
    }),
    ["2026-08-13", "2026-09-02", "2026-11-20"],
  );
});

test("parent completion is gated only while subtasks remain unfinished", () => {
  const task = {
    status: "open" as const,
    subtasks: [
      { id: "done", title: "Done", completed: true },
      { id: "open", title: "Open", completed: false },
    ],
  };
  assert.equal(unfinishedSubtaskCount(task), 1);
  assert.equal(needsSubtaskCompletionConfirmation(task, true), true);
  assert.equal(needsSubtaskCompletionConfirmation(task, false), false);
  assert.equal(
    needsSubtaskCompletionConfirmation(
      { ...task, subtasks: task.subtasks.map((item) => ({ ...item, completed: true })) },
      true,
    ),
    false,
  );
});

test("completed-work prompt is offered only for a newly completed linked task", () => {
  assert.equal(
    shouldOfferCompletedWork(
      { projectId: "project-1", status: "open" },
      true,
    ),
    true,
  );
  assert.equal(
    shouldOfferCompletedWork(
      { projectId: null, status: "open" },
      true,
    ),
    false,
  );
  assert.equal(
    shouldOfferCompletedWork(
      { projectId: "project-1", status: "completed" },
      false,
    ),
    false,
  );
});
