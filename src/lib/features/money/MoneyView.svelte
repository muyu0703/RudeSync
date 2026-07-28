<script lang="ts">
  import { createEventDispatcher, onMount, tick } from "svelte";
  import { fade, slide } from "svelte/transition";
  import { motionDuration } from "../../motion";
  import {
    calculateInvoiceDueDate,
    type InvoiceDueTerm,
  } from "../../domain/invoice.ts";
  import {
    calculateInvoiceTotals,
    type InvoiceDiscount,
    type InvoiceTotals,
  } from "../../domain/money.ts";
  import { addDays, addMonthsClamped } from "../../domain/date.ts";
  import { generateLoanDueDates } from "../../domain/loan-schedule.ts";
  import Card from "../../components/Card.svelte";
  import Icon from "../../components/Icon.svelte";
  import SectionHeader from "../../components/SectionHeader.svelte";
  import StatCard from "../../components/StatCard.svelte";
  import StatRow from "../../components/StatRow.svelte";
  import {
    outstandingByCurrency,
    overdueInvoiceCount,
    receivedInMonth,
    type CurrencyAmount,
  } from "../dashboard/stats.ts";
  import { createSettingsService } from "../settings/settingsService";
  import {
    createMoneyService,
    effectiveInvoiceStatus,
    invoiceTotals,
    moneyDateUtils,
  } from "./moneyService.ts";
  import type {
    Invoice,
    CreateInvoiceInput,
    InvoiceExportDetail,
    InvoiceProjectOption,
    InvoiceStatus,
    InvoiceTermKind,
    LoanFrequency,
    PersonalLoan,
  } from "./types.ts";

  type MoneyTab = "invoices" | "earnings" | "loans";
  type DiscountKind = "none" | "fixed" | "percentage";
  interface DraftLine {
    id: string;
    description: string;
    quantity: string;
    unitPrice: string;
  }

  const service = createMoneyService();
  const settingsService = createSettingsService();
  const dispatch = createEventDispatcher<{
    exportInvoice: InvoiceExportDetail;
  }>();
  let today = moneyDateUtils.localIsoDay();

  let activeTab: MoneyTab = "invoices";
  let loading = true;
  let saving = false;
  let errorMessage = "";
  let successMessage = "";
  let projects: InvoiceProjectOption[] = [];
  let invoices: Invoice[] = [];
  let loans: PersonalLoan[] = [];
  let showInvoiceForm = false;
  let showLoanForm = false;
  let editingInvoiceId: string | null = null;
  // The milestone the draft being edited already owns. It reads as "invoiced"
  // because of that very draft, so it must stay pickable in the form.
  let editingMilestoneId: string | null = null;

  let noticeRegion: HTMLDivElement | undefined;
  let composerFormEl: HTMLFormElement | undefined;
  let composerFirstFieldEl: HTMLSelectElement | undefined;

  let invoiceProjectId = "";
  let invoiceMilestoneId = "";
  let invoiceIssueDate = today;
  let invoiceTerm: InvoiceTermKind = "14-days";
  let defaultInvoiceTerm: InvoiceTermKind = "14-days";
  let invoiceCustomDueDate: string = addDays(today, 14);
  let invoiceDiscountKind: DiscountKind = "none";
  let invoiceDiscountValue = "";
  let invoiceTax = "";
  let invoiceNotes = "";
  let invoiceInstructions = "";
  let invoiceLines: DraftLine[] = [blankLine()];

  let paymentInvoiceId: string | null = null;
  let paymentAmount = "";
  let paymentDate = today;
  let paymentNote = "";

  let loanOperator = "";
  let loanDescription = "";
  let loanDate = today;
  let loanFirstPayment = addMonthsClamped(today, 1);
  let loanCount = 9;
  let loanFrequency: LoanFrequency = "monthly";
  let loanDayOne = Number(loanFirstPayment.slice(8, 10));
  let loanDayTwo = loanDayOne === 28 ? 13 : 28;
  let loanScheduleDates: string[] = [];
  let loanPreviewSignature = "";
  let busyInstallments = new Set<string>();
  let pendingPaidInstallmentId: string | null = null;
  let pendingPaidDate = today;

  $: selectedProject =
    projects.find((project) => project.id === invoiceProjectId) ?? null;
  $: selectedMilestone =
    selectedProject?.milestones.find(
      (milestone) => milestone.id === invoiceMilestoneId,
    ) ?? null;
  $: invoiceDueDate = getInvoiceDueDate();
  $: invoiceNumberPreview = getInvoiceNumberPreview();
  $: draftTotals = getDraftTotals();
  $: paymentInvoice =
    invoices.find((invoice) => invoice.id === paymentInvoiceId) ?? null;
  $: paymentRows = collectPaymentRows();
  $: earningGroups = groupEarnings();
  $: scheduleIsStale =
    loanScheduleDates.length > 0 &&
    loanPreviewSignature !== currentLoanSignature();
  $: moneyOutstanding = outstandingByCurrency(invoices);
  $: moneyReceived = receivedInMonth(invoices, today);
  $: overdueInvoices = overdueInvoiceCount(invoices, today);
  $: earningsSubtext = earningGroups.length
    ? `All-time collected · ${earningGroups
        .map((group) => formatMoney(group.total, group.currency))
        .join(" · ")}`
    : "All-time collected";
  // Invoice/loan actions can be triggered from far down a long list, while the
  // notice banner renders once, near the top. Bring it into view whenever its
  // content changes so feedback is never off-screen from the action.
  $: if (noticeRegion && (errorMessage || successMessage)) {
    noticeRegion.scrollIntoView({
      behavior: motionDuration(1) === 0 ? "auto" : "smooth",
      block: "nearest",
    });
  }

  function blankLine(): DraftLine {
    return {
      id: makeId(),
      description: "",
      quantity: "1",
      unitPrice: "",
    };
  }

  function makeId(): string {
    if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
      return crypto.randomUUID();
    }
    return `draft-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  }

  function parseMinorUnits(value: string, label: string): number {
    const normalized = value.trim();
    if (!/^(?:0|[1-9]\d*)(?:\.\d{1,2})?$/.test(normalized)) {
      throw new Error(`${label} must be a valid amount with up to 2 decimals.`);
    }
    const amount = Math.round(Number(normalized) * 100);
    if (!Number.isSafeInteger(amount)) {
      throw new Error(`${label} is too large.`);
    }
    return amount;
  }

  function errorText(error: unknown, fallback: string): string {
    if (error instanceof Error && error.message.trim()) {
      return error.message;
    }
    if (typeof error === "string" && error.trim()) {
      return error.trim();
    }
    if (error && typeof error === "object" && "message" in error) {
      const message = String((error as { message?: unknown }).message ?? "").trim();
      if (message) return message;
    }
    return fallback;
  }

  function minorInput(amount: number): string {
    return (amount / 100).toFixed(2);
  }

  function formatMoney(amount: number, currency = "USD"): string {
    try {
      return new Intl.NumberFormat("en-US", {
        style: "currency",
        currency,
      }).format(amount / 100);
    } catch {
      return `${currency} ${(amount / 100).toFixed(2)}`;
    }
  }

  // Per-currency amounts are never summed together. Only the first currency
  // is shown as the stat's headline value; any others are listed in the
  // detail line instead, matching the Today dashboard's stat cards.
  function extraCurrencies(entries: CurrencyAmount[]): string {
    return entries
      .slice(1)
      .map((entry) => formatMoney(entry.amountMinor, entry.currency))
      .join(" · ");
  }

  function formatDate(value: string): string {
    if (!value) return "—";
    const [year, month, day] = value.slice(0, 10).split("-").map(Number);
    return new Intl.DateTimeFormat("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
    }).format(new Date(year, month - 1, day, 12));
  }

  function termValue(): InvoiceDueTerm {
    if (invoiceTerm === "custom") {
      return { kind: "custom", dueDate: invoiceCustomDueDate };
    }
    const days: Record<Exclude<InvoiceTermKind, "custom">, 0 | 7 | 14 | 30> = {
      immediate: 0,
      "7-days": 7,
      "14-days": 14,
      "30-days": 30,
    };
    return days[invoiceTerm];
  }

  function getInvoiceDueDate(): string {
    try {
      return calculateInvoiceDueDate(invoiceIssueDate, termValue());
    } catch {
      return "";
    }
  }

  function getInvoiceNumberPreview(): string {
    try {
      const editing = invoices.find(
        (invoice) => invoice.id === editingInvoiceId,
      );
      if (editing && editing.issueDate === invoiceIssueDate) {
        return editing.number;
      }
      return /^\d{4}-\d{2}-\d{2}$/.test(invoiceIssueDate)
        ? `INV-${invoiceIssueDate}-####`
        : "INV-YYYY-MM-DD-####";
    } catch {
      return "INV-YYYY-MM-DD-####";
    }
  }

  function buildDiscount(): InvoiceDiscount | null {
    if (invoiceDiscountKind === "none") return null;
    if (invoiceDiscountKind === "fixed") {
      return {
        kind: "fixed",
        amountMinor: parseMinorUnits(invoiceDiscountValue, "Discount"),
      };
    }
    return {
      kind: "percentage",
      percentage: invoiceDiscountValue.trim() || "0",
    };
  }

  function getDraftTotals(): InvoiceTotals | null {
    try {
      return calculateInvoiceTotals({
        lineItems: invoiceLines.map((line) => ({
          quantity: line.quantity,
          unitPriceMinor: parseMinorUnits(
            line.unitPrice || "0",
            "Unit price",
          ),
        })),
        discount: buildDiscount(),
        taxPercentage: invoiceTax.trim() || null,
      });
    } catch {
      return null;
    }
  }

  function statusLabel(status: InvoiceStatus): string {
    return status.replace("-", " ");
  }

  function groupEarnings(): Array<{ currency: string; total: number }> {
    const groups = new Map<string, number>();
    for (const { invoice, payment } of paymentRows) {
      groups.set(
        invoice.currency,
        (groups.get(invoice.currency) ?? 0) + payment.amountMinor,
      );
    }
    return [...groups].map(([currency, total]) => ({ currency, total }));
  }

  function collectPaymentRows(): Array<{
    invoice: Invoice;
    payment: Invoice["payments"][number];
  }> {
    const rows: Array<{
      invoice: Invoice;
      payment: Invoice["payments"][number];
    }> = [];
    for (const invoice of invoices) {
      for (const payment of invoice.payments) {
        rows.push({ invoice, payment });
      }
    }
    return rows.sort((a, b) =>
      b.payment.receivedDate.localeCompare(a.payment.receivedDate),
    );
  }

  function openInvoiceComposer(): void {
    editingInvoiceId = null;
    resetInvoiceForm();
    showInvoiceForm = true;
    showLoanForm = false;
    if (!invoiceProjectId && projects[0]) {
      invoiceProjectId = projects[0].id;
      applyProjectDefaults();
    }
    void focusComposer();
  }

  function editDraftInvoice(invoice: Invoice): void {
    editingInvoiceId = invoice.id;
    invoiceProjectId = invoice.projectId;
    // Repopulate the picker from the draft's own link so saving again keeps
    // the milestone (and its label/kind snapshot) instead of silently
    // unlinking it. Existing line items are preserved as-is.
    invoiceMilestoneId = invoice.milestoneId ?? "";
    editingMilestoneId = invoice.milestoneId ?? null;
    invoiceIssueDate = invoice.issueDate;
    invoiceTerm = invoice.termKind;
    invoiceCustomDueDate = invoice.dueDate;
    invoiceDiscountKind = invoice.discount?.kind ?? "none";
    invoiceDiscountValue =
      invoice.discount?.kind === "fixed"
        ? minorInput(invoice.discount.amountMinor)
        : invoice.discount?.kind === "percentage"
          ? invoice.discount.percentage
          : "";
    invoiceTax = invoice.taxPercentage ?? "";
    invoiceNotes = invoice.notes ?? "";
    invoiceInstructions = invoice.paymentInstructions ?? "";
    invoiceLines = invoice.lineItems.map((line) => ({
      id: line.id || makeId(),
      description: line.description,
      quantity: line.quantity,
      unitPrice: minorInput(line.unitPriceMinor),
    }));
    showInvoiceForm = true;
    showLoanForm = false;
    void focusComposer();
  }

  // The backend's milestone_kind CHECK on invoices only allows
  // kickoff|completion|custom; a linked milestone's finer-grained kind
  // (phase/weekly/additional/custom) is snapshotted under "custom" there,
  // so the request must already use that mapping to pass validation.
  function backendMilestoneKind(kind: string | undefined): string {
    return kind === "kickoff" || kind === "completion" ? kind : "custom";
  }

  function applyProjectDefaults(): void {
    const project = projects.find((item) => item.id === invoiceProjectId);
    const defaultMilestone = project?.milestones.find(
      (milestone) => milestone.status === "not-invoiced",
    );
    invoiceMilestoneId = defaultMilestone?.id ?? "";
    applyMilestoneDefaults();
  }

  function applyMilestoneDefaults(): void {
    const project = projects.find((item) => item.id === invoiceProjectId);
    const milestone = project?.milestones.find(
      (item) => item.id === invoiceMilestoneId,
    );
    if (!milestone) return;
    invoiceLines = [
      {
        id: makeId(),
        description: `${project?.name ?? "Project"} — ${milestone.label}`,
        quantity: "1",
        unitPrice: minorInput(milestone.amountMinor),
      },
    ];
  }

  function addInvoiceLine(): void {
    invoiceLines = [...invoiceLines, blankLine()];
  }

  function removeInvoiceLine(id: string): void {
    if (invoiceLines.length === 1) return;
    invoiceLines = invoiceLines.filter((line) => line.id !== id);
  }

  async function loadMoney(): Promise<void> {
    loading = true;
    errorMessage = "";
    try {
      [projects, invoices, loans] = await Promise.all([
        service.listInvoiceProjects(),
        service.listInvoices(),
        service.listPersonalLoans(),
      ]);
    } catch (error) {
      errorMessage = errorText(error, "Money data could not be loaded.");
    } finally {
      loading = false;
    }
  }

  function currentInvoiceInput(
    status: "draft" | "issued",
  ): CreateInvoiceInput {
    if (!selectedProject) {
      throw new Error("Select a project.");
    }
    return {
      projectId: selectedProject.id,
      clientId: selectedProject.clientId,
      projectName: selectedProject.name,
      clientName: selectedProject.clientName,
      milestoneId: selectedMilestone?.id ?? null,
      milestoneKind: backendMilestoneKind(selectedMilestone?.kind),
      milestonePercentBasisPoints: null,
      milestoneLabel: selectedMilestone?.label ?? null,
      issueDate: invoiceIssueDate,
      dueDate: invoiceDueDate,
      termKind: invoiceTerm,
      currency: selectedProject.currency,
      lineItems: invoiceLines.map((line) => ({
        description: line.description,
        quantity: line.quantity,
        unitPriceMinor: parseMinorUnits(line.unitPrice, "Unit price"),
      })),
      discount: buildDiscount(),
      taxPercentage: invoiceTax.trim() || null,
      notes: invoiceNotes,
      paymentInstructions: invoiceInstructions,
      status,
    };
  }

  async function saveInvoice(status: "draft" | "issued" = "issued"): Promise<void> {
    if (!selectedProject || saving) return;
    saving = true;
    errorMessage = "";
    successMessage = "";
    try {
      const input = currentInvoiceInput(status);
      let saved: Invoice;
      if (editingInvoiceId) {
        saved = await service.updateDraftInvoice({
          ...input,
          invoiceId: editingInvoiceId,
        });
        invoices = invoices.map((invoice) =>
          invoice.id === saved.id ? saved : invoice,
        );
        if (status === "issued") {
          saved = await service.issueDraftInvoice(saved.id);
          invoices = invoices.map((invoice) =>
            invoice.id === saved.id ? saved : invoice,
          );
        }
      } else {
        saved = await service.createInvoice(input);
        invoices = [saved, ...invoices];
      }
      showInvoiceForm = false;
      successMessage = `${saved.number} was ${
        status === "issued" ? "issued" : "saved as a draft"
      }.`;
      resetInvoiceForm();
    } catch (error) {
      errorMessage = errorText(error, "Invoice could not be saved.");
    } finally {
      saving = false;
    }
  }

  // Enter must never irreversibly issue an invoice: the composer's <form>
  // submit action is "Save draft" (reversible), so "Create & issue" is a
  // type="button" that reaches saveInvoice("issued") only through this same
  // confirmation the invoice list already uses before locking the number and
  // issue date.
  function confirmIssueFromComposer(): void {
    if (saving) return;
    if (
      !window.confirm(
        `Issue ${invoiceNumberPreview}? Its number and issue date will be locked.`,
      )
    ) {
      return;
    }
    void saveInvoice("issued");
  }

  // Scrolls the composer into view and focuses its first field once it has
  // mounted, so opening it (especially "Edit draft" on an invoice far down
  // the list) is visibly obvious instead of appearing to do nothing.
  async function focusComposer(): Promise<void> {
    await tick();
    composerFormEl?.scrollIntoView({
      behavior: motionDuration(1) === 0 ? "auto" : "smooth",
      block: "start",
    });
    composerFirstFieldEl?.focus();
  }

  function resetInvoiceForm(): void {
    editingInvoiceId = null;
    editingMilestoneId = null;
    invoiceProjectId = "";
    invoiceMilestoneId = "";
    invoiceIssueDate = today;
    invoiceTerm = defaultInvoiceTerm;
    invoiceCustomDueDate = addDays(today, 14);
    invoiceDiscountKind = "none";
    invoiceDiscountValue = "";
    invoiceTax = "";
    invoiceNotes = "";
    invoiceInstructions = "";
    invoiceLines = [blankLine()];
  }

  async function issueDraft(invoice: Invoice): Promise<void> {
    if (saving) return;
    if (!window.confirm(`Issue ${invoice.number}? Its number and issue date will be locked.`)) {
      return;
    }
    saving = true;
    errorMessage = "";
    try {
      const updated = await service.issueDraftInvoice(invoice.id);
      invoices = invoices.map((item) =>
        item.id === updated.id ? updated : item,
      );
      successMessage = `${updated.number} was issued.`;
    } catch (error) {
      errorMessage = errorText(error, "Invoice could not be issued.");
    } finally {
      saving = false;
    }
  }

  async function voidInvoice(invoice: Invoice): Promise<void> {
    if (saving) return;
    if (
      !window.confirm(
        `Void ${invoice.number}? Its number, payments, and audit history will be preserved.`,
      )
    ) {
      return;
    }
    saving = true;
    errorMessage = "";
    try {
      const updated = await service.voidInvoice(invoice.id);
      invoices = invoices.map((item) =>
        item.id === updated.id ? updated : item,
      );
      successMessage = `${updated.number} was voided.`;
    } catch (error) {
      errorMessage = errorText(error, "Invoice could not be voided.");
    } finally {
      saving = false;
    }
  }

  // A draft can never be voided, and it holds its milestone while it exists,
  // so discarding is the only way to release a milestone from an abandoned
  // draft. Projects are reloaded so the milestone picker sees it freed.
  async function discardDraft(invoice: Invoice): Promise<void> {
    if (saving) return;
    if (
      !window.confirm(
        `Discard draft ${invoice.number}? It is not kept in the invoice history, and its milestone becomes billable again.`,
      )
    ) {
      return;
    }
    saving = true;
    errorMessage = "";
    try {
      await service.deleteDraftInvoice(invoice.id);
      invoices = invoices.filter((item) => item.id !== invoice.id);
      if (editingInvoiceId === invoice.id) {
        showInvoiceForm = false;
        resetInvoiceForm();
      }
      projects = await service.listInvoiceProjects();
      successMessage = `Draft ${invoice.number} was discarded.`;
    } catch (error) {
      errorMessage = errorText(error, "Draft could not be discarded.");
    } finally {
      saving = false;
    }
  }

  function startPayment(invoice: Invoice): void {
    const totals = invoiceTotals(invoice);
    paymentInvoiceId = invoice.id;
    paymentAmount = minorInput(totals.balanceDueMinor);
    paymentDate = today;
    paymentNote = "";
  }

  async function savePayment(): Promise<void> {
    if (!paymentInvoice || saving) return;
    errorMessage = "";
    try {
      const amountMinor = parseMinorUnits(paymentAmount, "Payment");
      const balance = invoiceTotals(paymentInvoice).balanceDueMinor;
      const allowOverpayment =
        amountMinor > balance &&
        window.confirm(
          "This payment exceeds the balance. Keep the overpayment as invoice credit?",
        );
      if (amountMinor > balance && !allowOverpayment) {
        return;
      }
      saving = true;
      const updated = await service.recordInvoicePayment(paymentInvoice.id, {
        amountMinor,
        receivedDate: paymentDate,
        note: paymentNote,
        allowOverpayment,
      });
      invoices = invoices.map((invoice) =>
        invoice.id === updated.id ? updated : invoice,
      );
      paymentInvoiceId = null;
      successMessage = `Payment recorded on ${updated.number}.`;
    } catch (error) {
      errorMessage = errorText(error, "Payment could not be saved.");
    } finally {
      saving = false;
    }
  }

  function currentLoanSignature(): string {
    return JSON.stringify({
      frequency: loanFrequency,
      first: loanFirstPayment,
      count: loanFrequency === "custom" ? loanScheduleDates.length : loanCount,
      dayOne: loanDayOne,
      dayTwo: loanDayTwo,
      custom: loanFrequency === "custom" ? loanScheduleDates : null,
    });
  }

  function changeLoanFrequency(): void {
    loanScheduleDates =
      loanFrequency === "custom" ? [loanFirstPayment] : [];
    if (loanFrequency === "twice-monthly") {
      loanDayOne = Number(loanFirstPayment.slice(8, 10));
      if (loanDayTwo === loanDayOne) {
        loanDayTwo = loanDayOne === 28 ? 13 : 28;
      }
    }
    loanPreviewSignature = "";
  }

  function changeFirstPayment(): void {
    if (loanFrequency === "custom" && loanScheduleDates.length) {
      loanScheduleDates[0] = loanFirstPayment;
      loanScheduleDates = [...loanScheduleDates];
    }
    if (loanFrequency === "twice-monthly") {
      loanDayOne = Number(loanFirstPayment.slice(8, 10));
      if (loanDayTwo === loanDayOne) {
        loanDayTwo = loanDayOne === 28 ? 13 : 28;
      }
    }
  }

  function addCustomDate(): void {
    const last =
      loanScheduleDates[loanScheduleDates.length - 1] ?? loanFirstPayment;
    loanScheduleDates = [...loanScheduleDates, addDays(last, 30)];
  }

  function removeCustomDate(index: number): void {
    if (index === 0 || loanScheduleDates.length === 1) return;
    loanScheduleDates = loanScheduleDates.filter(
      (_date, dateIndex) => dateIndex !== index,
    );
  }

  function generateLoanPreview(): void {
    errorMessage = "";
    try {
      if (loanCount < 1 || loanCount > 240) {
        throw new Error("Installment count must be between 1 and 240.");
      }
      if (loanFirstPayment < loanDate) {
        throw new Error("First payment cannot be before the loan date.");
      }
      if (loanFrequency === "custom") {
        loanScheduleDates = [
          ...generateLoanDueDates({
            frequency: "custom",
            firstPaymentDate: loanFirstPayment,
            dueDates: loanScheduleDates,
          }),
        ];
      } else if (loanFrequency === "twice-monthly") {
        loanScheduleDates = [
          ...generateLoanDueDates({
            frequency: "twice-monthly",
            firstPaymentDate: loanFirstPayment,
            installmentCount: loanCount,
            paymentDays: [loanDayOne, loanDayTwo],
          }),
        ];
      } else {
        loanScheduleDates = [
          ...generateLoanDueDates({
            frequency: loanFrequency,
            firstPaymentDate: loanFirstPayment,
            installmentCount: loanCount,
          }),
        ];
      }
      loanPreviewSignature = currentLoanSignature();
    } catch (error) {
      errorMessage = errorText(
        error,
        "The loan schedule could not be generated.",
      );
    }
  }

  async function saveLoan(): Promise<void> {
    if (
      !loanScheduleDates.length ||
      scheduleIsStale ||
      saving
    ) {
      return;
    }
    saving = true;
    errorMessage = "";
    try {
      const created = await service.createPersonalLoan({
        operator: loanOperator,
        description: loanDescription,
        loanDate,
        firstPaymentDate: loanFirstPayment,
        installmentCount: loanScheduleDates.length,
        frequency: loanFrequency,
        paymentDays:
          loanFrequency === "twice-monthly"
            ? [loanDayOne, loanDayTwo]
            : null,
        dueDates: loanScheduleDates,
      });
      loans = [created, ...loans];
      showLoanForm = false;
      successMessage = `${created.operator} loan schedule was saved.`;
      resetLoanForm();
    } catch (error) {
      errorMessage = errorText(error, "Personal loan could not be saved.");
    } finally {
      saving = false;
    }
  }

  function resetLoanForm(): void {
    loanOperator = "";
    loanDescription = "";
    loanDate = today;
    loanFirstPayment = addMonthsClamped(today, 1);
    loanCount = 9;
    loanFrequency = "monthly";
    loanDayOne = Number(loanFirstPayment.slice(8, 10));
    loanDayTwo = loanDayOne === 28 ? 13 : 28;
    loanScheduleDates = [];
    loanPreviewSignature = "";
  }

  // Marking an installment paid now reuses the inline type="date" picker
  // pattern (already used for editing an already-paid installment's date)
  // instead of a raw window.prompt asking for a typed YYYY-MM-DD string.
  function beginMarkPaid(installment: { id: string; paidDate: string | null }): void {
    pendingPaidInstallmentId = installment.id;
    pendingPaidDate = installment.paidDate ?? today;
  }

  function cancelMarkPaid(): void {
    pendingPaidInstallmentId = null;
  }

  async function toggleInstallment(
    loan: PersonalLoan,
    installmentId: string,
    paid: boolean,
    paidDate: string | null = null,
  ): Promise<void> {
    if (busyInstallments.has(installmentId)) return;
    const installment = loan.installments.find(
      (item) => item.id === installmentId,
    );
    if (
      installment?.paid &&
      !paid &&
      !window.confirm("Reopen this paid installment and clear its paid date?")
    ) {
      return;
    }
    if (paid && (!paidDate || !/^\d{4}-\d{2}-\d{2}$/.test(paidDate))) {
      errorMessage = "Select a valid paid date.";
      return;
    }
    busyInstallments = new Set(busyInstallments).add(installmentId);
    errorMessage = "";
    try {
      const updated = await service.setInstallmentPaid({
        loanId: loan.id,
        installmentId,
        paid,
        paidDate: paid ? paidDate : null,
      });
      loans = loans.map((item) => (item.id === updated.id ? updated : item));
      if (paid && pendingPaidInstallmentId === installmentId) {
        pendingPaidInstallmentId = null;
      }
    } catch (error) {
      errorMessage = errorText(error, "Installment could not be updated.");
    } finally {
      const next = new Set(busyInstallments);
      next.delete(installmentId);
      busyInstallments = next;
    }
  }

  async function updateInstallmentDueDate(
    loan: PersonalLoan,
    installmentId: string,
    dueDate: string,
  ): Promise<void> {
    if (busyInstallments.has(installmentId)) return;
    busyInstallments = new Set(busyInstallments).add(installmentId);
    errorMessage = "";
    successMessage = "";
    try {
      const updated = await service.updateInstallmentDueDate({
        loanId: loan.id,
        installmentId,
        dueDate,
      });
      loans = loans.map((item) =>
        item.id === updated.id ? updated : item,
      );
      successMessage = "Installment due date updated.";
    } catch (error) {
      const message = errorText(
        error,
        "Installment due date could not be updated.",
      );
      await loadMoney();
      errorMessage = message;
    } finally {
      const next = new Set(busyInstallments);
      next.delete(installmentId);
      busyInstallments = next;
    }
  }

  async function updateInstallmentPaidDate(
    loan: PersonalLoan,
    installmentId: string,
    paidDate: string,
  ): Promise<void> {
    if (busyInstallments.has(installmentId)) return;
    busyInstallments = new Set(busyInstallments).add(installmentId);
    errorMessage = "";
    successMessage = "";
    try {
      const updated = await service.setInstallmentPaid({
        loanId: loan.id,
        installmentId,
        paid: true,
        paidDate,
      });
      loans = loans.map((item) =>
        item.id === updated.id ? updated : item,
      );
      successMessage = "Installment paid date updated.";
    } catch (error) {
      const message = errorText(error, "Paid date could not be updated.");
      await loadMoney();
      errorMessage = message;
    } finally {
      const next = new Set(busyInstallments);
      next.delete(installmentId);
      busyInstallments = next;
    }
  }

  function exportInvoice(
    invoice: Invoice,
    format: InvoiceExportDetail["format"],
  ): void {
    dispatch("exportInvoice", { invoice, format });
  }

  async function loadMoneyDefaults(): Promise<void> {
    try {
      const settings = await settingsService.getSettings();
      invoiceTerm = settings.defaultInvoiceTerm;
      defaultInvoiceTerm = settings.defaultInvoiceTerm;
      invoiceCustomDueDate = getInvoiceDueDate();
    } catch {
      // The Money view remains usable with its 14-day default even if the
      // optional settings read fails.
    }
  }

  onMount(() => {
    void loadMoney();
    void loadMoneyDefaults();
    const calendarTimer = window.setInterval(() => {
      today = moneyDateUtils.localIsoDay();
    }, 60_000);
    return () => window.clearInterval(calendarTimer);
  });
