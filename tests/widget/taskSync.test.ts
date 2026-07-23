import { test } from "node:test";
import assert from "node:assert/strict";
import {
  emitTasksChanged,
  onTasksChanged,
  emitOpenTask,
  onOpenTask,
  TASKS_CHANGED_EVENT,
  OPEN_TASK_EVENT,
} from "../../src/lib/services/taskSync.ts";

test("event names are stable", () => {
  assert.equal(TASKS_CHANGED_EVENT, "tasks-changed");
  assert.equal(OPEN_TASK_EVENT, "open-task");
});

test("emit is a no-op resolve when not in Tauri", async () => {
  await assert.doesNotReject(emitTasksChanged());
});

test("subscribe returns a callable unsubscribe when not in Tauri", async () => {
  const unsubscribe = await onTasksChanged(() => {});
  assert.equal(typeof unsubscribe, "function");
  unsubscribe();
});

test("emitOpenTask is a no-op resolve when not in Tauri", async () => {
  await assert.doesNotReject(emitOpenTask("task-1"));
});

test("onOpenTask returns a callable unsubscribe when not in Tauri", async () => {
  const unsubscribe = await onOpenTask(() => {});
  assert.equal(typeof unsubscribe, "function");
  unsubscribe();
});
