import type {
  Subtask,
  Task,
  TaskPriority,
  TaskRecurrence,
} from "../../types";

export interface TaskProjectOption {
  id: string;
  name: string;
  clientName: string;
}

export interface TaskDraft {
  title: string;
  notes: string | null;
  plannedDate: string | null;
  dueDate: string | null;
  reminderAt: string | null;
  priority: TaskPriority;
  category: string | null;
  projectId: string | null;
  recurrence: TaskRecurrence;
  recurrenceRule: string | null;
  newSubtasks: string[];
}

export interface TaskDialogSaveDetail {
  task: Task | null;
  draft: TaskDraft;
}

export interface TaskDialogSubtaskDetail {
  subtask: Subtask;
  completed: boolean;
}
