<script lang="ts">
  import { onMount, tick } from "svelte";
  import Icon from "./lib/components/Icon.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import TaskRow from "./lib/components/TaskRow.svelte";
  import MoneyView from "./lib/features/money/MoneyView.svelte";
  import { handleInvoiceExport as exportInvoice } from "./lib/features/money/invoiceExport";
  import {
    createMoneyService,
    invoiceTotals,
  } from "./lib/features/money/moneyService";
  import type {
    Invoice as MoneyInvoice,
    PersonalLoan as MoneyLoan,
  } from "./lib/features/money/types";
  import SettingsView from "./lib/features/settings/SettingsView.svelte";
  import { createSettingsService } from "./lib/features/settings/settingsService";
  import type { AppSettings } from "./lib/features/settings/types";
  import TaskDialog from "./lib/features/tasks/TaskDialog.svelte";
  import {
    needsSubtaskCompletionConfirmation,
    shouldOfferCompletedWork,
    unfinishedSubtaskCount,
  } from "./lib/features/tasks/completion";
  import type {
    TaskDialogSaveDetail,
    TaskDialogSubtaskDetail,
    TaskProjectOption,
  } from "./lib/features/tasks/types";
  import WorkView from "./lib/features/work/WorkView.svelte";
  import { createWorkService } from "./lib/features/work/workService";
  import type {
    WorkEntry as RecordedWork,
    WorkEntryPrefill,
  } from "./lib/features/work/types";
  import { createTaskService, dateUtils } from "./lib/services/backend";
  import {
    emitTasksChanged,
    onTasksChanged,
    onOpenTask,
  } from "./lib/services/taskSync.ts";
  import type { AppSection, Task } from "./lib/types";
  import type { InvoiceExportDetail } from "./lib/features/money/types";

  type TaskFilter =
    | "all"
    | "inbox"
    | "today"
    | "upcoming"
    | "recurring"
    | "categories"
    | "completed";

  const taskService = createTaskService();
  const workService = createWorkService();
  const moneyService = createMoneyService();
  const settingsService = createSettingsService();
  let active: AppSection = "today";
  let taskFilter: TaskFilter = "all";
  let weekStartsOn: 0 | 1 = 1;
  let dateFormat: AppSettings["dateFormat"] = "MMMM d, yyyy";
  let todayIso = dateUtils.localIsoDay();
  let sevenDaysIso = dateUtils.shiftDay(7);
  let weekStartIso = currentWeekBoundary("start");
  let weekEndIso = currentWeekBoundary("end");
  let tasks: Task[] = [];
  let quickTitle = "";
  let searchQuery = "";
  let loading = true;
  let saving = false;
  let errorMessage = "";
  let backupWarning = "";
  let setupPromptOpen = false;
  let busyTaskIds = new Set<string>();
  let quickInput: HTMLInputElement;
  let searchInput: HTMLInputElement;
  let taskDialogOpen = false;
  let editingTask: Task | null = null;
  let taskDialogSaving = false;
  let taskProjects: TaskProjectOption[] = [];
  let reviewWorkEntries: RecordedWork[] = [];
  let reviewInvoices: MoneyInvoice[] = [];
  let reviewLoans: MoneyLoan[] = [];
  let pendingWorkPrefill: WorkEntryPrefill | null = null;

  $: openTasks = tasks.filter((task) => task.status === "open");
  $: completedTasks = tasks.filter((task) => task.status === "completed");
  $: completedToday = completedTasks.filter(
    (task) => task.completedAt && localDayFromTimestamp(task.completedAt) === todayIso,
  );
  $: completedThisWeek = completedTasks.filter((task) => {
    if (!task.completedAt) return false;
    const completedDay = localDayFromTimestamp(task.completedAt);
    return completedDay >= weekStartIso && completedDay <= weekEndIso;
  });
  $: overdueTasks = openTasks.filter(
    (task) => !!task.dueDate && task.dueDate < todayIso,
  );
  $: todayTasks = openTasks.filter(
    (task) =>
      task.plannedDate === todayIso ||
      task.dueDate === todayIso,
  );
  $: upcomingTasks = openTasks.filter(
    (task) =>
      (!!task.plannedDate && task.plannedDate > todayIso) ||
      (!!task.dueDate && task.dueDate > todayIso),
  );
  $: nextSevenTasks = upcomingTasks.filter((task) => {
    const scheduledDay = task.plannedDate ?? task.dueDate;
    return !!scheduledDay && scheduledDay <= sevenDaysIso;
  });
  $: weeklyOpenTasks = openTasks.filter((task) => {
    const scheduledDay = task.plannedDate ?? task.dueDate;
    return !!scheduledDay && scheduledDay >= weekStartIso && scheduledDay <= weekEndIso;
  });
  $: todayProgressTotal = todayTasks.length + completedToday.length;
  $: weeklyProgressTotal = weeklyOpenTasks.length + completedThisWeek.length;
  $: completedWorkThisWeek = reviewWorkEntries.filter(
    (entry) => entry.workDate >= weekStartIso && entry.workDate <= weekEndIso,
  );
  $: paymentsThisWeek = reviewInvoices.flatMap((invoice) =>
    invoice.payments
      .filter(
        (payment) =>
          payment.receivedDate >= weekStartIso &&
          payment.receivedDate <= weekEndIso,
      )
      .map((payment) => ({
        currency: invoice.currency,
        amountMinor: payment.amountMinor,
      })),
  );
  $: collectedThisWeek = collectedSummary(paymentsThisWeek);
  $: loanInstallmentsPaidThisWeek = reviewLoans.flatMap((loan) =>
    loan.installments.filter(
      (installment) =>
        !!installment.paidDate &&
        installment.paidDate >= weekStartIso &&
        installment.paidDate <= weekEndIso,
    ),
  );
  $: actionableInvoiceDues = reviewInvoices.filter((invoice) => {
    const status = invoice.status.replace("-", "_");
    return (
      !["draft", "paid", "void"].includes(status) &&
      invoice.dueDate <= sevenDaysIso &&
      invoiceTotals(invoice).balanceDueMinor > 0
    );
  });
  $: actionableLoanDues = reviewLoans.flatMap((loan) =>
    loan.installments
      .filter(
        (installment) =>
          !installment.paid && installment.dueDate <= sevenDaysIso,
      )
      .map((installment) => ({ loan, installment })),
  );
  $: filteredTasks = tasks
    .filter((task) => {
      if (taskFilter === "today") return todayTasks.some((item) => item.id === task.id);
      if (taskFilter === "upcoming") return upcomingTasks.some((item) => item.id === task.id);
      if (taskFilter === "inbox") {
        return (
          task.status === "open" &&
          !task.parentTaskId &&
          !task.plannedDate &&
          !task.dueDate
        );
      }
      if (taskFilter === "recurring") {
        return task.status === "open" && task.recurrence !== "none";
      }
      if (taskFilter === "categories") {
        return task.status === "open" && !!task.category;
      }
      if (taskFilter === "completed") return task.status === "completed";
      return task.status === "open";
    })
    .filter((task) =>
      task.title.toLocaleLowerCase().includes(searchQuery.trim().toLocaleLowerCase()),
    );

  const sectionCopy: Record<AppSection, { eyebrow: string; title: string }> = {
    today: { eyebrow: "Daily command center", title: "Today" },
    tasks: { eyebrow: "Plan and follow through", title: "Tasks" },
    work: { eyebrow: "Clients and deliverables", title: "Work" },
    money: { eyebrow: "Invoices and obligations", title: "Money" },
    review: { eyebrow: "Close the loop", title: "Weekly review" },
    settings: { eyebrow: "Your workspace", title: "Settings" },
  };

  function longDate(date = new Date()): string {
    return new Intl.DateTimeFormat("en-US", {
      weekday: "long",
      month: "long",
      day: "numeric",
      year: "numeric",
    }).format(date);
  }

  function displayDate(value: string): string {
    const date = new Date(`${value.slice(0, 10)}T12:00:00`);
    if (Number.isNaN(date.getTime())) return value;
    if (dateFormat === "yyyy-MM-dd") return value.slice(0, 10);
    if (dateFormat === "MM/dd/yyyy") {
      return new Intl.DateTimeFormat("en-US", {
        month: "2-digit",
        day: "2-digit",
        year: "numeric",
      }).format(date);
    }
    return new Intl.DateTimeFormat("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
    }).format(date);
  }

  function greeting(): string {
    const hour = new Date().getHours();
    if (hour < 12) return "Good morning";
    if (hour < 18) return "Good afternoon";
    return "Good evening";
  }

  function localDayFromTimestamp(value: string): string {
    const date = new Date(value);
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    return `${year}-${month}-${day}`;
  }

  function currentWeekBoundary(edge: "start" | "end"): string {
    const date = new Date();
    date.setHours(12, 0, 0, 0);
    const daysSinceWeekStart = (date.getDay() - weekStartsOn + 7) % 7;
    date.setDate(
      date.getDate() - daysSinceWeekStart + (edge === "end" ? 6 : 0),
    );
    return dateUtils.localIsoDay(date);
  }

  function refreshCalendarBoundaries(): void {
    todayIso = dateUtils.localIsoDay();
    sevenDaysIso = dateUtils.shiftDay(7);
    weekStartIso = currentWeekBoundary("start");
    weekEndIso = currentWeekBoundary("end");
  }

  function collectedSummary(
    values: Array<{ currency: string; amountMinor: number }>,
  ): string {
    const byCurrency = new Map<string, number>();
    for (const value of values) {
      byCurrency.set(
        value.currency,
        (byCurrency.get(value.currency) ?? 0) + value.amountMinor,
      );
    }
    if (!byCurrency.size) return "$0 USD";
    return [...byCurrency]
      .map(([currency, amountMinor]) => {
        try {
          return new Intl.NumberFormat("en-US", {
            style: "currency",
            currency,
            maximumFractionDigits: 2,
          }).format(amountMinor / 100);
        } catch {
          return `${currency} ${(amountMinor / 100).toFixed(2)}`;
        }
      })
      .join(" · ");
  }

  function formatMoney(minor: number, currency: string): string {
    try {
      return new Intl.NumberFormat("en-US", {
        style: "currency",
        currency,
        minimumFractionDigits: 2,
        maximumFractionDigits: 2,
      }).format(minor / 100);
    } catch {
      return `${currency} ${(minor / 100).toFixed(2)}`;
    }
  }

  async function loadTasks(): Promise<void> {
    loading = true;
    errorMessage = "";
    try {
      tasks = await taskService.listTasks();
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Tasks could not be loaded.";
    } finally {
      loading = false;
    }
  }

  async function loadReviewData(): Promise<void> {
    try {
      const [workEntries, invoices, loans, settings] = await Promise.all([
        workService.listWorkEntries(),
        moneyService.listInvoices(),
        moneyService.listPersonalLoans(),
        settingsService.getSettings(),
      ]);
      reviewWorkEntries = workEntries;
      reviewInvoices = invoices;
      reviewLoans = loans;
      weekStartsOn = settings.weekStartsOn;
      dateFormat = settings.dateFormat;
      updateBackupWarning(settings);
      refreshCalendarBoundaries();
    } catch {
      // Each feature owns its own actionable error state. The shell keeps the
      // last successful review snapshot instead of replacing it with zeros.
    }
  }

  function updateBackupWarning(settings: AppSettings): void {
    if (!settingsService.isDesktop || !settings.backupEnabled) {
      backupWarning = "";
    } else if (settings.backupSetupRequired || !settings.backupDirectory) {
      backupWarning =
        "Automatic backups are not set up yet. Choose a folder in Settings.";
    } else if (settings.lastBackupError) {
      backupWarning = `Automatic backup needs attention: ${settings.lastBackupError}`;
    } else {
      backupWarning = "";
    }
  }

  async function refreshBackupHealth(): Promise<void> {
    try {
      updateBackupWarning(await settingsService.getSettings());
    } catch {
      // Keep the last known warning; other feature loading remains independent.
    }
  }

  async function checkFirstRunSetup(): Promise<void> {
    if (!settingsService.isDesktop) return;
    try {
      const [settings, profile] = await Promise.all([
        settingsService.getSettings(),
        settingsService.getInvoiceProfile(),
      ]);
      const hasSellerIdentity = Boolean(
        profile.displayName.trim() || profile.businessName.trim(),
      );
      setupPromptOpen =
        settings.backupSetupRequired ||
        !settings.backupDirectory ||
        !hasSellerIdentity;
    } catch {
      // The regular settings and backup warnings remain available if this
      // lightweight first-run check cannot load.
    }
  }

  function openFirstRunSettings(): void {
    setupPromptOpen = false;
    active = "settings";
  }

  async function createQuickTask(): Promise<void> {
    const title = quickTitle.trim();
    if (!title || saving) return;
    saving = true;
    errorMessage = "";
    try {
      const created = await taskService.createTask({
        title,
        plannedDate: todayIso,
        priority: "medium",
      });
      tasks = [created, ...tasks];
      void emitTasksChanged();
      quickTitle = "";
      await tick();
      quickInput?.focus();
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Task could not be created.";
    } finally {
      saving = false;
    }
  }

  async function toggleTask(task: Task): Promise<void> {
    if (busyTaskIds.has(task.id)) return;
    const completing = task.status !== "completed";
    if (needsSubtaskCompletionConfirmation(task, completing)) {
      const unfinished = unfinishedSubtaskCount(task);
      const confirmed = window.confirm(
        `${unfinished} ${unfinished === 1 ? "subtask is" : "subtasks are"} still unfinished. Complete the parent task anyway?`,
      );
      if (!confirmed) return;
    }
    busyTaskIds = new Set(busyTaskIds).add(task.id);
    errorMessage = "";
    try {
      const updated = await taskService.setTaskCompleted(
        task.id,
        completing,
      );
      tasks = tasks.map((item) => (item.id === updated.id ? updated : item));
      void emitTasksChanged();
      if (
        shouldOfferCompletedWork(task, completing) &&
        window.confirm(
          `"${updated.title}" is complete. Record it as completed work?`,
        )
      ) {
        pendingWorkPrefill = {
          requestId: `${updated.id}-${updated.completedAt ?? Date.now()}`,
          projectId: updated.projectId as string,
          title: updated.title,
          details: updated.notes,
          workDate: todayIso,
        };
        active = "work";
      }
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : "Task could not be updated.";
    } finally {
      const nextBusyIds = new Set(busyTaskIds);
      nextBusyIds.delete(task.id);
      busyTaskIds = nextBusyIds;
    }
  }

  function selectSection(section: AppSection): void {
    active = section;
    if (section !== "settings") void refreshBackupHealth();
    if (section === "today") tick().then(() => quickInput?.focus());
    if (section === "today" || section === "review") {
      void loadReviewData();
    }
  }

  function openSearch(): void {
    active = "tasks";
    tick().then(() => searchInput?.focus());
  }

  async function loadTaskProjects(): Promise<void> {
    try {
      const [clients, projects] = await Promise.all([
        workService.listClients(),
        workService.listProjects(),
      ]);
      const clientNames = new Map(
        clients.map((client) => [client.id, client.name] as const),
      );
      taskProjects = projects
        .filter((project) => project.status !== "archived")
        .map((project) => ({
          id: project.id,
          name: project.name,
          clientName: clientNames.get(project.clientId) ?? "Personal",
        }));
    } catch {
      taskProjects = [];
    }
  }

  function openTaskEditor(task: Task | null = null): void {
    editingTask = task;
    taskDialogOpen = true;
    void loadTaskProjects();
  }

  async function openTaskWidget(): Promise<void> {
    const tauri = window as unknown as {
      __TAURI_INTERNALS__?: { invoke?: (cmd: string) => Promise<unknown> };
      __TAURI__?: { core?: { invoke?: (cmd: string) => Promise<unknown> } };
    };
    const invoke =
      tauri.__TAURI_INTERNALS__?.invoke ?? tauri.__TAURI__?.core?.invoke;
    if (invoke) await invoke("show_task_widget");
  }

  async function saveTask(event: CustomEvent<TaskDialogSaveDetail>): Promise<void> {
    if (taskDialogSaving) return;
    taskDialogSaving = true;
    errorMessage = "";
    const { task, draft } = event.detail;
    try {
      const input = {
        title: draft.title,
        notes: draft.notes,
        plannedDate: draft.plannedDate,
        dueDate: draft.dueDate,
        reminderAt: draft.reminderAt,
        priority: draft.priority,
        category: draft.category,
        projectId: draft.projectId,
        recurrence: draft.recurrence,
        recurrenceRule: draft.recurrenceRule,
      };
      const saved = task
        ? await taskService.updateTask(task.id, input)
        : await taskService.createTask(input);
      for (const subtaskTitle of draft.newSubtasks) {
        await taskService.createTask({
          title: subtaskTitle,
          priority: "none",
          projectId: draft.projectId,
          parentTaskId: saved.id,
        });
      }
      await loadTasks();
      void emitTasksChanged();
      taskDialogOpen = false;
      editingTask = null;
    } catch (error) {
      errorMessage =
        error instanceof Error ? error.message : "Task could not be saved.";
    } finally {
      taskDialogSaving = false;
    }
  }

  async function toggleSubtask(
    event: CustomEvent<TaskDialogSubtaskDetail>,
  ): Promise<void> {
    if (taskDialogSaving) return;
    taskDialogSaving = true;
    errorMessage = "";
    try {
      await taskService.setTaskCompleted(
        event.detail.subtask.id,
        event.detail.completed,
      );
      await loadTasks();
      void emitTasksChanged();
      editingTask =
        tasks.find((task) => task.id === editingTask?.id) ?? editingTask;
    } catch (error) {
      errorMessage =
        error instanceof Error ? error.message : "Subtask could not be updated.";
    } finally {
      taskDialogSaving = false;
    }
  }

  async function handleInvoiceExport(
    event: CustomEvent<InvoiceExportDetail>,
  ): Promise<void> {
    errorMessage = "";
    try {
      await exportInvoice(event);
    } catch (error) {
      errorMessage =
        error instanceof Error
          ? error.message
          : "The invoice could not be exported.";
    }
  }

  function handleShortcut(event: KeyboardEvent): void {
    if (!event.ctrlKey && !event.metaKey) return;
    if (event.key.toLocaleLowerCase() === "n") {
      event.preventDefault();
      openTaskEditor();
      return;
    }
    if (event.key.toLocaleLowerCase() === "k") {
      event.preventDefault();
      openSearch();
      return;
    }
    const sectionByKey: Partial<Record<string, AppSection>> = {
      "1": "today",
      "2": "tasks",
      "3": "work",
      "4": "money",
      "5": "review",
    };
    const next = sectionByKey[event.key];
    if (next) {
      event.preventDefault();
      active = next;
    }
  }

  onMount(() => {
    void loadTasks();
    void loadReviewData();
    void refreshBackupHealth();
    void checkFirstRunSetup();
    refreshCalendarBoundaries();
    const calendarTimer = window.setInterval(refreshCalendarBoundaries, 60_000);
    const backupTimer = window.setInterval(
      () => void refreshBackupHealth(),
      60_000,
    );
    window.addEventListener("keydown", handleShortcut);
    window.addEventListener("focus", refreshCalendarBoundaries);
    let unsubscribeTasksChanged: (() => void) | null = null;
    void onTasksChanged(() => void loadTasks()).then((off) => {
      unsubscribeTasksChanged = off;
    });
    let unsubscribeOpenTask: (() => void) | null = null;
    void onOpenTask((taskId) => {
      const task = tasks.find((item) => item.id === taskId);
      if (task) openTaskEditor(task);
    }).then((off) => {
      unsubscribeOpenTask = off;
    });
    return () => {
      window.clearInterval(calendarTimer);
      window.clearInterval(backupTimer);
      window.removeEventListener("keydown", handleShortcut);
      window.removeEventListener("focus", refreshCalendarBoundaries);
      unsubscribeTasksChanged?.();
      unsubscribeOpenTask?.();
    };
  });
