<script lang="ts">
  import { onMount, tick } from "svelte";
  import { flip } from "svelte/animate";
  import { cubicOut } from "svelte/easing";
  import { fade, scale } from "svelte/transition";
  import BarChart from "./lib/components/BarChart.svelte";
  import Card from "./lib/components/Card.svelte";
  import Icon from "./lib/components/Icon.svelte";
  import MeterBar from "./lib/components/MeterBar.svelte";
  import SectionHeader from "./lib/components/SectionHeader.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import StatCard from "./lib/components/StatCard.svelte";
  import StatRow from "./lib/components/StatRow.svelte";
  import TaskRow from "./lib/components/TaskRow.svelte";
  import { addDays } from "./lib/domain/date.ts";
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
  import { motionDuration } from "./lib/motion";
  import type { AppSection, Task } from "./lib/types";
  import type { InvoiceExportDetail } from "./lib/features/money/types";
  import {
    openTaskCount,
    completedTodayCount,
    overdueTaskCount,
    overdueInvoiceCount,
    outstandingByCurrency,
    receivedInMonth,
  } from "./lib/features/dashboard/stats.ts";
  import { completionsByDay } from "./lib/features/dashboard/chartSeries.ts";

  type TaskFilter =
    | "all"
    | "inbox"
    | "today"
    | "upcoming"
    | "recurring"
    | "categories"
    | "completed";

  const taskFilterLabels: Record<TaskFilter, string> = {
    all: "待处理",
    inbox: "收件箱",
    today: "今天",
    upcoming: "即将到来",
    recurring: "重复任务",
    categories: "分类",
    completed: "已完成",
  };

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
  let setupOverlayOutroing = false;
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
  let workDialogOpen = false;
  let settingsHasChanges = false;
  let moneyHasChanges = false;

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
  $: upcomingInstallmentCount = reviewLoans
    .flatMap((loan) => loan.installments)
    .filter(
      (installment) =>
        !installment.paid &&
        installment.dueDate >= todayIso &&
        installment.dueDate <= addDays(todayIso, 30),
    ).length;
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
  $: statOpen = openTaskCount(tasks);
  // Same local-day resolver as `completedToday` below, so the stat card and
  // the progress meter can never report different counts for the same day.
  $: statDoneToday = completedTodayCount(tasks, todayIso, localDayFromTimestamp);
  $: statOverdue = overdueTaskCount(tasks, todayIso) + overdueInvoiceCount(reviewInvoices, todayIso);
  $: statOutstanding = outstandingByCurrency(reviewInvoices);
  $: statReceived = receivedInMonth(reviewInvoices, todayIso);
  // Both charts bucket by local day via the same resolver `completedToday` and
  // `completedThisWeek` use, so a bar can never contradict the meter beside it.
  $: completionTrend = completionsByDay(tasks, todayIso, 14, localDayFromTimestamp);
  // Anchored to the review week, not a trailing 7 days. A trailing window
  // reaches back past the week boundary, so just after a week rolls over the
  // chart shows last week's completions beside a "0 done this week" card and
  // the page appears to contradict itself. Every figure on Review describes
  // the same Mon-Sun week; later days simply sit empty until they happen.
  $: reviewTrend = completionsByDay(tasks, weekEndIso, 7, localDayFromTimestamp);
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
  $: taskFilterSubtext = `${filteredTasks.length} shown · ${taskFilterLabels[taskFilter]}`;

  const sectionCopy: Record<AppSection, { eyebrow: string; title: string }> = {
    today: { eyebrow: "今日总览", title: "今天" },
    tasks: { eyebrow: "计划与执行", title: "待办事项" },
    work: { eyebrow: "客户与交付", title: "工作" },
    money: { eyebrow: "发票与收支", title: "财务" },
    review: { eyebrow: "每周复盘", title: "复盘" },
    settings: { eyebrow: "你的工作区", title: "设置" },
  };

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

  function formatStatMoney(entry: { currency: string; amountMinor: number }): string {
    return new Intl.NumberFormat("en-US", {
      style: "currency",
      currency: entry.currency,
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }).format(entry.amountMinor / 100);
  }

  function extraCurrencies(entries: Array<{ currency: string; amountMinor: number }>): string {
    return entries
      .slice(1)
      .map((entry) => formatStatMoney(entry))
      .join(" · ");
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
        "尚未设置自动备份，请在“设置”中选择备份文件夹。";
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
    if (
      active === "settings" &&
      section !== "settings" &&
      settingsHasChanges &&
      !window.confirm("Discard your unsaved settings changes?")
    ) {
      return;
    }
    if (
      active === "money" &&
      section !== "money" &&
      moneyHasChanges &&
      !window.confirm("Discard your unsaved money changes?")
    ) {
      return;
    }
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
    if (!invoke) {
      errorMessage = "The task widget is only available in the desktop app.";
      return;
    }
    errorMessage = "";
    try {
      await invoke("show_task_widget");
    } catch (error) {
      errorMessage =
        error instanceof Error
          ? error.message
          : "The task widget could not be opened.";
    }
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
    // A dialog already open owns the keyboard: don't let a shortcut
    // re-hydrate it (Ctrl+N over an in-progress edit) or switch sections out
    // from under it (unmounting WorkView with an open Work dialog).
    if (taskDialogOpen || workDialogOpen) return;
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
      selectSection(next);
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
        {#if active === "today" || active === "tasks"}
          <button class="search-trigger" type="button" on:click={openSearch}>
            <Icon name="search" size={15} /><span>查找任务</span><kbd>Ctrl K</kbd>
          </button>
        {/if}
      </div>
    </header>

    <div class="workspace-banners">
      {#if errorMessage}
        <div class="error-banner" role="alert">
          <span>{errorMessage}</span>
          <button type="button" aria-label="关闭错误提示" on:click={() => (errorMessage = "")}>
            <Icon name="x" size={15} />
          </button>
        </div>
      {/if}

      {#if backupWarning && active !== "settings"}
        <div class="backup-warning" role="status">
          <Icon name="database" size={15} />
          <span>{backupWarning}</span>
          <button type="button" on:click={() => (active = "settings")}>
            打开设置
          </button>
        </div>
      {/if}
    </div>

    <div class="content-scroll">
      {#key active}
      <div in:fade={{ duration: motionDuration(140) }}>
      {#if active === "today"}
        <StatRow>
          <StatCard
            icon="check"
            label="待处理"
            value={String(statOpen)}
            detail={`今天已完成 ${statDoneToday} 项`}
            tone="neutral"
          />
          <StatCard
            icon="clock"
            label="已逾期"
            value={String(statOverdue)}
            detail={statOverdue === 0 ? "暂无逾期" : "任务与发票"}
            tone={statOverdue > 0 ? "danger" : "neutral"}
          />
          <StatCard
            icon="invoice"
            label="待收款"
            value={statOutstanding.length ? formatStatMoney(statOutstanding[0]) : "无"}
            detail={statOutstanding.length > 1 ? extraCurrencies(statOutstanding) : "已开票，尚未收款"}
            tone={statOutstanding.length ? "warning" : "neutral"}
          />
          <StatCard
            icon="arrow-up-right"
            label="已收款"
            value={statReceived.length ? formatStatMoney(statReceived[0]) : "无"}
            detail={statReceived.length > 1 ? extraCurrencies(statReceived) : "本月"}
            tone={statReceived.length ? "positive" : "neutral"}
          />
        </StatRow>

        <form class="quick-add" on:submit|preventDefault={createQuickTask}>
          <span class="quick-plus"><Icon name="plus" size={17} /></span>
          <input
            bind:this={quickInput}
            bind:value={quickTitle}
            aria-label="快速添加待办"
            autocomplete="off"
            maxlength="240"
            placeholder="接下来要做什么？"
          />
          <span class="quick-hint">今天</span>
          <button type="submit" disabled={!quickTitle.trim() || saving}>
            {saving ? "正在添加…" : "添加任务"}
          </button>
        </form>

        <div class="today-grid">
          <Card padded={false}>
            <SectionHeader slot="header" title="今日任务" subtext="今天到期或计划执行">
              <svelte:fragment slot="actions">
                <button class="text-button" type="button" on:click={() => (active = "tasks")}>
                  查看全部 <Icon name="chevron-right" size={14} />
                </button>
              </svelte:fragment>
            </SectionHeader>
            {#if loading}
              <div class="skeleton-list" aria-label="正在加载任务"><span></span><span></span><span></span></div>
            {:else if overdueTasks.length || todayTasks.length}
              <div class="task-list">
                {#if overdueTasks.length}
                  <div class="task-group-heading overdue">
                    <span>已逾期</span><b>{overdueTasks.length}</b>
                  </div>
                  {#each overdueTasks as task (task.id)}
                    <div
                      class="task-row-outro"
                      out:fade={{ duration: motionDuration(180) }}
                      animate:flip={{ duration: motionDuration(180) }}
                    >
                      <TaskRow {task} busy={busyTaskIds.has(task.id)} onToggle={toggleTask} onOpen={openTaskEditor} />
                    </div>
                  {/each}
                {/if}
                {#if todayTasks.length}
                  <div class="task-group-heading">
                    <span>今天</span><b>{todayTasks.length}</b>
                  </div>
                  {#each todayTasks as task (task.id)}
                    <div
                      class="task-row-outro"
                      out:fade={{ duration: motionDuration(180) }}
                      animate:flip={{ duration: motionDuration(180) }}
                    >
                      <TaskRow {task} busy={busyTaskIds.has(task.id)} onToggle={toggleTask} onOpen={openTaskEditor} />
                    </div>
                  {/each}
                {/if}
              </div>
            {:else}
              <div class="empty-state compact">
                <span class="empty-icon"><Icon name="check" size={19} /></span>
                <div><strong>暂无紧急事项</strong><p>在上方添加任务，或安排到即将到来。</p></div>
              </div>
            {/if}
          </Card>

          <aside class="side-stack">
            <Card>
              <SectionHeader slot="header" title="今日进度" />
              <MeterBar
                label="Tasks completed"
                value={completedToday.length}
                max={todayProgressTotal}
                detail={`${todayTasks.length} still open today`}
                tone="positive"
              />
            </Card>

            <Card padded={false}>
              <SectionHeader slot="header" title="Payments" subtext="Coming up">
                <svelte:fragment slot="actions">
                  <span class="soft-badge">{actionableInvoiceDues.length + actionableLoanDues.length} actionable</span>
                </svelte:fragment>
              </SectionHeader>
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
                <div class="empty-inline">目前没有需要关注的发票或个人借款付款。</div>
              {/if}
            </Card>
          </aside>
        </div>

        <div class="chart-panel">
          <Card>
            <SectionHeader
              slot="header"
              title="Completed"
              subtext="Tasks finished each day, last 14 days"
            />
            <BarChart
              points={completionTrend}
              valueLabel="Completed"
              labelEvery={2}
              tableCaption="Tasks completed each day over the last 14 days"
              emptyMessage="No tasks completed in the last 14 days yet."
              loading={loading}
            />
          </Card>
        </div>

        <div class="upcoming-panel">
          <Card padded={false}>
            <SectionHeader slot="header" title="Upcoming" subtext="Next seven days">
              <svelte:fragment slot="actions">
                <span class="count-label">{nextSevenTasks.length} scheduled</span>
              </svelte:fragment>
            </SectionHeader>
            {#if nextSevenTasks.length}
              <div class="upcoming-strip">
                {#each nextSevenTasks.slice(0, 4) as task (task.id)}
                  <article
                    out:fade={{ duration: motionDuration(180) }}
                    animate:flip={{ duration: motionDuration(180) }}
                  >
                    <span class="upcoming-date">{displayDate(task.plannedDate ?? task.dueDate ?? "")}</span>
                    <strong>{task.title}</strong><span>{task.category ?? "Uncategorized"}</span>
                  </article>
                {/each}
              </div>
            {:else}<div class="empty-inline">暂无已安排任务。</div>{/if}
          </Card>
        </div>
      {:else if active === "tasks"}
        <div class="page-measure">
          <div class="page-actions">
            <span class="keyboard-note"><kbd>Ctrl N</kbd> 快速记录</span>
            <button
              class="primary-button"
              type="button"
              on:click={() => openTaskEditor()}
            ><Icon name="plus" size={15} /> 新建任务</button>
            <button
              class="primary-button"
              type="button"
              disabled={!settingsService.isDesktop}
              on:click={() => void openTaskWidget()}
            ><Icon name="spark" size={15} /> 打开悬浮窗</button>
          </div>
          <Card padded={false}>
            <SectionHeader slot="header" title="全部任务" subtext={taskFilterSubtext}>
              <svelte:fragment slot="actions">
                <div class="segmented" aria-label="任务筛选">
                  {#each [["all", "待处理"], ["inbox", "收件箱"], ["today", "今天"], ["upcoming", "即将到来"], ["recurring", "重复任务"], ["categories", "分类"], ["completed", "已完成"]] as filter}
                    <button
                      class:active={taskFilter === filter[0]}
                      type="button"
                      aria-pressed={taskFilter === filter[0]}
                      on:click={() => (taskFilter = filter[0] as TaskFilter)}
                    >{filter[1]}</button>
                  {/each}
                </div>
                <label class="inline-search">
                  <Icon name="search" size={14} /><input bind:this={searchInput} bind:value={searchQuery} aria-label="筛选任务" placeholder="筛选任务" />
                </label>
              </svelte:fragment>
            </SectionHeader>
            {#if loading}
              <div class="skeleton-list"><span></span><span></span><span></span></div>
            {:else if filteredTasks.length}
              <div class="task-list roomy">
                {#each filteredTasks as task (task.id)}
                  <div
                    class="task-row-outro"
                    out:fade={{ duration: motionDuration(180) }}
                    animate:flip={{ duration: motionDuration(180) }}
                  >
                    <TaskRow {task} busy={busyTaskIds.has(task.id)} onToggle={toggleTask} onOpen={openTaskEditor} />
                  </div>
                {/each}
              </div>
            {:else}
              <div class="empty-state large">
                <span class="empty-icon"><Icon name="tasks" size={22} /></span>
                <strong>没有匹配的任务</strong><p>更改筛选条件，或按 Ctrl+N 新建任务。</p>
              </div>
            {/if}
          </Card>
        </div>
      {:else if active === "work"}
        <WorkView
          workPrefill={pendingWorkPrefill}
          bind:dialogOpen={workDialogOpen}
          on:prefillHandled={() => (pendingWorkPrefill = null)}
        />
      {:else if active === "money"}
        <MoneyView on:exportInvoice={handleInvoiceExport} bind:hasChanges={moneyHasChanges} />
      {:else if active === "review"}
        <StatRow>
          <StatCard
            icon="check"
            label="已完成任务"
            value={String(completedThisWeek.length)}
            detail="本周"
            tone={completedThisWeek.length > 0 ? "positive" : "neutral"}
          />
          <StatCard icon="work" label="已记录工作" value={String(reviewWorkEntries.length)} detail="已记录条目" tone="neutral" />
          <StatCard
            icon="arrow-up-right"
            label="已收款"
            value={statReceived.length ? formatStatMoney(statReceived[0]) : "无"}
            detail={statReceived.length > 1 ? extraCurrencies(statReceived) : "本月"}
            tone={statReceived.length ? "positive" : "neutral"}
          />
          <StatCard
            icon="loan"
            label="待还借款"
            value={String(upcomingInstallmentCount)}
            detail="未来30天"
            tone={upcomingInstallmentCount > 0 ? "warning" : "neutral"}
          />
        </StatRow>

        <div class="review-grid">
          <Card>
            <SectionHeader slot="header" title="This week" subtext="Task completion" />
            <MeterBar
              label="Weekly progress"
              value={completedThisWeek.length}
              max={weeklyProgressTotal}
              detail={`${completedThisWeek.length} of ${weeklyProgressTotal} done`}
              valueText={`${completedThisWeek.length} of ${weeklyProgressTotal} done`}
              tone="positive"
            />
          </Card>

          <Card>
            <SectionHeader
              slot="header"
              title="每日节奏"
              subtext="Tasks finished each day this week"
            />
            <BarChart
              points={reviewTrend}
              valueLabel="Completed"
              tableCaption="Tasks completed each day this week"
              emptyMessage="Nothing completed this week yet."
              loading={loading}
            />
          </Card>
        </div>

        <section class="panel full-panel">
          <div class="panel-header">
            <div><span class="panel-kicker">下周计划</span><h3>即将到来的任务</h3></div>
            <span class="count-label">{upcomingTasks.length} planned</span>
          </div>
          {#if upcomingTasks.length}
            <div class="task-list">
              {#each upcomingTasks.slice(0, 5) as task (task.id)}
                <div
                  class="task-row-outro"
                  out:fade={{ duration: motionDuration(180) }}
                  animate:flip={{ duration: motionDuration(180) }}
                >
                  <TaskRow {task} busy={busyTaskIds.has(task.id)} onToggle={toggleTask} onOpen={openTaskEditor} />
                </div>
              {/each}
            </div>
          {:else}
            <div class="empty-state compact">
              <span class="empty-icon"><Icon name="calendar" size={19} /></span>
              <div><strong>安排下一步</strong><p>即将到来的任务会显示在这里。</p></div>
            </div>
          {/if}
        </section>
      {:else}
        <SettingsView bind:hasChanges={settingsHasChanges} />
      {/if}
      </div>
      {/key}
    </div>
  </main>
</div>

{#if setupPromptOpen}
  <div
    class="setup-overlay"
    class:outroing={setupOverlayOutroing}
    role="presentation"
    transition:fade={{ duration: motionDuration(140) }}
    on:outrostart={() => (setupOverlayOutroing = true)}
    on:outroend={() => (setupOverlayOutroing = false)}
  >
    <div
      class="setup-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="setup-title"
      aria-describedby="setup-description"
      in:scale={{ duration: motionDuration(200), start: 0.96, opacity: 0, easing: cubicOut }}
      out:scale={{ duration: motionDuration(140), start: 0.98, opacity: 0, easing: cubicOut }}
    >
      <span class="setup-mark"><Icon name="spark" size={20} /></span>
      <span class="eyebrow">一分钟设置</span>
      <h2 id="setup-title">完成 RudeSync 设置</h2>
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
