export type ProjectStatus = "draft" | "active" | "completed" | "archived";

export type MilestoneKind =
  | "kickoff"
  | "completion"
  | "phase"
  | "weekly"
  | "additional"
  | "custom";

export type MilestoneStatus = "not-invoiced" | "invoiced" | "paid";

export interface Client {
  id: string;
  name: string;
  companyName: string | null;
  email: string | null;
  billingAddress: string | null;
  currency: string;
  notes: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CreateClientInput {
  name: string;
  companyName?: string | null;
  email?: string | null;
  billingAddress?: string | null;
  currency: string;
  notes?: string | null;
}

export interface ProjectMilestone {
  id?: string;
  label: string;
  amountMinor: number;
  kind: MilestoneKind;
  sortOrder: number;
  status?: MilestoneStatus;
}

export interface Project {
  id: string;
  clientId: string;
  name: string;
  description: string | null;
  urls: string[];
  status: ProjectStatus;
  currency: string;
  quotedTotalMinor: number;
  milestones: ProjectMilestone[];
  startDate: string | null;
  dueDate: string | null;
  completedAt: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CreateProjectInput {
  clientId: string;
  name: string;
  description?: string | null;
  urls?: string[];
  status: ProjectStatus;
  currency: string;
  quotedTotalMinor: number;
  milestones: ProjectMilestone[];
  startDate?: string | null;
  dueDate?: string | null;
}

export interface WorkEntry {
  id: string;
  projectId: string | null;
  title: string;
  details: string | null;
  workDate: string;
  urls: string[];
  createdAt: string;
  updatedAt: string;
}

export interface CreateWorkEntryInput {
  projectId?: string | null;
  title: string;
  details?: string | null;
  workDate: string;
  urls?: string[];
}

export type UpdateClientInput = CreateClientInput;
export type UpdateProjectInput = CreateProjectInput;
export type UpdateWorkEntryInput = CreateWorkEntryInput;

export interface WorkEntryPrefill {
  requestId: string;
  projectId: string;
  title: string;
  details: string | null;
  workDate: string;
}

export interface WorkService {
  listClients(): Promise<Client[]>;
  createClient(input: CreateClientInput): Promise<Client>;
  updateClient(id: string, input: UpdateClientInput): Promise<Client>;
  deleteClient(id: string): Promise<void>;
  listProjects(): Promise<Project[]>;
  createProject(input: CreateProjectInput): Promise<Project>;
  updateProject(id: string, input: UpdateProjectInput): Promise<Project>;
  deleteProject(id: string): Promise<void>;
  listWorkEntries(): Promise<WorkEntry[]>;
  createWorkEntry(input: CreateWorkEntryInput): Promise<WorkEntry>;
  updateWorkEntry(id: string, input: UpdateWorkEntryInput): Promise<WorkEntry>;
  deleteWorkEntry(id: string): Promise<void>;
}
