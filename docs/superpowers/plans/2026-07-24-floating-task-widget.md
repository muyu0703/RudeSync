# Floating Task Widget Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a separate, always-on-top, hover-to-solidify desktop widget window that shows all open tasks with check-off, quick-add, subtask toggle, and inline title/priority edit, staying readable at any size.

**Architecture:** A second Tauri window (`task-widget`) loads its own minimal Vite entry (`widget.html` → `TaskWidget.svelte`), isolated from the main app. It reuses the existing `TaskService` (`createTaskService()`) so no new backend commands are added. The two windows stay in sync through a Tauri `tasks-changed` event. Window geometry and open/closed state persist across restarts via `tauri-plugin-window-state` (scoped to the widget).

**Tech Stack:** Tauri 2, Svelte 5 (`mount`), TypeScript, Vite (multi-page), SQLite (via existing commands), `node --test` for unit tests.

## Global Constraints

- Reuse existing commands only; add **no** new task data model, columns, or backend task commands. (Available: `list_tasks`, `create_task`, `toggle_task`, `set_task_completed`, `update_task`.)
- No polling or idle animation loop (spec §3). Cross-window refresh is event-driven only.
- Store money as integer minor units — N/A here but keep the no-floats rule in mind for any totals. (Not touched by this feature.)
- Visual language: near-black green-tinted surfaces, emerald accent (`#43d17f` / `--accent`), off-white text, system fonts. Match `src/lib/features/work/components/ClientForm.svelte` styling conventions.
- Platform: Windows 11; keep platform-specific window code isolated so a future macOS build stays feasible.
- Tests run `.ts` directly through Node's type-stripping (`node --test path/to/file.ts`), matching `package.json`'s `test` script. Use explicit `.ts` import extensions, as the codebase already does.

---

### Task 1: Multi-page Vite build + widget entry scaffold

**Files:**
- Create: `widget.html`
- Create: `src/widget/main.ts`
- Create: `src/widget/TaskWidget.svelte` (placeholder for this task)
- Create: `src/widget/widget.css`
- Modify: `vite.config.ts`

**Interfaces:**
- Produces: a second build entry reachable at `widget.html` (dev: `http://localhost:1420/widget.html`, prod: `dist/widget.html`). Later tasks fill in `TaskWidget.svelte`.

- [ ] **Step 1: Create the widget HTML entry**

Create `widget.html` (project root, mirrors `index.html`):

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <meta name="theme-color" content="#0b0f0d" />
    <title>RudeSync Tasks</title>
  </head>
  <body>
    <div id="widget"></div>
    <script type="module" src="/src/widget/main.ts"></script>
  </body>
</html>
```

- [ ] **Step 2: Create the widget mount entry**

Create `src/widget/main.ts` (mirrors `src/main.ts`):

```ts
import { mount } from "svelte";
import TaskWidget from "./TaskWidget.svelte";
import "./widget.css";

const target = document.getElementById("widget");

if (!target) {
  throw new Error("RudeSync could not find its task-widget root.");
}

mount(TaskWidget, { target });
```

- [ ] **Step 3: Create the widget base stylesheet (transparent surface)**

Create `src/widget/widget.css`:

```css
:root {
  color-scheme: dark;
}

html,
body {
  margin: 0;
  height: 100%;
  background: transparent;
  font-family: "Segoe UI", system-ui, sans-serif;
}

#widget {
  height: 100vh;
}
```

- [ ] **Step 4: Create a placeholder widget component**

Create `src/widget/TaskWidget.svelte`:

```svelte
<script lang="ts">
</script>

<main>Tasks</main>

<style>
  main {
    display: grid;
    place-items: center;
    height: 100vh;
    color: #edf5f0;
  }
