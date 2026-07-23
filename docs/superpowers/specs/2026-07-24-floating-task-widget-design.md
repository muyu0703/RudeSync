# Floating Task Widget — Design

Status: Approved scope, ready for implementation plan
Date: 2026-07-24
Feature area: Tasks (new desktop-widget surface)
Platform: Windows 11 (Tauri 2). Must not block a future macOS build.

## 1. Goal

Give RudeSync a floating, always-on-top task widget that lives on the monitor
like a sticky note: a small, semi-transparent, movable, resizable window that
shows all open tasks and lets the user act on them without opening the full app.
It must stay readable and usable at any size, from a tall narrow strip to a tiny
square (responsive the way a good website stays readable at phone width).

## 2. Confirmed decisions

These were settled during brainstorming and are fixed inputs to the design:

- **Window model:** a **separate** always-on-top widget window, alongside the
  main app. The main window is unchanged; both can be open at once.
- **Interactivity:** **check off + quick add**, plus the richer scope below.
- **Task scope shown:** **all open (uncompleted) tasks**, not just Today.
- **Richer editing:** expand a task to check off its **subtasks**, and **inline
  edit** a task's title and priority. (Deeper editing — dates, recurrence,
  notes — stays in the main app's TaskDialog.)
- **Summon & control:** **tray menu item**, **button in the main app**, and
  **auto-open at login** (restore if it was open last session). No global hotkey.
- **Transparency:** **hover-to-solidify** — semi-transparent at rest, opaque on
  mouse-over.

## 3. Implementation approach

**Chosen: Option A — dedicated widget page.**

A second Tauri window loads its own minimal HTML entry point (`widget.html`)
that mounts a focused `TaskWidget.svelte`, separate from the main app's
`index.html` / `App.svelte`. Vite is configured for two entry points; Tauri
bundles both into the app.

Why A over "one bundle, widget mode via `?view=widget`":

- The widget stays lightweight and opens instantly — none of the main app's
  sidebar, routing, or global state is loaded into the translucent window.
- Clean isolation: the widget is a bounded unit with its own styles, so its
  transparency and responsive rules never fight the main app's CSS.
- Cost is a small, well-supported multi-page Vite config change.

Both windows talk to the same SQLite through the **existing** task commands;
no new persistence logic is introduced.

## 4. Backend surface (reused, no new commands)

The widget uses only commands already registered in `src-tauri/src/lib.rs`:

| Need | Command | Notes |
|------|---------|-------|
| Load open tasks | `list_tasks` | Call with a filter that excludes completed; returns top-level tasks each with their `subtasks` array already populated. |
| Complete / reopen a task | `toggle_task` | Toggles `is_completed`. |
| Complete / reopen a **subtask** | `toggle_task` | Subtasks are rows in the same `tasks` table with a `parent_task_id`; the same command toggles them by id. |
| Quick-add a task | `create_task` | Title only; other fields default. |
| Add a subtask | `create_task` | With `parent_task_id` set to the parent. |
| Inline edit (title, priority) | `update_task` | Patch-style update already supports these fields. |

If implementation reveals a genuinely missing capability, that is a plan-time
finding — the design assumes the above suffice, which the command list confirms.

## 5. Components and boundaries

New, self-contained units (each does one thing, testable in isolation):

1. **Widget window (Tauri config + lifecycle)** — declares the `task-widget`
   window and the show/hide/restore logic. Owns window chrome only, not tasks.
2. **`widget.html` + `widgetMain.ts`** — the second Vite entry that mounts the
   widget component. Thin.
3. **`TaskWidget.svelte`** — the UI: header strip, scrollable open-task list
   (with expandable subtasks and inline edit), and the quick-add input. Talks to
   the backend via a small widget-scoped task client.
4. **Widget task client (`widgetTasks.ts`)** — pure-ish wrappers over the invoke
   commands plus the derive/sort logic for "all open tasks". This is where the
   unit-testable logic lives (filtering, sorting, quick-add validation).
5. **Cross-window sync (`taskSync` events)** — a tiny module both windows use to
   emit and listen for a `tasks-changed` event.

