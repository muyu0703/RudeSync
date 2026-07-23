import { test } from "node:test";
import assert from "node:assert/strict";
import {
  emitTasksChanged,
  onTasksChanged,
  TASKS_CHANGED_EVENT,
} from "../../src/lib/services/taskSync.ts";

test("event name is stable", () => {
  assert.equal(TASKS_CHANGED_EVENT, "tasks-changed");
});

test("emit is a no-op resolve when not in Tauri", async () => {
  await assert.doesNotReject(emitTasksChanged());
});

test("subscribe returns a callable unsubscribe when not in Tauri", async () => {
  const unsubscribe = await onTasksChanged(() => {});
  assert.equal(typeof unsubscribe, "function");
  unsubscribe();
});
