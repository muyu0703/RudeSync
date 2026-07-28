<script lang="ts">
  import type { Task } from "../types";
  import Icon from "./Icon.svelte";

  export let task: Task;
  export let busy = false;
  export let onToggle: (task: Task) => void;
  export let onOpen: ((task: Task) => void) | null = null;

  const priorityLabels = {
    none: "",
    low: "Low",
    medium: "Medium",
    high: "High",
    urgent: "Urgent",
  };

  function displayDate(value: string | null): string {
    if (!value) return "";
    const date = new Date(`${value.slice(0, 10)}T12:00:00`);
    return new Intl.DateTimeFormat("en-US", {
      month: "short",
      day: "numeric",
    }).format(date);
  }

  $: completedSubtasks = task.subtasks.filter((subtask) => subtask.completed).length;
  $: dateLabel = displayDate(task.dueDate ?? task.plannedDate);
</script>

<article class:completed={task.status === "completed"} class="task-row">
  <button
    class="task-check"
    class:checked={task.status === "completed"}
    type="button"
    aria-label={task.status === "completed"
      ? `Reopen ${task.title}`
      : `Complete ${task.title}`}
    disabled={busy}
    on:click={() => onToggle(task)}
  >
    {#if task.status === "completed"}
      <Icon name="check" size={14} strokeWidth={2.4} />
    {/if}
  </button>

  <div class="task-copy">
    <div class="task-heading">
      <span class="task-title">{task.title}</span>
      {#if task.priority !== "none"}
        <span class:urgent={task.priority === "urgent"} class="priority">
          <i class={`priority-dot ${task.priority}`}></i>
          {priorityLabels[task.priority]}
        </span>
      {/if}
    </div>

    <div class="task-meta">
      {#if task.category}<span>{task.category}</span>{/if}
      {#if dateLabel}
        <span class="meta-date"><Icon name="calendar" size={13} />{dateLabel}</span>
      {/if}
      {#if task.subtasks.length}
        <span>{completedSubtasks}/{task.subtasks.length} subtasks</span>
      {/if}
      {#if task.recurrence !== "none"}
        <span class="recurrence">{task.recurrence}</span>
      {/if}
    </div>
  </div>

  {#if onOpen}
    <button
      class="row-action"
      type="button"
      aria-label={`Edit ${task.title}`}
      title={`Edit ${task.title}`}
      on:click={() => onOpen?.(task)}
    >
      <Icon name="edit" size={16} />
    </button>
  {:else}
    <span class="row-action-placeholder" aria-hidden="true"></span>
  {/if}
</article>

<style>
  .task-row {
    display: grid;
    grid-template-columns: 20px minmax(0, 1fr) 24px;
    align-items: center;
    gap: var(--space-3);
    min-height: 52px;
    padding: var(--space-2);
    border-radius: var(--radius-control);
    transition: background var(--duration) var(--ease);
  }
  /* Row separators live in app.css: Svelte scoping cannot match siblings
     across component instances. */
  .task-row:hover { background: var(--surface-hover); }
  .task-row.completed { opacity: 0.5; }

  .task-check {
    display: grid;
    width: 18px;
    height: 18px;
    padding: 0;
    place-items: center;
    color: var(--on-accent);
    background: transparent;
    border: 1.5px solid var(--separator-strong);
    border-radius: 50%;
    transition: border-color var(--duration) var(--ease), background var(--duration) var(--ease);
  }
  .task-check:hover { border-color: var(--accent); }
  .task-check.checked { background: var(--accent); border-color: var(--accent); }
  .task-check:disabled { cursor: wait; opacity: 0.5; }

  .task-copy { min-width: 0; }
  .task-heading { display: flex; align-items: center; gap: var(--space-2); min-width: 0; }
  .task-title {
    overflow: hidden;
    color: var(--text-primary);
    font-size: var(--text-13);
    font-weight: var(--weight-medium);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .completed .task-title { text-decoration: line-through; }

  /* The dot carries the priority; the word stays quiet beside it. */
  .priority {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    flex: 0 0 auto;
    color: var(--text-tertiary);
    font-size: var(--text-11);
    font-weight: var(--weight-regular);
    letter-spacing: 0;
    text-transform: none;
  }
  .priority.urgent { color: var(--danger); }
  .priority-dot { width: 5px; height: 5px; border-radius: 50%; background: var(--text-tertiary); }
  .priority-dot.low { background: var(--blue); }
  .priority-dot.medium { background: var(--amber); }
  .priority-dot.high, .priority-dot.urgent { background: var(--danger); }

  .task-meta {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: 16px;
    margin-top: 2px;
    overflow: hidden;
    color: var(--text-tertiary);
    font-size: var(--text-11);
    white-space: nowrap;
  }
  .meta-date { display: inline-flex; align-items: center; gap: var(--space-1); }
  .recurrence { text-transform: capitalize; }

  .row-action-placeholder { display: block; width: 24px; height: 24px; }
  .row-action {
    display: grid;
    width: 24px;
    height: 24px;
    padding: 0;
    place-items: center;
    color: var(--text-tertiary);
    background: transparent;
    border: 0;
    border-radius: var(--radius-control);
    opacity: 0;
    transition: color var(--duration) var(--ease), background var(--duration) var(--ease),
      opacity var(--duration) var(--ease);
  }
  .task-row:hover .row-action,
  .row-action:focus-visible { opacity: 1; }
  .row-action:hover { color: var(--text-primary); background: var(--surface-active); }
</style>
