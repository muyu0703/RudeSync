import type {
  Client,
  CreateClientInput,
  CreateProjectInput,
  CreateWorkEntryInput,
  Project,
  ProjectMilestone,
  ProjectStatus,
  WorkEntry,
  WorkService,
} from "./types";

const STORAGE_KEY = "rudesync.work.v1";

type Invoke = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

interface TauriWindow extends Window {
  __TAURI_INTERNALS__?: { invoke?: Invoke };
  __TAURI__?: { core?: { invoke?: Invoke } };
}

interface BrowserWorkState {
  clients: Client[];
  projects: Project[];
  workEntries: WorkEntry[];
}

/*
 * Keep native command names in one place. If the Rust API changes, this is the
 * only mapping that needs to move with it.
 */
export const WORK_COMMANDS = Object.freeze({
  listClients: "list_clients",
  createClient: "create_client",
  updateClient: "update_client",
  listProjects: "list_projects",
  createProject: "create_project",
  updateProject: "update_project",
  listWorkEntries: "list_work_entries",
  createWorkEntry: "create_work_entry",
  updateWorkEntry: "update_work_entry",
});

function valueFrom(
  raw: Record<string, unknown>,
  camelCase: string,
  snakeCase: string,
): unknown {
  return raw[camelCase] ?? raw[snakeCase];
}

function stringOrNull(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  const normalized = String(value).trim();
  return normalized || null;
}

function normalizeCurrency(value: unknown): string {
  const currency = String(value ?? "USD").trim().toUpperCase();
  return /^[A-Z]{3}$/.test(currency) ? currency : "USD";
}

function safeInteger(value: unknown, fallback = 0): number {
  const number = Number(value);
  return Number.isSafeInteger(number) ? number : fallback;
}

function nowIso(): string {
  return new Date().toISOString();
}

function localIsoDay(date = new Date()): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function uid(prefix: string): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function normalizeMilestones(
  value: unknown,
  kickoffBasisPoints: number,
  completionBasisPoints: number,
  kickoffLabel = "Kickoff",
  completionLabel = "Completion",
): ProjectMilestone[] {
  if (Array.isArray(value) && value.length >= 2) {
    const normalized = value
      .map((item, index): ProjectMilestone | null => {
        const raw = (item ?? {}) as Record<string, unknown>;
        const kind =
          raw.kind === "kickoff" || raw.kind === "completion"
            ? raw.kind
            : index === 0
              ? "kickoff"
              : "completion";
        const fallback = kind === "kickoff" ? 5000 : 5000;
        const percentBasisPoints = safeInteger(
          valueFrom(raw, "percentBasisPoints", "percent_basis_points"),
          fallback,
        );
        const defaultLabel = kind === "kickoff" ? "Kickoff" : "Completion";
        return {
          kind,
          label: String(raw.label ?? defaultLabel).trim() || defaultLabel,
          percentBasisPoints,
        };
      })
      .filter((item): item is ProjectMilestone => item !== null);

    const kickoff = normalized.find((item) => item.kind === "kickoff");
    const completion = normalized.find((item) => item.kind === "completion");
    if (kickoff && completion) return [kickoff, completion];
  }

  return [
    {
      kind: "kickoff",
      label: kickoffLabel,
      percentBasisPoints: kickoffBasisPoints,
    },
    {
      kind: "completion",
      label: completionLabel,
      percentBasisPoints: completionBasisPoints,
    },
  ];
}

function parseClient(value: unknown): Client {
  const raw = (value ?? {}) as Record<string, unknown>;
  return {
    id: String(raw.id ?? uid("client")),
    name: String(raw.name ?? "").trim(),
    companyName: stringOrNull(valueFrom(raw, "companyName", "company_name")),
    email: stringOrNull(raw.email),
    billingAddress: stringOrNull(
      valueFrom(raw, "billingAddress", "billing_address"),
    ),
    currency: normalizeCurrency(raw.currency),
    notes: stringOrNull(raw.notes),
    createdAt: String(
      valueFrom(raw, "createdAt", "created_at") ?? nowIso(),
    ),
    updatedAt: String(
      valueFrom(raw, "updatedAt", "updated_at") ?? nowIso(),
    ),
  };
}

