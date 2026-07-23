export type AppSection =
  | "today"
  | "tasks"
  | "work"
  | "money"
  | "review"
  | "settings";

export type TaskPriority = "none" | "low" | "medium" | "high" | "urgent";
export type TaskStatus = "open" | "completed";
export type TaskRecurrence =
  | "none"
  | "daily"
  | "weekdays"
  | "weekly"
  | "monthly"
  | "custom";

export interface Subtask {
  id: string;
  title: string;
  completed: boolean;
}

export interface Task {
  id: string;
  title: string;
  notes: string | null;
  plannedDate: string | null;
  dueDate: string | null;
  reminderAt: string | null;
  priority: TaskPriority;
  category: string | null;
  projectId: string | null;
  recurrence: TaskRecurrence;
  recurrenceRule?: string | null;
  parentTaskId?: string | null;
  subtasks: Subtask[];
  status: TaskStatus;
  completedAt: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CreateTaskInput {
  title: string;
  notes?: string | null;
  plannedDate?: string | null;
  dueDate?: string | null;
  reminderAt?: string | null;
  priority?: TaskPriority;
  category?: string | null;
  projectId?: string | null;
  recurrence?: TaskRecurrence;
  recurrenceRule?: string | null;
  parentTaskId?: string | null;
}

export interface UpdateTaskInput extends Partial<CreateTaskInput> {
  status?: TaskStatus;
}

export interface Client {
  id: string;
  name: string;
  company: string | null;
  currency: string;
  accent: string;
}

export type ProjectStatus = "planning" | "active" | "review" | "completed";

export interface Project {
  id: string;
  clientId: string;
  name: string;
  status: ProjectStatus;
  value: number;
  currency: string;
  progress: number;
  nextMilestone: string | null;
  nextMilestoneDate: string | null;
}

export interface WorkEntry {
  id: string;
  projectId: string | null;
  title: string;
  details: string | null;
  completedOn: string;
  url: string | null;
}

export type InvoiceStatus =
  | "draft"
  | "issued"
  | "partially_paid"
  | "paid"
  | "overdue"
  | "void";

export interface Invoice {
  id: string;
  number: string;
  clientName: string;
  projectName: string;
  milestone: string;
  amount: number;
  paidAmount: number;
  currency: string;
  dueDate: string;
  status: InvoiceStatus;
}

export type LoanFrequency =
  | "weekly"
  | "every-two-weeks"
  | "twice-monthly"
  | "monthly"
  | "custom";

export interface LoanInstallment {
  id: string;
  dueDate: string;
  paid: boolean;
  paidAt: string | null;
}

export interface PersonalLoan {
  id: string;
  operator: string;
  description: string;
  frequency: LoanFrequency;
  installments: LoanInstallment[];
}

export interface WeeklyDay {
  date: string;
  label: string;
  completed: number;
  total: number;
}

export interface AppPreferences {
  startWithWindows: boolean;
  closeToTray: boolean;
  notifications: boolean;
  weekStartsMonday: boolean;
  defaultCurrency: string;
  invoiceTermDays: 0 | 7 | 14 | 30;
  backupPath: string;
  dailyBackups: boolean;
  backupRetention: number;
}
