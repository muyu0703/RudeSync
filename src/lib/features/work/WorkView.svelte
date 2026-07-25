<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
  import Icon from "../../components/Icon.svelte";
  import {
    createMoneyService,
    effectiveInvoiceStatus,
    invoiceTotals,
  } from "../money/moneyService";
  import type { Invoice } from "../money/types";
  import { createSettingsService } from "../settings/settingsService";
  import ClientForm from "./components/ClientForm.svelte";
  import ProjectForm from "./components/ProjectForm.svelte";
  import WorkDialog from "./components/WorkDialog.svelte";
  import WorkEntryForm from "./components/WorkEntryForm.svelte";
  import { planTotalMinor } from "./milestonePlans";
  import { createWorkService } from "./workService";
  import type {
    Client,
    CreateClientInput,
    CreateProjectInput,
    CreateWorkEntryInput,
    MilestoneStatus,
    Project,
    ProjectMilestone,
    WorkEntry,
    WorkEntryPrefill,
  } from "./types";

  type DialogKind = "client" | "project" | "work" | null;

  export let workPrefill: WorkEntryPrefill | null = null;

  const dispatch = createEventDispatcher<{ prefillHandled: void }>();
  const service = createWorkService();
  const moneyService = createMoneyService();
  const settingsService = createSettingsService();

  let clients: Client[] = [];
  let projects: Project[] = [];
  let workEntries: WorkEntry[] = [];
  let invoices: Invoice[] = [];
  let loading = true;
  let saving = false;
  let errorMessage = "";
  let dialog: DialogKind = null;
  let editingClient: Client | null = null;
  let editingProject: Project | null = null;
  let editingWorkEntry: WorkEntry | null = null;
  let selectedClientId = "all";
  let workEntryProjectId: string | null = null;
  let workEntryTitle = "";
  let workEntryDetails = "";
  let workEntryDate = "";
  let handledPrefillId = "";
  let searchQuery = "";
  let defaultCurrency = "USD";

  $: clientsById = new Map(clients.map((client) => [client.id, client]));
  $: projectsById = new Map(projects.map((project) => [project.id, project]));
  $: activeProjects = projects.filter((project) => project.status === "active");
  $: completedThisWeek = countCompletedThisWeek(workEntries);
  $: expectedTotals = groupTotalsByCurrency(activeProjects);
  $: normalizedSearch = searchQuery.trim().toLocaleLowerCase();
  $: filteredProjects = projects.filter((project) => {
    if (
      selectedClientId !== "all" &&
      project.clientId !== selectedClientId
    ) {
      return false;
    }
    if (!normalizedSearch) return true;
    const client = clientsById.get(project.clientId);
    return [
      project.name,
      project.description,
      ...project.urls,
      client?.name,
      client?.companyName,
    ]
      .filter(Boolean)
      .some((value) =>
        String(value).toLocaleLowerCase().includes(normalizedSearch),
      );
  });
  $: sortedWorkEntries = [...workEntries].sort((a, b) =>
    b.workDate.localeCompare(a.workDate),
  );
  $: if (
    workPrefill &&
    !loading &&
    workPrefill.requestId !== handledPrefillId
  ) {
    handledPrefillId = workPrefill.requestId;
    openWorkDialog(workPrefill.projectId, workPrefill);
    dispatch("prefillHandled");
  }

  onMount(() => {
    void loadWork();
    void settingsService
      .getSettings()
      .then((settings) => {
        defaultCurrency = settings.defaultCurrency;
      })
      .catch(() => {});
  });

  async function loadWork(): Promise<void> {
    loading = true;
    errorMessage = "";
    let invoiceLoadError = "";
    try {
      const invoicesPromise = moneyService.listInvoices().catch((error) => {
        invoiceLoadError = errorText(
          error,
          "Project billing could not be loaded.",
        );
        return [] as Invoice[];
      });
      [clients, projects, workEntries, invoices] = await Promise.all([
        service.listClients(),
        service.listProjects(),
        service.listWorkEntries(),
        invoicesPromise,
      ]);
      if (invoiceLoadError) errorMessage = invoiceLoadError;
    } catch (error) {
      errorMessage = errorText(
        error,
        "Your work workspace could not be loaded.",
      );
    } finally {
      loading = false;
    }
  }

  async function saveClient(input: CreateClientInput): Promise<void> {
    if (saving) return;
    saving = true;
    errorMessage = "";
    try {
      const saved = editingClient
        ? await service.updateClient(editingClient.id, input)
        : await service.createClient(input);
      clients = editingClient
        ? clients.map((client) => (client.id === saved.id ? saved : client))
        : [...clients, saved];
      clients = [...clients].sort((a, b) =>
        a.name.localeCompare(b.name),
      );
      selectedClientId = saved.id;
      dialog = null;
      editingClient = null;
    } catch (error) {
      errorMessage = errorText(error, "The client could not be saved.");
    } finally {
      saving = false;
    }
  }

  async function saveProject(input: CreateProjectInput): Promise<void> {
    if (saving) return;
    saving = true;
    errorMessage = "";
    try {
      const saved = editingProject
        ? await service.updateProject(editingProject.id, input)
        : await service.createProject(input);
      projects = editingProject
        ? projects.map((project) =>
            project.id === saved.id ? saved : project,
          )
        : [saved, ...projects];
      selectedClientId = saved.clientId;
      dialog = null;
      editingProject = null;
    } catch (error) {
      errorMessage = errorText(error, "The project could not be saved.");
    } finally {
      saving = false;
    }
  }

  async function saveWorkEntry(
    input: CreateWorkEntryInput,
  ): Promise<void> {
    if (saving) return;
    saving = true;
    errorMessage = "";
    try {
      const saved = editingWorkEntry
        ? await service.updateWorkEntry(editingWorkEntry.id, input)
        : await service.createWorkEntry(input);
      workEntries = editingWorkEntry
        ? workEntries.map((entry) =>
            entry.id === saved.id ? saved : entry,
          )
        : [saved, ...workEntries];
      dialog = null;
      resetWorkDraft();
    } catch (error) {
      errorMessage = errorText(
        error,
        "The completed-work record could not be saved.",
      );
    } finally {
      saving = false;
    }
  }

  function openWorkDialog(
    projectId: string | null = null,
    prefill: WorkEntryPrefill | null = null,
  ): void {
    editingWorkEntry = null;
    workEntryProjectId = projectId;
    workEntryTitle = prefill?.title ?? "";
    workEntryDetails = prefill?.details ?? "";
    workEntryDate = prefill?.workDate ?? workDateToday();
    dialog = "work";
  }

  function openClientDialog(client: Client | null = null): void {
    editingClient = client;
    dialog = "client";
  }

  function openProjectDialog(project: Project | null = null): void {
    editingProject = project;
    dialog = "project";
  }

  function editWorkEntry(entry: WorkEntry): void {
    editingWorkEntry = entry;
    dialog = "work";
  }

  function closeDialog(): void {
    if (saving) return;
    dialog = null;
    editingClient = null;
    editingProject = null;
    resetWorkDraft();
  }

  function resetWorkDraft(): void {
    workEntryProjectId = null;
    workEntryTitle = "";
    workEntryDetails = "";
    workEntryDate = "";
    editingWorkEntry = null;
  }

  function workDateToday(): string {
    return localIsoDay(new Date());
  }

  function errorText(error: unknown, fallback: string): string {
    if (error instanceof Error && error.message) return error.message;
    if (typeof error === "string" && error.trim()) return error;
    return fallback;
  }

  function localIsoDay(date: Date): string {
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    return `${year}-${month}-${day}`;
  }

  function countCompletedThisWeek(entries: WorkEntry[]): number {
    const today = new Date();
    today.setHours(12, 0, 0, 0);
    const day = today.getDay();
    const distanceFromMonday = day === 0 ? 6 : day - 1;
    const monday = new Date(today);
    monday.setDate(today.getDate() - distanceFromMonday);
    const sunday = new Date(monday);
    sunday.setDate(monday.getDate() + 6);
    const from = localIsoDay(monday);
    const to = localIsoDay(sunday);
    return entries.filter(
      (entry) => entry.workDate >= from && entry.workDate <= to,
    ).length;
  }

  function groupTotalsByCurrency(
    values: Project[],
  ): Array<{ currency: string; totalMinor: number }> {
    const totals = new Map<string, number>();
    for (const project of values) {
      totals.set(
        project.currency,
        (totals.get(project.currency) ?? 0) + project.quotedTotalMinor,
      );
    }
    return [...totals.entries()]
      .map(([currency, totalMinor]) => ({ currency, totalMinor }))
      .sort((a, b) => a.currency.localeCompare(b.currency));
  }

  function formatMoney(minor: number, currency: string): string {
    try {
      return new Intl.NumberFormat("en-US", {
        style: "currency",
        currency,
        minimumFractionDigits: 0,
        maximumFractionDigits: 2,
      }).format(minor / 100);
    } catch {
      return `${currency} ${(minor / 100).toLocaleString("en-US")}`;
    }
  }

  function formatDate(value: string | null): string {
    if (!value) return "";
    const date = new Date(`${value.slice(0, 10)}T12:00:00`);
    if (Number.isNaN(date.getTime())) return value;
    return new Intl.DateTimeFormat("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
    }).format(date);
  }

  function milestoneEffectiveStatus(
    milestone: ProjectMilestone,
  ): MilestoneStatus {
    return milestone.status ?? "not-invoiced";
  }

  function milestoneStatusLabel(status: MilestoneStatus): string {
    if (status === "paid") return "Paid";
    if (status === "invoiced") return "Invoiced";
    return "Not invoiced";
  }

  function remainingToInvoiceMinor(project: Project): number {
    return project.milestones
      .filter(
        (milestone) => milestoneEffectiveStatus(milestone) === "not-invoiced",
      )
      .reduce((sum, milestone) => sum + milestone.amountMinor, 0);
  }

  function workForProject(projectId: string): WorkEntry[] {
    return sortedWorkEntries.filter((entry) => entry.projectId === projectId);
  }

  function invoicesForProject(projectId: string): Invoice[] {
    return invoices
      .filter((invoice) => invoice.projectId === projectId)
      .sort((left, right) => {
        const dateOrder = right.issueDate.localeCompare(left.issueDate);
        return dateOrder || right.number.localeCompare(left.number);
      });
  }

  function invoiceStatusLabel(invoice: Invoice): string {
    return effectiveInvoiceStatus(invoice)
      .replace(/[-_]+/g, " ")
      .replace(/\b\w/g, (letter) => letter.toUpperCase());
  }

  function projectCount(clientId: string): number {
    return projects.filter((project) => project.clientId === clientId).length;
  }

  function hostname(url: string): string {
    try {
      return new URL(url).hostname.replace(/^www\./, "");
    } catch {
      return url;
    }
  }
