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
      on:click={() => onOpen?.(task)}
    >
      <Icon name="more" size={16} />
    </button>
  {:else}
    <span class="row-action-placeholder" aria-hidden="true"></span>
  {/if}
</article>

<style>
  .task-row {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) 30px;
    align-items: center;
    gap: 12px;
    min-height: 68px;
    padding: 10px 10px 10px 4px;
    border-bottom: 1px solid var(--border-subtle);
    transition: background 120ms ease;
  }
  .task-row:last-child { border-bottom: 0; }
  .task-row:hover { background: var(--surface-hover); }
  .task-row.completed { opacity: 0.58; }
  .task-check {
    display: grid;
    width: 20px;
    height: 20px;
    padding: 0;
    place-items: center;
    color: var(--surface-0);
    background: transparent;
    border: 1px solid var(--border-strong);
    border-radius: 50%;
    cursor: pointer;
    transition: border-color 120ms ease, background 120ms ease, transform 120ms ease;
  }
  .task-check:hover { border-color: var(--accent); transform: scale(1.06); }
  .task-check.checked { background: var(--accent); border-color: var(--accent); }
  .task-check:disabled { cursor: wait; opacity: 0.6; }
  .task-copy { min-width: 0; }
  .task-heading { display: flex; align-items: center; gap: 10px; min-width: 0; }
  .task-title {
    overflow: hidden;
    color: var(--text-primary);
    font-size: 13.5px;
    font-weight: 560;
    line-height: 1.4;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .completed .task-title { text-decoration: line-through; }
  .priority {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    flex: 0 0 auto;
    color: var(--text-muted);
    font-size: 10px;
    font-weight: 620;
    letter-spacing: 0.02em;
    text-transform: uppercase;
  }
  .priority.urgent { color: var(--danger); }
  .priority-dot { width: 5px; height: 5px; border-radius: 50%; background: var(--text-muted); }
  .priority-dot.low { background: var(--blue); }
  .priority-dot.medium { background: var(--amber); }
  .priority-dot.high, .priority-dot.urgent { background: var(--danger); }
  .task-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 18px;
    margin-top: 3px;
    overflow: hidden;
    color: var(--text-muted);
    font-size: 11px;
    white-space: nowrap;
  }
  .task-meta > span:not(:last-child)::after {
    margin-left: 8px;
    color: var(--border-strong);
    content: "•";
  }
  .meta-date { display: inline-flex; align-items: center; gap: 4px; }
  .recurrence { text-transform: capitalize; }
  .row-action-placeholder {
    display: block;
    width: 28px;
    height: 28px;
  }
  .row-action {
    display: grid;
    width: 28px;
    height: 28px;
    padding: 0;
    place-items: center;
    color: var(--text-faint);
    background: transparent;
    border: 1px solid transparent;
    border-radius: 7px;
    cursor: pointer;
    opacity: 0;
    transition: color 120ms ease, background 120ms ease, opacity 120ms ease;
  }
  .task-row:hover .row-action,
  .row-action:focus-visible { opacity: 1; }
  .row-action:hover { color: var(--accent); background: var(--accent-soft); }
  @media (prefers-reduced-motion: reduce) {
    .task-row, .task-check { transition: none; }
  }
</style>