</script>

<svelte:head><title>{sectionCopy[active].title} · RudeSync</title></svelte:head>

<div class="app-shell">
  <Sidebar {active} openTaskCount={openTasks.length} onSelect={selectSection} />

  <main class="workspace">
    <header class="topbar">
      <div>
        <span class="eyebrow">{sectionCopy[active].eyebrow}</span>
        <h1>{sectionCopy[active].title}</h1>
      </div>
      <div class="topbar-actions">
        <button class="search-trigger" type="button" on:click={openSearch}>
          <Icon name="search" size={15} /><span>Search</span><kbd>Ctrl K</kbd>
        </button>
        <button class="icon-button notification-button" type="button" aria-label="Notifications">
          <Icon name="bell" size={17} /><span class="notification-dot"></span>
        </button>
        <button class="avatar" type="button" aria-label="Open profile">RS</button>
      </div>
    </header>

    {#if errorMessage}
      <div class="error-banner" role="alert">
        <span>{errorMessage}</span>
        <button type="button" aria-label="Dismiss error" on:click={() => (errorMessage = "")}>
          <Icon name="x" size={15} />
        </button>
      </div>
    {/if}

    {#if backupWarning && active !== "settings"}
      <div class="backup-warning" role="status">
        <Icon name="database" size={15} />
        <span>{backupWarning}</span>
        <button type="button" on:click={() => (active = "settings")}>
          Open Settings
        </button>
      </div>
    {/if}

    <div class="content-scroll">
      {#if active === "today"}
        <section class="today-intro">
          <div>
            <span class="date-line">{longDate()}</span>
            <h2>{greeting()}. Let’s make today count.</h2>
            <p>
              {overdueTasks.length
                ? `${overdueTasks.length} overdue and ${todayTasks.length} planned for today.`
                : todayTasks.length
                  ? `${todayTasks.length} ${todayTasks.length === 1 ? "task needs" : "tasks need"} your attention today.`
                : "Your slate is clear. Capture the next useful thing."}
            </p>
          </div>
          <div class="streak-chip">
            <Icon name="spark" size={16} />
            <div><strong>{completedTasks.length}</strong><span>finished</span></div>
          </div>
        </section>

        <form class="quick-add" on:submit|preventDefault={createQuickTask}>
          <span class="quick-plus"><Icon name="plus" size={17} /></span>
          <input
            bind:this={quickInput}
            bind:value={quickTitle}
            aria-label="Quick-add a task"
            autocomplete="off"
            maxlength="240"
            placeholder="What needs to get done?"
          />
          <span class="quick-hint">Today</span>
          <button type="submit" disabled={!quickTitle.trim() || saving}>
            {saving ? "Adding…" : "Add task"}
          </button>
        </form>

        <div class="today-grid">
          <section class="panel task-panel">
            <div class="panel-header">
              <div><span class="panel-kicker">Focus queue</span><h3>Today’s tasks</h3></div>
              <button class="text-button" type="button" on:click={() => (active = "tasks")}>
                View all <Icon name="chevron-right" size={14} />
              </button>
            </div>
            {#if loading}
              <div class="skeleton-list" aria-label="Loading tasks"><span></span><span></span><span></span></div>
            {:else if overdueTasks.length || todayTasks.length}
              <div class="task-list">
                {#if overdueTasks.length}
                  <div class="task-group-heading overdue">
                    <span>Overdue</span><b>{overdueTasks.length}</b>
                  </div>
                  {#each overdueTasks as task (task.id)}
                    <TaskRow {task} busy={busyTaskIds.has(task.id)} onToggle={toggleTask} onOpen={openTaskEditor} />
                  {/each}
                {/if}
                {#if todayTasks.length}
                  <div class="task-group-heading">
                    <span>Today</span><b>{todayTasks.length}</b>
                  </div>
                  {#each todayTasks as task (task.id)}
                    <TaskRow {task} busy={busyTaskIds.has(task.id)} onToggle={toggleTask} onOpen={openTaskEditor} />
                  {/each}
                {/if}
              </div>
            {:else}
              <div class="empty-state compact">
                <span class="empty-icon"><Icon name="check" size={19} /></span>
                <div><strong>Nothing urgent</strong><p>Add a task above or plan something in Upcoming.</p></div>
              </div>
            {/if}
          </section>

          <aside class="side-stack">
            <section class="panel metric-panel">
              <div class="metric-heading">
                <span class="metric-icon green"><Icon name="tasks" size={17} /></span>
                <span>Today’s progress</span>
              </div>
              <div class="metric-value"><strong>{completedToday.length}</strong><span>completed today</span></div>
              <div class="progress-track">
                <i style={`width: ${todayProgressTotal ? Math.round((completedToday.length / todayProgressTotal) * 100) : 0}%`}></i>
              </div>
              <p>{todayTasks.length} still open today</p>
            </section>

            <section class="panel attention-panel">
              <div class="panel-header small">
                <div><span class="panel-kicker">Coming up</span><h3>Payments</h3></div>
                <span class="soft-badge">{actionableInvoiceDues.length + actionableLoanDues.length} actionable</span>
              </div>
              {#each actionableLoanDues.slice(0, 3) as item (item.installment.id)}
                <div class:overdue={item.installment.dueDate < todayIso} class="attention-item">
                  <span class="attention-icon amber"><Icon name="loan" size={16} /></span>
                  <div>
                    <strong>{item.loan.operator} · installment #{item.installment.installmentNumber}</strong>
                    <span>{item.installment.dueDate < todayIso ? "Overdue" : "Due"} {displayDate(item.installment.dueDate)}</span>
                  </div>
                </div>
              {/each}
              {#each actionableInvoiceDues.slice(0, 3) as invoice (invoice.id)}
                <div class:overdue={invoice.dueDate < todayIso} class="attention-item">
                  <span class="attention-icon green"><Icon name="invoice" size={16} /></span>
                  <div>
                    <strong>{invoice.number} · {formatMoney(invoiceTotals(invoice).balanceDueMinor, invoice.currency)}</strong>
                    <span>{invoice.clientName} · {invoice.dueDate < todayIso ? "Overdue" : "Due"} {displayDate(invoice.dueDate)}</span>
                  </div>
                </div>
              {/each}
              {#if !actionableLoanDues.length && !actionableInvoiceDues.length}
                <div class="empty-inline">No invoice or personal-loan payments need attention.</div>
              {/if}
            </section>
          </aside>
        </div>

        <section class="panel upcoming-panel">
          <div class="panel-header">
            <div><span class="panel-kicker">Next seven days</span><h3>Upcoming</h3></div>
            <span class="count-label">{nextSevenTasks.length} scheduled</span>
          </div>
          {#if nextSevenTasks.length}
            <div class="upcoming-strip">
              {#each nextSevenTasks.slice(0, 4) as task (task.id)}
                <article>
                  <span class="upcoming-date">{displayDate(task.plannedDate ?? task.dueDate ?? "")}</span>
                  <strong>{task.title}</strong><span>{task.category ?? "Uncategorized"}</span>
                </article>
              {/each}
            </div>
          {:else}<div class="empty-inline">No tasks are scheduled yet.</div>{/if}
        </section>
      {:else if active === "tasks"}
        <section class="section-toolbar">
          <div class="segmented" aria-label="Task filters">
            {#each [["all", "Open"], ["inbox", "Inbox"], ["today", "Today"], ["upcoming", "Upcoming"], ["recurring", "Recurring"], ["categories", "Categories"], ["completed", "Completed"]] as filter}
              <button
                class:active={taskFilter === filter[0]}
                type="button"
                aria-pressed={taskFilter === filter[0]}
                on:click={() => (taskFilter = filter[0] as TaskFilter)}
              >{filter[1]}</button>
            {/each}
          </div>
          <label class="inline-search">
            <Icon name="search" size={14} /><input bind:this={searchInput} bind:value={searchQuery} aria-label="Filter tasks" placeholder="Filter tasks" />
          </label>
          <button
            class="primary-button"
            type="button"
            on:click={() => openTaskEditor()}
          ><Icon name="plus" size={15} /> New task</button>
          <button
            class="primary-button"
            type="button"
            on:click={() => void openTaskWidget()}
          ><Icon name="spark" size={15} /> Pop out widget</button>
        </section>

        <section class="panel full-panel">
          <div class="panel-header">
            <div><span class="panel-kicker">{taskFilter}</span><h3>{filteredTasks.length} tasks</h3></div>
            <span class="keyboard-note"><kbd>Ctrl N</kbd> quick capture</span>
          </div>
          {#if loading}
            <div class="skeleton-list"><span></span><span></span><span></span></div>
          {:else if filteredTasks.length}
            <div class="task-list roomy">
              {#each filteredTasks as task (task.id)}
                <TaskRow {task} busy={busyTaskIds.has(task.id)} onToggle={toggleTask} onOpen={openTaskEditor} />
              {/each}
            </div>
          {:else}
            <div class="empty-state large">
              <span class="empty-icon"><Icon name="tasks" size={22} /></span>
              <strong>No matching tasks</strong><p>Change the filter or capture a new task with Ctrl N.</p>
            </div>
          {/if}
        </section>
      {:else if active === "work"}
        <WorkView
          workPrefill={pendingWorkPrefill}
          on:prefillHandled={() => (pendingWorkPrefill = null)}
        />
      {:else if active === "money"}
        <MoneyView on:exportInvoice={handleInvoiceExport} />
      {:else if active === "review"}
        <section class="review-hero panel">
          <div>
            <span class="panel-kicker">This week</span><h2>{completedThisWeek.length} things moved forward.</h2>
            <p>RudeSync keeps completed work, collected earnings, and paid loan installments separate so your review stays honest.</p>
          </div>
          <div class="review-score">
            <strong>{weeklyProgressTotal ? Math.round((completedThisWeek.length / weeklyProgressTotal) * 100) : 0}%</strong>
            <span>task completion</span>
          </div>
        </section>
        <section class="summary-grid four">
          <article class="summary-card"><span>Tasks done</span><strong>{completedThisWeek.length}</strong><small>This week</small></article>
          <article class="summary-card"><span>Work records</span><strong>{completedWorkThisWeek.length}</strong><small>Logged</small></article>
          <article class="summary-card"><span>Collected</span><strong>{collectedThisWeek}</strong><small>Received payments</small></article>
          <article class="summary-card"><span>Loan dues paid</span><strong>{loanInstallmentsPaidThisWeek.length}</strong><small>This week</small></article>
        </section>
        <section class="panel full-panel">
          <div class="panel-header">
            <div><span class="panel-kicker">Next-week planning</span><h3>Upcoming tasks</h3></div>
            <span class="count-label">{upcomingTasks.length} planned</span>
          </div>
          {#if upcomingTasks.length}
            <div class="task-list">
              {#each upcomingTasks.slice(0, 5) as task (task.id)}
                <TaskRow {task} busy={busyTaskIds.has(task.id)} onToggle={toggleTask} onOpen={openTaskEditor} />
              {/each}
            </div>
          {:else}
            <div class="empty-state compact">
              <span class="empty-icon"><Icon name="calendar" size={19} /></span>
              <div><strong>Plan the next move</strong><p>Upcoming tasks will appear here.</p></div>
            </div>
          {/if}
        </section>
      {:else}
        <SettingsView />
      {/if}
    </div>
  </main>
</div>

{#if setupPromptOpen}
  <div class="setup-overlay" role="presentation">
    <div
      class="setup-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="setup-title"
      aria-describedby="setup-description"
    >
      <span class="setup-mark"><Icon name="spark" size={20} /></span>
      <span class="eyebrow">One-minute setup</span>
      <h2 id="setup-title">Finish setting up RudeSync</h2>
      <p id="setup-description">
        Choose a backup folder and add the name that should appear on invoices.
        You can change both at any time.
      </p>
      <div class="setup-actions">
        <button class="secondary-button" type="button" on:click={() => (setupPromptOpen = false)}>
          Not now
        </button>
        <button class="primary-button" type="button" on:click={openFirstRunSettings}>
          Open setup
        </button>
      </div>
    </div>
  </div>
{/if}

<TaskDialog
  open={taskDialogOpen}
  task={editingTask}
  projects={taskProjects}
  saving={taskDialogSaving}
  on:close={() => {
    taskDialogOpen = false;
    editingTask = null;
  }}
  on:save={saveTask}
  on:toggleSubtask={toggleSubtask}
/>
