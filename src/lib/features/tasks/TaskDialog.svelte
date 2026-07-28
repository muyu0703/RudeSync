<script lang="ts">
  import { createEventDispatcher, tick } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { scale } from "svelte/transition";
  import Icon from "../../components/Icon.svelte";
  import { motionDuration } from "../../motion";
  import type { Task, TaskPriority, TaskRecurrence } from "../../types";
  import type {
    TaskDialogSaveDetail,
    TaskDialogSubtaskDetail,
    TaskDraft,
    TaskProjectOption,
  } from "./types";

  export let open = false;
  export let task: Task | null = null;
  export let projects: TaskProjectOption[] = [];
  export let saving = false;

  const dispatch = createEventDispatcher<{
    close: void;
    save: TaskDialogSaveDetail;
    toggleSubtask: TaskDialogSubtaskDetail;
  }>();
  const weekdayOptions = [
    ["MO", "Mon"],
    ["TU", "Tue"],
    ["WE", "Wed"],
    ["TH", "Thu"],
    ["FR", "Fri"],
    ["SA", "Sat"],
    ["SU", "Sun"],
  ] as const;

  let title = "";
  let notes = "";
  let plannedDate = "";
  let dueDate = "";
  let reminderLocal = "";
  let priority: TaskPriority = "medium";
  let category = "";
  let projectId = "";
  let recurrence: TaskRecurrence = "none";
  let weeklyDays: string[] = [];
  let customInterval = 2;
  let customUnit: "days" | "weeks" | "months" = "weeks";
  let subtaskDrafts: Array<{ id: string; title: string }> = [];
  let validationMessage = "";
  let titleInput: HTMLInputElement;
  let lastIdentity = "";
  let dialog: HTMLDialogElement;
  let hydratedSnapshot = "";

  // The dialog stays mounted for the app's lifetime (App.svelte renders it
  // unconditionally), so — unlike WorkDialog, which is created fresh each
  // time it opens — open/close has to be driven imperatively off the `open`
  // prop rather than from onMount.
  $: if (dialog) {
    if (open) {
      if (!dialog.open) dialog.showModal();
    } else if (dialog.open) {
      dialog.close();
    }
  }

  $: if (open) {
    const identity = task?.id ?? "new";
    if (identity !== lastIdentity) {
      lastIdentity = identity;
      hydrate(task);
      tick().then(() => titleInput?.focus());
    }
  } else {
    lastIdentity = "";
  }

  // Compared against the snapshot `hydrate()` takes when the dialog opens
  // (or switches to a different task), so an untouched dialog never
  // triggers the discard confirmation below.
  $: isDirty =
    open &&
    JSON.stringify({
      title,
      notes,
      plannedDate,
      dueDate,
      reminderLocal,
      priority,
      category,
      projectId,
      recurrence,
      weeklyDays,
      customInterval,
      customUnit,
      subtaskDrafts,
    }) !== hydratedSnapshot;

  function nullable(value: string): string | null {
    const normalized = value.trim();
    return normalized || null;
  }

  function localDateTime(value: string | null): string {
    if (!value) return "";
    const parsed = new Date(value);
    if (Number.isNaN(parsed.getTime())) return value.slice(0, 16);
    const offset = parsed.getTimezoneOffset() * 60_000;
    return new Date(parsed.getTime() - offset).toISOString().slice(0, 16);
  }

  function rfc3339(value: string): string | null {
    if (!value) return null;
    const parsed = new Date(value);
    return Number.isNaN(parsed.getTime()) ? null : parsed.toISOString();
  }

  function hydrate(value: Task | null): void {
    title = value?.title ?? "";
    notes = value?.notes ?? "";
    plannedDate = value?.plannedDate ?? "";
    dueDate = value?.dueDate ?? "";
    reminderLocal = localDateTime(value?.reminderAt ?? null);
    priority = value?.priority ?? "medium";
    category = value?.category ?? "";
    projectId = value?.projectId ?? "";
    recurrence = value?.recurrence ?? "none";
    const byDay = value?.recurrenceRule?.match(/(?:^|;)BYDAY=([^;]+)/i)?.[1];
    weeklyDays = byDay
      ? byDay
          .split(",")
          .map((day) => day.trim().toUpperCase())
          .filter((day) => weekdayOptions.some(([code]) => code === day))
      : [weekdayCode(value?.plannedDate ?? value?.dueDate ?? "")];
    const customMatch = value?.recurrenceRule?.match(
      /FREQ=(DAILY|WEEKLY|MONTHLY);INTERVAL=(\d+)/i,
    );
    customInterval = customMatch ? Number(customMatch[2]) : 2;
    customUnit =
      customMatch?.[1]?.toUpperCase() === "DAILY"
        ? "days"
        : customMatch?.[1]?.toUpperCase() === "MONTHLY"
          ? "months"
          : "weeks";
    subtaskDrafts = [];
    validationMessage = "";
    hydratedSnapshot = JSON.stringify({
      title,
      notes,
      plannedDate,
      dueDate,
      reminderLocal,
      priority,
      category,
      projectId,
      recurrence,
      weeklyDays,
      customInterval,
      customUnit,
      subtaskDrafts,
    });
  }

  function weekdayCode(value: string): string {
    const date = value
      ? new Date(`${value.slice(0, 10)}T12:00:00`)
      : new Date();
    const codes = ["SU", "MO", "TU", "WE", "TH", "FR", "SA"];
    return (
      codes[
        Number.isNaN(date.getTime()) ? new Date().getDay() : date.getDay()
      ] ?? "MO"
    );
  }

  function toggleWeeklyDay(day: string): void {
    weeklyDays = weeklyDays.includes(day)
      ? weeklyDays.filter((value) => value !== day)
      : weekdayOptions
          .map(([code]) => code)
          .filter((code) => [...weeklyDays, day].includes(code));
  }

  function addSubtask(): void {
    subtaskDrafts = [
      ...subtaskDrafts,
      { id: crypto.randomUUID(), title: "" },
    ];
  }

  function removeSubtask(id: string): void {
    subtaskDrafts = subtaskDrafts.filter((item) => item.id !== id);
  }

  // Backdrop click, Escape (via on:cancel), and the header's icon-close
  // button all funnel through here and get a confirmation when the form has
  // unsaved edits. The explicit Cancel button calls `cancelClick` instead
  // and stays a no-questions exit.
  function close(): void {
    if (saving) return;
    if (isDirty && !window.confirm("Discard this task? What you've typed will be lost.")) {
      return;
    }
    dispatch("close");
  }

  function cancelClick(): void {
    if (!saving) dispatch("close");
  }

  function handleBackdrop(event: MouseEvent): void {
    if (event.target === dialog) close();
  }

  function submit(): void {
    validationMessage = "";
    if (!title.trim()) {
      validationMessage = "Give this task a clear title.";
      titleInput?.focus();
      return;
    }
    if (plannedDate && dueDate && dueDate < plannedDate) {
      validationMessage = "The deadline cannot be earlier than the planned date.";
      return;
    }
    if (reminderLocal && !rfc3339(reminderLocal)) {
      validationMessage = "Choose a valid reminder date and time.";
      return;
    }
    if (
      recurrence === "custom" &&
      (!Number.isSafeInteger(customInterval) ||
        customInterval < 1 ||
        customInterval > 365)
    ) {
      validationMessage = "Custom recurrence must be between 1 and 365.";
      return;
    }
    if (recurrence === "weekly" && weeklyDays.length === 0) {
      validationMessage = "Choose at least one weekday for a weekly task.";
      return;
    }

    const draft: TaskDraft = {
      title: title.trim(),
      notes: nullable(notes),
      plannedDate: nullable(plannedDate),
      dueDate: nullable(dueDate),
      reminderAt: rfc3339(reminderLocal),
      priority,
      category: nullable(category),
      projectId: nullable(projectId),
      recurrence,
      recurrenceRule:
        recurrence === "none"
          ? null
          : recurrence === "custom"
            ? `FREQ=${customUnit === "days" ? "DAILY" : customUnit === "weeks" ? "WEEKLY" : "MONTHLY"};INTERVAL=${customInterval}`
            : recurrence === "weekly"
              ? `FREQ=WEEKLY;INTERVAL=1;BYDAY=${weeklyDays.join(",")}`
              : recurrence,
      newSubtasks: subtaskDrafts
        .map((item) => item.title.trim())
        .filter(Boolean),
    };
    dispatch("save", { task, draft });
  }