function parseProject(value: unknown): Project {
  const raw = (value ?? {}) as Record<string, unknown>;
  const kickoffBasisPoints = safeInteger(
    valueFrom(
      raw,
      "kickoffPercentBasisPoints",
      "kickoff_percent_basis_points",
    ),
    5000,
  );
  const completionBasisPoints = safeInteger(
    valueFrom(
      raw,
      "completionPercentBasisPoints",
      "completion_percent_basis_points",
    ),
    5000,
  );
  const rawStatus = String(raw.status ?? "active");
  const status: ProjectStatus = (
    ["draft", "active", "completed", "archived"] as const
  ).includes(rawStatus as ProjectStatus)
    ? (rawStatus as ProjectStatus)
    : "active";

  return {
    id: String(raw.id ?? uid("project")),
    clientId: String(valueFrom(raw, "clientId", "client_id") ?? ""),
    name: String(raw.name ?? "").trim(),
    description: stringOrNull(raw.description),
    urls: parseUrls(valueFrom(raw, "urls", "urls_json")),
    status,
    currency: normalizeCurrency(raw.currency),
    quotedTotalMinor: safeInteger(
      valueFrom(raw, "quotedTotalMinor", "quoted_total_minor"),
    ),
    milestones: normalizeMilestones(
      raw.milestones,
      kickoffBasisPoints,
      completionBasisPoints,
      String(
        valueFrom(raw, "kickoffLabel", "kickoff_label") ?? "Kickoff",
      ),
      String(
        valueFrom(raw, "completionLabel", "completion_label") ??
          "Completion",
      ),
    ),
    startDate: stringOrNull(valueFrom(raw, "startDate", "start_date")),
    dueDate: stringOrNull(valueFrom(raw, "dueDate", "due_date")),
    completedAt: stringOrNull(
      valueFrom(raw, "completedAt", "completed_at"),
    ),
    createdAt: String(
      valueFrom(raw, "createdAt", "created_at") ?? nowIso(),
    ),
    updatedAt: String(
      valueFrom(raw, "updatedAt", "updated_at") ?? nowIso(),
    ),
  };
}

function parseUrls(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value
      .map((item) => String(item).trim())
      .filter(Boolean);
  }
  if (typeof value !== "string" || !value.trim()) return [];
  try {
    const parsed = JSON.parse(value) as unknown;
    return Array.isArray(parsed)
      ? parsed.map((item) => String(item).trim()).filter(Boolean)
      : [];
  } catch {
    return value
      .split(/\r?\n|,/)
      .map((item) => item.trim())
      .filter(Boolean);
  }
}

function parseWorkEntry(value: unknown): WorkEntry {
  const raw = (value ?? {}) as Record<string, unknown>;
  return {
    id: String(raw.id ?? uid("work")),
    projectId: stringOrNull(valueFrom(raw, "projectId", "project_id")),
    title: String(raw.title ?? "").trim(),
    details: stringOrNull(raw.details),
    workDate: String(
      valueFrom(raw, "workDate", "work_date") ?? localIsoDay(),
    ),
    urls: parseUrls(valueFrom(raw, "urls", "urls_json")),
    createdAt: String(
      valueFrom(raw, "createdAt", "created_at") ?? nowIso(),
    ),
    updatedAt: String(
      valueFrom(raw, "updatedAt", "updated_at") ?? nowIso(),
    ),
  };
}

function compactText(value: string | null | undefined): string | null {
  const text = value?.trim();
  return text ? text : null;
}