</script>

<section class="work-view" aria-label="Clients and project work" aria-busy={loading}>
  <div class="summary-grid">
    <article class="summary-card">
      <span>Active projects</span>
      <strong>{activeProjects.length}</strong>
      <small>{clients.length} {clients.length === 1 ? "client" : "clients"} in your workspace</small>
    </article>
    <article class="summary-card">
      <span>Completed this week</span>
      <strong>{completedThisWeek}</strong>
      <small>{workEntries.length} work {workEntries.length === 1 ? "record" : "records"} overall</small>
    </article>
    <article class="summary-card accent-card">
      <span>Expected project value</span>
      {#if expectedTotals.length === 0}
        <strong>$0</strong>
        <small>USD · active fixed-price work</small>
      {:else if expectedTotals.length === 1}
        <strong>{formatMoney(expectedTotals[0].totalMinor, expectedTotals[0].currency)}</strong>
        <small>{expectedTotals[0].currency} · active fixed-price work</small>
      {:else}
        <strong>{expectedTotals.length} currencies</strong>
        <small>
          {expectedTotals
            .map((total) => formatMoney(total.totalMinor, total.currency))
            .join(" · ")}
        </small>
      {/if}
    </article>
  </div>

  {#if errorMessage}
    <div class="work-error" role="alert">
      <span>{errorMessage}</span>
      <button type="button" aria-label="Dismiss error" on:click={() => (errorMessage = "")}>
        <Icon name="x" size={14} />
      </button>
    </div>
  {/if}

  <section class="work-toolbar" aria-label="Work actions">
    <label class="search-field">
      <span class="sr-only">Search projects</span>
      <Icon name="search" size={14} />
      <input bind:value={searchQuery} type="search" placeholder="Search projects or clients" />
    </label>
    <div class="toolbar-actions">
      <button class="secondary-button" type="button" on:click={() => openClientDialog()}>
        <Icon name="plus" size={14} /> Client
      </button>
      <button
        class="secondary-button"
        type="button"
        disabled={!clients.length}
        title={!clients.length ? "Create a client first" : "Create a fixed-price project"}
        on:click={() => openProjectDialog()}
      >
        <Icon name="briefcase" size={14} /> Project
      </button>
      <button class="primary-button" type="button" on:click={() => openWorkDialog()}>
        <Icon name="check" size={14} /> Record work
      </button>
    </div>
  </section>

  <div class="work-layout">
    <section class="panel projects-panel" aria-labelledby="projects-title">
      <header class="panel-header">
        <div>
          <span class="panel-kicker">Project containers</span>
          <h2 id="projects-title">
            {filteredProjects.length}
            {filteredProjects.length === 1 ? "project" : "projects"}
          </h2>
        </div>
        {#if projects.length}
          <span class="header-note">Fixed price · no timers</span>
        {/if}
      </header>

      {#if clients.length}
        <div class="client-filters" aria-label="Filter projects by client">
          <button
            class:active={selectedClientId === "all"}
            type="button"
            aria-pressed={selectedClientId === "all"}
            on:click={() => (selectedClientId = "all")}
          >
            All <span>{projects.length}</span>
          </button>
          {#each clients as client (client.id)}
            <button
              class:active={selectedClientId === client.id}
              type="button"
              aria-pressed={selectedClientId === client.id}
              on:click={() => (selectedClientId = client.id)}
            >
              {client.name} <span>{projectCount(client.id)}</span>
            </button>
          {/each}
        </div>
      {/if}

      {#if loading}
        <div class="project-skeletons" aria-label="Loading projects">
          <i></i><i></i><i></i>
        </div>
      {:else if filteredProjects.length}
        <div class="project-list">
          {#each filteredProjects as project (project.id)}
            {@const client = clientsById.get(project.clientId)}
            {@const projectWork = workForProject(project.id)}
            {@const projectInvoices = invoicesForProject(project.id)}
            <article class="project-card">
              <div class="project-heading">
                <div class="project-title">
                  <span class={`status-dot ${project.status}`}></span>
                  <div>
                    <span class="client-name">{client?.name ?? "Unassigned client"}</span>
                    <h3>{project.name}</h3>
                  </div>
                </div>
                <div class="project-value">
                  <strong>{formatMoney(project.milestones.length > 0 ? planTotalMinor(project.milestones) : project.quotedTotalMinor, project.currency)}</strong>
                  <span>{project.status}</span>
                </div>
              </div>

              {#if project.description}
                <p class="project-description">{project.description}</p>
              {/if}

              {#if project.urls.length}
                <div class="project-links" aria-label={`${project.name} reference URLs`}>
                  {#each project.urls.slice(0, 4) as url}
                    <a href={url} target="_blank" rel="noreferrer">
                      <Icon name="link" size={11} /> {hostname(url)}
                    </a>
                  {/each}
                </div>
              {/if}

              {#if project.startDate || project.dueDate}
                <div class="date-range">
                  <Icon name="calendar" size={13} />
                  {#if project.startDate}<time datetime={project.startDate}>{formatDate(project.startDate)}</time>{/if}
                  {#if project.startDate && project.dueDate}<span aria-hidden="true">→</span>{/if}
                  {#if project.dueDate}<time datetime={project.dueDate}>{formatDate(project.dueDate)}</time>{/if}
                </div>
              {/if}

              <div class="milestone-section-header">
                <span>Invoice milestones</span>
                <b>
                  {formatMoney(planTotalMinor(project.milestones), project.currency)} total ·
                  {formatMoney(remainingToInvoiceMinor(project), project.currency)} remaining
                </b>
              </div>
              <div class="milestone-grid" aria-label={`${project.name} invoice milestones`}>
                {#each project.milestones as milestone, index (milestone.id ?? `${project.id}-${milestone.kind}-${index}`)}
                  <div class="milestone">
                    <span class="milestone-index">{String(index + 1).padStart(2, "0")}</span>
                    <div>
                      <span>{milestone.label}</span>
                      <strong>{formatMoney(milestone.amountMinor, project.currency)}</strong>
                    </div>
                    <span class={`invoice-status ${milestoneEffectiveStatus(milestone)}`}>
                      {milestoneStatusLabel(milestoneEffectiveStatus(milestone))}
                    </span>
                  </div>
                {/each}
              </div>

              <section class="project-billing" aria-label={`${project.name} invoices and payments`}>
                <header>
                  <span>Invoices and payments</span>
                  <b>
                    {projectInvoices.length}
                    {projectInvoices.length === 1 ? "invoice" : "invoices"}
                  </b>
                </header>
                {#if projectInvoices.length}
                  <div class="invoice-list">
                    {#each projectInvoices as invoice (invoice.id)}
                      {@const totals = invoiceTotals(invoice)}
                      <article class="invoice-record">
                        <div class="invoice-main">
                          <span class="invoice-icon"><Icon name="invoice" size={14} /></span>
                          <div class="invoice-copy">
                            <strong>{invoice.milestoneLabel ?? "Project invoice"}</strong>
                            <span>
                              {invoice.number}
                              <i aria-hidden="true">·</i>
                              Due {formatDate(invoice.dueDate)}
                            </span>
                          </div>
                          <span class={`invoice-status ${effectiveInvoiceStatus(invoice)}`}>
                            {invoiceStatusLabel(invoice)}
                          </span>
                          <div class="invoice-total">
                            <strong>{formatMoney(totals.totalMinor, invoice.currency)}</strong>
                            <span>{formatMoney(totals.balanceDueMinor, invoice.currency)} due</span>
                          </div>
                        </div>
                        {#if invoice.payments.length}
                          <div class="invoice-payments" aria-label={`${invoice.number} payments`}>
                            {#each invoice.payments as payment, index (payment.id)}
                              <div>
                                <span class="payment-check"><Icon name="check" size={10} strokeWidth={2.3} /></span>
                                <span>Payment {index + 1}</span>
                                <time datetime={payment.receivedDate}>{formatDate(payment.receivedDate)}</time>
                                <strong>{formatMoney(payment.amountMinor, invoice.currency)}</strong>
                              </div>
                            {/each}
                          </div>
                        {/if}
                      </article>
                    {/each}
                  </div>
                {:else}
                  <p>No invoices are attached to this project yet.</p>
                {/if}
              </section>

              {#if projectWork.length}
                <div class="project-work">
                  <span class="project-work-label">Recent completed work</span>
                  {#each projectWork.slice(0, 2) as entry (entry.id)}
                    <div class="mini-work-entry">
                      <span class="work-check"><Icon name="check" size={11} strokeWidth={2.3} /></span>
                      <strong>{entry.title}</strong>
                      <time datetime={entry.workDate}>{formatDate(entry.workDate)}</time>
                      <button type="button" aria-label={`Edit ${entry.title}`} on:click={() => editWorkEntry(entry)}>
                        Edit
                      </button>
                    </div>
                  {/each}
                </div>
              {/if}

              <footer class="project-footer">
                <span>
                  {projectWork.length}
                  {projectWork.length === 1 ? "completed-work record" : "completed-work records"}
                </span>
                <div>
                  <button type="button" on:click={() => openProjectDialog(project)}>
                    <Icon name="more" size={13} /> Edit
                  </button>
                  <button type="button" on:click={() => openWorkDialog(project.id)}>
                    <Icon name="plus" size={13} /> Add work
                  </button>
                </div>
              </footer>
            </article>
          {/each}
        </div>
      {:else if !clients.length}
        <div class="empty-state">
          <span class="empty-icon"><Icon name="briefcase" size={21} /></span>
          <h3>Start with a client</h3>
          <p>Clients organize your fixed-price projects, invoices, and completed work.</p>
          <button class="primary-button" type="button" on:click={() => openClientDialog()}>
            <Icon name="plus" size={14} /> Add your first client
          </button>
        </div>
      {:else if !projects.length}
        <div class="empty-state">
          <span class="empty-icon"><Icon name="briefcase" size={21} /></span>
          <h3>Create the first project container</h3>
          <p>It starts with an editable 50% kickoff and 50% completion plan.</p>
          <button class="primary-button" type="button" on:click={() => openProjectDialog()}>
            <Icon name="plus" size={14} /> Create project
          </button>
        </div>
      {:else}
        <div class="empty-state compact">
          <span class="empty-icon"><Icon name="search" size={19} /></span>
          <h3>No matching projects</h3>
          <p>Clear the search or choose a different client.</p>
          <button
            class="text-button"
            type="button"
            on:click={() => {
              searchQuery = "";
              selectedClientId = "all";
            }}
          >Clear filters</button>
        </div>
      {/if}
    </section>

    <aside class="side-stack">
      <section class="panel clients-panel" aria-labelledby="clients-title">
        <header class="side-header">
          <div>
            <span class="panel-kicker">Relationships</span>
            <h2 id="clients-title">Clients</h2>
          </div>
          <button type="button" aria-label="Add client" on:click={() => openClientDialog()}>
            <Icon name="plus" size={14} />
          </button>
        </header>
        {#if loading}
          <div class="side-skeleton"><i></i><i></i><i></i></div>
        {:else if clients.length}
          <div class="client-list">
            {#each clients as client (client.id)}
              <div class:active={selectedClientId === client.id} class="client-row">
                <button
                  class="client-select"
                  type="button"
                  aria-pressed={selectedClientId === client.id}
                  on:click={() =>
                    (selectedClientId =
                      selectedClientId === client.id ? "all" : client.id)}
                >
                  <span class="client-avatar">{client.name.slice(0, 2).toUpperCase()}</span>
                  <span class="client-copy">
                    <strong>{client.name}</strong>
                    <small>{client.companyName ?? `${projectCount(client.id)} projects`}</small>
                  </span>
                  <b>{client.currency}</b>
                </button>
                <button
                  class="client-edit"
                  type="button"
                  aria-label={`Edit ${client.name}`}
                  title={`Edit ${client.name}`}
                  on:click={() => openClientDialog(client)}
                >
                  <Icon name="more" size={14} />
                </button>
              </div>
            {/each}
          </div>
        {:else}
          <p class="side-empty">No clients yet.</p>
        {/if}
      </section>

      <section class="panel activity-panel" aria-labelledby="activity-title">
        <header class="side-header">
          <div>
            <span class="panel-kicker">Completed work</span>
            <h2 id="activity-title">Recent records</h2>
          </div>
          <button type="button" aria-label="Record completed work" on:click={() => openWorkDialog()}>
            <Icon name="plus" size={14} />
          </button>
        </header>
        {#if loading}
          <div class="side-skeleton"><i></i><i></i><i></i></div>
        {:else if sortedWorkEntries.length}
          <div class="activity-list">
            {#each sortedWorkEntries.slice(0, 6) as entry (entry.id)}
              {@const project = entry.projectId ? projectsById.get(entry.projectId) : null}
              <article>
                <span class="activity-dot"></span>
                <div>
                  <strong>{entry.title}</strong>
                  <span>
                    <time datetime={entry.workDate}>{formatDate(entry.workDate)}</time>
                    {#if project} · {project.name}{/if}
                  </span>
                  {#if entry.urls.length}
                    <div class="entry-links">
                      {#each entry.urls.slice(0, 2) as url}
                        <a href={url} target="_blank" rel="noreferrer" aria-label={`Open ${hostname(url)} in your browser`}>
                          <Icon name="link" size={11} /> {hostname(url)}
                        </a>
                      {/each}
                    </div>
                  {/if}
                </div>
                <button
                  class="entry-edit"
                  type="button"
                  aria-label={`Edit ${entry.title}`}
                  title={`Edit ${entry.title}`}
                  on:click={() => editWorkEntry(entry)}
                >
                  <Icon name="more" size={13} />
                </button>
              </article>
            {/each}
          </div>
        {:else}
          <div class="side-empty">
            <p>Finished something? Keep a lightweight record of it here.</p>
            <button class="text-button" type="button" on:click={() => openWorkDialog()}>Record work</button>
          </div>
        {/if}
      </section>
    </aside>
  </div>
</section>

{#if dialog === "client"}
  <WorkDialog
    title={editingClient ? "Edit client" : "New client"}
    description={editingClient
      ? "Correct contact, billing, currency, or private notes. Issued invoices keep their original snapshot."
      : "Create the relationship once, then keep every project and invoice organized beneath it."}
    onClose={closeDialog}
  >
    <ClientForm
      {defaultCurrency}
      client={editingClient}
      busy={saving}
      onSave={saveClient}
      onCancel={closeDialog}
    />
  </WorkDialog>
{:else if dialog === "project"}
  <WorkDialog
    title={editingProject ? "Edit fixed-price project" : "New fixed-price project"}
    description={editingProject
      ? "Correct value, status, dates, notes, links, or milestone split. Existing invoices remain unchanged."
      : "The project is the container for milestones, completed work, invoices, and payments."}
    wide
    onClose={closeDialog}
  >
    <ProjectForm
      {clients}
      initialClientId={selectedClientId === "all" ? null : selectedClientId}
      project={editingProject}
      busy={saving}
      onSave={saveProject}
      onCancel={closeDialog}
    />
  </WorkDialog>
{:else if dialog === "work"}
  <WorkDialog
    title={editingWorkEntry ? "Edit completed work" : "Record completed work"}
    description={editingWorkEntry
      ? "Correct the project, date, result, details, or reference URLs."
      : "Capture the result, not the hours. URLs stay as lightweight references."}
    onClose={closeDialog}
  >
    <WorkEntryForm
      {projects}
      entry={editingWorkEntry}
      initialProjectId={workEntryProjectId}
      initialTitle={workEntryTitle}
      initialDetails={workEntryDetails}
      initialWorkDate={workEntryDate || workDateToday()}
      busy={saving}
      onSave={saveWorkEntry}
      onCancel={closeDialog}
    />
  </WorkDialog>
{/if}

<style>
  .work-view {
    display: grid;
    gap: 16px;
    max-width: 1220px;
    margin: 0 auto;
    color: var(--text-primary, #edf5f0);
  }

  .summary-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
  }

  .summary-card {
    display: flex;
    min-width: 0;
    min-height: 104px;
    flex-direction: column;
    justify-content: center;
    padding: 17px 19px;
    background: var(--surface-1, #0c1210);
    border: 1px solid var(--border-subtle, #1b2821);
    border-radius: 11px;
  }

  .summary-card > span {
    color: var(--text-muted, #75847b);
    font-size: 9.5px;
    font-weight: 620;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .summary-card strong {
    margin-top: 5px;
    overflow: hidden;
    color: var(--text-primary, #edf5f0);
    font-size: 23px;
    font-weight: 650;
    letter-spacing: -0.035em;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .summary-card small {
    margin-top: 4px;
    overflow: hidden;
    color: var(--text-faint, #536158);
    font-size: 9.5px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .summary-card.accent-card {
    background: linear-gradient(135deg, rgba(67, 209, 127, 0.08), rgba(67, 209, 127, 0.025));
    border-color: var(--accent-border, rgba(67, 209, 127, 0.26));
  }

  .summary-card.accent-card strong {
    color: var(--accent-bright, #60e596);
  }

  .work-error {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 40px;
    padding: 8px 12px 8px 14px;
    color: #ffd2ce;
    font-size: 11px;
    background: rgba(164, 49, 44, 0.12);
    border: 1px solid rgba(239, 118, 111, 0.24);
    border-radius: 9px;
  }

  .work-error button {
    display: grid;
    width: 26px;
    height: 26px;
    padding: 0;
    place-items: center;
    color: inherit;
    background: transparent;
    border: 0;
    border-radius: 6px;
    cursor: pointer;
  }

  .work-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px;
    background: var(--surface-1, #0c1210);
    border: 1px solid var(--border-subtle, #1b2821);
    border-radius: 10px;
  }

  .search-field {
    display: flex;
    width: min(330px, 100%);
    height: 34px;
    align-items: center;
    gap: 8px;
    padding: 0 10px;
    color: var(--text-faint, #536158);
    background: var(--surface-0, #090d0b);
    border: 1px solid var(--border-subtle, #1b2821);
    border-radius: 8px;
  }

  .search-field:focus-within {
    color: var(--accent, #43d17f);
    border-color: var(--accent-border, rgba(67, 209, 127, 0.26));
  }

  .search-field input {
    width: 100%;
    min-width: 0;
    height: 100%;
    padding: 0;
    color: var(--text-primary, #edf5f0);
    font: inherit;
    font-size: 11px;
    background: transparent;
    border: 0;
    outline: 0;
  }

  .search-field input::placeholder {
    color: var(--text-faint, #536158);
  }

  .toolbar-actions {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .primary-button,
  .secondary-button {
    display: inline-flex;
    min-height: 34px;
    align-items: center;
    justify-content: center;
    gap: 7px;
    padding: 0 12px;
    font: inherit;
    font-size: 10.5px;
    font-weight: 650;
    border-radius: 8px;
    cursor: pointer;
  }

  .primary-button {
    color: #07120c;
    background: var(--accent, #43d17f);
    border: 1px solid var(--accent, #43d17f);
  }

  .primary-button:hover {
    background: var(--accent-bright, #60e596);
  }

  .secondary-button {
    color: var(--text-secondary, #aebdb4);
    background: transparent;
    border: 1px solid var(--border-strong, #2a3a31);
  }

  .secondary-button:hover {
    color: var(--text-primary, #edf5f0);
    background: var(--surface-raised, #141d18);
  }

  .primary-button:disabled,
  .secondary-button:disabled {
    cursor: not-allowed;
    opacity: 0.43;
  }

  button:focus-visible,
  a:focus-visible {
    outline: 2px solid var(--accent, #43d17f);
    outline-offset: 2px;
  }

  .work-layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 292px;
    align-items: start;
    gap: 14px;
  }

  .panel {
    min-width: 0;
    background: var(--surface-1, #0c1210);
    border: 1px solid var(--border-subtle, #1b2821);
    border-radius: 11px;
  }

  .projects-panel {
    min-height: 430px;
    padding: 0 17px 17px;
  }

  .panel-header,
  .side-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .panel-header {
    min-height: 68px;
  }

  .panel-kicker {
    color: var(--text-muted, #75847b);
    font-size: 8.5px;
    font-weight: 700;
    letter-spacing: 0.11em;
    text-transform: uppercase;
  }

  .panel-header h2,
  .side-header h2 {
    margin: 3px 0 0;
    color: var(--text-primary, #edf5f0);
    font-size: 14px;
    font-weight: 630;
    letter-spacing: -0.015em;
  }

  .header-note {
    color: var(--text-faint, #536158);
    font-size: 9px;
  }

  .client-filters {
    display: flex;
    gap: 5px;
    margin: 0 -2px 13px;
    padding: 1px 2px 7px;
    overflow-x: auto;
    scrollbar-width: none;
  }

  .client-filters::-webkit-scrollbar {
    display: none;
  }

  .client-filters button {
    display: inline-flex;
    min-height: 28px;
    flex: 0 0 auto;
    align-items: center;
    gap: 6px;
    padding: 0 9px;
    color: var(--text-muted, #75847b);
    font: inherit;
    font-size: 9.5px;
    background: transparent;
    border: 1px solid var(--border-subtle, #1b2821);
    border-radius: 99px;
    cursor: pointer;
  }

  .client-filters button:hover,
  .client-filters button.active {
    color: var(--text-primary, #edf5f0);
    background: var(--accent-soft, rgba(67, 209, 127, 0.1));
    border-color: var(--accent-border, rgba(67, 209, 127, 0.26));
  }

  .client-filters button span {
    color: var(--text-faint, #536158);
    font-size: 8.5px;
  }

  .client-filters button.active span {
    color: var(--accent, #43d17f);
  }

  .project-list {
    display: grid;
    gap: 10px;
  }

  .project-card {
    min-width: 0;
    padding: 15px;
    background: var(--surface-2, #101713);
    border: 1px solid var(--border-subtle, #1b2821);
    border-radius: 10px;
  }

  .project-card:hover {
    border-color: var(--border-strong, #2a3a31);
  }

  .project-heading {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 20px;
  }

  .project-title {
    display: flex;
    min-width: 0;
    align-items: flex-start;
    gap: 9px;
  }

  .status-dot {
    width: 7px;
    height: 7px;
    flex: 0 0 auto;
    margin-top: 15px;
    background: var(--text-faint, #536158);
    border-radius: 50%;
    box-shadow: 0 0 0 3px rgba(83, 97, 88, 0.1);
  }

  .status-dot.active {
    background: var(--accent, #43d17f);
    box-shadow: 0 0 0 3px var(--accent-soft, rgba(67, 209, 127, 0.1));
  }

  .status-dot.completed {
    background: var(--blue, #67a8e6);
    box-shadow: 0 0 0 3px rgba(103, 168, 230, 0.1);
  }

  .client-name {
    color: var(--text-muted, #75847b);
    font-size: 9px;
  }

  .project-title h3 {
    margin: 2px 0 0;
    overflow: hidden;
    color: var(--text-primary, #edf5f0);
    font-size: 13px;
    font-weight: 620;
    line-height: 1.35;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .project-value {
    display: flex;
    flex: 0 0 auto;
    flex-direction: column;
    align-items: flex-end;
  }

  .project-value strong {
    color: var(--text-primary, #edf5f0);
    font-size: 12.5px;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
  }

  .project-value span {
    margin-top: 3px;
    color: var(--text-faint, #536158);
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
  }

  .project-description {
    display: -webkit-box;
    margin: 11px 0 0 16px;
    overflow: hidden;
    color: var(--text-muted, #75847b);
    font-size: 10.5px;
    line-height: 1.55;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }

  .project-links {
    display: flex;
    gap: 5px;
    margin: 8px 0 0 16px;
    overflow: hidden;
  }

  .project-links a {
    display: inline-flex;
    min-width: 0;
    align-items: center;
    gap: 4px;
    padding: 3px 6px;
    overflow: hidden;
    color: var(--accent, #43d17f);
    font-size: 8.5px;
    background: var(--accent-soft, rgba(67, 209, 127, 0.1));
    border-radius: 5px;
    text-decoration: none;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .date-range {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 10px 0 0 16px;
    color: var(--text-faint, #536158);
    font-size: 9.5px;
  }

  .milestone-section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-top: 14px;
    margin-bottom: 8px;
  }

  .milestone-section-header span {
    color: var(--text-faint, #536158);
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .milestone-section-header b {
    color: var(--text-muted, #75847b);
    font-size: 8.5px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .milestone-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .milestone {
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    padding: 9px;
    background: var(--surface-0, #090d0b);
    border: 1px solid var(--border-subtle, #1b2821);
    border-radius: 8px;
  }

  .milestone-index {
    display: grid;
    width: 27px;
    height: 27px;
    place-items: center;
    color: var(--accent, #43d17f);
    font-size: 8px;
    font-weight: 700;
    background: var(--accent-soft, rgba(67, 209, 127, 0.1));
    border-radius: 50%;
  }

  .milestone > div {
    display: flex;
    min-width: 0;
    flex-direction: column;
  }

  .milestone > div span {
    overflow: hidden;
    color: var(--text-secondary, #aebdb4);
    font-size: 9.5px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .milestone > div strong {
    margin-top: 2px;
    color: var(--text-muted, #75847b);
    font-size: 9px;
    font-weight: 500;
    font-variant-numeric: tabular-nums;
  }

  .project-billing {
    margin-top: 13px;
    padding-top: 12px;
    border-top: 1px solid var(--border-subtle, #1b2821);
  }

  .project-billing > header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 8px;
  }

  .project-billing > header > span {
    color: var(--text-faint, #536158);
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .project-billing > header > b {
    color: var(--text-muted, #75847b);
    font-size: 8.5px;
    font-weight: 600;
  }

  .project-billing > p {
    margin: 0;
    padding: 8px 10px;
    color: var(--text-faint, #536158);
    font-size: 9px;
    background: var(--surface-0, #090d0b);
    border: 1px dashed var(--border-subtle, #1b2821);
    border-radius: 7px;
  }

  .invoice-list {
    display: grid;
    gap: 7px;
  }

  .invoice-record {
    min-width: 0;
    padding: 9px;
    background: var(--surface-0, #090d0b);
    border: 1px solid var(--border-subtle, #1b2821);
    border-radius: 8px;
  }

  .invoice-main {
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .invoice-icon {
    display: grid;
    width: 27px;
    height: 27px;
    place-items: center;
    color: var(--accent, #43d17f);
    background: var(--accent-soft, rgba(67, 209, 127, 0.1));
    border-radius: 7px;
  }

  .invoice-copy {
    display: flex;
    min-width: 0;
    flex-direction: column;
  }

  .invoice-copy strong {
    overflow: hidden;
    color: var(--text-secondary, #aebdb4);
    font-size: 9.5px;
    font-weight: 620;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .invoice-copy > span {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 2px;
    overflow: hidden;
    color: var(--text-faint, #536158);
    font-size: 8px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .invoice-copy i {
    color: var(--border-strong, #2a3a31);
    font-style: normal;
  }

  .invoice-status {
    padding: 3px 6px;
    color: var(--text-muted, #75847b);
    font-size: 7.5px;
    font-weight: 700;
    letter-spacing: 0.04em;
    background: var(--surface-raised, #141d18);
    border-radius: 99px;
    text-transform: uppercase;
    white-space: nowrap;
  }

  .invoice-status.paid {
    color: var(--accent, #43d17f);
    background: var(--accent-soft, rgba(67, 209, 127, 0.1));
  }

  .invoice-status.partially-paid,
  .invoice-status.overdue,
  .invoice-status.invoiced {
    color: var(--amber, #e6b85c);
    background: var(--amber-soft, rgba(230, 184, 92, 0.1));
  }

  .invoice-total {
    display: flex;
    min-width: 86px;
    flex-direction: column;
    align-items: flex-end;
  }

  .invoice-total strong {
    color: var(--text-primary, #edf5f0);
    font-size: 9px;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .invoice-total span {
    margin-top: 2px;
    color: var(--text-faint, #536158);
    font-size: 7.5px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .invoice-payments {
    display: grid;
    gap: 5px;
    margin: 8px 0 0 35px;
    padding-top: 7px;
    border-top: 1px solid var(--border-subtle, #1b2821);
  }

  .invoice-payments > div {
    display: grid;
    grid-template-columns: 15px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 6px;
    color: var(--text-muted, #75847b);
    font-size: 8px;
  }

  .invoice-payments time {
    color: var(--text-faint, #536158);
  }

  .invoice-payments strong {
    color: var(--accent, #43d17f);
    font-size: 8px;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
  }

  .payment-check {
    display: grid;
    width: 14px;
    height: 14px;
    place-items: center;
    color: var(--accent, #43d17f);
    background: var(--accent-soft, rgba(67, 209, 127, 0.1));
    border-radius: 50%;
  }

  .project-work {
    display: grid;
    gap: 7px;
    margin-top: 13px;
    padding-top: 12px;
    border-top: 1px solid var(--border-subtle, #1b2821);
  }

  .project-work-label {
    color: var(--text-faint, #536158);
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .mini-work-entry {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 7px;
  }

  .work-check {
    display: grid;
    width: 17px;
    height: 17px;
    place-items: center;
    color: var(--accent, #43d17f);
    background: var(--accent-soft, rgba(67, 209, 127, 0.1));
    border-radius: 50%;
  }

  .mini-work-entry strong {
    overflow: hidden;
    color: var(--text-secondary, #aebdb4);
    font-size: 9.5px;
    font-weight: 550;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mini-work-entry time {
    color: var(--text-faint, #536158);
    font-size: 8.5px;
  }

  .mini-work-entry button {
    padding: 2px 4px;
    color: var(--text-muted, #75847b);
    font: inherit;
    font-size: 8.5px;
    background: transparent;
    border: 0;
    border-radius: 4px;
    cursor: pointer;
  }

  .mini-work-entry button:hover {
    color: var(--accent, #43d17f);
    background: var(--accent-soft, rgba(67, 209, 127, 0.1));
  }

  .project-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    margin-top: 12px;
    padding-top: 10px;
    border-top: 1px solid var(--border-subtle, #1b2821);
  }

  .project-footer > span {
    color: var(--text-faint, #536158);
    font-size: 8.5px;
  }

  .project-footer > div {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .project-footer button,
  .text-button {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 5px;
    color: var(--accent, #43d17f);
    font: inherit;
    font-size: 9.5px;
    font-weight: 620;
    background: transparent;
    border: 0;
    border-radius: 5px;
    cursor: pointer;
  }

  .project-footer button:hover,
  .text-button:hover {
    background: var(--accent-soft, rgba(67, 209, 127, 0.1));
  }

  .side-stack {
    display: grid;
    gap: 14px;
  }

  .clients-panel,
  .activity-panel {
    padding: 0 13px 12px;
  }

  .side-header {
    min-height: 62px;
    padding: 0 2px;
  }

  .side-header button {
    display: grid;
    width: 28px;
    height: 28px;
    padding: 0;
    place-items: center;
    color: var(--text-muted, #75847b);
    background: transparent;
    border: 1px solid var(--border-subtle, #1b2821);
    border-radius: 7px;
    cursor: pointer;
  }

  .side-header button:hover {
    color: var(--accent, #43d17f);
    background: var(--accent-soft, rgba(67, 209, 127, 0.1));
    border-color: var(--accent-border, rgba(67, 209, 127, 0.26));
  }

  .client-list {
    display: grid;
    gap: 2px;
  }

  .client-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 28px;
    align-items: center;
    width: 100%;
    min-height: 47px;
    padding: 2px 4px 2px 3px;
    color: inherit;
    background: transparent;
    border-radius: 8px;
  }

  .client-row:hover,
  .client-row.active {
    background: var(--surface-hover, rgba(76, 154, 109, 0.06));
  }

  .client-select {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) auto;
    align-items: center;
    gap: 9px;
    min-width: 0;
    min-height: 43px;
    padding: 3px 4px;
    color: inherit;
    font: inherit;
    background: transparent;
    border: 0;
    cursor: pointer;
    text-align: left;
  }

  .client-edit {
    display: grid;
    width: 27px;
    height: 27px;
    padding: 0;
    place-items: center;
    color: var(--text-faint, #536158);
    background: transparent;
    border: 0;
    border-radius: 6px;
    cursor: pointer;
  }

  .client-edit:hover,
  .client-edit:focus-visible {
    color: var(--accent, #43d17f);
    background: var(--accent-soft, rgba(67, 209, 127, 0.1));
  }

  .client-avatar {
    display: grid;
    width: 30px;
    height: 30px;
    place-items: center;
    color: var(--accent, #43d17f);
    font-size: 8px;
    font-weight: 750;
    background: var(--accent-soft, rgba(67, 209, 127, 0.1));
    border: 1px solid var(--accent-border, rgba(67, 209, 127, 0.26));
    border-radius: 8px;
  }

  .client-copy {
    display: flex;
    min-width: 0;
    flex-direction: column;
  }

  .client-copy strong {
    overflow: hidden;
    color: var(--text-secondary, #aebdb4);
    font-size: 10px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .client-copy small {
    margin-top: 2px;
    overflow: hidden;
    color: var(--text-faint, #536158);
    font-size: 8.5px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .client-select b {
    color: var(--text-faint, #536158);
    font-size: 8px;
    letter-spacing: 0.05em;
  }

  .activity-list {
    display: grid;
  }

  .activity-list article {
    display: grid;
    grid-template-columns: 9px minmax(0, 1fr) 26px;
    gap: 8px;
    padding: 9px 5px;
    border-bottom: 1px solid var(--border-subtle, #1b2821);
  }

  .activity-list article:last-child {
    border-bottom: 0;
  }

  .activity-dot {
    width: 6px;
    height: 6px;
    margin-top: 5px;
    background: var(--accent, #43d17f);
    border-radius: 50%;
    box-shadow: 0 0 0 3px var(--accent-soft, rgba(67, 209, 127, 0.1));
  }

  .activity-list article > div {
    display: flex;
    min-width: 0;
    flex-direction: column;
  }

  .activity-list strong {
    overflow: hidden;
    color: var(--text-secondary, #aebdb4);
    font-size: 9.5px;
    font-weight: 580;
    line-height: 1.4;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .activity-list article > div > span {
    margin-top: 2px;
    overflow: hidden;
    color: var(--text-faint, #536158);
    font-size: 8.5px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .entry-links {
    display: flex;
    gap: 5px;
    margin-top: 5px;
    overflow: hidden;
  }

  .entry-links a {
    display: inline-flex;
    min-width: 0;
    align-items: center;
    gap: 3px;
    padding: 2px 5px;
    overflow: hidden;
    color: var(--accent, #43d17f);
    font-size: 8px;
    background: var(--accent-soft, rgba(67, 209, 127, 0.1));
    border-radius: 4px;
    text-decoration: none;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .entry-edit {
    display: grid;
    width: 25px;
    height: 25px;
    padding: 0;
    place-items: center;
    color: var(--text-faint, #536158);
    background: transparent;
    border: 0;
    border-radius: 6px;
    cursor: pointer;
  }

  .entry-edit:hover,
  .entry-edit:focus-visible {
    color: var(--accent, #43d17f);
    background: var(--accent-soft, rgba(67, 209, 127, 0.1));
  }

  .side-empty {
    margin: 2px 4px 8px;
    color: var(--text-faint, #536158);
    font-size: 9.5px;
    line-height: 1.55;
  }

  div.side-empty {
    padding: 8px 4px;
  }

  .side-empty p {
    margin: 0 0 5px;
  }

  .empty-state {
    display: flex;
    min-height: 300px;
    align-items: center;
    justify-content: center;
    flex-direction: column;
    padding: 34px 20px;
    text-align: center;
  }

  .empty-state.compact {
    min-height: 240px;
  }

  .empty-icon {
    display: grid;
    width: 42px;
    height: 42px;
    place-items: center;
    color: var(--accent, #43d17f);
    background: var(--accent-soft, rgba(67, 209, 127, 0.1));
    border: 1px solid var(--accent-border, rgba(67, 209, 127, 0.26));
    border-radius: 11px;
  }

  .empty-state h3 {
    margin: 13px 0 4px;
    font-size: 13px;
    font-weight: 620;
  }

  .empty-state p {
    max-width: 330px;
    margin: 0 0 15px;
    color: var(--text-muted, #75847b);
    font-size: 10.5px;
    line-height: 1.55;
  }

  .project-skeletons,
  .side-skeleton {
    display: grid;
    gap: 9px;
  }

  .project-skeletons i,
  .side-skeleton i {
    display: block;
    background: linear-gradient(
      90deg,
      var(--surface-2, #101713),
      var(--surface-raised, #141d18),
      var(--surface-2, #101713)
    );
    background-size: 200% 100%;
    border-radius: 9px;
    animation: shimmer 1.4s linear infinite;
  }

  .project-skeletons i {
    height: 150px;
  }

  .side-skeleton i {
    height: 43px;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }

  @keyframes shimmer {
    to {
      background-position: -200% 0;
    }
  }

  @media (max-width: 1040px) {
    .work-layout {
      grid-template-columns: minmax(0, 1fr) 255px;
    }
  }

  @media (max-width: 860px) {
    .work-layout {
      grid-template-columns: 1fr;
    }

    .side-stack {
      grid-template-columns: 1fr 1fr;
    }
  }

  @media (max-width: 700px) {
    .summary-grid {
      grid-template-columns: 1fr;
    }

    .summary-card {
      min-height: 88px;
    }

    .work-toolbar {
      align-items: stretch;
      flex-direction: column;
    }

    .search-field {
      width: 100%;
    }

    .toolbar-actions {
      display: grid;
      grid-template-columns: 1fr 1fr 1.25fr;
    }

    .milestone-grid,
    .side-stack {
      grid-template-columns: 1fr;
    }

    .invoice-main {
      grid-template-columns: 28px minmax(0, 1fr) auto;
    }

    .invoice-total {
      grid-column: 2 / -1;
      min-width: 0;
      align-items: center;
      flex-direction: row;
      gap: 7px;
    }
  }

  @media (max-width: 460px) {
    .toolbar-actions {
      grid-template-columns: 1fr;
    }

    .project-heading {
      gap: 10px;
    }

    .project-value strong {
      font-size: 10.5px;
    }

    .mini-work-entry {
      grid-template-columns: 18px minmax(0, 1fr) auto;
    }

    .mini-work-entry time {
      display: none;
    }

    .invoice-payments {
      margin-left: 0;
    }

    .invoice-payments > div {
      grid-template-columns: 15px minmax(0, 1fr) auto;
    }

    .invoice-payments time {
      display: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .project-skeletons i,
    .side-skeleton i {
      animation: none;
    }
  }
</style>