## 6. Window behavior (Tauri)

A new window labeled `task-widget`:

- `transparent: true`, `decorations: false`, `alwaysOnTop: true`,
  `skipTaskbar: true`, `resizable: true`, `shadow: true`.
- Default size ~300×420; `minWidth` ~200, `minHeight` ~160 so it can never
  shrink into an unusable sliver.
- **Move:** a slim top strip marked `data-tauri-drag-region` drags the window.
- **Resize:** because an undecorated Windows window loses native resize handles,
  add thin edge/corner grip regions that call the window's
  `startResizeDragging(direction)`.
- **Close button** hides the widget (does not quit the app); consistent with the
  app's tray-based lifecycle.

## 7. UI, content, and transparency

- A single compact card: header (title + drag region + pin/close), a scrollable
  list of all open tasks, and a quick-add input pinned to the bottom.
- Each row: completion checkbox, title, a priority indicator; a disclosure
  control expands the row to reveal subtasks (each with its own checkbox) and an
  inline edit affordance for title/priority.
- Visual language matches the app (near-black green-tinted surface, emerald
  accent, off-white text) so it reads as part of RudeSync.
- **Hover-to-solidify:** on the transparent window, the card background sits at
  ~60% opacity at rest and transitions to ~96% on `:hover`. Pure CSS; no timers,
  honoring the spec's "no idle animation loop" rule.

## 8. Live two-way sync

- After any task mutation in either window, the mutating side emits a Tauri
  event `tasks-changed`.
- Each window listens for `tasks-changed` and re-runs its task load
  (`list_tasks`) to refresh.
- Event-driven, not polled — ticking a task in the widget updates the main app
  and vice-versa with no background loop.

## 9. Launch and persistence

- **Tray:** add "Show task widget" to the existing tray menu (near Open/Quit).
- **Main app:** a button on the Today/Tasks header pops the widget out (shows /
  focuses it).
- **Auto-open at login:** on startup, if the widget was open when the app last
  closed, show it; otherwise leave it hidden.
- **Persistence:** remember the widget's position, size, and open/closed state
  across restarts, so it reappears exactly where the user left it. Mechanism
  (the `tauri-plugin-window-state` plugin vs. the existing settings store) is
  chosen during planning; the requirement is the persisted state above.

## 10. Responsiveness

- Fluid, mobile-first layout using CSS container queries scoped to the widget
  card, so behavior depends on the widget's own width, not the screen.
- Text stays legible at all sizes (sensible minimum font size); rows truncate
  with ellipsis rather than overflow.
- Non-essential chrome degrades gracefully at very small widths (e.g. header
  labels and priority text collapse to icons/dots), keeping the core list and
  checkboxes usable whether the widget is a narrow strip or a tiny square.

## 11. Testing

- **Unit (`node --test`, existing setup):** the widget task client's pure logic
  — open-task filtering, sort order, quick-add trimming/validation, and the
  derive step that pairs subtasks under parents.
- **Reused backend:** task commands are already covered by existing tests; the
  widget adds no new backend logic to test.
- **Manual verification:** window chrome that can't be unit-tested — drag, edge
  resize, always-on-top, hover-to-solidify opacity, tray/button/auto-open
  summoning, and cross-window live sync.

## 12. Non-goals (v1 of the widget)

- No global hotkey.
- No deep task editing in the widget (dates, recurrence, reminders, notes,
  client/project links) — that stays in the main app's TaskDialog.
- No multiple simultaneous widget windows; one `task-widget` instance.
- No per-widget theme/opacity slider — transparency is the fixed
  hover-to-solidify behavior.
- No new task data model, columns, or backend commands.

## 13. Open questions for planning

- Which persistence mechanism for window state (`tauri-plugin-window-state` vs.
  extending the settings store).
- Exact `list_tasks` filter arguments to return "all open" top-level tasks with
  subtasks populated.
- Where the emit of `tasks-changed` lives (frontend after each invoke, vs. a
  thin backend emit) to keep it single-sourced.
