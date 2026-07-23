<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { createTaskService } from "../lib/services/backend.ts";
  import { emitTasksChanged, onTasksChanged, emitOpenTask } from "../lib/services/taskSync.ts";
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
  let editingNotesId: string | null = null;
  let editNotes = "";
  let confirmingDeleteId: string | null = null;
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

  function startEditNotes(task: Task): void {
    editingNotesId = task.id;
    editNotes = task.notes ?? "";
  }

  function commitNotes(task: Task): void {
    const notes = editNotes.trim();
    editingNotesId = null;
    if (notes === (task.notes ?? "")) return;
    void mutate(() => service.updateTask(task.id, { notes: notes || null }));
  }

  function deleteTask(task: Task): void {
    confirmingDeleteId = null;
    expandedId = null;
    void mutate(() => service.deleteTask(task.id));
  }

  function openInApp(task: Task): void {
    void emitOpenTask(task.id);
    const tauri = window as unknown as {
      __TAURI_INTERNALS__?: { invoke?: (cmd: string) => Promise<unknown> };
      __TAURI__?: { core?: { invoke?: (cmd: string) => Promise<unknown> } };
    };
    const invoke =
      tauri.__TAURI_INTERNALS__?.invoke ?? tauri.__TAURI__?.core?.invoke;
    if (invoke) void invoke("focus_main_window");
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

          <button
            class="expand"
            aria-label="Toggle details"
            aria-expanded={expandedId === task.id}
            on:click={() => toggleExpand(task.id)}
          >{expandedId === task.id ? "▾" : "▸"}</button>
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
            {#if task.subtasks.length > 0}
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
            {/if}

            {#if editingNotesId === task.id}
              <textarea
                class="note-edit"
                bind:value={editNotes}
                maxlength="1200"
                rows="3"
                placeholder="Add a note…"
                on:blur={() => commitNotes(task)}
                on:keydown={(e) => {
                  if (e.key === "Escape") editingNotesId = null;
                }}
              ></textarea>
            {:else}
              <button
                class="note"
                class:empty-note={!task.notes}
                title="Click to edit note"
                on:click={() => startEditNotes(task)}
              >{task.notes ? task.notes : "Add a note…"}</button>
            {/if}

            <div class="actions">
              <button class="act" on:click={() => openInApp(task)}>Open in app</button>
              {#if confirmingDeleteId === task.id}
                <span class="confirm">
                  <span class="confirm-label">Delete?</span>
                  <button class="act danger" on:click={() => deleteTask(task)}>Delete</button>
                  <button class="act" on:click={() => (confirmingDeleteId = null)}>Cancel</button>
                </span>
              {:else}
                <button class="act ghost-danger" on:click={() => (confirmingDeleteId = task.id)}>Delete</button>
              {/if}
            </div>
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

  .note {
    display: block;
    width: 100%;
    margin-top: 6px;
    padding: 0;
    color: #aebdb4;
    font: inherit;
    font-size: 11px;
    line-height: 1.5;
    text-align: left;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    background: transparent;
    border: none;
    cursor: text;
  }
  .note.empty-note {
    color: #536158;
    font-style: italic;
  }
  .note-edit {
    width: 100%;
    margin-top: 6px;
    padding: 6px 8px;
    color: #edf5f0;
    font: inherit;
    font-size: 11px;
    line-height: 1.5;
    background: #090d0b;
    border: 1px solid var(--accent, #43d17f);
    border-radius: 6px;
    resize: vertical;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    padding-top: 6px;
    border-top: 1px solid rgba(42, 58, 49, 0.4);
  }
  .act {
    padding: 3px 8px;
    color: #aebdb4;
    font: inherit;
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    background: transparent;
    border: 1px solid #2a3a31;
    border-radius: 6px;
    cursor: pointer;
  }
  .act:hover {
    border-color: #3a4c42;
  }
  .act.ghost-danger {
    margin-left: auto;
    color: #d98a8a;
  }
  .act.danger {
    color: #07120c;
    background: #ef7676;
    border-color: #ef7676;
  }
  .confirm {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
  }
  .confirm-label {
    color: #f0a5a5;
    font-size: 9px;
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
