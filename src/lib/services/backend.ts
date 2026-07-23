import type {
  CreateTaskInput,
  Task,
  TaskPriority,
  TaskRecurrence,
  TaskStatus,
  UpdateTaskInput,
} from "../types";

const STORAGE_KEY = "rudesync.tasks.v1";

type Invoke = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

interface TauriWindow extends Window {
  __TAURI_INTERNALS__?: { invoke?: Invoke };
  __TAURI__?: { core?: { invoke?: Invoke } };
}

export interface TaskService {
  listTasks(): Promise<Task[]>;
  createTask(input: CreateTaskInput): Promise<Task>;
  updateTask(id: string, input: UpdateTaskInput): Promise<Task>;
  setTaskCompleted(id: string, completed: boolean): Promise<Task>;
  deleteTask(id: string): Promise<void>;
}

function localIsoDay(date = new Date()): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function shiftDay(days: number): string {
  const date = new Date();
  date.setHours(12, 0, 0, 0);
  date.setDate(date.getDate() + days);
  return localIsoDay(date);
}

function uid(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `task-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function makeSeedTasks(): Task[] {
  const now = new Date().toISOString();
  return [
    {
      id: uid(),
      title: "Review Northstar landing page feedback",
      notes: "Resolve the final copy notes before the client call.",
      plannedDate: shiftDay(0),
      dueDate: shiftDay(0),
      reminderAt: null,
      priority: "high",
      category: "Client work",
      projectId: "project-northstar",
      recurrence: "none",
      subtasks: [
        { id: uid(), title: "Check mobile copy", completed: true },
        { id: uid(), title: "Confirm CTA wording", completed: false },
      ],
      status: "open",
      completedAt: null,
      createdAt: now,
      updatedAt: now,
    },
    {
      id: uid(),
      title: "Send kickoff invoice to Coastline Studio",
      notes: null,
      plannedDate: shiftDay(0),
      dueDate: shiftDay(0),
      reminderAt: null,
      priority: "medium",
      category: "Invoices",
      projectId: "project-coastline",
      recurrence: "none",
      subtasks: [],
      status: "open",
      completedAt: null,
      createdAt: now,
      updatedAt: now,
    },
    {
      id: uid(),
      title: "Outline API handoff notes",
      notes: null,
      plannedDate: shiftDay(1),
      dueDate: shiftDay(2),
      reminderAt: null,
      priority: "low",
      category: "Deep work",
      projectId: "project-northstar",
      recurrence: "none",
      subtasks: [],
      status: "open",
      completedAt: null,
      createdAt: now,
      updatedAt: now,
    },
    {
      id: uid(),
      title: "Clear the development inbox",
      notes: null,
      plannedDate: shiftDay(0),
      dueDate: null,
      reminderAt: null,
      priority: "low",
      category: "Admin",
      projectId: null,
      recurrence: "weekdays",
      subtasks: [],
      status: "completed",
      completedAt: now,
      createdAt: now,
      updatedAt: now,
    },
  ];
}

function parseTask(value: unknown): Task {
  const raw = (value ?? {}) as Record<string, unknown>;
  const snake = (camel: string, snakeCase: string) =>
    raw[camel] ?? raw[snakeCase] ?? null;

  return {
    id: String(raw.id ?? uid()),
    title: String(raw.title ?? ""),
    notes: snake("notes", "notes") as string | null,
    plannedDate: snake("plannedDate", "planned_date") as string | null,
    dueDate: snake("dueDate", "due_date") as string | null,
    reminderAt: snake("reminderAt", "reminder_at") as string | null,
    priority: String(raw.priority ?? "medium") as TaskPriority,
    category: snake("category", "category") as string | null,
    projectId: snake("projectId", "project_id") as string | null,
    recurrence: String(raw.recurrence ?? "none") as TaskRecurrence,
    recurrenceRule: snake("recurrenceRule", "recurrence_rule") as string | null,
    parentTaskId: snake("parentTaskId", "parent_task_id") as string | null,
    subtasks: Array.isArray(raw.subtasks)
      ? (raw.subtasks as Task["subtasks"])
      : [],
    status:
      raw.isCompleted === true || raw.is_completed === true
        ? "completed"
        : (String(raw.status ?? "open") as TaskStatus),
    completedAt: snake("completedAt", "completed_at") as string | null,
    createdAt: String(snake("createdAt", "created_at") ?? new Date().toISOString()),
    updatedAt: String(snake("updatedAt", "updated_at") ?? new Date().toISOString()),
  };
}

class BrowserTaskService implements TaskService {
  private read(): Task[] {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (stored) {
      try {
        const values = JSON.parse(stored) as unknown[];
        return values.map(parseTask);
      } catch {
        window.localStorage.removeItem(STORAGE_KEY);
      }
    }

    const tasks = makeSeedTasks();
    this.write(tasks);
    return tasks;
  }

  private write(tasks: Task[]): void {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(tasks));
  }

  async listTasks(): Promise<Task[]> {
    const stored = this.read();
    const children = new Map<string, Task[]>();
    for (const task of stored) {
      if (!task.parentTaskId) continue;
      const values = children.get(task.parentTaskId) ?? [];
      values.push(task);
      children.set(task.parentTaskId, values);
    }
    return stored
      .filter((task) => !task.parentTaskId)
      .map((task) => {
        const childSummaries = (children.get(task.id) ?? []).map((child) => ({
          id: child.id,
          title: child.title,
          completed: child.status === "completed",
        }));
        const merged = new Map(
          [...task.subtasks, ...childSummaries].map((subtask) => [
            subtask.id,
            subtask,
          ]),
        );
        return { ...task, subtasks: [...merged.values()] };
      });
  }

  async createTask(input: CreateTaskInput): Promise<Task> {
    const now = new Date().toISOString();
    const task: Task = {
      id: uid(),
      title: input.title.trim(),
      notes: input.notes ?? null,
      plannedDate: input.plannedDate ?? localIsoDay(),
      dueDate: input.dueDate ?? null,
      reminderAt: input.reminderAt ?? null,
      priority: input.priority ?? "medium",
      category: input.category ?? null,
      projectId: input.projectId ?? null,
      recurrence: input.recurrence ?? "none",
      recurrenceRule: input.recurrenceRule ?? null,
      parentTaskId: input.parentTaskId ?? null,
      subtasks: [],
      status: "open",
      completedAt: null,
      createdAt: now,
      updatedAt: now,
    };
    const tasks = [task, ...this.read()];
    this.write(tasks);
    return task;
  }

  async updateTask(id: string, input: UpdateTaskInput): Promise<Task> {
    const tasks = this.read();
    const index = tasks.findIndex((task) => task.id === id);
    if (index < 0) throw new Error("Task not found.");
    const current = tasks[index];
    const updated: Task = {
      ...current,
      ...input,
      title: input.title?.trim() || current.title,
      updatedAt: new Date().toISOString(),
    };
    tasks[index] = updated;
    this.write(tasks);
    return updated;
  }

  async setTaskCompleted(id: string, completed: boolean): Promise<Task> {
    const stored = this.read();
    const embeddedParentIndex = stored.findIndex((task) =>
      task.subtasks.some((subtask) => subtask.id === id),
    );
    if (
      stored.findIndex((task) => task.id === id) < 0 &&
      embeddedParentIndex >= 0
    ) {
      const parent = stored[embeddedParentIndex];
      stored[embeddedParentIndex] = {
        ...parent,
        subtasks: parent.subtasks.map((subtask) =>
          subtask.id === id ? { ...subtask, completed } : subtask,
        ),
        updatedAt: new Date().toISOString(),
      };
      this.write(stored);
      return {
        ...parseTask({
          id,
          title:
            parent.subtasks.find((subtask) => subtask.id === id)?.title ??
            "Subtask",
          status: completed ? "completed" : "open",
          isCompleted: completed,
        }),
        completedAt: completed ? new Date().toISOString() : null,
      };
    }
    return this.updateTask(id, {
      status: completed ? "completed" : "open",
    }).then((task) => {
      const tasks = this.read();
      const index = tasks.findIndex((item) => item.id === task.id);
      const updated = {
        ...task,
        completedAt: completed ? new Date().toISOString() : null,
      };
      tasks[index] = updated;
      this.write(tasks);
      return updated;
    });
  }

  async deleteTask(id: string): Promise<void> {
    const tasks = this.read().filter(
      (task) => task.id !== id && task.parentTaskId !== id,
    );
    this.write(tasks);
  }
}

class TauriTaskService implements TaskService {
  constructor(private invoke: Invoke) {}

  async listTasks(): Promise<Task[]> {
    const tasks = await this.invoke<unknown[]>("list_tasks", {
      filter: { includeCompleted: true, includeDeleted: false },
    });
    return tasks.map(parseTask);
  }

  async createTask(input: CreateTaskInput): Promise<Task> {
    const created = await this.invoke<unknown>("create_task", {
      input: {
        title: input.title,
        notes: input.notes,
        plannedDate: input.plannedDate,
        dueDate: input.dueDate,
        reminderAt: input.reminderAt,
        priority: input.priority,
        category: input.category,
        parentTaskId: input.parentTaskId,
        projectId: input.projectId,
        recurrenceRule:
          input.recurrenceRule ??
          (input.recurrence && input.recurrence !== "none"
            ? input.recurrence
            : null),
      },
    });
    return parseTask(created);
  }

  async updateTask(id: string, input: UpdateTaskInput): Promise<Task> {
    const updated = await this.invoke<unknown>("update_task", { id, input });
    return parseTask(updated);
  }

  async setTaskCompleted(id: string, completed: boolean): Promise<Task> {
    const updated = await this.invoke<unknown>("set_task_completed", {
      taskId: id,
      completed,
    });
    return parseTask(updated);
  }

  async deleteTask(id: string): Promise<void> {
    await this.invoke<unknown>("delete_task", { taskId: id });
  }
}

function resolveTauriInvoke(): Invoke | null {
  if (typeof window === "undefined") return null;
  const tauriWindow = window as TauriWindow;
  return (
    tauriWindow.__TAURI_INTERNALS__?.invoke ??
    tauriWindow.__TAURI__?.core?.invoke ??
    null
  );
}

export function createTaskService(): TaskService {
  const invoke = resolveTauriInvoke();
  return invoke ? new TauriTaskService(invoke) : new BrowserTaskService();
}

export const dateUtils = {
  localIsoDay,
  shiftDay,
};