</script>

<dialog
  bind:this={dialog}
  aria-labelledby="task-dialog-title"
  on:cancel|preventDefault={close}
  on:click={handleBackdrop}
>
  <!--
    No out:transition on .dialog-card: every close path calls the native
    dialog.close() synchronously, which strips the `open` attribute and lets
    the UA stylesheet hide the element before an outro could render — see
    WorkDialog.svelte for the same rationale.
  -->
  {#if open}
    <section
      class="dialog-card"
      in:scale={{ duration: motionDuration(200), start: 0.96, opacity: 0, easing: cubicOut }}
    >
      <header>
        <div>
          <span class="eyebrow">{task ? "Refine the plan" : "Capture the work"}</span>
          <h2 id="task-dialog-title">{task ? "Edit task" : "New task"}</h2>
        </div>
        <button class="icon-close" type="button" aria-label="Close task editor" on:click={close}>
          <Icon name="x" size={17} />
        </button>
      </header>

      <div class="dialog-body">
      <form id="task-dialog-form" on:submit|preventDefault={submit}>
        <label class="field span-2">
          <span class="field-label">Task title</span>
          <input
            class="field-input"
            bind:this={titleInput}
            bind:value={title}
            maxlength="240"
            autocomplete="off"
            placeholder="Ship the client dashboard"
          />
        </label>

        <label class="field span-2">
          <span class="field-label">Notes <small>optional</small></span>
          <textarea
            class="field-textarea"
            bind:value={notes}
            rows="3"
            maxlength="20000"
            placeholder="Context, acceptance criteria, or the next concrete action"
          ></textarea>
        </label>

        <label class="field">
          <span class="field-label">Planned date</span>
          <input class="field-input" bind:value={plannedDate} type="date" />
        </label>

        <label class="field">
          <span class="field-label">Deadline</span>
          <input class="field-input" bind:value={dueDate} type="date" />
        </label>

        <label class="field">
          <span class="field-label">Priority</span>
          <select class="field-input" bind:value={priority}>
            <option value="none">None</option>
            <option value="low">Low</option>
            <option value="medium">Medium</option>
            <option value="high">High</option>
            <option value="urgent">Urgent</option>
          </select>
        </label>

        <label class="field">
          <span class="field-label">Category</span>
          <input class="field-input" bind:value={category} maxlength="80" placeholder="Client work" />
        </label>

        <label class="field span-2">
          <span class="field-label">Project <small>optional</small></span>
          <select class="field-input" bind:value={projectId}>
            <option value="">Personal / no project</option>
            {#each projects as project (project.id)}
              <option value={project.id}>{project.clientName} — {project.name}</option>
            {/each}
          </select>
        </label>

        <label class="field">
          <span class="field-label">Repeats</span>
          <select class="field-input" bind:value={recurrence}>
            <option value="none">Does not repeat</option>
            <option value="daily">Daily</option>
            <option value="weekdays">Weekdays</option>
            <option value="weekly">Weekly</option>
            <option value="monthly">Monthly</option>
            <option value="custom">Custom rule</option>
          </select>
        </label>

        <label class="field">
          <span class="field-label">Reminder</span>
          <input class="field-input" bind:value={reminderLocal} type="datetime-local" />
        </label>

        {#if recurrence === "custom"}
          <label class="field">
            <span class="field-label">Repeat every</span>
            <input class="field-input" bind:value={customInterval} type="number" min="1" max="365" />
          </label>
          <label class="field">
            <span class="field-label">Interval</span>
            <select class="field-input" bind:value={customUnit}>
              <option value="days">Days</option>
              <option value="weeks">Weeks</option>
              <option value="months">Months</option>
            </select>
          </label>
        {/if}

        {#if recurrence === "weekly"}
          <fieldset class="weekday-picker span-2">
            <legend class="field-label">Repeat on</legend>
            <div>
              {#each weekdayOptions as [code, label]}
                <button
                  class:active={weeklyDays.includes(code)}
                  type="button"
                  aria-pressed={weeklyDays.includes(code)}
                  on:click={() => toggleWeeklyDay(code)}
                >{label}</button>
              {/each}
            </div>
          </fieldset>
        {/if}

        <div class="subtasks span-2">
          <div class="subtask-heading">
            <div><strong>Subtasks</strong><span>Break the task into small checkable steps.</span></div>
            <button class="add-subtask" type="button" on:click={addSubtask}>
              <Icon name="plus" size={14} /> Add step
            </button>
          </div>

          {#if task?.subtasks.length}
            <div class="saved-subtasks">
              {#each task.subtasks as subtask (subtask.id)}
                <label>
                  <input
                    type="checkbox"
                    checked={subtask.completed}
                    disabled={saving}
                    on:change={(event) =>
                      dispatch("toggleSubtask", {
                        subtask,
                        completed: event.currentTarget.checked,
                      })}
                  />
                  <span class:done={subtask.completed}>{subtask.title}</span>
                </label>
              {/each}
            </div>
          {/if}

          {#each subtaskDrafts as subtask (subtask.id)}
            <div class="subtask-row">
              <span class="step-dot"></span>
              <input class="field-input" bind:value={subtask.title} maxlength="240" placeholder="Describe a step" />
              <button type="button" aria-label="Remove subtask" on:click={() => removeSubtask(subtask.id)}>
                <Icon name="x" size={14} />
              </button>
            </div>
          {/each}
        </div>

        {#if validationMessage}
          <p class="field-error span-2" role="alert">{validationMessage}</p>
        {/if}
      </form>
      </div>

      <footer>
        <button class="cancel" type="button" on:click={cancelClick}>Cancel</button>
        <button class="save" type="submit" form="task-dialog-form" disabled={saving || !title.trim()}>
          {saving ? "Saving…" : task ? "Save changes" : "Create task"}
        </button>
      </footer>
    </section>
  {/if}
</dialog>

<style>
  dialog {
    width: min(690px, calc(100vw - 32px));
    max-height: min(860px, calc(100vh - 32px));
    padding: 0;
    color: var(--text-primary);
    background: transparent;
    border: 0;
    overflow: visible;
  }

  dialog::backdrop {
    background: rgb(2 7 6 / 78%);
    backdrop-filter: blur(5px);
  }

  .dialog-card {
    display: flex;
    flex-direction: column;
    max-height: min(860px, calc(100vh - 32px));
    overflow: hidden;
    background: var(--surface-overlay);
    border-radius: var(--radius-sheet);
    box-shadow: var(--shadow-sheet);
  }

  header {
    display: flex;
    flex: 0 0 auto;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-5) var(--space-6) var(--space-4);
    background: var(--surface-overlay);
    border-bottom: 1px solid var(--separator);
  }

  h2 { margin: 4px 0 0; font-size: var(--text-20); letter-spacing: -0.02em; }

  .icon-close,
  .subtask-row button {
    display: grid;
    width: 32px;
    height: 32px;
    padding: 0;
    place-items: center;
    color: var(--text-tertiary);
    background: transparent;
    border: 1px solid var(--separator);
    border-radius: var(--radius-control);
    cursor: pointer;
  }

  .dialog-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    scrollbar-color: var(--separator-strong) transparent;
    scrollbar-width: thin;
  }

  form {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-4);
    padding: var(--space-5) var(--space-6) var(--space-6);
  }

  .span-2 { grid-column: 1 / -1; }
  .field { display: grid; gap: var(--space-2); }

  .field-label small {
    color: var(--text-tertiary);
    font-size: var(--text-11);
    font-weight: var(--weight-regular);
    text-transform: none;
    letter-spacing: 0;
  }

  select.field-input {
    color-scheme: dark;
  }

  .subtasks {
    padding: var(--space-3);
    background: var(--surface-raised);
    border-radius: var(--radius-panel);
  }

  .weekday-picker {
    min-width: 0;
    margin: 0;
    padding: var(--space-3) var(--space-4) var(--space-4);
    border: 1px solid var(--separator);
    border-radius: var(--radius-panel);
  }

  .weekday-picker legend {
    padding: 0 var(--space-1);
  }

  .weekday-picker div {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: var(--space-2);
  }

  .weekday-picker button {
    min-height: 32px;
    color: var(--text-tertiary);
    font-size: var(--text-11);
    font-weight: var(--weight-semibold);
    background: var(--surface-raised);
    border: 1px solid var(--separator-strong);
    border-radius: var(--radius-control);
    cursor: pointer;
  }

  .weekday-picker button.active {
    color: var(--accent);
    background: var(--accent-fill);
    border-color: var(--accent-line);
  }

  .subtask-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
  }

  .subtask-heading div { display: grid; gap: 3px; }
  .subtask-heading strong { font-size: var(--text-12); }
  .subtask-heading span { color: var(--text-tertiary); font-size: var(--text-11); }

  .add-subtask {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    color: var(--accent);
    background: var(--accent-fill);
    border: 1px solid var(--accent-line);
    border-radius: var(--radius-control);
    cursor: pointer;
  }

  .saved-subtasks { display: grid; gap: var(--space-2); margin-top: var(--space-4); }
  .saved-subtasks label { display: flex; align-items: center; gap: var(--space-2); font-size: var(--text-12); }
  .saved-subtasks input { width: 15px; height: 15px; accent-color: var(--accent); }
  .saved-subtasks .done { color: var(--text-tertiary); text-decoration: line-through; }

  .subtask-row {
    display: grid;
    grid-template-columns: 8px minmax(0, 1fr) 32px;
    align-items: center;
    gap: var(--space-2);
    margin-top: var(--space-3);
  }

  .step-dot { width: 6px; height: 6px; background: var(--accent); border-radius: 50%; }

  footer {
    display: flex;
    flex: 0 0 auto;
    justify-content: flex-end;
    gap: var(--space-2);
    padding: var(--space-4) var(--space-6) var(--space-5);
    border-top: 1px solid var(--separator);
  }

  footer button {
    padding: var(--space-2) var(--space-4);
    border-radius: var(--radius-control);
    cursor: pointer;
  }

  .cancel {
    color: var(--text-secondary);
    background: transparent;
    border: 1px solid var(--separator);
  }

  .save {
    color: var(--on-accent);
    font-weight: var(--weight-semibold);
    background: var(--accent);
    border: 1px solid var(--accent);
  }

  .save:disabled { cursor: not-allowed; opacity: 0.5; }

  @media (max-width: 620px) {
    dialog {
      width: calc(100vw - 20px);
      max-height: calc(100vh - 20px);
    }

    .dialog-card {
      max-height: calc(100vh - 20px);
    }

    header,
    form,
    footer {
      padding-right: var(--space-4);
      padding-left: var(--space-4);
    }

    form { grid-template-columns: 1fr; }
    .span-2 { grid-column: 1; }
  }
</style>