function normalizeClientInput(input: CreateClientInput): CreateClientInput {
  return {
    name: input.name.trim(),
    companyName: compactText(input.companyName),
    email: compactText(input.email),
    billingAddress: compactText(input.billingAddress),
    currency: normalizeCurrency(input.currency),
    notes: compactText(input.notes),
  };
}

function normalizeProjectInput(input: CreateProjectInput): CreateProjectInput {
  const milestones = input.milestones.map((milestone) => ({
    ...milestone,
    label: milestone.label.trim(),
    percentBasisPoints: Math.round(milestone.percentBasisPoints),
  }));
  return {
    clientId: input.clientId,
    name: input.name.trim(),
    description: compactText(input.description),
    urls: (input.urls ?? []).map((url) => url.trim()).filter(Boolean),
    status: input.status,
    currency: normalizeCurrency(input.currency),
    quotedTotalMinor: Math.max(0, Math.round(input.quotedTotalMinor)),
    milestones,
    startDate: compactText(input.startDate),
    dueDate: compactText(input.dueDate),
  };
}

function normalizeWorkEntryInput(
  input: CreateWorkEntryInput,
): CreateWorkEntryInput {
  return {
    projectId: compactText(input.projectId),
    title: input.title.trim(),
    details: compactText(input.details),
    workDate: input.workDate || localIsoDay(),
    urls: (input.urls ?? []).map((url) => url.trim()).filter(Boolean),
  };
}

function projectInvokeInput(
  input: CreateProjectInput,
): Record<string, unknown> {
  const normalized = normalizeProjectInput(input);
  const kickoff =
    normalized.milestones.find((item) => item.kind === "kickoff")
      ?.percentBasisPoints ?? 5000;
  const completion =
    normalized.milestones.find((item) => item.kind === "completion")
      ?.percentBasisPoints ?? 5000;
  const kickoffLabel =
    normalized.milestones.find((item) => item.kind === "kickoff")?.label ??
    "Kickoff";
  const completionLabel =
    normalized.milestones.find((item) => item.kind === "completion")?.label ??
    "Completion";

  return {
    ...normalized,
    kickoffPercentBasisPoints: kickoff,
    kickoffLabel,
    completionPercentBasisPoints: completion,
    completionLabel,
  };
}

class BrowserWorkService implements WorkService {
  private read(): BrowserWorkState {
    const empty: BrowserWorkState = {
      clients: [],
      projects: [],
      workEntries: [],
    };
    if (typeof window === "undefined") return empty;

    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (!stored) return empty;

    try {
      const raw = JSON.parse(stored) as Partial<BrowserWorkState>;
      return {
        clients: Array.isArray(raw.clients)
          ? raw.clients.map(parseClient)
          : [],
        projects: Array.isArray(raw.projects)
          ? raw.projects.map(parseProject)
          : [],
        workEntries: Array.isArray(raw.workEntries)
          ? raw.workEntries.map(parseWorkEntry)
          : [],
      };
    } catch {
      window.localStorage.removeItem(STORAGE_KEY);
      return empty;
    }
  }

  private write(state: BrowserWorkState): void {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  }

  async listClients(): Promise<Client[]> {
    return this.read().clients.sort((a, b) => a.name.localeCompare(b.name));
  }

  async createClient(input: CreateClientInput): Promise<Client> {
    const normalized = normalizeClientInput(input);
    if (!normalized.name) throw new Error("Client name is required.");

    const state = this.read();
    const timestamp = nowIso();
    const client: Client = {
      id: uid("client"),
      ...normalized,
      companyName: normalized.companyName ?? null,
      email: normalized.email ?? null,
      billingAddress: normalized.billingAddress ?? null,
      notes: normalized.notes ?? null,
      createdAt: timestamp,
      updatedAt: timestamp,
    };
    state.clients.push(client);
    this.write(state);
    return client;
  }

