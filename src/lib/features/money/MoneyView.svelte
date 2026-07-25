<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
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
  $: invoiceSummaries = summarizeInvoices();
  $: paymentRows = collectPaymentRows();
  $: earningGroups = groupEarnings();
  $: unpaidInstallmentCount = loans.reduce(
    (total, loan) =>
      total + loan.installments.filter((item) => !item.paid).length,
    0,
  );
  $: scheduleIsStale =
    loanScheduleDates.length > 0 &&
    loanPreviewSignature !== currentLoanSignature();

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

  function summarizeInvoices(): Array<{
    currency: string;
    collected: number;
    outstanding: number;
  }> {
    const groups = new Map<
      string,
      { currency: string; collected: number; outstanding: number }
    >();
    for (const invoice of invoices) {
      const totals = invoiceTotals(invoice);
      const group = groups.get(invoice.currency) ?? {
        currency: invoice.currency,
        collected: 0,
        outstanding: 0,
      };
      group.collected += totals.paidMinor;
      if (
        effectiveInvoiceStatus(invoice) !== "void" &&
        effectiveInvoiceStatus(invoice) !== "draft"
      ) {
        group.outstanding += totals.balanceDueMinor;
      }
      groups.set(invoice.currency, group);
    }
    return [...groups.values()].sort((a, b) =>
      a.currency.localeCompare(b.currency),
    );
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

  async function toggleInstallment(
    loan: PersonalLoan,
    installmentId: string,
    paid: boolean,
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
    let paidDate: string | null = null;
    if (paid) {
      const selectedDate = window.prompt(
        "Paid date (YYYY-MM-DD)",
        installment?.paidDate ?? today,
      );
      if (selectedDate === null) return;
      paidDate = selectedDate.trim();
      if (!/^\d{4}-\d{2}-\d{2}$/.test(paidDate)) {
        errorMessage = "Paid date must use YYYY-MM-DD.";
        return;
      }
    }
    busyInstallments = new Set(busyInstallments).add(installmentId);
    errorMessage = "";
    try {
      const updated = await service.setInstallmentPaid({
        loanId: loan.id,
        installmentId,
        paid,
        paidDate,
      });
      loans = loans.map((item) => (item.id === updated.id ? updated : item));
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

<section class="money-view" aria-labelledby="money-heading">
  <header class="money-heading">
    <div>
      <span class="eyebrow">Invoices and obligations</span>
      <h2 id="money-heading">Money</h2>
      <p>Client earnings and personal loans stay deliberately separate.</p>
    </div>
    <div class="heading-actions">
      {#if activeTab === "invoices"}
        <button
          class="primary"
          type="button"
          disabled={!projects.length}
          title={projects.length ? "" : "Create a project in Work first"}
          on:click={openInvoiceComposer}
        >+ New invoice</button>
      {:else if activeTab === "loans"}
        <button
          class="primary"
          type="button"
          on:click={() => {
            showLoanForm = true;
            showInvoiceForm = false;
          }}
        >+ New personal loan</button>
      {/if}
    </div>
  </header>

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

  <div class="summary-row">
    <article>
      <span>Collected earnings</span>
      {#if invoiceSummaries.length}
        {#each invoiceSummaries as summary}
          <strong>{formatMoney(summary.collected, summary.currency)}</strong>
        {/each}
      {:else}<strong>$0.00</strong>{/if}
      <small>Recorded payments only</small>
    </article>
    <article>
      <span>Outstanding</span>
      {#if invoiceSummaries.length}
        {#each invoiceSummaries as summary}
          <strong>{formatMoney(summary.outstanding, summary.currency)}</strong>
        {/each}
      {:else}<strong>$0.00</strong>{/if}
      <small>Issued invoice balances</small>
    </article>
    <article class="accent">
      <span>Loan installments due</span>
      <strong>{unpaidInstallmentCount}</strong>
      <small>Paid status only · no balances</small>
    </article>
  </div>

  <div class="tabs" role="tablist" aria-label="Money sections">
    <button
      id="money-tab-invoices"
      role="tab"
      aria-selected={activeTab === "invoices"}
      aria-controls="money-panel-invoices"
      class:active={activeTab === "invoices"}
      type="button"
      on:click={() => (activeTab = "invoices")}
    >Invoices <span>{invoices.length}</span></button>
    <button
      id="money-tab-earnings"
      role="tab"
      aria-selected={activeTab === "earnings"}
      aria-controls="money-panel-earnings"
      class:active={activeTab === "earnings"}
      type="button"
      on:click={() => (activeTab = "earnings")}
    >Earnings <span>{paymentRows.length}</span></button>
    <button
      id="money-tab-loans"
      role="tab"
      aria-selected={activeTab === "loans"}
      aria-controls="money-panel-loans"
      class:active={activeTab === "loans"}
      type="button"
      on:click={() => (activeTab = "loans")}
    >Personal loans <span>{loans.length}</span></button>
  </div>

  {#if activeTab === "invoices"}
    <div
      id="money-panel-invoices"
      role="tabpanel"
      aria-labelledby="money-tab-invoices"
      class="tab-panel"
    >
      {#if showInvoiceForm}
        <form class="composer" on:submit|preventDefault={() => saveInvoice("issued")}>
          <div class="composer-head">
            <div>
              <span class="eyebrow">Milestone billing</span>
              <h3>{editingInvoiceId ? "Edit draft invoice" : "Create invoice"}</h3>
            </div>
            <span class="number-chip">{invoiceNumberPreview}</span>
          </div>
          <div class="form-grid">
            <label class="span-2">
              <span>Project</span>
              <select bind:value={invoiceProjectId} required on:change={applyProjectDefaults}>
                <option value="" disabled>Select project</option>
                {#each projects as project}
                  <option value={project.id}>{project.clientName} · {project.name}</option>
                {/each}
              </select>
            </label>
            <label>
              <span>Milestone</span>
              <select bind:value={invoiceMilestoneId} on:change={applyMilestoneDefaults}>
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
              <span>Issue date</span>
              <input type="date" bind:value={invoiceIssueDate} required />
            </label>
            <label>
              <span>Payment term</span>
              <select bind:value={invoiceTerm}>
                <option value="immediate">Due immediately</option>
                <option value="7-days">7 days</option>
                <option value="14-days">14 days</option>
                <option value="30-days">30 days</option>
                <option value="custom">Custom date</option>
              </select>
            </label>
            {#if invoiceTerm === "custom"}
              <label>
                <span>Custom due date</span>
                <input type="date" min={invoiceIssueDate} bind:value={invoiceCustomDueDate} required />
              </label>
            {:else}
              <div class="read-field"><span>Due date</span><strong>{formatDate(invoiceDueDate)}</strong></div>
            {/if}
          </div>

          <fieldset class="line-items">
            <legend>Line items</legend>
            <div class="line-labels" aria-hidden="true">
              <span>Description</span><span>Qty</span><span>Unit price</span><span></span>
            </div>
            {#each invoiceLines as line (line.id)}
              <div class="line-row">
                <label>
                  <span class="sr-only">Description</span>
                  <input bind:value={line.description} maxlength="180" placeholder="Milestone or deliverable" required />
                </label>
                <label>
                  <span class="sr-only">Quantity</span>
                  <input bind:value={line.quantity} inputmode="decimal" aria-label="Quantity" required />
                </label>
                <label>
                  <span class="sr-only">Unit price</span>
                  <span class="money-input"><i>{selectedProject?.currency ?? "USD"}</i><input bind:value={line.unitPrice} inputmode="decimal" aria-label="Unit price" required /></span>
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
              <span>Discount</span>
              <select bind:value={invoiceDiscountKind}>
                <option value="none">None</option>
                <option value="fixed">Fixed amount</option>
                <option value="percentage">Percentage</option>
              </select>
            </label>
            {#if invoiceDiscountKind !== "none"}
              <label>
                <span>{invoiceDiscountKind === "fixed" ? "Discount amount" : "Discount %"}</span>
                <input bind:value={invoiceDiscountValue} inputmode="decimal" required />
              </label>
            {/if}
            <label>
              <span>Tax %</span>
              <input bind:value={invoiceTax} inputmode="decimal" placeholder="Optional" />
            </label>
          </div>

          <div class="form-grid notes-grid">
            <label><span>Notes</span><textarea bind:value={invoiceNotes} rows="2" placeholder="Optional invoice note"></textarea></label>
            <label><span>Payment instructions</span><textarea bind:value={invoiceInstructions} rows="2" placeholder="Bank, PayPal, or other instructions"></textarea></label>
          </div>
          <div class="totals" aria-live="polite">
            <span>Subtotal <b>{draftTotals ? formatMoney(draftTotals.subtotalMinor, selectedProject?.currency) : "—"}</b></span>
            <span>Discount <b>−{draftTotals ? formatMoney(draftTotals.discountMinor, selectedProject?.currency) : "—"}</b></span>
            <span>Tax <b>{draftTotals ? formatMoney(draftTotals.taxMinor, selectedProject?.currency) : "—"}</b></span>
            <strong>Total <b>{draftTotals ? formatMoney(draftTotals.totalMinor, selectedProject?.currency) : "Check values"}</b></strong>
          </div>
          <div class="form-actions">
            <button
              class="secondary"
              type="button"
              on:click={() => {
                showInvoiceForm = false;
                resetInvoiceForm();
              }}
            >Cancel</button>
            <button
              class="secondary"
              type="button"
              disabled={saving || !selectedProject || !draftTotals}
              on:click={() => saveInvoice("draft")}
            >{saving ? "Saving…" : "Save draft"}</button>
            <button class="primary" type="submit" disabled={saving || !selectedProject || !draftTotals}>
              {saving ? "Saving…" : editingInvoiceId ? "Save & issue" : "Create & issue"}
            </button>
          </div>
        </form>
      {/if}

      {#if loading}
        <div class="loading-state" aria-live="polite">Loading invoices…</div>
      {:else if !projects.length && !invoices.length}
        <div class="empty-state">
          <span>INV</span><h3>Create a project first</h3>
          <p>Invoices live inside project containers. Add a client and project in Work, then return here.</p>
        </div>
      {:else if !invoices.length}
        <div class="empty-state">
          <span>INV</span><h3>No invoices yet</h3>
          <p>Create a kickoff or completion invoice from one of your projects.</p>
          <button class="primary" type="button" on:click={openInvoiceComposer}>Create first invoice</button>
        </div>
      {:else}
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
                <form class="payment-form" on:submit|preventDefault={savePayment}>
                  <label><span>Payment amount</span><input bind:value={paymentAmount} inputmode="decimal" required /></label>
                  <label><span>Received date</span><input type="date" bind:value={paymentDate} required /></label>
                  <label class="grow"><span>Note / reference</span><input bind:value={paymentNote} placeholder="Optional" /></label>
                  <button class="primary small" type="submit" disabled={saving}>{saving ? "Saving…" : "Record"}</button>
                  <button class="secondary small" type="button" on:click={() => (paymentInvoiceId = null)}>Cancel</button>
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
                  {/if}
                  {#if !["draft", "void", "paid"].includes(status)}
                    <button class="text-button" type="button" on:click={() => startPayment(invoice)}>Record payment</button>
                  {/if}
                  {#if ["issued", "partially-paid", "overdue"].includes(status)}
                    <button class="text-button danger" type="button" disabled={saving} on:click={() => voidInvoice(invoice)}>Void</button>
                  {/if}
                  <button class="text-button" type="button" on:click={() => exportInvoice(invoice, "print")}>Print</button>
                  <button class="secondary small" type="button" on:click={() => exportInvoice(invoice, "pdf")}>Export PDF</button>
                </div>
              </footer>
            </article>
          {/each}
        </div>
      {/if}
    </div>
  {:else if activeTab === "earnings"}
    <div
      id="money-panel-earnings"
      role="tabpanel"
      aria-labelledby="money-tab-earnings"
      class="tab-panel"
    >
      <div class="earnings-head">
        <div><span class="eyebrow">Received, not expected</span><h3>Collected earnings</h3></div>
        <div class="earning-totals">
          {#if earningGroups.length}
            {#each earningGroups as group}<strong>{formatMoney(group.total, group.currency)}</strong>{/each}
          {:else}<strong>$0.00</strong>{/if}
        </div>
      </div>
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
        <div class="empty-state"><span>USD</span><h3>No earnings recorded</h3><p>Payments recorded against invoices will appear here.</p></div>
      {/if}
    </div>
  {:else}
    <div
      id="money-panel-loans"
      role="tabpanel"
      aria-labelledby="money-tab-loans"
      class="tab-panel"
    >
      {#if showLoanForm}
        <form class="composer" on:submit|preventDefault={saveLoan}>
          <div class="composer-head">
            <div><span class="eyebrow">Personal obligation</span><h3>Create loan schedule</h3></div>
            <span class="separation-chip">Never linked to clients</span>
          </div>
          <div class="form-grid">
            <label class="span-2"><span>Lender / operator</span><input bind:value={loanOperator} maxlength="120" placeholder="Operator name" required /></label>
            <label><span>Loan date</span><input type="date" bind:value={loanDate} required /></label>
            <label><span>Explicit first payment</span><input type="date" min={loanDate} bind:value={loanFirstPayment} on:change={changeFirstPayment} required /></label>
            <label>
              <span>Frequency</span>
              <select bind:value={loanFrequency} on:change={changeLoanFrequency}>
                <option value="monthly">Monthly</option>
                <option value="weekly">Weekly</option>
                <option value="every-two-weeks">Every two weeks</option>
                <option value="twice-monthly">Twice monthly</option>
                <option value="custom">Custom dates</option>
              </select>
            </label>
            {#if loanFrequency !== "custom"}
              <label><span>Number of installments</span><input type="number" min="1" max="240" bind:value={loanCount} required /></label>
            {/if}
            {#if loanFrequency === "twice-monthly"}
              <label><span>First day of month</span><input type="number" min="1" max="31" bind:value={loanDayOne} required /></label>
              <label><span>Second day of month</span><input type="number" min="1" max="31" bind:value={loanDayTwo} required /></label>
            {/if}
            <label class="span-2"><span>Description</span><textarea bind:value={loanDescription} rows="2" placeholder="Optional note about this loan"></textarea></label>
          </div>

          {#if loanFrequency === "custom"}
            <fieldset class="custom-dates">
              <legend>Custom due dates</legend>
              {#each loanScheduleDates as date, index}
                <div>
                  <label><span>Installment {index + 1}</span><input type="date" bind:value={loanScheduleDates[index]} required /></label>
                  <button class="icon-button" type="button" disabled={index === 0} aria-label={`Remove installment ${index + 1}`} on:click={() => removeCustomDate(index)}>×</button>
                </div>
              {/each}
              <button class="text-button" type="button" on:click={addCustomDate}>+ Add due date</button>
            </fieldset>
          {/if}

          <div class="preview-toolbar">
            <button class="secondary" type="button" on:click={generateLoanPreview}>
              {loanScheduleDates.length ? "Refresh schedule preview" : "Generate schedule preview"}
            </button>
            {#if scheduleIsStale}<span role="status">Inputs changed · refresh before saving</span>{/if}
          </div>

          {#if loanScheduleDates.length && !scheduleIsStale}
            <div class="schedule-preview">
              <div><strong>{loanScheduleDates.length} installments</strong><span>First: {formatDate(loanScheduleDates[0])} · Last: {formatDate(loanScheduleDates[loanScheduleDates.length - 1] ?? "")}</span></div>
              <ol>
                {#each loanScheduleDates as date, index}
                  <li><span>{String(index + 1).padStart(2, "0")}</span><time datetime={date}>{formatDate(date)}</time></li>
                {/each}
              </ol>
            </div>
          {/if}
          <div class="form-actions">
            <button class="secondary" type="button" on:click={() => (showLoanForm = false)}>Cancel</button>
            <button class="primary" type="submit" disabled={saving || !loanScheduleDates.length || scheduleIsStale}>
              {saving ? "Saving…" : "Save personal loan"}
            </button>
          </div>
        </form>
      {/if}

      {#if loading}
        <div class="loading-state">Loading personal loans…</div>
      {:else if !loans.length}
        <div class="empty-state">
          <span>LOAN</span><h3>No personal loans</h3>
          <p>Track due dates and paid status without mixing personal debt into client earnings.</p>
          <button class="primary" type="button" on:click={() => (showLoanForm = true)}>Add personal loan</button>
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
                  <li class:paid={installment.paid}>
                    <button
                      type="button"
                      class="check"
                      aria-pressed={installment.paid}
                      aria-label={`${installment.paid ? "Reopen" : "Mark paid"} installment ${installment.installmentNumber}`}
                      disabled={busyInstallments.has(installment.id)}
                      on:click={() => toggleInstallment(loan, installment.id, !installment.paid)}
                    >{installment.paid ? "✓" : ""}</button>
                    <span>Installment {installment.installmentNumber}</span>
                    <label class="inline-date">
                      <span class="sr-only">Due date</span>
                      <input
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
    --mv-bg: #0d1210;
    --mv-panel: #121a16;
    --mv-raised: #17211c;
    --mv-border: #26332c;
    --mv-border-soft: #202b25;
    --mv-text: #eef5f0;
    --mv-muted: #8fa298;
    --mv-green: #34d17b;
    --mv-green-soft: rgba(52, 209, 123, 0.12);
    --mv-amber: #e5b85b;
    --mv-red: #ef7676;
    color: var(--mv-text);
    font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    width: 100%;
    max-width: 1240px;
    margin: 0 auto;
  }
  button, input, select, textarea { font: inherit; }
  button { color: inherit; }
  .money-heading, .composer-head, .invoice-main, .invoice-card footer, .earnings-head, .preview-toolbar {
    display: flex; align-items: center; justify-content: space-between; gap: 18px;
  }
  .money-heading { margin-bottom: 20px; }
  .money-heading h2 { font-size: 25px; margin: 3px 0 4px; letter-spacing: -0.03em; }
  .money-heading p, .composer-head h3, .earnings-head h3 { margin: 0; }
  .money-heading p { color: var(--mv-muted); font-size: 13px; }
  .eyebrow { color: var(--mv-green); font-size: 10px; font-weight: 800; letter-spacing: .12em; text-transform: uppercase; }
  .primary, .secondary, .text-button, .icon-button {
    border: 0; border-radius: 7px; cursor: pointer; font-weight: 700;
  }
  .primary { background: var(--mv-green); color: #07110b; padding: 10px 14px; }
  .secondary { background: var(--mv-raised); border: 1px solid var(--mv-border); padding: 9px 13px; }
  .primary.small, .secondary.small { padding: 7px 10px; font-size: 11px; }
  .text-button { background: transparent; color: var(--mv-green); padding: 6px; font-size: 12px; }
  .icon-button { width: 31px; height: 31px; background: transparent; color: var(--mv-muted); font-size: 18px; }
  button:hover:not(:disabled) { filter: brightness(1.08); }
  button:focus-visible, input:focus-visible, select:focus-visible, textarea:focus-visible, summary:focus-visible {
    outline: 2px solid var(--mv-green); outline-offset: 2px;
  }
  button:disabled { cursor: not-allowed; opacity: .45; }
  .notice { display: flex; justify-content: space-between; gap: 14px; padding: 10px 12px; border-radius: 7px; margin: 0 0 14px; font-size: 12px; }
  .notice.error { color: #ffb3b3; background: rgba(239,118,118,.09); border: 1px solid rgba(239,118,118,.25); }
  .notice.success { color: #9aefbe; background: var(--mv-green-soft); border: 1px solid rgba(52,209,123,.25); }
  .notice button { background: none; border: 0; color: inherit; cursor: pointer; }
  .summary-row { display: grid; grid-template-columns: repeat(3, 1fr); gap: 11px; margin-bottom: 18px; }
  .summary-row article { min-height: 95px; background: var(--mv-panel); border: 1px solid var(--mv-border-soft); border-radius: 9px; padding: 14px; display: flex; flex-direction: column; gap: 5px; }
  .summary-row article.accent { border-color: rgba(52,209,123,.24); background: linear-gradient(115deg, var(--mv-panel), rgba(52,209,123,.055)); }
  .summary-row span, .summary-row small { color: var(--mv-muted); font-size: 11px; }
  .summary-row strong { font-size: 20px; letter-spacing: -.03em; }
  .tabs { display: flex; gap: 3px; border-bottom: 1px solid var(--mv-border); }
  .tabs button { position: relative; border: 0; background: transparent; color: var(--mv-muted); padding: 11px 13px; cursor: pointer; font-weight: 700; font-size: 12px; }
  .tabs button.active { color: var(--mv-text); }
  .tabs button.active::after { content: ""; position: absolute; height: 2px; left: 10px; right: 10px; bottom: -1px; background: var(--mv-green); }
  .tabs button span { margin-left: 5px; color: var(--mv-muted); background: var(--mv-raised); border-radius: 9px; padding: 1px 6px; font-size: 9px; }
  .tab-panel { padding-top: 15px; }
  .composer { background: var(--mv-panel); border: 1px solid var(--mv-border); border-radius: 10px; padding: 17px; margin-bottom: 14px; }
  .composer-head { padding-bottom: 14px; border-bottom: 1px solid var(--mv-border-soft); margin-bottom: 14px; }
  .composer-head h3, .earnings-head h3 { font-size: 17px; margin-top: 3px; }
  .number-chip, .separation-chip { color: var(--mv-green); background: var(--mv-green-soft); border: 1px solid rgba(52,209,123,.2); padding: 7px 9px; border-radius: 6px; font: 700 10px ui-monospace, SFMono-Regular, Consolas, monospace; }
  .form-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 11px; }
  .form-grid .span-2 { grid-column: span 2; }
  label, .read-field { display: flex; flex-direction: column; gap: 5px; min-width: 0; }
  label > span, .read-field > span, legend { color: var(--mv-muted); font-size: 10px; font-weight: 750; letter-spacing: .04em; }
  input, select, textarea { width: 100%; box-sizing: border-box; color: var(--mv-text); background: #0e1511; border: 1px solid var(--mv-border); border-radius: 6px; padding: 8px 9px; min-height: 35px; }
  textarea { resize: vertical; }
  select { color-scheme: dark; }
  .read-field strong { min-height: 35px; box-sizing: border-box; display: flex; align-items: center; background: #0e1511; border: 1px solid var(--mv-border-soft); border-radius: 6px; padding: 8px 9px; font-size: 12px; }
  fieldset { border: 0; padding: 0; margin: 15px 0 0; }
  legend { margin-bottom: 7px; }
  .line-labels, .line-row { display: grid; grid-template-columns: minmax(220px, 1fr) 75px 150px 34px; gap: 7px; align-items: center; }
  .line-labels { color: var(--mv-muted); font-size: 9px; padding: 0 4px 4px; }
  .line-row { margin-bottom: 6px; }
  .money-input { display: flex; align-items: center; position: relative; }
  .money-input i { position: absolute; left: 8px; color: var(--mv-muted); font: 700 9px ui-monospace, monospace; }
  .money-input input { padding-left: 36px; }
  .adjustments { display: flex; align-items: end; gap: 10px; padding: 13px 0; border-bottom: 1px solid var(--mv-border-soft); }
  .adjustments label { width: 180px; }
  .notes-grid { grid-template-columns: 1fr 1fr; margin-top: 13px; }
  .totals { width: min(360px, 100%); margin: 14px 0 0 auto; display: grid; gap: 6px; }
  .totals > span, .totals > strong { display: flex; justify-content: space-between; font-size: 11px; color: var(--mv-muted); }
  .totals b { color: var(--mv-text); }
  .totals > strong { color: var(--mv-text); border-top: 1px solid var(--mv-border); padding-top: 8px; font-size: 13px; }
  .form-actions { display: flex; justify-content: flex-end; gap: 8px; border-top: 1px solid var(--mv-border-soft); margin-top: 14px; padding-top: 13px; }
  .invoice-list, .loan-list { display: grid; gap: 9px; }
  .invoice-card, .loan-card { background: var(--mv-panel); border: 1px solid var(--mv-border-soft); border-radius: 9px; }
  .invoice-main { padding: 13px 14px; }
  .invoice-identity { display: grid; grid-template-columns: auto 1fr; align-items: center; gap: 5px 9px; min-width: 240px; }
  .invoice-identity small { grid-column: 1 / -1; }
  .status { border-radius: 10px; padding: 3px 6px; font-size: 8px; font-weight: 850; letter-spacing: .06em; text-transform: uppercase; background: var(--mv-raised); color: var(--mv-muted); }
  .status.paid { background: var(--mv-green-soft); color: var(--mv-green); }
  .status.overdue { background: rgba(239,118,118,.1); color: var(--mv-red); }
  .status.partially-paid { background: rgba(229,184,91,.1); color: var(--mv-amber); }
  .invoice-identity strong { font: 750 12px ui-monospace, monospace; }
  .invoice-identity small, .invoice-dates span, .invoice-amount span, .invoice-card footer > span { color: var(--mv-muted); font-size: 10px; }
  .invoice-dates { display: flex; gap: 18px; }
  .invoice-dates span, .invoice-amount { display: flex; flex-direction: column; gap: 3px; }
  .invoice-dates b { color: var(--mv-text); font-weight: 650; }
  .invoice-amount { text-align: right; min-width: 135px; }
  .invoice-amount strong { font-size: 16px; }
  .invoice-card footer { border-top: 1px solid var(--mv-border-soft); padding: 8px 12px; }
  .invoice-card footer div { display: flex; align-items: center; gap: 5px; }
  .text-button.danger { color: var(--mv-red); }
  .payment-form { display: flex; align-items: end; gap: 8px; background: #0e1511; border-top: 1px solid var(--mv-border-soft); padding: 11px 13px; }
  .payment-form label { width: 145px; }
  .payment-form label.grow { flex: 1; }
  .payment-form input { min-height: 32px; padding: 6px 8px; }
  .earnings-head { background: var(--mv-panel); border: 1px solid var(--mv-border-soft); border-radius: 9px 9px 0 0; padding: 14px; }
  .earning-totals { display: grid; text-align: right; color: var(--mv-green); }
  .payment-list article { display: grid; grid-template-columns: 32px minmax(180px, 1fr) 130px 120px minmax(120px, .7fr); gap: 10px; align-items: center; border: 1px solid var(--mv-border-soft); border-top: 0; background: var(--mv-panel); padding: 11px 13px; font-size: 11px; }
  .payment-list article:last-child { border-radius: 0 0 9px 9px; }
  .payment-mark { width: 26px; height: 26px; display: grid; place-items: center; border-radius: 7px; color: var(--mv-green); background: var(--mv-green-soft); }
  .payment-list div { display: flex; flex-direction: column; gap: 2px; }
  .payment-list small, .payment-list time { color: var(--mv-muted); }
  .payment-list > article > b { text-align: right; color: var(--mv-green); }
  .preview-toolbar { margin-top: 14px; }
  .preview-toolbar span { color: var(--mv-amber); font-size: 10px; }
  .custom-dates { display: grid; gap: 6px; }
  .custom-dates > div { display: flex; align-items: end; gap: 5px; max-width: 300px; }
  .custom-dates label { flex: 1; }
  .schedule-preview { margin-top: 12px; border: 1px solid var(--mv-border-soft); background: #0e1511; border-radius: 8px; padding: 11px; }
  .schedule-preview > div { display: flex; justify-content: space-between; color: var(--mv-muted); font-size: 10px; }
  .schedule-preview > div strong { color: var(--mv-text); }
  .schedule-preview ol { list-style: none; display: grid; grid-template-columns: repeat(4, 1fr); gap: 5px; max-height: 190px; overflow: auto; margin: 10px 0 0; padding: 0; }
  .schedule-preview li { display: flex; gap: 7px; align-items: center; border: 1px solid var(--mv-border-soft); border-radius: 5px; padding: 6px; font-size: 9px; }
  .schedule-preview li span { color: var(--mv-green); font-family: ui-monospace, monospace; }
  .loan-card { overflow: hidden; }
  .loan-card summary { list-style: none; display: grid; grid-template-columns: 32px minmax(220px, 1fr) 160px 50px; align-items: center; gap: 11px; cursor: pointer; padding: 13px 14px; }
  .loan-card summary::-webkit-details-marker { display: none; }
  .loan-icon { width: 29px; height: 29px; display: grid; place-items: center; border-radius: 7px; color: var(--mv-amber); background: rgba(229,184,91,.1); font: 800 11px ui-monospace, monospace; }
  .loan-card summary div { display: flex; flex-direction: column; gap: 3px; }
  .loan-card summary small, .loan-next span { color: var(--mv-muted); font-size: 10px; }
  .loan-card summary > b { color: var(--mv-green); font-size: 12px; text-align: right; }
  .loan-meta { display: flex; gap: 25px; padding: 9px 14px; background: #0e1511; border-top: 1px solid var(--mv-border-soft); border-bottom: 1px solid var(--mv-border-soft); }
  .loan-meta span { display: flex; flex-direction: column; gap: 3px; color: var(--mv-muted); font-size: 9px; }
  .loan-meta b { color: var(--mv-text); text-transform: capitalize; }
  .installment-list { list-style: none; margin: 0; padding: 0; max-height: 330px; overflow: auto; }
  .installment-list li { display: grid; grid-template-columns: 28px 1fr 150px 100px; align-items: center; gap: 8px; border-bottom: 1px solid var(--mv-border-soft); padding: 8px 14px; font-size: 10px; }
  .installment-list li:last-child { border: 0; }
  .installment-list li.paid { color: var(--mv-muted); }
  .installment-list small { color: var(--mv-muted); text-align: right; }
  .installment-list li:not(.paid) small { color: var(--mv-amber); }
  .inline-date input { min-height: 28px; padding: 4px 7px; font-size: 10px; }
  .check { width: 21px; height: 21px; border: 1px solid var(--mv-border); background: #0e1511; border-radius: 5px; color: #07110b; cursor: pointer; padding: 0; }
  .check[aria-pressed="true"] { background: var(--mv-green); border-color: var(--mv-green); }
  .empty-state, .loading-state { display: flex; flex-direction: column; align-items: center; text-align: center; padding: 55px 20px; background: var(--mv-panel); border: 1px solid var(--mv-border-soft); border-radius: 9px; color: var(--mv-muted); }
  .empty-state > span { display: grid; place-items: center; width: 45px; height: 45px; border-radius: 10px; background: var(--mv-green-soft); color: var(--mv-green); font: 800 10px ui-monospace, monospace; }
  .empty-state h3 { color: var(--mv-text); margin: 12px 0 3px; font-size: 15px; }
  .empty-state p { max-width: 440px; margin: 0 0 14px; font-size: 11px; line-height: 1.6; }
  .loading-state { font-size: 11px; }
  .sr-only { position: absolute !important; width: 1px !important; height: 1px !important; padding: 0 !important; margin: -1px !important; overflow: hidden !important; clip: rect(0, 0, 0, 0) !important; white-space: nowrap !important; border: 0 !important; }
  @media (max-width: 900px) {
    .summary-row { grid-template-columns: 1fr; }
    .form-grid { grid-template-columns: 1fr 1fr; }
    .invoice-main { align-items: flex-start; flex-wrap: wrap; }
    .payment-list article { grid-template-columns: 32px 1fr auto; }
    .payment-list article > small { grid-column: 2 / -1; }
    .schedule-preview ol { grid-template-columns: repeat(2, 1fr); }
  }
  @media (max-width: 620px) {
    .money-heading, .invoice-card footer, .payment-form { align-items: stretch; flex-direction: column; }
    .heading-actions .primary { width: 100%; }
    .form-grid, .notes-grid { grid-template-columns: 1fr; }
    .form-grid .span-2 { grid-column: auto; }
    .line-labels { display: none; }
    .line-row { grid-template-columns: 1fr 70px 110px 30px; }
    .adjustments { align-items: stretch; flex-direction: column; }
    .adjustments label, .payment-form label { width: 100%; }
    .tabs { overflow-x: auto; }
    .invoice-dates { width: 100%; }
    .invoice-amount { text-align: left; }
    .loan-card summary { grid-template-columns: 32px 1fr 45px; }
    .loan-next { grid-column: 2; }
    .installment-list li { grid-template-columns: 28px 1fr; }
    .installment-list .inline-date, .installment-list small { grid-column: 2; text-align: left; }
  }
</style>
