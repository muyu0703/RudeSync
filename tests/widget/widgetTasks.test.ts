import { test } from "node:test";
import assert from "node:assert/strict";
import { openTasks, isValidQuickAdd } from "../../src/widget/widgetTasks.ts";
import type { Task } from "../../src/lib/types.ts";

function task(partial: Partial<Task> & { id: string; title: string }): Task {
  return {
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
    createdAt: "2026-07-24T00:00:00.000Z",
    updatedAt: "2026-07-24T00:00:00.000Z",
    ...partial,
  };
}

test("openTasks drops completed tasks", () => {
  const result = openTasks([
    task({ id: "1", title: "A", status: "open" }),
    task({ id: "2", title: "B", status: "completed" }),
  ]);
  assert.deepEqual(result.map((t) => t.id), ["1"]);
});

test("openTasks sorts by priority then title", () => {
  const result = openTasks([
    task({ id: "low", title: "Zeta", priority: "low" }),
    task({ id: "urgent", title: "Beta", priority: "urgent" }),
    task({ id: "urgent2", title: "Alpha", priority: "urgent" }),
  ]);
  assert.deepEqual(result.map((t) => t.id), ["urgent2", "urgent", "low"]);
});

test("isValidQuickAdd rejects blank and over-long titles", () => {
  assert.equal(isValidQuickAdd("  "), false);
  assert.equal(isValidQuickAdd("Fix the PDF"), true);
  assert.equal(isValidQuickAdd("x".repeat(121)), false);
});
