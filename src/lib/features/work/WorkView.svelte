<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
  import { slide } from "svelte/transition";
  import { motionDuration } from "../../motion";
  import Card from "../../components/Card.svelte";
  import Icon from "../../components/Icon.svelte";
  import MeterBar from "../../components/MeterBar.svelte";
  import SectionHeader from "../../components/SectionHeader.svelte";
  import StatCard from "../../components/StatCard.svelte";
  import StatRow from "../../components/StatRow.svelte";
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
  $: expectedTotals = groupByCurrency(activeProjects, projectTotalMinor);
  $: remainingTotals = groupByCurrency(activeProjects, remainingToInvoiceMinor);
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

  // A project is worth the sum of its milestones. `quotedTotalMinor` is only a
  // legacy reference figure, used as a fallback for a project that has no
  // milestone plan yet. Single definition so the project card and the
  // workspace headline can never disagree.
  function projectTotalMinor(project: Project): number {
    return project.milestones.length > 0
      ? planTotalMinor(project.milestones)
      : project.quotedTotalMinor;
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

  // Money is per-currency only: each currency's total is kept separate and
  // never summed with another currency's total.
  function groupByCurrency(
    values: Project[],
    amountMinor: (project: Project) => number,
  ): Array<{ currency: string; totalMinor: number }> {
    const totals = new Map<string, number>();
    for (const project of values) {
      const amount = amountMinor(project);
      if (amount <= 0) continue;
      totals.set(
        project.currency,
        (totals.get(project.currency) ?? 0) + amount,
      );
    }
    return [...totals.entries()]
      .map(([currency, totalMinor]) => ({ currency, totalMinor }))
      .sort((a, b) => a.currency.localeCompare(b.currency));
  }

  function extraCurrencies(
    totals: Array<{ currency: string; totalMinor: number }>,
  ): string {
    return totals
      .slice(1)
      .map((total) => formatMoney(total.totalMinor, total.currency))
      .join(" · ");
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
  <StatRow>
    <StatCard
      icon="briefcase"
      label="Active projects"
      value={String(activeProjects.length)}
      detail="In flight"
      tone="neutral"
    />
    <StatCard
      icon="money"
      label="Expected project value"
      value={expectedTotals.length ? formatMoney(expectedTotals[0].totalMinor, expectedTotals[0].currency) : "None"}
      detail={expectedTotals.length > 1 ? extraCurrencies(expectedTotals) : "Sum of milestone plans"}
      tone={expectedTotals.length ? "positive" : "neutral"}
    />
    <StatCard
      icon="invoice"
      label="Remaining to invoice"
      value={remainingTotals.length ? formatMoney(remainingTotals[0].totalMinor, remainingTotals[0].currency) : "None"}
      detail={remainingTotals.length > 1
        ? extraCurrencies(remainingTotals)
        : "Across active projects"}
      tone={remainingTotals.length ? "warning" : "neutral"}
    />
  </StatRow>

  {#if errorMessage}
    <div class="work-error" role="alert">
      <span>{errorMessage}</span>
      <button type="button" aria-label="Dismiss error" on:click={() => (errorMessage = "")}>
        <Icon name="x" size={14} />
      </button>
    </div>
  {/if}

  <div class="page-actions">
    <button class="primary-button" type="button" on:click={() => openClientDialog()}>
      <Icon name="plus" size={15} /> Client
    </button>
    <button
      class="primary-button"
      type="button"
      disabled={!clients.length}
      title={!clients.length ? "Create a client first" : "Create a fixed-price project"}
      on:click={() => openProjectDialog()}
    >
      <Icon name="briefcase" size={15} /> Project
    </button>
    <button class="primary-button" type="button" on:click={() => openWorkDialog()}>
      <Icon name="check" size={15} /> Record work
    </button>
  </div>

  <div class="work-grid">
    <div class="project-column">
      <div class="list-header">
        <SectionHeader
          title={`${filteredProjects.length} ${filteredProjects.length === 1 ? "project" : "projects"}`}
          subtext="Fixed price · no timers"
        >
          <svelte:fragment slot="actions">
            {#if clients.length}
              <div class="segmented" aria-label="Filter projects by client">
                <button
                  class:active={selectedClientId === "all"}
                  type="button"
                  aria-pressed={selectedClientId === "all"}
                  on:click={() => (selectedClientId = "all")}
                >All · {projects.length}</button>
                {#each clients as client (client.id)}
                  <button
                    class:active={selectedClientId === client.id}
                    type="button"
                    aria-pressed={selectedClientId === client.id}
                    on:click={() => (selectedClientId = client.id)}
                  >{client.name} · {projectCount(client.id)}</button>
                {/each}
              </div>
            {/if}
            <label class="inline-search">
              <Icon name="search" size={14} />
              <input
                bind:value={searchQuery}
                type="search"
                placeholder="Search projects or clients"
                aria-label="Search projects or clients"
              />
            </label>
          </svelte:fragment>
        </SectionHeader>
      </div>

      {#if loading}
        <div class="skeleton-list project-skeletons" aria-label="Loading projects">
          <span></span><span></span><span></span>
        </div>
      {:else if filteredProjects.length}
        <div class="project-list">
          {#each filteredProjects as project (project.id)}
            {@const client = clientsById.get(project.clientId)}
            {@const projectWork = workForProject(project.id)}
            {@const projectInvoices = invoicesForProject(project.id)}
            <Card>
              <SectionHeader slot="header" title={project.name} subtext={client?.name ?? "Unassigned client"}>
                <svelte:fragment slot="actions">
                  <span class="soft-badge">{project.status}</span>
                </svelte:fragment>
              </SectionHeader>

              <div class="project-summary">
                <div>
                  <span>Contract value</span>
                  <strong>{formatMoney(projectTotalMinor(project), project.currency)}</strong>
                </div>
                <div>
                  <span>Remaining to invoice</span>
                  <strong>{formatMoney(remainingToInvoiceMinor(project), project.currency)}</strong>
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

              <span class="group-label">Invoice milestones</span>
              <div class="milestone-list" aria-label={`${project.name} invoice milestones`}>
                {#each project.milestones as milestone, index (milestone.id ?? `${project.id}-${milestone.kind}-${index}`)}
                  <MeterBar
                    label={`${milestone.label} · ${formatMoney(milestone.amountMinor, project.currency)}`}
                    value={milestoneEffectiveStatus(milestone) === "not-invoiced" ? 0 : 1}
                    max={1}
                    detail={milestoneStatusLabel(milestoneEffectiveStatus(milestone))}
                    valueText={milestoneStatusLabel(milestoneEffectiveStatus(milestone))}
                    tone={milestoneEffectiveStatus(milestone) === "paid"
                      ? "positive"
                      : milestoneEffectiveStatus(milestone) === "invoiced"
                        ? "warning"
                        : "neutral"}
                  />
                {/each}
              </div>

              <section class="project-billing" aria-label={`${project.name} invoices and payments`}>
                <header>
                  <span class="group-label">Invoices and payments</span>
                  <b>{projectInvoices.length} {projectInvoices.length === 1 ? "invoice" : "invoices"}</b>
                </header>
                {#if projectInvoices.length}
                  <div class="invoice-list" transition:slide={{ duration: motionDuration(180) }}>
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
                          <div
                            class="invoice-payments"
                            aria-label={`${invoice.number} payments`}
                            transition:slide={{ duration: motionDuration(180) }}
                          >
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
                  <p transition:slide={{ duration: motionDuration(180) }}>No invoices are attached to this project yet.</p>
                {/if}
              </section>

              {#if projectWork.length}
                <div class="project-work" transition:slide={{ duration: motionDuration(180) }}>
                  <span class="group-label">Recent completed work</span>
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
                  <button class="text-button" type="button" on:click={() => openProjectDialog(project)}>
                    <Icon name="more" size={13} /> Edit
                  </button>
                  <button class="text-button" type="button" on:click={() => openWorkDialog(project.id)}>
                    <Icon name="plus" size={13} /> Add work
                  </button>
                </div>
              </footer>
            </Card>
          {/each}
        </div>
      {:else if !clients.length}
        <div class="empty-state large">
          <span class="empty-icon"><Icon name="briefcase" size={21} /></span>
          <strong>Start with a client</strong>
          <p>Clients organize your fixed-price projects, invoices, and completed work.</p>
          <button class="primary-button" type="button" on:click={() => openClientDialog()}>
            <Icon name="plus" size={14} /> Add your first client
          </button>
        </div>
      {:else if !projects.length}
        <div class="empty-state large">
          <span class="empty-icon"><Icon name="briefcase" size={21} /></span>
          <strong>Create the first project container</strong>
          <p>It starts with an editable 50% kickoff and 50% completion plan.</p>
          <button class="primary-button" type="button" on:click={() => openProjectDialog()}>
            <Icon name="plus" size={14} /> Create project
          </button>
        </div>
      {:else}
        <div class="empty-state compact">
          <span class="empty-icon"><Icon name="search" size={19} /></span>
          <div>
            <strong>No matching projects</strong>
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
        </div>
      {/if}
    </div>

    <aside class="side-stack">
      <Card padded={false}>
        <SectionHeader slot="header" title="Clients" subtext="Relationships">
          <svelte:fragment slot="actions">
            <button class="text-button" type="button" aria-label="Add client" on:click={() => openClientDialog()}>
              <Icon name="plus" size={14} />
            </button>
          </svelte:fragment>
        </SectionHeader>
        {#if loading}
          <div class="skeleton-list"><span></span><span></span><span></span></div>
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
      </Card>

      <Card padded={false}>
        <SectionHeader slot="header" title="Recent records" subtext="Completed work">
          <svelte:fragment slot="actions">
            <button
              class="text-button"
              type="button"
              aria-label="Record completed work"
              on:click={() => openWorkDialog()}
            >
              <Icon name="plus" size={14} />
            </button>
          </svelte:fragment>
        </SectionHeader>
        {#if loading}
          <div class="skeleton-list"><span></span><span></span><span></span></div>
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
      </Card>
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
    max-width: 1120px;
    margin: 0 auto;
  }

  .work-error {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    min-height: 40px;
    padding: var(--space-2) var(--space-3);
    color: var(--danger);
    font-size: var(--text-12);
    background: var(--danger-fill);
    border-radius: var(--radius-control);
    margin: 0 0 var(--space-4);
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
    border-radius: var(--radius-control);
  }

  .work-grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 292px;
    align-items: start;
    gap: var(--space-4);
  }

  @media (max-width: 1020px) {
    .work-grid {
      grid-template-columns: 1fr;
    }
  }

  .list-header {
    margin-bottom: var(--space-3);
  }

  .project-list {
    display: grid;
    gap: var(--space-4);
  }

  .project-summary {
    display: flex;
    gap: var(--space-5);
    margin-bottom: var(--space-3);
  }

  .project-summary > div {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .project-summary span {
    color: var(--text-tertiary);
    font-size: var(--text-11);
  }

  .project-summary strong {
    font-size: var(--text-15);
    font-weight: var(--weight-semibold);
    font-variant-numeric: tabular-nums;
  }

  .project-description {
    display: -webkit-box;
    margin: 0 0 var(--space-3);
    overflow: hidden;
    color: var(--text-secondary);
    font-size: var(--text-12);
    line-height: 1.55;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }

  .project-links {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin: 0 0 var(--space-3);
    overflow: hidden;
  }

  .project-links a {
    display: inline-flex;
    min-width: 0;
    align-items: center;
    gap: 4px;
    padding: 3px 6px;
    overflow: hidden;
    color: var(--accent);
    font-size: var(--text-11);
    background: var(--accent-fill);
    border-radius: var(--radius-control);
    text-decoration: none;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .date-range {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 0 var(--space-3);
    color: var(--text-tertiary);
    font-size: var(--text-11);
  }

  .group-label {
    display: block;
    margin-bottom: var(--space-2);
    color: var(--text-tertiary);
    font-size: var(--text-11);
    font-weight: var(--weight-medium);
  }

  .milestone-list {
    display: grid;
    gap: var(--space-3);
    margin-bottom: var(--space-4);
  }

  .project-billing {
    margin-top: var(--space-3);
    padding-top: var(--space-3);
    border-top: 1px solid var(--separator);
  }

  .project-billing > header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    margin-bottom: var(--space-2);
  }

  .project-billing > header .group-label {
    margin-bottom: 0;
  }

  .project-billing > header > b {
    color: var(--text-secondary);
    font-size: var(--text-11);
    font-weight: var(--weight-medium);
  }

  .project-billing > p {
    margin: 0;
    padding: var(--space-2) var(--space-3);
    color: var(--text-secondary);
    font-size: var(--text-11);
    background: var(--surface-raised);
    border: 1px dashed var(--separator-strong);
    border-radius: var(--radius-control);
  }

  .invoice-list {
    display: grid;
    gap: var(--space-2);
  }

  .invoice-record {
    min-width: 0;
    padding: var(--space-3);
    background: var(--surface-raised);
    border-radius: var(--radius-control);
  }

  .invoice-main {
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }

  .invoice-icon {
    display: grid;
    width: 27px;
    height: 27px;
    place-items: center;
    color: var(--accent);
    background: var(--accent-fill);
    border-radius: var(--radius-control);
  }

  .invoice-copy {
    display: flex;
    min-width: 0;
    flex-direction: column;
  }

  .invoice-copy strong {
    overflow: hidden;
    color: var(--text-primary);
    font-size: var(--text-12);
    font-weight: var(--weight-medium);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .invoice-copy > span {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 2px;
    overflow: hidden;
    color: var(--text-tertiary);
    font-size: var(--text-11);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .invoice-copy i {
    color: var(--text-quaternary);
    font-style: normal;
  }

  .invoice-status {
    padding: 3px 6px;
    color: var(--text-secondary);
    font-size: var(--text-11);
    font-weight: var(--weight-medium);
    background: var(--surface-active);
    border-radius: var(--radius-pill);
    white-space: nowrap;
  }

  .invoice-status.paid {
    color: var(--accent);
    background: var(--accent-fill);
  }

  .invoice-status.partially-paid {
    color: var(--amber);
    background: var(--amber-fill);
  }

  /* Danger means overdue, on Work exactly as on Money. */
  .invoice-status.overdue {
    color: var(--danger);
    background: var(--danger-fill);
  }

  .invoice-total {
    display: flex;
    min-width: 86px;
    flex-direction: column;
    align-items: flex-end;
  }

  .invoice-total strong {
    color: var(--text-primary);
    font-size: var(--text-12);
    font-weight: var(--weight-semibold);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .invoice-total span {
    margin-top: 2px;
    color: var(--text-tertiary);
    font-size: var(--text-11);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .invoice-payments {
    display: grid;
    gap: 5px;
    margin: var(--space-2) 0 0 35px;
    padding-top: var(--space-2);
    border-top: 1px solid var(--separator);
  }

  .invoice-payments > div {
    display: grid;
    grid-template-columns: 15px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 6px;
    color: var(--text-secondary);
    font-size: var(--text-11);
  }

  .invoice-payments time {
    color: var(--text-tertiary);
  }

  .invoice-payments strong {
    color: var(--accent);
    font-size: var(--text-11);
    font-weight: var(--weight-semibold);
    font-variant-numeric: tabular-nums;
  }

  .payment-check {
    display: grid;
    width: 14px;
    height: 14px;
    place-items: center;
    color: var(--accent);
    background: var(--accent-fill);
    border-radius: 50%;
  }

  .project-work {
    display: grid;
    gap: var(--space-2);
    margin-top: var(--space-3);
    padding-top: var(--space-3);
    border-top: 1px solid var(--separator);
  }

  .mini-work-entry {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: var(--space-2);
  }

  .work-check {
    display: grid;
    width: 17px;
    height: 17px;
    place-items: center;
    color: var(--accent);
    background: var(--accent-fill);
    border-radius: 50%;
  }

  .mini-work-entry strong {
    overflow: hidden;
    color: var(--text-secondary);
    font-size: var(--text-12);
    font-weight: var(--weight-medium);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mini-work-entry time {
    color: var(--text-tertiary);
    font-size: var(--text-11);
  }

  .mini-work-entry button {
    padding: 2px 5px;
    color: var(--text-secondary);
    font: inherit;
    font-size: var(--text-11);
    background: transparent;
    border: 0;
    border-radius: var(--radius-control);
    cursor: pointer;
  }

  .mini-work-entry button:hover {
    color: var(--accent);
    background: var(--accent-fill);
  }

  .project-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    margin-top: var(--space-3);
    padding-top: var(--space-3);
    border-top: 1px solid var(--separator);
  }

  .project-footer > span {
    color: var(--text-tertiary);
    font-size: var(--text-11);
  }

  .project-footer > div {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .client-list {
    display: grid;
    gap: 2px;
    padding: 0 var(--space-2) var(--space-2);
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
    border-radius: var(--radius-control);
  }

  .client-row:hover,
  .client-row.active {
    background: var(--surface-hover);
  }

  .client-select {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--space-2);
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
    color: var(--text-tertiary);
    background: transparent;
    border: 0;
    border-radius: var(--radius-control);
    cursor: pointer;
  }

  .client-edit:hover,
  .client-edit:focus-visible {
    color: var(--accent);
    background: var(--accent-fill);
  }

  .client-avatar {
    display: grid;
    width: 30px;
    height: 30px;
    place-items: center;
    color: var(--accent);
    font-size: var(--text-11);
    font-weight: var(--weight-semibold);
    background: var(--accent-fill);
    border: 1px solid var(--accent-line);
    border-radius: var(--radius-control);
  }

  .client-copy {
    display: flex;
    min-width: 0;
    flex-direction: column;
  }

  .client-copy strong {
    overflow: hidden;
    color: var(--text-secondary);
    font-size: var(--text-12);
    font-weight: var(--weight-medium);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .client-copy small {
    margin-top: 2px;
    overflow: hidden;
    color: var(--text-tertiary);
    font-size: var(--text-11);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .client-select b {
    color: var(--text-tertiary);
    font-size: var(--text-11);
  }

  .activity-list {
    display: grid;
  }

  .activity-list article {
    display: grid;
    grid-template-columns: 9px minmax(0, 1fr) 26px;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-2);
    margin: 0 var(--space-2);
    border-bottom: 1px solid var(--separator);
  }

  .activity-list article:last-child {
    border-bottom: 0;
  }

  .activity-dot {
    width: 6px;
    height: 6px;
    margin-top: 5px;
    background: var(--accent);
    border-radius: 50%;
    box-shadow: 0 0 0 3px var(--accent-fill);
  }

  .activity-list article > div {
    display: flex;
    min-width: 0;
    flex-direction: column;
  }

  .activity-list strong {
    overflow: hidden;
    color: var(--text-secondary);
    font-size: var(--text-12);
    font-weight: var(--weight-medium);
    line-height: 1.4;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .activity-list article > div > span {
    margin-top: 2px;
    overflow: hidden;
    color: var(--text-tertiary);
    font-size: var(--text-11);
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
    color: var(--accent);
    font-size: var(--text-11);
    background: var(--accent-fill);
    border-radius: var(--radius-control);
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
    color: var(--text-tertiary);
    background: transparent;
    border: 0;
    border-radius: var(--radius-control);
    cursor: pointer;
  }

  .entry-edit:hover,
  .entry-edit:focus-visible {
    color: var(--accent);
    background: var(--accent-fill);
  }

  .side-empty {
    margin: 2px var(--space-2) var(--space-2);
    color: var(--text-secondary);
    font-size: var(--text-12);
    line-height: 1.55;
  }

  div.side-empty {
    padding: var(--space-2) var(--space-2);
  }

  .side-empty p {
    margin: 0 0 5px;
  }

  @media (max-width: 700px) {
    .invoice-main {
      grid-template-columns: 28px minmax(0, 1fr) auto;
    }

    .invoice-total {
      grid-column: 2 / -1;
      min-width: 0;
      align-items: center;
      flex-direction: row;
      gap: var(--space-2);
    }
  }

  @media (max-width: 460px) {
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
</style>