  async updateClient(
    id: string,
    input: CreateClientInput,
  ): Promise<Client> {
    const normalized = normalizeClientInput(input);
    if (!normalized.name) throw new Error("Client name is required.");

    const state = this.read();
    const index = state.clients.findIndex((client) => client.id === id);
    if (index < 0) throw new Error("Client not found.");
    const client: Client = {
      ...state.clients[index],
      ...normalized,
      companyName: normalized.companyName ?? null,
      email: normalized.email ?? null,
      billingAddress: normalized.billingAddress ?? null,
      notes: normalized.notes ?? null,
      updatedAt: nowIso(),
    };
    state.clients[index] = client;
    this.write(state);
    return client;
  }

  async listProjects(): Promise<Project[]> {
    return this.read().projects.sort((a, b) =>
      b.updatedAt.localeCompare(a.updatedAt),
    );
  }

  async createProject(input: CreateProjectInput): Promise<Project> {
    const normalized = normalizeProjectInput(input);
    if (!normalized.name) throw new Error("Project name is required.");
    if (!Number.isSafeInteger(normalized.quotedTotalMinor)) {
      throw new Error("Project value is invalid.");
    }
    const milestoneTotal = normalized.milestones.reduce(
      (sum, milestone) => sum + milestone.percentBasisPoints,
      0,
    );
    if (milestoneTotal !== 10000) {
      throw new Error("Milestone percentages must total 100%.");
    }

    const state = this.read();
    if (!state.clients.some((client) => client.id === normalized.clientId)) {
      throw new Error("Select an existing client.");
    }
    const timestamp = nowIso();
    const project: Project = {
      id: uid("project"),
      ...normalized,
      description: normalized.description ?? null,
      urls: normalized.urls ?? [],
      startDate: normalized.startDate ?? null,
      dueDate: normalized.dueDate ?? null,
      completedAt: normalized.status === "completed" ? timestamp : null,
      createdAt: timestamp,
      updatedAt: timestamp,
    };
    state.projects.unshift(project);
    this.write(state);
    return project;
  }

  async updateProject(
    id: string,
    input: CreateProjectInput,
  ): Promise<Project> {
    const normalized = normalizeProjectInput(input);
    if (!normalized.name) throw new Error("Project name is required.");
    if (!Number.isSafeInteger(normalized.quotedTotalMinor)) {
      throw new Error("Project value is invalid.");
    }
    const milestoneTotal = normalized.milestones.reduce(
      (sum, milestone) => sum + milestone.percentBasisPoints,
      0,
    );
    if (milestoneTotal !== 10000) {
      throw new Error("Milestone percentages must total 100%.");
    }

    const state = this.read();
    if (!state.clients.some((client) => client.id === normalized.clientId)) {
      throw new Error("Select an existing client.");
    }
    const index = state.projects.findIndex((project) => project.id === id);
    if (index < 0) throw new Error("Project not found.");
    const timestamp = nowIso();
    const existing = state.projects[index];
    const project: Project = {
      ...existing,
      ...normalized,
      description: normalized.description ?? null,
      urls: normalized.urls ?? [],
      startDate: normalized.startDate ?? null,
      dueDate: normalized.dueDate ?? null,
      completedAt:
        normalized.status === "completed"
          ? existing.completedAt ?? timestamp
          : null,
      updatedAt: timestamp,
    };
    state.projects[index] = project;
    this.write(state);
    return project;
  }

  async listWorkEntries(): Promise<WorkEntry[]> {
    return this.read().workEntries.sort((a, b) =>
      b.workDate.localeCompare(a.workDate),
    );
  }

  async createWorkEntry(input: CreateWorkEntryInput): Promise<WorkEntry> {
    const normalized = normalizeWorkEntryInput(input);
    if (!normalized.title) throw new Error("A work-record title is required.");

    const state = this.read();
    if (
      normalized.projectId &&
      !state.projects.some((project) => project.id === normalized.projectId)
    ) {
      throw new Error("Select an existing project.");
    }
    const timestamp = nowIso();
    const entry: WorkEntry = {
      id: uid("work"),
      projectId: normalized.projectId ?? null,
      title: normalized.title,
      details: normalized.details ?? null,
      workDate: normalized.workDate,
      urls: normalized.urls ?? [],
      createdAt: timestamp,
      updatedAt: timestamp,
    };
    state.workEntries.unshift(entry);
    this.write(state);
    return entry;
  }