</style>
```

- [ ] **Step 5: Register both HTML entries in Vite**

Modify `vite.config.ts` — add a `rollupOptions.input` map inside the existing `build` block:

```ts
  build: {
    target: ["es2021", "chrome105", "safari13"],
    cssMinify: true,
    sourcemap: false,
    rollupOptions: {
      input: {
        main: "index.html",
        widget: "widget.html"
      }
    }
  }
```

- [ ] **Step 6: Verify both pages build**

Run: `npm run build`
Expected: build succeeds and prints both `dist/index.html` and `dist/widget.html` in the output list.

- [ ] **Step 7: Commit**

```bash
git add widget.html src/widget/ vite.config.ts
git commit -m "feat(widget): scaffold multi-page widget entry"
```

---

### Task 2: Widget task logic module (pure, unit-tested)

**Files:**
- Create: `src/widget/widgetTasks.ts`
- Test: `tests/widget/widgetTasks.test.ts`

**Interfaces:**
- Consumes: `Task` from `src/lib/types.ts`.
- Produces:
  - `openTasks(tasks: Task[]): Task[]` — returns only non-completed top-level tasks, sorted urgent→none then by title.
  - `isValidQuickAdd(title: string): boolean` — true when the trimmed title is non-empty and ≤ 120 chars.
  - `PRIORITY_ORDER: Record<TaskPriority, number>` — used for sorting and by the UI's priority control.

- [ ] **Step 1: Write the failing test**

Create `tests/widget/widgetTasks.test.ts`:

```ts
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test tests/widget/widgetTasks.test.ts`
Expected: FAIL — cannot find module `../../src/widget/widgetTasks.ts`.

- [ ] **Step 3: Write minimal implementation**

Create `src/widget/widgetTasks.ts`:

```ts
import type { Task, TaskPriority } from "../lib/types.ts";

export const PRIORITY_ORDER: Record<TaskPriority, number> = {
  urgent: 0,
  high: 1,
  medium: 2,
  low: 3,
  none: 4,
};

export function openTasks(tasks: Task[]): Task[] {
  return tasks
    .filter((task) => task.status !== "completed")
    .sort((a, b) => {
      const byPriority = PRIORITY_ORDER[a.priority] - PRIORITY_ORDER[b.priority];
      if (byPriority !== 0) return byPriority;
      return a.title.localeCompare(b.title);
    });
}