</script>

<section class="money-view" aria-label="Money">
  <StatRow>
    <StatCard
      icon="invoice"
      label="Outstanding"
      value={moneyOutstanding.length ? formatMoney(moneyOutstanding[0].amountMinor, moneyOutstanding[0].currency) : "None"}
      detail={moneyOutstanding.length > 1 ? extraCurrencies(moneyOutstanding) : "Invoiced, not yet paid"}
      tone={moneyOutstanding.length ? "warning" : "neutral"}
    />
    <StatCard
      icon="arrow-up-right"
      label="Received"
      value={moneyReceived.length ? formatMoney(moneyReceived[0].amountMinor, moneyReceived[0].currency) : "None"}
      detail={moneyReceived.length > 1 ? extraCurrencies(moneyReceived) : "This month"}
      tone={moneyReceived.length ? "positive" : "neutral"}
    />
    <StatCard
      icon="clock"
      label="Overdue invoices"
      value={String(overdueInvoices)}
      detail={overdueInvoices === 0 ? "All current" : "Past due date"}
      tone={overdueInvoices > 0 ? "danger" : "neutral"}
    />
  </StatRow>

  <div class="notice-region" bind:this={noticeRegion}>
    {#if errorMessage}
      <div class="notice error" role="alert">
        <span>{errorMessage}</span>
        <button type="button" aria-label="Dismiss error" on:click={() => (errorMessage = "")}>×</button>
      </div>
    {/if}
    {#if successMessage}
      <div class="notice success" role="status">
        <span>{successMessage}</span>
        <button type="button" aria-label="Dismiss message" on:click={() => (successMessage = "")}>×</button>
      </div>
    {/if}
  </div>

  <div class="segmented" role="tablist" aria-label="Money sections">
    <button
      id="money-tab-invoices"
      role="tab"
      aria-selected={activeTab === "invoices"}
      aria-controls="money-panel-invoices"
      class:active={activeTab === "invoices"}
      type="button"
      on:click={() => (activeTab = "invoices")}
    >Invoices <span class="soft-badge">{invoices.length}</span></button>
    <button
      id="money-tab-earnings"
      role="tab"
      aria-selected={activeTab === "earnings"}
      aria-controls="money-panel-earnings"
      class:active={activeTab === "earnings"}
      type="button"
      on:click={() => (activeTab = "earnings")}
    >Earnings <span class="soft-badge">{paymentRows.length}</span></button>
    <button
      id="money-tab-loans"
      role="tab"
      aria-selected={activeTab === "loans"}
      aria-controls="money-panel-loans"
      class:active={activeTab === "loans"}
      type="button"
      on:click={() => (activeTab = "loans")}
    >Personal loans <span class="soft-badge">{loans.length}</span></button>
  </div>

  {#if activeTab === "invoices"}
    <div
      id="money-panel-invoices"
      role="tabpanel"
      aria-labelledby="money-tab-invoices"
      class="tab-panel"
    >
      <p class="tab-intro">Client earnings and personal loans stay deliberately separate.</p>
      <div class="page-actions">
        <button
          class="primary-button"
          type="button"
          disabled={!projects.length}
          title={projects.length ? "" : "Create a project in Work first"}
          on:click={openInvoiceComposer}
        >
          <Icon name="invoice" size={15} /> New invoice
        </button>
      </div>

      {#if showInvoiceForm}
        <form
          class="composer"
          bind:this={composerFormEl}
          on:submit|preventDefault={() => saveInvoice("draft")}
          transition:slide={{ duration: motionDuration(180) }}
        >
          <div class="composer-head">
            <div>
              <span class="eyebrow">Milestone billing</span>
              <h3>{editingInvoiceId ? "Edit draft invoice" : "Create invoice"}</h3>
            </div>
            <span class="number-chip">{invoiceNumberPreview}</span>
          </div>
          <div class="form-grid">
            <label class="span-2">
              <span class="field-label">Project</span>
              <select
                class="field-input"
                bind:value={invoiceProjectId}
                bind:this={composerFirstFieldEl}
                required
                on:change={applyProjectDefaults}
              >
                <option value="" disabled>Select project</option>
                {#each projects as project}
                  <option value={project.id}>{project.clientName} · {project.name}</option>
                {/each}
              </select>
            </label>
            <label>
              <span class="field-label">Milestone</span>
              <select class="field-input" bind:value={invoiceMilestoneId} on:change={applyMilestoneDefaults}>
                <option value="">Custom invoice</option>
                {#if selectedProject?.milestones.length}
                  {#each selectedProject.milestones as milestone}
                    <option
                      value={milestone.id}
                      disabled={milestone.status !== "not-invoiced" &&
                        milestone.id !== editingMilestoneId}
                    >
                      {milestone.label} — {formatMoney(milestone.amountMinor, selectedProject?.currency)} ({milestone.status})
                    </option>
                  {/each}
                {/if}
              </select>
            </label>
            <label>
              <span class="field-label">Issue date</span>
              <input class="field-input" type="date" bind:value={invoiceIssueDate} required />
            </label>
            <label>
              <span class="field-label">Payment term</span>
              <select class="field-input" bind:value={invoiceTerm}>
                <option value="immediate">Due immediately</option>
                <option value="7-days">7 days</option>
                <option value="14-days">14 days</option>
                <option value="30-days">30 days</option>
                <option value="custom">Custom date</option>
              </select>
            </label>
            {#if invoiceTerm === "custom"}
              <label>
                <span class="field-label">Custom due date</span>
                <input class="field-input" type="date" min={invoiceIssueDate} bind:value={invoiceCustomDueDate} required />
              </label>
            {:else}
              <div class="read-field"><span class="field-label">Due date</span><strong class="field-input">{formatDate(invoiceDueDate)}</strong></div>
            {/if}
          </div>

          <fieldset class="line-items">
            <legend class="field-label">Line items</legend>
            <div class="line-labels" aria-hidden="true">
              <span>Description</span><span>Qty</span><span>Unit price</span><span></span>
            </div>
            {#each invoiceLines as line (line.id)}
              <div class="line-row">
                <label>
                  <span class="sr-only">Description</span>
                  <input class="field-input" bind:value={line.description} maxlength="180" placeholder="Milestone or deliverable" required />
                </label>
                <label>
                  <span class="sr-only">Quantity</span>
                  <input class="field-input" bind:value={line.quantity} inputmode="decimal" aria-label="Quantity" required />
                </label>
                <label>
                  <span class="sr-only">Unit price</span>
                  <span class="money-input"><i>{selectedProject?.currency ?? "USD"}</i><input class="field-input" bind:value={line.unitPrice} inputmode="decimal" aria-label="Unit price" required /></span>
                </label>
                <button
                  class="icon-button"
                  type="button"
                  disabled={invoiceLines.length === 1}
                  aria-label="Remove line item"
                  on:click={() => removeInvoiceLine(line.id)}
                >×</button>
              </div>
            {/each}
            <button class="text-button" type="button" on:click={addInvoiceLine}>+ Add line item</button>
          </fieldset>

          <div class="adjustments">
            <label>
              <span class="field-label">Discount</span>
              <select class="field-input" bind:value={invoiceDiscountKind}>
                <option value="none">None</option>
                <option value="fixed">Fixed amount</option>
                <option value="percentage">Percentage</option>
              </select>
            </label>
            {#if invoiceDiscountKind !== "none"}
              <label transition:fade={{ duration: motionDuration(180) }}>
                <span class="field-label">{invoiceDiscountKind === "fixed" ? "Discount amount" : "Discount %"}</span>
                <input class="field-input" bind:value={invoiceDiscountValue} inputmode="decimal" required />
              </label>
            {/if}
            <label>
              <span class="field-label">Tax %</span>
              <input class="field-input" bind:value={invoiceTax} inputmode="decimal" placeholder="Optional" />
            </label>
          </div>

          <div class="form-grid notes-grid">
            <label><span class="field-label">Notes</span><textarea class="field-textarea" bind:value={invoiceNotes} rows="2" placeholder="Optional invoice note"></textarea></label>
            <label><span class="field-label">Payment instructions</span><textarea class="field-textarea" bind:value={invoiceInstructions} rows="2" placeholder="Bank, PayPal, or other instructions"></textarea></label>
          </div>
          <div class="totals" aria-live="polite">
            <span>Subtotal <b>{draftTotals ? formatMoney(draftTotals.subtotalMinor, selectedProject?.currency) : "—"}</b></span>
            <span>Discount <b>−{draftTotals ? formatMoney(draftTotals.discountMinor, selectedProject?.currency) : "—"}</b></span>
            <span>Tax <b>{draftTotals ? formatMoney(draftTotals.taxMinor, selectedProject?.currency) : "—"}</b></span>
            <strong>Total <b>{draftTotals ? formatMoney(draftTotals.totalMinor, selectedProject?.currency) : "Check values"}</b></strong>
          </div>
          <div class="form-actions">
            <button
              class="secondary-button"
              type="button"
              on:click={() => {
                showInvoiceForm = false;
                resetInvoiceForm();
              }}
            >Cancel</button>
            <button
              class="secondary-button"
              type="submit"
              disabled={saving || !selectedProject || !draftTotals}
            >{saving ? "Saving…" : "Save draft"}</button>
            <button
              class="primary-button"
              type="button"
              disabled={saving || !selectedProject || !draftTotals}
              on:click={confirmIssueFromComposer}
            >
              {saving ? "Saving…" : editingInvoiceId ? "Save & issue" : "Create & issue"}
            </button>
          </div>
        </form>
      {/if}

      {#if loading}
        <div class="loading-state" aria-live="polite">Loading invoices…</div>
      {:else if !projects.length && !invoices.length}
        <div class="empty-state">
          <span class="empty-icon"><Icon name="invoice" size={21} /></span><h3>Create a project first</h3>
          <p>Invoices live inside project containers. Add a client and project in Work, then return here.</p>
        </div>
      {:else if !invoices.length}
        <div class="empty-state">
          <span class="empty-icon"><Icon name="invoice" size={21} /></span><h3>No invoices yet</h3>
          <p>Create a kickoff or completion invoice from one of your projects.</p>
          <button class="primary-button" type="button" on:click={openInvoiceComposer}>Create first invoice</button>
        </div>
      {:else}
        <Card>
          <SectionHeader
            slot="header"
            title={`${invoices.length} ${invoices.length === 1 ? "invoice" : "invoices"}`}
          />
          <div class="invoice-list">
          {#each invoices as invoice (invoice.id)}
            {@const totals = invoiceTotals(invoice)}
            {@const status = effectiveInvoiceStatus(invoice)}
            <article class="invoice-card">
              <div class="invoice-main">
                <div class="invoice-identity">
                  <span class:overdue={status === "overdue"} class={`status ${status}`}>{statusLabel(status)}</span>
                  <strong>{invoice.number}</strong>
                  <small>{invoice.clientName} · {invoice.projectName}{invoice.milestoneLabel ? ` · ${invoice.milestoneLabel}` : ""}</small>
                </div>
                <div class="invoice-dates">
                  <span>Issued <b>{formatDate(invoice.issueDate)}</b></span>
                  <span>Due <b>{formatDate(invoice.dueDate)}</b></span>
                </div>
                <div class="invoice-amount">
                  <strong>{formatMoney(totals.totalMinor, invoice.currency)}</strong>
                  {#if totals.creditMinor > 0}
                    <span>{formatMoney(totals.creditMinor, invoice.currency)} credit</span>
                  {:else}
                    <span>{formatMoney(totals.balanceDueMinor, invoice.currency)} due</span>
                  {/if}
                </div>
              </div>

              {#if paymentInvoiceId === invoice.id}
                <form
                  class="payment-form"
                  on:submit|preventDefault={savePayment}
                  transition:slide={{ duration: motionDuration(180) }}
                >
                  <label><span class="field-label">Payment amount</span><input class="field-input" bind:value={paymentAmount} inputmode="decimal" required /></label>
                  <label><span class="field-label">Received date</span><input class="field-input" type="date" bind:value={paymentDate} required /></label>
                  <label class="grow"><span class="field-label">Note / reference</span><input class="field-input" bind:value={paymentNote} placeholder="Optional" /></label>
                  <button class="primary-button small" type="submit" disabled={saving}>{saving ? "Saving…" : "Record"}</button>
                  <button class="secondary-button small" type="button" on:click={() => (paymentInvoiceId = null)}>Cancel</button>
                </form>
              {/if}

              <footer>
                <span>
                  {invoice.payments.length} {invoice.payments.length === 1 ? "payment" : "payments"} · {formatMoney(totals.paidMinor, invoice.currency)} collected
                  {totals.creditMinor > 0 ? ` · ${formatMoney(totals.creditMinor, invoice.currency)} credit` : ""}
                </span>
                <div>
                  {#if status === "draft"}
                    <button class="text-button" type="button" on:click={() => editDraftInvoice(invoice)}>Edit draft</button>
                    <button class="text-button" type="button" disabled={saving} on:click={() => issueDraft(invoice)}>Issue</button>
                    <button class="text-button danger" type="button" disabled={saving} on:click={() => discardDraft(invoice)}>Discard draft</button>
                  {/if}
                  {#if !["draft", "void", "paid"].includes(status)}
                    <button class="text-button" type="button" on:click={() => startPayment(invoice)}>Record payment</button>
                  {/if}
                  {#if ["issued", "partially-paid", "overdue"].includes(status)}
                    <button class="text-button danger" type="button" disabled={saving} on:click={() => voidInvoice(invoice)}>Void</button>
                  {/if}
                  <button class="text-button" type="button" on:click={() => exportInvoice(invoice, "print")}>Print</button>
                  <button class="secondary-button small" type="button" on:click={() => exportInvoice(invoice, "pdf")}>Export PDF</button>
                </div>
              </footer>
            </article>
          {/each}
          </div>
        </Card>
      {/if}
    </div>
  {:else if activeTab === "earnings"}
    <div
      id="money-panel-earnings"
      role="tabpanel"
      aria-labelledby="money-tab-earnings"
      class="tab-panel"
    >
      <Card>
        <SectionHeader
          slot="header"
          title={`${paymentRows.length} ${paymentRows.length === 1 ? "payment" : "payments"}`}
          subtext={earningsSubtext}
        />
        {#if paymentRows.length}
          <div class="payment-list">
            {#each paymentRows as row (row.payment.id)}
              <article>
                <span class="payment-mark">↙</span>
                <div><strong>{row.invoice.clientName}</strong><small>{row.invoice.projectName} · {row.invoice.number}</small></div>
                <time datetime={row.payment.receivedDate}>{formatDate(row.payment.receivedDate)}</time>
                <b>{formatMoney(row.payment.amountMinor, row.invoice.currency)}</b>
                <small>{row.payment.note ?? "Payment received"}</small>
              </article>
            {/each}
          </div>
        {:else}
          <div class="empty-state nested"><span class="empty-icon"><Icon name="money" size={19} /></span><h3>No earnings recorded</h3><p>Payments recorded against invoices will appear here.</p></div>
        {/if}
      </Card>
    </div>
  {:else}
    <div
      id="money-panel-loans"
      role="tabpanel"
      aria-labelledby="money-tab-loans"
      class="tab-panel"
    >
      <div class="page-actions">
        <button
          class="primary-button"
          type="button"
          on:click={() => {
            showLoanForm = true;
            showInvoiceForm = false;
          }}
        >
          <Icon name="loan" size={15} /> New personal loan
        </button>
      </div>

      {#if showLoanForm}
        <form class="composer" on:submit|preventDefault={saveLoan} transition:slide={{ duration: motionDuration(180) }}>
          <div class="composer-head">
            <div><span class="eyebrow">Personal obligation</span><h3>Create loan schedule</h3></div>
            <span class="separation-chip">Never linked to clients</span>
          </div>
          <div class="form-grid">
            <label class="span-2"><span class="field-label">Lender / operator</span><input class="field-input" bind:value={loanOperator} maxlength="120" placeholder="Operator name" required /></label>
            <label><span class="field-label">Loan date</span><input class="field-input" type="date" bind:value={loanDate} required /></label>
            <label><span class="field-label">Explicit first payment</span><input class="field-input" type="date" min={loanDate} bind:value={loanFirstPayment} on:change={changeFirstPayment} required /></label>
            <label>
              <span class="field-label">Frequency</span>
              <select class="field-input" bind:value={loanFrequency} on:change={changeLoanFrequency}>
                <option value="monthly">Monthly</option>
                <option value="weekly">Weekly</option>
                <option value="every-two-weeks">Every two weeks</option>
                <option value="twice-monthly">Twice monthly</option>
                <option value="custom">Custom dates</option>
              </select>
            </label>
            {#if loanFrequency !== "custom"}
              <label><span class="field-label">Number of installments</span><input class="field-input" type="number" min="1" max="240" bind:value={loanCount} required /></label>
            {/if}
            {#if loanFrequency === "twice-monthly"}
              <label><span class="field-label">First day of month</span><input class="field-input" type="number" min="1" max="31" bind:value={loanDayOne} required /></label>
              <label><span class="field-label">Second day of month</span><input class="field-input" type="number" min="1" max="31" bind:value={loanDayTwo} required /></label>
            {/if}
            <label class="span-2"><span class="field-label">Description</span><textarea class="field-textarea" bind:value={loanDescription} rows="2" placeholder="Optional note about this loan"></textarea></label>
          </div>

          {#if loanFrequency === "custom"}
            <fieldset class="custom-dates" transition:slide={{ duration: motionDuration(180) }}>
              <legend class="field-label">Custom due dates</legend>
              {#each loanScheduleDates as date, index}
                <div>
                  <label><span class="field-label">Installment {index + 1}</span><input class="field-input" type="date" bind:value={loanScheduleDates[index]} required /></label>
                  <button class="icon-button" type="button" disabled={index === 0} aria-label={`Remove installment ${index + 1}`} on:click={() => removeCustomDate(index)}>×</button>
                </div>
              {/each}
              <button class="text-button" type="button" on:click={addCustomDate}>+ Add due date</button>
            </fieldset>
          {/if}

          <div class="preview-toolbar">
            <button class="secondary-button" type="button" on:click={generateLoanPreview}>
              {loanScheduleDates.length ? "Refresh schedule preview" : "Generate schedule preview"}
            </button>
            {#if scheduleIsStale}<span role="status">Inputs changed · refresh before saving</span>{/if}
          </div>

          {#if loanScheduleDates.length && !scheduleIsStale}
            <div class="schedule-preview" transition:slide={{ duration: motionDuration(180) }}>
              <div><strong>{loanScheduleDates.length} installments</strong><span>First: {formatDate(loanScheduleDates[0])} · Last: {formatDate(loanScheduleDates[loanScheduleDates.length - 1] ?? "")}</span></div>
              <ol>
                {#each loanScheduleDates as date, index}
                  <li><span>{String(index + 1).padStart(2, "0")}</span><time datetime={date}>{formatDate(date)}</time></li>
                {/each}
              </ol>
            </div>
          {/if}
          <div class="form-actions">
            <button class="secondary-button" type="button" on:click={() => (showLoanForm = false)}>Cancel</button>
            <button class="primary-button" type="submit" disabled={saving || !loanScheduleDates.length || scheduleIsStale}>
              {saving ? "Saving…" : "Save personal loan"}
            </button>
          </div>
        </form>
      {/if}

      {#if loading}
        <div class="loading-state">Loading personal loans…</div>
      {:else if !loans.length}
        <div class="empty-state">
          <span class="empty-icon"><Icon name="loan" size={21} /></span><h3>No personal loans</h3>
          <p>Track due dates and paid status without mixing personal debt into client earnings.</p>
          <button class="primary-button" type="button" on:click={() => (showLoanForm = true)}>Add personal loan</button>
        </div>
      {:else}
        <div class="loan-list">
          {#each loans as loan (loan.id)}
            {@const unpaid = loan.installments.filter((item) => !item.paid)}
            <details class="loan-card">
              <summary>
                <span class="loan-icon">L</span>
                <div><strong>{loan.operator}</strong><small>{loan.description ?? `${loan.installmentCount} installment schedule`}</small></div>
                <div class="loan-next"><span>{unpaid.length ? "Next due" : "Complete"}</span><strong>{unpaid.length ? formatDate(unpaid[0].dueDate) : "All paid"}</strong></div>
                <b>{loan.installments.length - unpaid.length}/{loan.installments.length}</b>
              </summary>
              <div class="loan-meta">
                <span>Loan date <b>{formatDate(loan.loanDate)}</b></span>
                <span>First payment <b>{formatDate(loan.firstPaymentDate)}</b></span>
                <span>Frequency <b>{loan.frequency.replace(/-/g, " ")}</b></span>
              </div>
              <ol class="installment-list">
                {#each loan.installments as installment (installment.id)}
                  <li
                    class:paid={installment.paid}
                    class:overdue={!installment.paid && installment.dueDate < today}
                  >
                    <button
                      type="button"
                      class="check"
                      aria-pressed={installment.paid}
                      aria-label={`${installment.paid ? "Reopen" : pendingPaidInstallmentId === installment.id ? "Confirm paid date for" : "Mark paid"} installment ${installment.installmentNumber}`}
                      disabled={busyInstallments.has(installment.id)}
                      on:click={() =>
                        installment.paid
                          ? toggleInstallment(loan, installment.id, false)
                          : pendingPaidInstallmentId === installment.id
                            ? toggleInstallment(loan, installment.id, true, pendingPaidDate)
                            : beginMarkPaid(installment)}
                    >{installment.paid ? "✓" : ""}</button>
                    <span>Installment {installment.installmentNumber}</span>
                    <label class="inline-date">
                      <span class="sr-only">Due date</span>
                      <input
                        class="field-input"
                        type="date"
                        value={installment.dueDate}
                        disabled={busyInstallments.has(installment.id)}
                        aria-label={`Due date for installment ${installment.installmentNumber}`}
                        on:change={(event) =>
                          updateInstallmentDueDate(
                            loan,
                            installment.id,
                            event.currentTarget.value,
                          )}
                      />
                    </label>
                    {#if installment.paid}
                      <label class="inline-date">
                        <span class="sr-only">Paid date</span>
                        <input
                          class="field-input"
                          type="date"
                          max={today}
                          value={installment.paidDate ?? today}
                          disabled={busyInstallments.has(installment.id)}
                          aria-label={`Paid date for installment ${installment.installmentNumber}`}
                          on:change={(event) =>
                            updateInstallmentPaidDate(
                              loan,
                              installment.id,
                              event.currentTarget.value,
                            )}
                        />
                      </label>
                    {:else if pendingPaidInstallmentId === installment.id}
                      <label class="inline-date">
                        <span class="sr-only">Paid date</span>
                        <input
                          class="field-input"
                          type="date"
                          max={today}
                          value={pendingPaidDate}
                          disabled={busyInstallments.has(installment.id)}
                          aria-label={`Paid date for installment ${installment.installmentNumber}`}
                          on:change={(event) =>
                            toggleInstallment(
                              loan,
                              installment.id,
                              true,
                              event.currentTarget.value,
                            )}
                          on:keydown={(event) => {
                            if (event.key === "Escape") cancelMarkPaid();
                          }}
                        />
                      </label>
                    {:else}
                      <small>{installment.dueDate < today ? "Overdue" : "Unpaid"}</small>
                    {/if}
                  </li>
                {/each}
              </ol>
            </details>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  .money-view {
    color: var(--text-primary);
    width: 100%;
    max-width: 1120px;
    margin: 0 auto;
  }
  .composer-head, .invoice-main, .invoice-card footer, .preview-toolbar {
    display: flex; align-items: center; justify-content: space-between; gap: 18px;
  }
  .composer-head h3 { margin: 0; }
  .tab-intro { color: var(--text-secondary); font-size: 13px; margin: 0 0 14px; }
  /* .primary-button / .secondary-button / .text-button are shared (app.css);
     .small is a local compact modifier for the dense payment/footer rows,
     and .icon-button (the × remove-line controls) has no shared equivalent. */
  .small { padding: 7px var(--space-3); font-size: var(--text-11); }
  .text-button.danger { color: var(--danger); }
  .icon-button {
    width: 31px; height: 31px; border: 0; border-radius: var(--radius-control);
    cursor: pointer; background: transparent; color: var(--text-secondary); font-size: var(--text-17);
  }
  .notice { display: flex; justify-content: space-between; gap: 14px; padding: 10px 12px; border-radius: var(--radius-control); margin: 0 0 14px; font-size: 12px; }
  .notice.error { color: var(--danger); background: var(--danger-fill); border: 1px solid var(--danger-fill); }
  .notice.success { color: var(--accent); background: var(--accent-fill); border: 1px solid var(--accent-line); }
  .notice button { background: none; border: 0; color: inherit; cursor: pointer; }
  .segmented span { margin-left: 5px; }
  .tab-panel { display: flex; flex-direction: column; gap: var(--space-4); padding-top: 15px; }
  .composer { background: var(--surface-content); border-radius: var(--radius-panel); box-shadow: var(--shadow-raised); padding: 17px; margin-bottom: 14px; }
  .composer-head { padding-bottom: 14px; border-bottom: 1px solid var(--separator); margin-bottom: 14px; }
  .composer-head h3 { font-size: 17px; margin-top: 3px; }
  .number-chip, .separation-chip { color: var(--accent); background: var(--accent-fill); border: 1px solid var(--accent-line); padding: 7px 9px; border-radius: var(--radius-control); font-weight: var(--weight-bold); font-size: var(--text-11); font-family: var(--font-mono); }
  .form-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 11px; }
  .form-grid .span-2 { grid-column: span 2; }
  /* Field labels, inputs, selects and textareas use the shared .field-label /
     .field-input / .field-textarea classes (app.css) instead of restyling the
     bare elements; only layout and density stay local to this view. */
  label, .read-field { display: flex; flex-direction: column; gap: var(--space-2); min-width: 0; }
  select.field-input { color-scheme: dark; }
  /* .field-input's box (background/border/padding/height) only renders on a
     block-level box; <strong> is inline by default, so it still needs an
     explicit display here. */
  .read-field strong { display: flex; align-items: center; }
  fieldset { border: 0; padding: 0; margin: 15px 0 0; }
  .line-labels, .line-row { display: grid; grid-template-columns: minmax(220px, 1fr) 75px 150px 34px; gap: 7px; align-items: center; }
  .line-labels { color: var(--text-tertiary); font-size: var(--text-11); padding: 0 4px 4px; }
  .line-row { margin-bottom: 6px; }
  .money-input { display: flex; align-items: center; position: relative; }
  .money-input i { position: absolute; left: 8px; color: var(--text-tertiary); font-weight: var(--weight-bold); font-size: var(--text-11); font-family: var(--font-mono); }
  .money-input input { padding-left: 36px; }
  .adjustments { display: flex; align-items: end; gap: 10px; padding: 13px 0; border-bottom: 1px solid var(--separator); }
  .adjustments label { width: 180px; }
  .notes-grid { grid-template-columns: 1fr 1fr; margin-top: 13px; }
  .totals { width: min(360px, 100%); margin: 14px 0 0 auto; display: grid; gap: 6px; }
  .totals > span, .totals > strong { display: flex; justify-content: space-between; font-size: 11px; color: var(--text-tertiary); }
  .totals b { color: var(--text-primary); }
  .totals > strong { color: var(--text-primary); border-top: 1px solid var(--separator-strong); padding-top: 8px; font-size: 13px; }
  .form-actions { display: flex; justify-content: flex-end; gap: 8px; border-top: 1px solid var(--separator); margin-top: 14px; padding-top: 13px; }
  .invoice-list { display: grid; gap: var(--space-2); }
  .loan-list { display: grid; gap: 9px; }
  /* .invoice-card rows sit inside the invoice-list Card, so they take the
     nested-row treatment: a lighter rung on the surface ramp, tighter radius,
     no shadow (an identically-shadowed slab against its own Card would be
     invisible). .loan-card sits directly on --surface-window with no
     enclosing Card, so it keeps the top-level Card treatment. */
  .invoice-card { background: var(--surface-raised); border-radius: var(--radius-control); }
  .loan-card { background: var(--surface-content); border-radius: var(--radius-panel); box-shadow: var(--shadow-raised); }
  .invoice-main { padding: 13px 14px; }
  .invoice-identity { display: grid; grid-template-columns: auto 1fr; align-items: center; gap: 5px 9px; min-width: 240px; }
  .invoice-identity small { grid-column: 1 / -1; }
  .status { border-radius: var(--radius-pill); padding: 3px 6px; font-size: var(--text-11); font-weight: var(--weight-bold); letter-spacing: .06em; text-transform: uppercase; background: var(--surface-active); color: var(--text-secondary); }
  .status.paid { background: var(--accent-fill); color: var(--accent); }
  .status.overdue { background: var(--danger-fill); color: var(--danger); }
  .status.partially-paid { background: var(--amber-fill); color: var(--amber); }
  .invoice-identity strong { font-weight: var(--weight-bold); font-size: var(--text-12); font-family: var(--font-mono); }
  .invoice-identity small, .invoice-dates span, .invoice-amount span, .invoice-card footer > span { color: var(--text-tertiary); font-size: var(--text-11); }
  .invoice-dates { display: flex; gap: 18px; }
  .invoice-dates span, .invoice-amount { display: flex; flex-direction: column; gap: 3px; }
  .invoice-dates b { color: var(--text-primary); font-weight: var(--weight-semibold); }
  .invoice-amount { text-align: right; min-width: 135px; }
  .invoice-amount strong { font-size: var(--text-17); }
  .invoice-card footer { border-top: 1px solid var(--separator); padding: 8px 12px; }
  .invoice-card footer div { display: flex; align-items: center; gap: 5px; }
  .payment-form { display: flex; align-items: end; gap: 8px; background: var(--surface-window); border-top: 1px solid var(--separator); padding: 11px 13px; }
  .payment-form label { width: 145px; }
  .payment-form label.grow { flex: 1; }
  .payment-form input { min-height: 32px; padding: 6px 8px; }
  .payment-list { display: grid; gap: var(--space-2); }
  /* Nested inside the earnings Card: same lighter-rung, no-shadow treatment
     as .invoice-card above. */
  .payment-list article { display: grid; grid-template-columns: 32px minmax(180px, 1fr) 130px 120px minmax(120px, .7fr); gap: 10px; align-items: center; border-radius: var(--radius-control); background: var(--surface-raised); padding: 11px 13px; font-size: 11px; }
  .payment-mark { width: 26px; height: 26px; display: grid; place-items: center; border-radius: var(--radius-control); color: var(--accent); background: var(--accent-fill); }
  .payment-list div { display: flex; flex-direction: column; gap: 2px; }
  .payment-list small, .payment-list time { color: var(--text-tertiary); }
  .payment-list > article > b { text-align: right; color: var(--accent); }
  .preview-toolbar { margin-top: 14px; }
  .preview-toolbar span { color: var(--amber); font-size: var(--text-11); }
  .custom-dates { display: grid; gap: 6px; }
  .custom-dates > div { display: flex; align-items: end; gap: 5px; max-width: 300px; }
  .custom-dates label { flex: 1; }
  .schedule-preview { margin-top: 12px; border: 1px solid var(--separator); background: var(--surface-window); border-radius: var(--radius-control); padding: 11px; }
  .schedule-preview > div { display: flex; justify-content: space-between; color: var(--text-tertiary); font-size: var(--text-11); }
  .schedule-preview > div strong { color: var(--text-primary); }
  .schedule-preview ol { list-style: none; display: grid; grid-template-columns: repeat(4, 1fr); gap: 5px; max-height: 190px; overflow: auto; margin: 10px 0 0; padding: 0; }
  .schedule-preview li { display: flex; gap: 7px; align-items: center; border: 1px solid var(--separator); border-radius: var(--radius-control); padding: 6px; font-size: var(--text-11); }
  .schedule-preview li span { color: var(--accent); font-family: var(--font-mono); }
  .loan-card { overflow: hidden; }
  .loan-card summary { list-style: none; display: grid; grid-template-columns: 32px minmax(220px, 1fr) 160px 50px; align-items: center; gap: 11px; cursor: pointer; padding: 13px 14px; }
  .loan-card summary::-webkit-details-marker { display: none; }
  .loan-icon { width: 29px; height: 29px; display: grid; place-items: center; border-radius: var(--radius-control); color: var(--amber); background: var(--amber-fill); font-weight: var(--weight-bold); font-size: var(--text-11); font-family: var(--font-mono); }
  .loan-card summary div { display: flex; flex-direction: column; gap: 3px; }
  .loan-card summary small, .loan-next span { color: var(--text-tertiary); font-size: var(--text-11); }
  .loan-card summary > b { color: var(--accent); font-size: 12px; text-align: right; }
  .loan-meta { display: flex; gap: 25px; padding: 9px 14px; background: var(--surface-window); border-top: 1px solid var(--separator); border-bottom: 1px solid var(--separator); }
  .loan-meta span { display: flex; flex-direction: column; gap: 3px; color: var(--text-tertiary); font-size: var(--text-11); }
  .loan-meta b { color: var(--text-primary); text-transform: capitalize; }
  .installment-list { list-style: none; margin: 0; padding: 0; max-height: 330px; overflow: auto; }
  .installment-list li { display: grid; grid-template-columns: 28px 1fr 150px 100px; align-items: center; gap: 8px; border-bottom: 1px solid var(--separator); padding: 8px 14px; font-size: var(--text-11); }
  .installment-list li:last-child { border: 0; }
  .installment-list li.paid { color: var(--text-tertiary); }
  .installment-list small { color: var(--text-tertiary); text-align: right; }
  /* Danger means overdue, and the row already says "Overdue" in words. A merely
     unpaid installment due months out stays quiet. */
  .installment-list li.overdue small { color: var(--danger); }
  .inline-date input { height: 28px; padding: 4px 7px; font-size: var(--text-11); }
  .check { width: 21px; height: 21px; border: 1px solid var(--separator-strong); background: var(--surface-window); border-radius: var(--radius-control); color: var(--on-accent); cursor: pointer; padding: 0; }
  .check[aria-pressed="true"] { background: var(--accent); border-color: var(--accent); }
  /* Default: top-level slab directly on --surface-window (invoice-tab and
     loans-tab empty/loading states) — keeps the Card treatment. */
  .empty-state, .loading-state { display: flex; flex-direction: column; align-items: center; text-align: center; padding: 55px 20px; background: var(--surface-content); border-radius: var(--radius-panel); box-shadow: var(--shadow-raised); color: var(--text-secondary); }
  /* .nested: used where the empty state renders inside an already-elevated
     Card (the earnings tab) — same lighter-rung, no-shadow treatment as
     .invoice-card / .payment-list article. */
  .empty-state.nested, .loading-state.nested { background: var(--surface-raised); border-radius: var(--radius-control); box-shadow: none; }
  .empty-state h3 { color: var(--text-primary); margin: 12px 0 3px; font-size: 15px; }
  .empty-state p { max-width: 440px; margin: 0 0 14px; font-size: 11px; line-height: 1.6; }
  .loading-state { font-size: 11px; }
  .sr-only { position: absolute !important; width: 1px !important; height: 1px !important; padding: 0 !important; margin: -1px !important; overflow: hidden !important; clip: rect(0, 0, 0, 0) !important; white-space: nowrap !important; border: 0 !important; }
  @media (max-width: 900px) {
    .form-grid { grid-template-columns: 1fr 1fr; }
    .invoice-main { align-items: flex-start; flex-wrap: wrap; }
    .payment-list article { grid-template-columns: 32px 1fr auto; }
    .payment-list article > small { grid-column: 2 / -1; }
    .schedule-preview ol { grid-template-columns: repeat(2, 1fr); }
  }
  @media (max-width: 620px) {
    .invoice-card footer, .payment-form { align-items: stretch; flex-direction: column; }
    .form-grid, .notes-grid { grid-template-columns: 1fr; }
    .form-grid .span-2 { grid-column: auto; }
    .line-labels { display: none; }
    .line-row { grid-template-columns: 1fr 70px 110px 30px; }
    .adjustments { align-items: stretch; flex-direction: column; }
    .adjustments label, .payment-form label { width: 100%; }
    .invoice-dates { width: 100%; }
    .invoice-amount { text-align: left; }
    .loan-card summary { grid-template-columns: 32px 1fr 45px; }
    .loan-next { grid-column: 2; }
    .installment-list li { grid-template-columns: 28px 1fr; }
    .installment-list .inline-date, .installment-list small { grid-column: 2; text-align: left; }
  }
</style>