  async updateWorkEntry(
    id: string,
    input: CreateWorkEntryInput,
  ): Promise<WorkEntry> {
    const normalized = normalizeWorkEntryInput(input);
    if (!normalized.title) throw new Error("A work-record title is required.");

    const state = this.read();
    if (
      normalized.projectId &&
      !state.projects.some((project) => project.id === normalized.projectId)
    ) {
      throw new Error("Select an existing project.");
    }
    const index = state.workEntries.findIndex((entry) => entry.id === id);
    if (index < 0) throw new Error("Completed-work record not found.");
    const entry: WorkEntry = {
      ...state.workEntries[index],
      projectId: normalized.projectId ?? null,
      title: normalized.title,
      details: normalized.details ?? null,
      workDate: normalized.workDate,
      urls: normalized.urls ?? [],
      updatedAt: nowIso(),
    };
    state.workEntries[index] = entry;
    this.write(state);
    return entry;
  }
}

class TauriWorkService implements WorkService {
  constructor(private readonly invoke: Invoke) {}

  async listClients(): Promise<Client[]> {
    const rows = await this.invoke<unknown[]>(WORK_COMMANDS.listClients, {
      includeDeleted: false,
    });
    return rows.map(parseClient);
  }

  async createClient(input: CreateClientInput): Promise<Client> {
    const row = await this.invoke<unknown>(WORK_COMMANDS.createClient, {
      input: normalizeClientInput(input),
    });
    return parseClient(row);
  }

  async updateClient(
    id: string,
    input: CreateClientInput,
  ): Promise<Client> {
    const row = await this.invoke<unknown>(WORK_COMMANDS.updateClient, {
      id,
      input: normalizeClientInput(input),
    });
    return parseClient(row);
  }

  async listProjects(): Promise<Project[]> {
    const rows = await this.invoke<unknown[]>(WORK_COMMANDS.listProjects, {
      filter: { includeDeleted: false },
    });
    return rows.map(parseProject);
  }

  async createProject(input: CreateProjectInput): Promise<Project> {
    const row = await this.invoke<unknown>(WORK_COMMANDS.createProject, {
      input: projectInvokeInput(input),
    });
    return parseProject(row);
  }

  async updateProject(
    id: string,
    input: CreateProjectInput,
  ): Promise<Project> {
    const row = await this.invoke<unknown>(WORK_COMMANDS.updateProject, {
      id,
      input: projectInvokeInput(input),
    });
    return parseProject(row);
  }

  async listWorkEntries(): Promise<WorkEntry[]> {
    const rows = await this.invoke<unknown[]>(WORK_COMMANDS.listWorkEntries, {
      filter: { includeDeleted: false },
    });
    return rows.map(parseWorkEntry);
  }

  async createWorkEntry(input: CreateWorkEntryInput): Promise<WorkEntry> {
    const row = await this.invoke<unknown>(WORK_COMMANDS.createWorkEntry, {
      input: normalizeWorkEntryInput(input),
    });
    return parseWorkEntry(row);
  }

  async updateWorkEntry(
    id: string,
    input: CreateWorkEntryInput,
  ): Promise<WorkEntry> {
    const row = await this.invoke<unknown>(WORK_COMMANDS.updateWorkEntry, {
      id,
      input: normalizeWorkEntryInput(input),
    });
    return parseWorkEntry(row);
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

export function createWorkService(): WorkService {
  const invoke = resolveTauriInvoke();
  return invoke ? new TauriWorkService(invoke) : new BrowserWorkService();
}

export const workDateUtils = {
  localIsoDay,
};