export function isValidQuickAdd(title: string): boolean {
  const trimmed = title.trim();
  return trimmed.length > 0 && trimmed.length <= 120;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `node --test tests/widget/widgetTasks.test.ts`
Expected: PASS — 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/widget/widgetTasks.ts tests/widget/widgetTasks.test.ts
git commit -m "feat(widget): add open-task filtering and quick-add validation"
```

---

### Task 3: Cross-window sync module + wire into main app

**Files:**
- Create: `src/lib/services/taskSync.ts`
- Test: `tests/widget/taskSync.test.ts`
- Modify: `src/App.svelte` (emit after mutations; refresh on event)

**Interfaces:**
- Produces:
  - `emitTasksChanged(): Promise<void>` — fires the `tasks-changed` Tauri event; no-op outside Tauri.
  - `onTasksChanged(handler: () => void): Promise<() => void>` — subscribes; returns an unsubscribe function; no-op (returns a no-op unsubscribe) outside Tauri.
  - `TASKS_CHANGED_EVENT = "tasks-changed"`.

- [ ] **Step 1: Write the failing test (browser fallback is safe)**

Create `tests/widget/taskSync.test.ts`:

```ts
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test tests/widget/taskSync.test.ts`
Expected: FAIL — cannot find module `../../src/lib/services/taskSync.ts`.

- [ ] **Step 3: Write minimal implementation**

Create `src/lib/services/taskSync.ts`. It lazy-imports `@tauri-apps/api/event` only inside Tauri so Node tests never load it:

```ts
export const TASKS_CHANGED_EVENT = "tasks-changed";

function inTauri(): boolean {
  if (typeof window === "undefined") return false;
  return "__TAURI_INTERNALS__" in window || "__TAURI__" in window;
}

export async function emitTasksChanged(): Promise<void> {
  if (!inTauri()) return;
  const { emit } = await import("@tauri-apps/api/event");
  await emit(TASKS_CHANGED_EVENT);
}

export async function onTasksChanged(
  handler: () => void,
): Promise<() => void> {
  if (!inTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen(TASKS_CHANGED_EVENT, () => handler());
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `node --test tests/widget/taskSync.test.ts`
Expected: PASS — 3 tests pass.

- [ ] **Step 5: Emit from the main app after each task mutation**

In `src/App.svelte`, add the import near the other service imports:

```ts
import { emitTasksChanged, onTasksChanged } from "./lib/services/taskSync.ts";
```

Then, after each successful task mutation, emit. The mutation handlers are around `src/App.svelte:376` (create → `tasks = [created, ...tasks]`), `:404` (update → `tasks = tasks.map(...)`), and the completion handlers near `:499` and `:521` (which already call `await loadTasks()`). Immediately after each of these state updates, add:

```ts
    void emitTasksChanged();
```

For the two handlers that already `await loadTasks();` (near `:499` and `:521`), place `void emitTasksChanged();` on the line right after `await loadTasks();`.

- [ ] **Step 6: Refresh the main app when the widget changes tasks**

In `src/App.svelte`, find the `onMount` that calls `void loadTasks();` (near `:573`). Replace that mount body so it also subscribes and cleans up:

```ts
  onMount(() => {
    void loadTasks();
    let unsubscribe: (() => void) | null = null;
    void onTasksChanged(() => void loadTasks()).then((off) => {
      unsubscribe = off;
    });
    return () => unsubscribe?.();
  });
```

(If the existing `onMount` is not an arrow returning a cleanup, adapt it to the shape above; keep any other existing mount logic that was in the body.)

- [ ] **Step 7: Type-check the app**

Run: `npm run check`
Expected: PASS — 0 errors.

- [ ] **Step 8: Commit**

```bash
git add src/lib/services/taskSync.ts tests/widget/taskSync.test.ts src/App.svelte
git commit -m "feat(widget): add tasks-changed sync and wire main app"
```

---

### Task 4: TaskWidget UI (list, subtasks, inline edit, quick-add, hover-to-solidify, responsive)

**Files:**
- Modify: `src/widget/TaskWidget.svelte` (replace the placeholder from Task 1)

**Interfaces:**
- Consumes: `createTaskService` from `src/lib/services/backend.ts`; `openTasks`, `isValidQuickAdd`, `PRIORITY_ORDER` from `./widgetTasks.ts`; `emitTasksChanged`, `onTasksChanged` from `../lib/services/taskSync.ts`; `Task`, `TaskPriority` from `../lib/types.ts`; `getCurrentWindow` from `@tauri-apps/api/window`.
- Produces: the full widget UI. No exports consumed by other tasks.

- [ ] **Step 1: Replace the placeholder component with the full widget**

Overwrite `src/widget/TaskWidget.svelte`:

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { createTaskService } from "../lib/services/backend.ts";
  import { emitTasksChanged, onTasksChanged } from "../lib/services/taskSync.ts";
  import { openTasks, isValidQuickAdd, PRIORITY_ORDER } from "./widgetTasks.ts";
  import type { Task, TaskPriority } from "../lib/types.ts";

  const service = createTaskService();
  const appWindow = getCurrentWindow();
  const priorities: TaskPriority[] = ["urgent", "high", "medium", "low", "none"];

  let tasks: Task[] = [];
  let quickAddTitle = "";
  let expandedId: string | null = null;
  let editingId: string | null = null;
  let editTitle = "";
  let error = "";

  $: visibleTasks = openTasks(tasks);

  async function load(): Promise<void> {
    try {
      tasks = await service.listTasks();
      error = "";
    } catch {
      error = "Could not load tasks.";
    }
  }

  async function mutate(action: () => Promise<unknown>): Promise<void> {
    try {
      await action();
      await load();
      void emitTasksChanged();
    } catch {
      error = "That change did not save.";
    }
  }

  function toggleTask(task: Task): void {
    void mutate(() => service.setTaskCompleted(task.id, task.status !== "completed"));
  }

  function toggleSubtask(subtaskId: string, completed: boolean): void {
    void mutate(() => service.setTaskCompleted(subtaskId, !completed));
  }

  function quickAdd(): void {
    if (!isValidQuickAdd(quickAddTitle)) return;
    const title = quickAddTitle.trim();
    quickAddTitle = "";
    void mutate(() => service.createTask({ title }));
  }

  function startEdit(task: Task): void {
    editingId = task.id;
    editTitle = task.title;
  }

  function commitEdit(task: Task): void {
    const title = editTitle.trim();
    editingId = null;
    if (!title || title === task.title) return;
    void mutate(() => service.updateTask(task.id, { title }));
  }

  function setPriority(task: Task, priority: TaskPriority): void {
    void mutate(() => service.updateTask(task.id, { priority }));
  }

  function toggleExpand(id: string): void {
    expandedId = expandedId === id ? null : id;
  }

  function startResize(event: PointerEvent, direction: string): void {
    event.preventDefault();
    // Tauri accepts direction strings like "East", "SouthEast", "South".
    void appWindow.startResizeDragging(direction as never);
  }

  function closeWidget(): void {
    void appWindow.hide();
  }

  onMount(() => {
    void load();
    let unsubscribe: (() => void) | null = null;
    void onTasksChanged(() => void load()).then((off) => {
      unsubscribe = off;
    });
    return () => unsubscribe?.();
  });
</script>

<div class="widget">
  <header data-tauri-drag-region>
    <span class="title" data-tauri-drag-region>Tasks</span>
    <button class="icon" title="Hide widget" on:click={closeWidget} aria-label="Hide widget">×</button>
  </header>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  <ul class="list">
    {#each visibleTasks as task (task.id)}
      <li class="row" class:expanded={expandedId === task.id}>
        <div class="main">
          <button
            class="check"
            role="checkbox"
            aria-checked={task.status === "completed"}
            aria-label={`Complete ${task.title}`}
            on:click={() => toggleTask(task)}
          ></button>

          {#if editingId === task.id}
            <input
              class="edit"
              bind:value={editTitle}
              maxlength="120"
              on:blur={() => commitEdit(task)}
              on:keydown={(e) => {
                if (e.key === "Enter") commitEdit(task);
                if (e.key === "Escape") editingId = null;
              }}
              autofocus
            />
          {:else}
            <button class="label" title="Click to edit" on:click={() => startEdit(task)}>
              <i class={`dot p-${task.priority}`} aria-hidden="true"></i>
              <span class="text">{task.title}</span>
            </button>
          {/if}

          {#if task.subtasks.length > 0}
            <button
              class="expand"
              aria-label="Toggle subtasks"
              aria-expanded={expandedId === task.id}
              on:click={() => toggleExpand(task.id)}
            >{expandedId === task.id ? "▾" : "▸"}</button>
          {/if}
        </div>

        {#if expandedId === task.id}
          <div class="detail">
            <div class="priority-row">
              {#each priorities as level}
                <button
                  class="pri"
                  class:active={task.priority === level}
                  style={`--i:${PRIORITY_ORDER[level]}`}
                  on:click={() => setPriority(task, level)}
                >{level}</button>
              {/each}
            </div>
            <ul class="subtasks">
              {#each task.subtasks as subtask (subtask.id)}
                <li>
                  <button
                    class="check small"
                    role="checkbox"
                    aria-checked={subtask.completed}
                    aria-label={`Complete ${subtask.title}`}
                    on:click={() => toggleSubtask(subtask.id, subtask.completed)}
                  ></button>
                  <span class:done={subtask.completed}>{subtask.title}</span>
                </li>
              {/each}
            </ul>
          </div>
        {/if}
      </li>
    {/each}

    {#if visibleTasks.length === 0}
      <li class="empty">No open tasks</li>
    {/if}
  </ul>

  <form class="quick-add" on:submit|preventDefault={quickAdd}>
    <input
      bind:value={quickAddTitle}
      maxlength="120"
      placeholder="+ new task"
      aria-label="New task title"
    />
  </form>

  <!-- Resize grips (undecorated windows have no native handles) -->
  <span class="grip e" on:pointerdown={(e) => startResize(e, "East")}></span>
  <span class="grip s" on:pointerdown={(e) => startResize(e, "South")}></span>
  <span class="grip se" on:pointerdown={(e) => startResize(e, "SouthEast")}></span>
</div>

<style>
  .widget {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100vh;
    container-type: inline-size;
    color: #edf5f0;
    background: rgba(9, 13, 11, 0.6);
    border: 1px solid rgba(42, 58, 49, 0.6);
    border-radius: 12px;
    overflow: hidden;
    transition: background 120ms ease, border-color 120ms ease;
  }
  .widget:hover {
    background: rgba(9, 13, 11, 0.97);
    border-color: rgba(67, 209, 127, 0.5);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 7px 10px;
    background: rgba(27, 40, 33, 0.5);
    cursor: grab;
  }
  .title {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #aebdb4;
  }
  .icon {
    color: #aebdb4;
    background: transparent;
    border: none;
    font-size: 16px;
    line-height: 1;
    cursor: pointer;
  }

  .error {
    margin: 0;
    padding: 6px 10px;
    color: #f0a5a5;
    font-size: 10px;
  }

  .list {
    flex: 1;
    margin: 0;
    padding: 4px;
    list-style: none;
    overflow-y: auto;
  }
  .row {
    border-radius: 8px;
  }
  .row:hover {
    background: rgba(42, 58, 49, 0.35);
  }
  .main {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
  }

  .check {
    flex: none;
    width: 15px;
    height: 15px;
    background: transparent;
    border: 1.5px solid #3a4c42;
    border-radius: 4px;
    cursor: pointer;
  }
  .check[aria-checked="true"] {
    background: var(--accent, #43d17f);
    border-color: var(--accent, #43d17f);
  }
  .check.small {
    width: 13px;
    height: 13px;
  }

  .label {
    display: flex;
    align-items: center;
    gap: 7px;
    flex: 1;
    min-width: 0;
    padding: 0;
    color: inherit;
    background: transparent;
    border: none;
    text-align: left;
    cursor: text;
  }
  .text {
    overflow: hidden;
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dot {
    flex: none;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #536158;
  }
  .dot.p-urgent { background: #ef7676; }
  .dot.p-high { background: #f0a860; }
  .dot.p-medium { background: #43d17f; }
  .dot.p-low { background: #6aa0c9; }
  .dot.p-none { background: #536158; }

  .edit {
    flex: 1;
    min-width: 0;
    padding: 2px 6px;
    color: #edf5f0;
    font: inherit;
    font-size: 12px;
    background: #090d0b;
    border: 1px solid var(--accent, #43d17f);
    border-radius: 6px;
  }

  .expand {
    flex: none;
    color: #aebdb4;
    background: transparent;
    border: none;
    cursor: pointer;
  }

  .detail {
    padding: 0 8px 8px 30px;
  }
  .priority-row {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-bottom: 6px;
  }
  .pri {
    padding: 2px 6px;
    color: #aebdb4;
    font-size: 9px;
    text-transform: uppercase;
    background: transparent;
    border: 1px solid #2a3a31;
    border-radius: 99px;
    cursor: pointer;
  }
  .pri.active {
    color: #07120c;
    background: var(--accent, #43d17f);
    border-color: var(--accent, #43d17f);
  }
  .subtasks {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .subtasks li {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 3px 0;
    font-size: 11px;
  }
  .subtasks .done {
    color: #536158;
    text-decoration: line-through;
  }

  .empty {
    padding: 16px;
    color: #536158;
    font-size: 11px;
    text-align: center;
  }

  .quick-add {
    padding: 6px;
    border-top: 1px solid rgba(27, 40, 33, 0.8);
  }
  .quick-add input {
    width: 100%;
    height: 30px;
    padding: 0 10px;
    color: #edf5f0;
    font: inherit;
    font-size: 12px;
    background: #090d0b;
    border: 1px solid #2a3a31;
    border-radius: 8px;
  }
  .quick-add input:focus {
    border-color: var(--accent, #43d17f);
    outline: none;
  }

  .grip {
    position: absolute;
    z-index: 5;
  }
  .grip.e { top: 0; right: 0; width: 6px; height: 100%; cursor: ew-resize; }
  .grip.s { left: 0; bottom: 0; width: 100%; height: 6px; cursor: ns-resize; }
  .grip.se { right: 0; bottom: 0; width: 12px; height: 12px; cursor: nwse-resize; }

  /* Responsive: at very small widths, drop non-essential chrome. */
  @container (max-width: 220px) {
    .title { display: none; }
    .pri { font-size: 8px; padding: 2px 4px; }
    .detail { padding-left: 20px; }
  }
</style>
```

- [ ] **Step 2: Type-check the widget**

Run: `npm run check`
Expected: PASS — 0 errors. (If `startResizeDragging` typing complains, the `as never` cast in `startResize` already isolates it.)

- [ ] **Step 3: Commit**

```bash
git add src/widget/TaskWidget.svelte
git commit -m "feat(widget): build task list UI with subtasks, inline edit, quick-add"
```

---

### Task 5: Declare the widget window + capabilities

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Create: `src-tauri/capabilities/task-widget.json`

**Interfaces:**
- Produces: a `task-widget` window (hidden at startup) that later Rust/JS code shows; permissions for that window to drag, resize, show/hide, focus, and use events.

- [ ] **Step 1: Add the widget window to the Tauri config**

In `src-tauri/tauri.conf.json`, add a second entry to `app.windows` (after the existing `"main"` object). The full `windows` array becomes:

```json
      "windows": [
        {
          "label": "main",
          "title": "RudeSync",
          "width": 1180,
          "height": 760,
          "minWidth": 900,
          "minHeight": 620,
          "center": true,
          "visible": true,
          "resizable": true,
          "decorations": true
        },
        {
          "label": "task-widget",
          "url": "widget.html",
          "title": "RudeSync Tasks",
          "width": 300,
          "height": 420,
          "minWidth": 200,
          "minHeight": 160,
          "visible": false,
          "resizable": true,
          "decorations": false,
          "transparent": true,
          "alwaysOnTop": true,
          "skipTaskbar": true,
          "shadow": true
        }
      ]
```

- [ ] **Step 2: Create a capability for the widget window**

Create `src-tauri/capabilities/task-widget.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "task-widget",
  "description": "Permissions for the floating task widget window.",
  "windows": ["task-widget"],
  "permissions": [
    "core:default",
    "core:window:allow-show",
    "core:window:allow-hide",
    "core:window:allow-set-focus",
    "core:window:allow-start-dragging",
    "core:window:allow-start-resize-dragging"
  ]
}
```

- [ ] **Step 3: Verify the app builds and the widget entry loads**

Run: `npm run build`
Expected: PASS. (Full Tauri run is verified in Task 6 once the tray/show path exists. `core:default` on the main capability already permits events, so main-window emit/listen keep working.)

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tauri.conf.json src-tauri/capabilities/task-widget.json
git commit -m "feat(widget): declare task-widget window and capabilities"
```

---

### Task 6: Rust — show helper, tray item, persistence

**Files:**
- Modify: `src-tauri/Cargo.toml` (add `tauri-plugin-window-state`)
- Modify: `src-tauri/src/lib.rs` (helper, tray item, plugin, command registration)

**Interfaces:**
- Produces:
  - Rust command `show_task_widget` (invocable from the frontend) that shows + focuses the widget window.
  - Tray menu item "Show task widget".
  - Window geometry + open/closed persistence for the widget across restarts (auto-open if it was open last session).

- [ ] **Step 1: Add the window-state plugin dependency**

In `src-tauri/Cargo.toml`, under `[dependencies]`, add (matching the Tauri 2.x line already used by the other `tauri-plugin-*` crates):

```toml
tauri-plugin-window-state = "2"
```

- [ ] **Step 2: Add a `show_task_widget` helper and command**

In `src-tauri/src/lib.rs`, directly below the existing `show_main_window` function (ends at line ~1437), add:

```rust
fn reveal_task_widget(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("task-widget") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn show_task_widget(app: AppHandle) {
    reveal_task_widget(&app);
}
```

- [ ] **Step 3: Add the tray menu item**

In `setup_tray` (line ~1490), add a widget item and insert it into the menu, and handle its event. Replace the menu construction and `on_menu_event` block with:

```rust
    let open_item = MenuItem::with_id(app, "open", "Open RudeSync", true, None::<&str>)?;
    let widget_item =
        MenuItem::with_id(app, "widget", "Show task widget", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_item, &widget_item, &quit_item])?;
```

and, in the same builder, change the `.on_menu_event(...)` match arms to:

```rust
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "widget" => reveal_task_widget(app),
            "quit" => app.exit(0),
            _ => {}
        })
```

- [ ] **Step 4: Register the plugin (scoped to the widget) and the command**

In `run()` (line ~1531), add the window-state plugin after the existing plugins (before `.setup`). Scoping it with a denylist keeps it from touching the main window's tray-hide behavior:

```rust
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_denylist(&["main"])
                .build(),
        )
```

Then add `show_task_widget` to the `invoke_handler` list (line ~1584), right after `update_task,`:

```rust
            update_task,
            show_task_widget,
```

- [ ] **Step 5: Build the backend**

Run: `cd src-tauri && cargo build`
Expected: compiles with no errors (a first build downloads the new crate).

- [ ] **Step 6: Manual verification (dev run)**

Run: `npm run tauri dev`
Verify:
- Right-click the tray icon → "Show task widget" opens a small translucent always-on-top window.
- It shows open tasks; the card is see-through at rest and solidifies on hover.
- Ticking a checkbox completes the task; opening the main app shows it reflected (live sync).
- Drag the header to move it; drag the right/bottom/corner grips to resize; the × button hides it.
- Quit and relaunch: if the widget was showing at quit, it reopens at the same position/size.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/lib.rs
git commit -m "feat(widget): tray entry, show command, and window-state persistence"
```

---

### Task 7: Main-app button to open the widget

**Files:**
- Modify: `src/App.svelte` (add a button that invokes `show_task_widget`)

**Interfaces:**
- Consumes: the `show_task_widget` command from Task 6.
- Produces: user-facing entry point in the main window.

- [ ] **Step 1: Add an invoke helper for showing the widget**

In `src/App.svelte`, add a small function near the other action handlers. It resolves the Tauri invoke the same way `backend.ts` does and calls the command; outside Tauri it is a safe no-op:

```ts
  async function openTaskWidget(): Promise<void> {
    const tauri = window as unknown as {
      __TAURI_INTERNALS__?: { invoke?: (cmd: string) => Promise<unknown> };
      __TAURI__?: { core?: { invoke?: (cmd: string) => Promise<unknown> } };
    };
    const invoke =
      tauri.__TAURI_INTERNALS__?.invoke ?? tauri.__TAURI__?.core?.invoke;
    if (invoke) await invoke("show_task_widget");
  }
```

- [ ] **Step 2: Add the button to the tasks/today header**

In `src/App.svelte`, near the existing "New task" button in the tasks header (around `src/App.svelte:786`, the `+ New task` button), add a sibling button:

```svelte
        <button class="ghost" type="button" on:click={() => void openTaskWidget()}>
          <Icon name="spark" size={15} /> Pop out widget
        </button>
```

(Use whatever `class` the neighboring header buttons use; match the existing markup. `Icon` is already imported in `App.svelte`.)

- [ ] **Step 3: Type-check**

Run: `npm run check`
Expected: PASS — 0 errors.

- [ ] **Step 4: Manual verification**

Run: `npm run tauri dev`
Verify: clicking "Pop out widget" in the main window shows/focuses the task widget.

- [ ] **Step 5: Commit**

```bash
git add src/App.svelte
git commit -m "feat(widget): add main-app button to pop out the task widget"
```

---

## Self-Review

**Spec coverage:**

- Separate always-on-top widget window → Task 5 (config), Task 6 (show/tray). ✓
- All open tasks shown → Task 2 (`openTasks`), Task 4 (render). ✓
- Check off + quick add → Task 4 (`toggleTask`, `quickAdd`). ✓
- Subtask toggle + inline title/priority edit → Task 4 (`toggleSubtask`, `startEdit`/`commitEdit`, `setPriority`). ✓
- Hover-to-solidify transparency → Task 4 CSS (`.widget` / `.widget:hover`), Task 5 (`transparent: true`). ✓
- Movable + resizable → Task 4 (`data-tauri-drag-region`, resize grips), Task 5 (`decorations:false`, `resizable:true`), Task 6 capability. ✓
- Responsive at small sizes → Task 4 (`container-type` + `@container` query), Task 5 (`minWidth/minHeight`). ✓
- Summon: tray + main-app button + auto-open at login → Task 6 (tray + persistence VISIBLE restore), Task 7 (button). ✓
- Position/size/open-state persistence → Task 6 (`tauri-plugin-window-state`, denylist main). ✓
- Live two-way sync, event-driven (no polling) → Task 3 (`taskSync`), Task 4 (subscribe + emit). ✓
- No new backend task commands → confirmed; only the thin `show_task_widget` window command is added. ✓

**Placeholder scan:** No TBD/TODO; every code step contains complete code. The one non-literal instruction ("match the neighboring header button's class") is a codebase-fidelity note, not a logic gap.

**Type consistency:** `openTasks`/`isValidQuickAdd`/`PRIORITY_ORDER` signatures match between Task 2 definition and Task 4 usage. `emitTasksChanged`/`onTasksChanged`/`TASKS_CHANGED_EVENT` match between Task 3 and Task 4. `service.setTaskCompleted(id, completed)`, `service.createTask({title[, parentTaskId]})`, `service.updateTask(id, {title|priority})` match the `TaskService` interface in `src/lib/services/backend.ts`. `show_task_widget` defined in Task 6 and invoked in Task 7.

## Open Risks (verify during implementation)

- `startResizeDragging` direction argument type differs across `@tauri-apps/api` minor versions (string vs. enum). The `as never` cast avoids a compile break; confirm resize works at runtime in Task 6/4 manual checks and switch to the `ResizeDirection` enum import if the string form is rejected at runtime.
- If `core:default` already grants `allow-start-dragging`/`allow-start-resize-dragging`, the explicit entries in Task 5 are redundant but harmless.
- Confirm `tauri-plugin-window-state`'s default `StateFlags` includes `VISIBLE` in the installed 2.x; if visibility isn't restored, set flags explicitly via the builder (`.with_state_flags(StateFlags::all())`).
