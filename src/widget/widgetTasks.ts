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
