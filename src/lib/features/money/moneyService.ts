import {
  calculateInvoiceTotals,
  type InvoiceDiscount,
  type InvoiceTotals,
} from "../../domain/money.ts";
import { normalizeIsoDate } from "../../domain/date.ts";
import { MONEY_COMMANDS, moneyCommandArgs } from "./moneyCommands.ts";
import type {
  CreateInvoiceInput,
  CreatePersonalLoanInput,
  Invoice,
  InvoiceLineItem,
  InvoicePayment,
  InvoiceProjectMilestone,
  InvoiceProjectOption,
  InvoiceStatus,
  InvoiceTermKind,
  LoanFrequency,
  LoanInstallment,
  MilestoneStatus,
  MoneyService,
  PersonalLoan,
  RecordInvoicePaymentInput,
  SetInstallmentPaidInput,
  UpdateDraftInvoiceInput,
  UpdateInstallmentDueDateInput,
} from "./types.ts";

const MONEY_STORAGE_KEY = "rudesync.money.v1";
const WORK_STORAGE_KEY = "rudesync.work.v1";
const INVOICE_PROFILE_STORAGE_KEY = "rudesync.invoice-profile.v1";

type Invoke = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

interface TauriWindow extends Window {
  __TAURI_INTERNALS__?: { invoke?: Invoke };
  __TAURI__?: { core?: { invoke?: Invoke } };
}

interface BrowserMoneyState {
  invoices: Invoice[];
  loans: PersonalLoan[];
  invoiceSequences: Record<string, number>;
}

function uid(prefix: string): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
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

function field(
  value: Record<string, unknown>,
  camel: string,
  snake: string,
): unknown {
  return value[camel] ?? value[snake];
}

function nullableText(value: unknown): string | null {
  if (value == null) return null;
  const normalized = String(value).trim();
  return normalized || null;
}

function integer(value: unknown, fallback = 0): number {
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) ? parsed : fallback;
}

function normalizeCurrency(value: unknown): string {
  const currency = String(value ?? "USD").trim().toUpperCase();
  return /^[A-Z]{3}$/.test(currency) ? currency : "USD";
}

function normalizeMilestoneStatus(value: unknown): MilestoneStatus {
  return value === "invoiced" || value === "paid" ? value : "not-invoiced";
}

function basisPointsToPercentage(value: number): string {
  return (value / 100).toFixed(2).replace(/\.?0+$/, "");
}

function percentageToBasisPoints(value: string): number {
  const normalized = value.trim();
  if (!/^(?:0|[1-9]\d*)(?:\.\d{1,2})?$/.test(normalized)) {
    throw new Error("Percentages may use up to two decimal places.");
  }
  return Math.round(Number(normalized) * 100);
}

function quantityToMillis(value: string): number {
  const normalized = value.trim();
  if (!/^(?:0|[1-9]\d*)(?:\.\d{1,3})?$/.test(normalized)) {
    throw new Error("Quantities may use up to three decimal places.");
  }
  const result = Math.round(Number(normalized) * 1000);
  if (!Number.isSafeInteger(result) || result <= 0) {
    throw new Error("Quantity must be greater than zero.");
  }
  return result;
}

function parseJsonArray(value: unknown): unknown[] {
  if (Array.isArray(value)) return value;
  if (typeof value !== "string" || !value.trim()) return [];
  try {
    const parsed = JSON.parse(value) as unknown;
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function parseDiscount(value: unknown): InvoiceDiscount | null {
  if (value == null) return null;
  const raw = value as Record<string, unknown>;
  if (raw.kind === "fixed") {
    return {
      kind: "fixed",
      amountMinor: Math.max(
        0,
        integer(field(raw, "amountMinor", "amount_minor")),
      ),
    };
  }
  if (raw.kind === "percentage") {
    return {
      kind: "percentage",
      percentage: String(raw.percentage ?? "0"),
    };
  }
  return null;
}

function parseLineItem(value: unknown): InvoiceLineItem {
  const raw = (value ?? {}) as Record<string, unknown>;
  const quantityMillis = integer(
    field(raw, "quantityMillis", "quantity_millis"),
    1000,
  );
  return {
    id: String(raw.id ?? uid("line")),
    description: String(raw.description ?? "").trim(),
    quantity: String(
      raw.quantity ??
        (quantityMillis % 1000 === 0
          ? quantityMillis / 1000
          : (quantityMillis / 1000).toFixed(3).replace(/0+$/, "")),
    ),
    unitPriceMinor: Math.max(
      0,
      integer(field(raw, "unitPriceMinor", "unit_price_minor")),
    ),
  };
}

function parsePayment(value: unknown): InvoicePayment {
  const raw = (value ?? {}) as Record<string, unknown>;
  return {
    id: String(raw.id ?? uid("payment")),
    amountMinor: Math.max(
      0,
      integer(field(raw, "amountMinor", "amount_minor")),
    ),
    receivedDate: String(
      field(raw, "receivedDate", "received_date") ??
        field(raw, "paidAt", "paid_at") ??
        localIsoDay(),
    ).slice(0, 10),
    note: nullableText(raw.note ?? raw.notes ?? raw.reference),
    createdAt: String(
      field(raw, "createdAt", "created_at") ?? nowIso(),
    ),
  };
}

function parseInvoice(value: unknown): Invoice {
  const raw = (value ?? {}) as Record<string, unknown>;
  const rawStatus = String(raw.status ?? "issued").replace(/_/g, "-");
  const validStatuses: InvoiceStatus[] = [
    "draft",
    "issued",
    "partially-paid",
    "paid",
    "overdue",
    "void",
  ];
  const rawTerm = String(
    field(raw, "termKind", "term_kind") ?? "14-days",
  ).replace(/_/g, "-");
  const validTerms: InvoiceTermKind[] = [
    "immediate",
    "7-days",
    "14-days",
    "30-days",
    "custom",
  ];

  const adjustments = parseJsonArray(raw.adjustments);
  const discountAdjustment = adjustments.find((value) => {
    const adjustment = (value ?? {}) as Record<string, unknown>;
    return adjustment.kind === "discount";
  }) as Record<string, unknown> | undefined;
  const taxAdjustment = adjustments.find((value) => {
    const adjustment = (value ?? {}) as Record<string, unknown>;
    return adjustment.kind === "tax";
  }) as Record<string, unknown> | undefined;
  const parsedAdjustmentDiscount: InvoiceDiscount | null =
    discountAdjustment == null
      ? null
      : String(
            field(
              discountAdjustment,
              "calculationType",
              "calculation_type",
            ),
          ) === "fixed"
        ? {
            kind: "fixed",
            amountMinor: Math.max(
              0,
              integer(
                field(
                  discountAdjustment,
                  "amountMinor",
                  "amount_minor",
                ),
              ),
            ),
          }
        : {
            kind: "percentage",
            percentage: basisPointsToPercentage(
              integer(
                field(
                  discountAdjustment,
                  "rateBasisPoints",
                  "rate_basis_points",
                ),
              ),
            ),
          };

  return {
    id: String(raw.id ?? uid("invoice")),
    number: String(
      raw.number ??
        field(raw, "invoiceNumber", "invoice_number") ??
        "INV-0000-00-00-0000",
    ),
    projectId: String(field(raw, "projectId", "project_id") ?? ""),
    projectName: String(
      field(raw, "projectName", "project_name") ??
        field(raw, "projectNameSnapshot", "project_name_snapshot") ??
        "Untitled project",
    ),
    clientName: String(
      field(raw, "clientName", "client_name") ??
        field(raw, "clientNameSnapshot", "client_name_snapshot") ??
        "Unknown client",
    ),
    billToEmail: nullableText(
      field(raw, "billToEmail", "bill_to_email"),
    ),
    billToAddress: nullableText(
      field(raw, "billToAddress", "bill_to_address"),
    ),
    sellerName: String(
      field(raw, "sellerName", "seller_name") ?? "",
    ),
    sellerEmail: nullableText(
      field(raw, "sellerEmail", "seller_email"),
    ),
    sellerAddress: nullableText(
      field(raw, "sellerAddress", "seller_address"),
    ),
    sellerLogoPath: nullableText(
      field(raw, "sellerLogoPath", "seller_logo_path"),
    ),
    milestoneLabel: nullableText(
      field(raw, "milestoneLabel", "milestone_label"),
    ),
    milestoneKind: String(
      field(raw, "milestoneKind", "milestone_kind") ?? "custom",
    ),
    milestonePercentBasisPoints:
      field(
        raw,
        "milestonePercentBasisPoints",
        "milestone_percent_basis_points",
      ) == null
        ? null
        : integer(
            field(
              raw,
              "milestonePercentBasisPoints",
              "milestone_percent_basis_points",
            ),
          ),
    issueDate: String(
      field(raw, "issueDate", "issue_date") ?? localIsoDay(),
    ),
    dueDate: String(field(raw, "dueDate", "due_date") ?? localIsoDay()),
    termKind: validTerms.includes(rawTerm as InvoiceTermKind)
      ? (rawTerm as InvoiceTermKind)
      : "custom",
    currency: normalizeCurrency(raw.currency),
    lineItems: parseJsonArray(
      field(raw, "lineItems", "line_items") ??
        field(raw, "lineItemsJson", "line_items_json"),
    ).map(parseLineItem),
    discount:
      parseDiscount(
        raw.discount ??
          field(raw, "discountJson", "discount_json"),
      ) ?? parsedAdjustmentDiscount,
    taxPercentage:
      nullableText(field(raw, "taxPercentage", "tax_percentage")) ??
      (taxAdjustment
        ? basisPointsToPercentage(
            integer(
              field(
                taxAdjustment,
                "rateBasisPoints",
                "rate_basis_points",
              ),
            ),
          )
        : null),
    notes: nullableText(raw.notes),
    paymentInstructions: nullableText(
      field(raw, "paymentInstructions", "payment_instructions"),
    ),
    status: validStatuses.includes(rawStatus as InvoiceStatus)
      ? (rawStatus as InvoiceStatus)
      : "issued",
    payments: parseJsonArray(raw.payments).map(parsePayment),
    createdAt: String(
      field(raw, "createdAt", "created_at") ?? nowIso(),
    ),
    updatedAt: String(
      field(raw, "updatedAt", "updated_at") ?? nowIso(),
    ),
  };
}

function parseInstallment(value: unknown, index: number): LoanInstallment {
  const raw = (value ?? {}) as Record<string, unknown>;
  const paid =
    raw.paid === true ||
    raw.isPaid === true ||
    raw.is_paid === true ||
    raw.status === "paid";
  return {
    id: String(raw.id ?? uid("installment")),
    installmentNumber: Math.max(
      1,
      integer(
        field(raw, "installmentNumber", "installment_number"),
        index + 1,
      ),
    ),
    dueDate: String(
      field(raw, "dueDate", "due_date") ?? localIsoDay(),
    ),
    paid,
    paidDate: paid
      ? nullableText(
          field(raw, "paidDate", "paid_date") ??
            field(raw, "paidAt", "paid_at"),
        )?.slice(0, 10) ?? null
      : null,
  };
}

function parseFrequency(value: unknown): LoanFrequency {
  const normalized = String(value ?? "monthly")
    .toLowerCase()
    .replace(/_/g, "-");
  if (normalized === "biweekly") return "every-two-weeks";
  const valid: LoanFrequency[] = [
    "monthly",
    "weekly",
    "every-two-weeks",
    "twice-monthly",
    "custom",
  ];
  return valid.includes(normalized as LoanFrequency)
    ? (normalized as LoanFrequency)
    : "monthly";
}

function parsePersonalLoan(value: unknown): PersonalLoan {
  const raw = (value ?? {}) as Record<string, unknown>;
  const paymentDaysRaw = parseJsonArray(
    field(raw, "paymentDays", "payment_days") ??
      field(raw, "paymentDaysJson", "payment_days_json"),
  ).map(Number);
  const installments = parseJsonArray(raw.installments).map(
    parseInstallment,
  );

  return {
    id: String(raw.id ?? uid("loan")),
    operator: String(
      raw.operator ??
        field(raw, "operatorName", "operator_name") ??
        raw.lender ??
        "",
    ).trim(),
    description: nullableText(raw.description),
    loanDate: String(
      field(raw, "loanDate", "loan_date") ?? localIsoDay(),
    ),
    firstPaymentDate: String(
      field(raw, "firstPaymentDate", "first_payment_date") ??
        installments[0]?.dueDate ??
        localIsoDay(),
    ),
    installmentCount: Math.max(
      1,
      integer(
        field(raw, "installmentCount", "installment_count") ??
          field(raw, "paymentCount", "payment_count"),
        installments.length || 1,
      ),
    ),
    frequency: parseFrequency(raw.frequency),
    paymentDays:
      paymentDaysRaw.length === 2
        ? [paymentDaysRaw[0], paymentDaysRaw[1]]
        : null,
    installments,
    createdAt: String(
      field(raw, "createdAt", "created_at") ?? nowIso(),
    ),
    updatedAt: String(
      field(raw, "updatedAt", "updated_at") ?? nowIso(),
    ),
  };
}

function normalizeInvoiceInput(
  input: CreateInvoiceInput,
): CreateInvoiceInput {
  if (!input.projectId) throw new Error("Select a project.");
  if (!input.clientId) throw new Error("The selected project needs a client.");
  if (!input.projectName.trim()) throw new Error("Project name is required.");
  if (!input.clientName.trim()) throw new Error("Client name is required.");
  if (!input.lineItems.length) {
    throw new Error("Add at least one invoice line item.");
  }

  const lineItems = input.lineItems.map((item) => {
    const description = item.description.trim();
    if (!description) throw new Error("Every line item needs a description.");
    if (!/^(?:0|[1-9]\d*)(?:\.\d+)?$/.test(item.quantity.trim())) {
      throw new Error("Line item quantities must be positive numbers.");
    }
    if (Number(item.quantity) <= 0) {
      throw new Error("Line item quantities must be greater than zero.");
    }
    if (
      !Number.isSafeInteger(item.unitPriceMinor) ||
      item.unitPriceMinor < 0
    ) {
      throw new Error("Line item prices must be valid non-negative amounts.");
    }
    return {
      description,
      quantity: item.quantity.trim(),
      unitPriceMinor: item.unitPriceMinor,
    };
  });

  const normalized: CreateInvoiceInput = {
    ...input,
    projectName: input.projectName.trim(),
    clientName: input.clientName.trim(),
    milestoneLabel: nullableText(input.milestoneLabel),
    issueDate: normalizeIsoDate(input.issueDate),
    dueDate: normalizeIsoDate(input.dueDate),
    currency: normalizeCurrency(input.currency),
    lineItems,
    discount: input.discount ?? null,
    taxPercentage: nullableText(input.taxPercentage),
    notes: nullableText(input.notes),
    paymentInstructions: nullableText(input.paymentInstructions),
  };

  // Exact domain calculation doubles as validation for tax, discount,
  // quantities, and safe integer boundaries before data reaches persistence.
  calculateInvoiceTotals({
    lineItems: normalized.lineItems,
    discount: normalized.discount,
    taxPercentage: normalized.taxPercentage,
  });

  return normalized;
}

function invoiceBackendPayload(
  normalized: CreateInvoiceInput,
): Record<string, unknown> {
  const paymentTermDaysByKind: Partial<
    Record<InvoiceTermKind, 0 | 7 | 14 | 30>
  > = {
    immediate: 0,
    "7-days": 7,
    "14-days": 14,
    "30-days": 30,
  };
  const adjustments: Record<string, unknown>[] = [];
  if (normalized.discount?.kind === "fixed") {
    adjustments.push({
      kind: "discount",
      label: "Discount",
      calculationType: "fixed",
      amountMinor: normalized.discount.amountMinor,
    });
  } else if (normalized.discount?.kind === "percentage") {
    adjustments.push({
      kind: "discount",
      label: "Discount",
      calculationType: "percentage",
      rateBasisPoints: percentageToBasisPoints(
        normalized.discount.percentage,
      ),
    });
  }
  if (normalized.taxPercentage) {
    adjustments.push({
      kind: "tax",
      label: "Tax",
      calculationType: "percentage",
      rateBasisPoints: percentageToBasisPoints(
        normalized.taxPercentage,
      ),
    });
  }
  return {
    projectId: normalized.projectId,
    clientId: normalized.clientId,
    milestoneId: normalized.milestoneId ?? undefined,
    milestoneKind: normalized.milestoneKind ?? "custom",
    milestoneLabel: normalized.milestoneLabel,
    milestonePercentBasisPoints:
      normalized.milestonePercentBasisPoints ?? undefined,
    issueDate: normalized.issueDate,
    paymentTermDays: paymentTermDaysByKind[normalized.termKind],
    customDueDate:
      normalized.termKind === "custom"
        ? normalized.dueDate
        : undefined,
    currency: normalized.currency,
    notes: normalized.notes,
    paymentInstructions: normalized.paymentInstructions,
    status: normalized.status,
    items: normalized.lineItems.map((item) => ({
      description: item.description,
      quantityMillis: quantityToMillis(item.quantity),
      unitPriceMinor: item.unitPriceMinor,
    })),
    adjustments,
  };
}

function normalizePaymentInput(
  input: RecordInvoicePaymentInput,
): RecordInvoicePaymentInput {
  if (!Number.isSafeInteger(input.amountMinor) || input.amountMinor <= 0) {
    throw new Error("Payment amount must be greater than zero.");
  }
  return {
    amountMinor: input.amountMinor,
    receivedDate: normalizeIsoDate(input.receivedDate),
    note: nullableText(input.note),
    allowOverpayment: input.allowOverpayment === true,
  };
}

function normalizeLoanInput(
  input: CreatePersonalLoanInput,
): CreatePersonalLoanInput {
  const operator = input.operator.trim();
  if (!operator) throw new Error("Lender or operator is required.");
  const loanDate = normalizeIsoDate(input.loanDate);
  const firstPaymentDate = normalizeIsoDate(input.firstPaymentDate);
  if (firstPaymentDate < loanDate) {
    throw new Error("The first payment cannot be before the loan date.");
  }
  if (
    !Number.isSafeInteger(input.installmentCount) ||
    input.installmentCount < 1 ||
    input.installmentCount > 240
  ) {
    throw new Error("Installment count must be between 1 and 240.");
  }
  if (input.dueDates.length !== input.installmentCount) {
    throw new Error("Regenerate the schedule before saving this loan.");
  }
  const dueDates = input.dueDates.map(normalizeIsoDate);
  if (dueDates[0] !== firstPaymentDate) {
    throw new Error(
      "The first schedule date must match the explicit first payment date.",
    );
  }
  for (let index = 1; index < dueDates.length; index += 1) {
    const order = dueDates[index].localeCompare(dueDates[index - 1]);
    if (
      order < 0 ||
      (order === 0 && input.frequency !== "twice-monthly")
    ) {
      throw new Error(
        input.frequency === "twice-monthly"
          ? "Installment dates must be in chronological order."
          : "Installment dates must be unique and in chronological order.",
      );
    }
  }
  if (
    input.frequency === "twice-monthly" &&
    (!input.paymentDays ||
      input.paymentDays[0] === input.paymentDays[1] ||
      input.paymentDays.some((day) => day < 1 || day > 31))
  ) {
    throw new Error("Choose two different twice-monthly payment days.");
  }

  return {
    ...input,
    operator,
    description: nullableText(input.description),
    loanDate,
    firstPaymentDate,
    installmentCount: dueDates.length,
    paymentDays: input.paymentDays ?? null,
    dueDates,
  };
}

export function invoiceTotals(invoice: Invoice): InvoiceTotals {
  return calculateInvoiceTotals({
    lineItems: invoice.lineItems,
    discount: invoice.discount,
    taxPercentage: invoice.taxPercentage,
    paymentsMinor: invoice.payments.map((payment) => payment.amountMinor),
  });
}

export function effectiveInvoiceStatus(
  invoice: Invoice,
  today = localIsoDay(),
): InvoiceStatus {
  if (
    invoice.status === "draft" ||
    invoice.status === "void"
  ) {
    return invoice.status;
  }
  const totals = invoiceTotals(invoice);
  if (totals.balanceDueMinor === 0) return "paid";
  if (invoice.dueDate < today) return "overdue";
  if (totals.paidMinor > 0) return "partially-paid";
  return "issued";
}

function projectOptionsFromValues(
  rawClients: unknown[],
  rawProjects: unknown[],
): InvoiceProjectOption[] {
  const clients = new Map<string, { name: string; currency: string }>();
  for (const value of rawClients) {
    const raw = (value ?? {}) as Record<string, unknown>;
    clients.set(String(raw.id ?? ""), {
      name: String(
        raw.name ?? field(raw, "companyName", "company_name") ?? "Client",
      ),
      currency: normalizeCurrency(raw.currency),
    });
  }

  return rawProjects
    .map((value) => {
      const raw = (value ?? {}) as Record<string, unknown>;
      const clientId = String(field(raw, "clientId", "client_id") ?? "");
      const client = clients.get(clientId);
      const quotedTotalMinor = Math.max(
        0,
        integer(field(raw, "quotedTotalMinor", "quoted_total_minor")),
      );
      const storedMilestones = parseJsonArray(raw.milestones);
      const milestones = storedMilestones
        .map((milestone): InvoiceProjectMilestone => {
          const item = (milestone ?? {}) as Record<string, unknown>;
          return {
            id: String(item.id ?? ""),
            label: String(item.label ?? "Milestone"),
            amountMinor: Math.max(
              0,
              integer(field(item, "amountMinor", "amount_minor")),
            ),
            kind: String(item.kind ?? "custom"),
            sortOrder: integer(field(item, "sortOrder", "sort_order")),
            status: normalizeMilestoneStatus(item.status),
          };
        })
        .filter((milestone) => milestone.id)
        .sort((a, b) => a.sortOrder - b.sortOrder);
      return {
        id: String(raw.id ?? ""),
        name: String(raw.name ?? "Untitled project"),
        clientId,
        clientName: client?.name ?? "Unknown client",
        currency: normalizeCurrency(raw.currency ?? client?.currency),
        quotedTotalMinor,
        milestones,
        status: String(raw.status ?? "active"),
      };
    })
    .filter(
      (project) =>
        project.id &&
        project.status !== "archived" &&
        project.status !== "deleted",
    )
    .map(({ status: _status, ...project }) => project)
    .sort((a, b) => a.name.localeCompare(b.name));
}

interface BrowserInvoiceSnapshot {
  billToEmail: string | null;
  billToAddress: string | null;
  sellerName: string;
  sellerEmail: string | null;
  sellerAddress: string | null;
  sellerLogoPath: string | null;
  paymentInstructions: string | null;
}

function reserveBrowserInvoiceNumber(
  state: BrowserMoneyState,
  issueDate: string,
): string {
  const next = (state.invoiceSequences[issueDate] ?? 0) + 1;
  state.invoiceSequences[issueDate] = next;
  return `INV-${issueDate}-${String(next).padStart(4, "0")}`;
}

function browserInvoiceSnapshot(clientId: string): BrowserInvoiceSnapshot {
  let billToEmail: string | null = null;
  let billToAddress: string | null = null;
  let sellerName = "";
  let sellerEmail: string | null = null;
  let sellerAddress: string | null = null;
  let sellerLogoPath: string | null = null;
  let paymentInstructions: string | null = null;

  try {
    const storedWork = window.localStorage.getItem(WORK_STORAGE_KEY);
    const work = storedWork
      ? (JSON.parse(storedWork) as { clients?: unknown[] })
      : {};
    const client = (Array.isArray(work.clients) ? work.clients : []).find(
      (value) =>
        String((value as Record<string, unknown> | null)?.id ?? "") ===
        clientId,
    ) as Record<string, unknown> | undefined;
    if (client) {
      billToEmail = nullableText(client.email);
      billToAddress = nullableText(
        field(client, "billingAddress", "billing_address"),
      );
    }
  } catch {
    // A malformed browser-only work cache must not prevent invoice creation.
  }

  try {
    const storedProfile = window.localStorage.getItem(
      INVOICE_PROFILE_STORAGE_KEY,
    );
    const profile = storedProfile
      ? (JSON.parse(storedProfile) as Record<string, unknown>)
      : {};
    sellerName =
      nullableText(
        field(profile, "businessName", "business_name"),
      ) ??
      nullableText(field(profile, "displayName", "display_name")) ??
      "";
    sellerEmail = nullableText(profile.email);
    sellerAddress = nullableText(profile.address);
    sellerLogoPath = nullableText(
      field(profile, "logoPath", "logo_path"),
    );
    paymentInstructions = nullableText(
      field(profile, "paymentInstructions", "payment_instructions"),
    );
  } catch {
    // Keep the stable RudeSync fallback when the browser profile is malformed.
  }

  return {
    billToEmail,
    billToAddress,
    sellerName,
    sellerEmail,
    sellerAddress,
    sellerLogoPath,
    paymentInstructions,
  };
}

class BrowserMoneyService implements MoneyService {
  private read(): BrowserMoneyState {
    const empty: BrowserMoneyState = {
      invoices: [],
      loans: [],
      invoiceSequences: {},
    };
    if (typeof window === "undefined") return empty;
    const stored = window.localStorage.getItem(MONEY_STORAGE_KEY);
    if (!stored) return empty;
    try {
      const raw = JSON.parse(stored) as Partial<BrowserMoneyState>;
      const invoices = Array.isArray(raw.invoices)
        ? raw.invoices.map(parseInvoice)
        : [];
      const loans = Array.isArray(raw.loans)
        ? raw.loans.map(parsePersonalLoan)
        : [];
      const invoiceSequences: Record<string, number> = {};
      if (
        raw.invoiceSequences &&
        typeof raw.invoiceSequences === "object" &&
        !Array.isArray(raw.invoiceSequences)
      ) {
        for (const [date, value] of Object.entries(raw.invoiceSequences)) {
          const sequence = Number(value);
          if (
            /^\d{4}-\d{2}-\d{2}$/.test(date) &&
            Number.isSafeInteger(sequence) &&
            sequence >= 0
          ) {
            invoiceSequences[date] = sequence;
          }
        }
      }
      for (const invoice of invoices) {
        const match = /^INV-(\d{4}-\d{2}-\d{2})-(\d{4,})$/.exec(
          invoice.number,
        );
        if (!match) continue;
        invoiceSequences[match[1]] = Math.max(
          invoiceSequences[match[1]] ?? 0,
          Number(match[2]),
        );
      }
      return { invoices, loans, invoiceSequences };
    } catch {
      window.localStorage.removeItem(MONEY_STORAGE_KEY);
      return empty;
    }
  }

  private write(state: BrowserMoneyState): void {
    window.localStorage.setItem(MONEY_STORAGE_KEY, JSON.stringify(state));
  }

  async listInvoiceProjects(): Promise<InvoiceProjectOption[]> {
    const stored = window.localStorage.getItem(WORK_STORAGE_KEY);
    if (!stored) return [];
    try {
      const raw = JSON.parse(stored) as {
        clients?: unknown[];
        projects?: unknown[];
      };
      return projectOptionsFromValues(
        Array.isArray(raw.clients) ? raw.clients : [],
        Array.isArray(raw.projects) ? raw.projects : [],
      );
    } catch {
      return [];
    }
  }

  async listInvoices(): Promise<Invoice[]> {
    return this.read().invoices.sort((a, b) =>
      b.issueDate.localeCompare(a.issueDate) ||
      b.createdAt.localeCompare(a.createdAt),
    );
  }

  async createInvoice(input: CreateInvoiceInput): Promise<Invoice> {
    const normalized = normalizeInvoiceInput(input);
    const state = this.read();
    const timestamp = nowIso();
    const snapshot = browserInvoiceSnapshot(normalized.clientId);
    if (normalized.status === "issued" && !snapshot.sellerName.trim()) {
      throw new Error(
        "An issued invoice needs a seller display or business name. Open Settings → Invoice profile and add one.",
      );
    }
    const invoice: Invoice = {
      id: uid("invoice"),
      number: reserveBrowserInvoiceNumber(state, normalized.issueDate),
      projectId: normalized.projectId,
      projectName: normalized.projectName,
      clientName: normalized.clientName,
      billToEmail: snapshot.billToEmail,
      billToAddress: snapshot.billToAddress,
      sellerName: snapshot.sellerName,
      sellerEmail: snapshot.sellerEmail,
      sellerAddress: snapshot.sellerAddress,
      sellerLogoPath: snapshot.sellerLogoPath,
      milestoneLabel: normalized.milestoneLabel ?? null,
      milestoneKind: normalized.milestoneKind ?? "custom",
      milestonePercentBasisPoints:
        normalized.milestonePercentBasisPoints ?? null,
      issueDate: normalized.issueDate,
      dueDate: normalized.dueDate,
      termKind: normalized.termKind,
      currency: normalized.currency,
      lineItems: normalized.lineItems.map((item) => ({
        ...item,
        id: uid("line"),
      })),
      discount: normalized.discount ?? null,
      taxPercentage: normalized.taxPercentage ?? null,
      notes: normalized.notes ?? null,
      paymentInstructions:
        normalized.paymentInstructions ??
        snapshot.paymentInstructions,
      status: normalized.status,
      payments: [],
      createdAt: timestamp,
      updatedAt: timestamp,
    };
    state.invoices.unshift(invoice);
    this.write(state);
    return invoice;
  }

  async updateDraftInvoice(
    input: UpdateDraftInvoiceInput,
  ): Promise<Invoice> {
    const normalized = normalizeInvoiceInput({ ...input, status: "draft" });
    const state = this.read();
    const index = state.invoices.findIndex(
      (invoice) => invoice.id === input.invoiceId,
    );
    if (index < 0) throw new Error("Invoice not found.");
    const current = state.invoices[index];
    if (current.status !== "draft") {
      throw new Error("Only draft invoices can be edited.");
    }
    const snapshot = browserInvoiceSnapshot(normalized.clientId);
    const updated: Invoice = {
      ...current,
      number:
        current.issueDate === normalized.issueDate
          ? current.number
          : reserveBrowserInvoiceNumber(state, normalized.issueDate),
      projectId: normalized.projectId,
      projectName: normalized.projectName,
      clientName: normalized.clientName,
      billToEmail: snapshot.billToEmail,
      billToAddress: snapshot.billToAddress,
      sellerName: snapshot.sellerName,
      sellerEmail: snapshot.sellerEmail,
      sellerAddress: snapshot.sellerAddress,
      sellerLogoPath: snapshot.sellerLogoPath,
      milestoneLabel: normalized.milestoneLabel ?? null,
      milestoneKind: normalized.milestoneKind ?? "custom",
      milestonePercentBasisPoints:
        normalized.milestonePercentBasisPoints ?? null,
      issueDate: normalized.issueDate,
      dueDate: normalized.dueDate,
      termKind: normalized.termKind,
      currency: normalized.currency,
      lineItems: normalized.lineItems.map((item) => ({
        ...item,
        id: uid("line"),
      })),
      discount: normalized.discount ?? null,
      taxPercentage: normalized.taxPercentage ?? null,
      notes: normalized.notes ?? null,
      paymentInstructions:
        normalized.paymentInstructions ?? snapshot.paymentInstructions,
      updatedAt: nowIso(),
    };
    state.invoices[index] = updated;
    this.write(state);
    return updated;
  }

  async issueDraftInvoice(invoiceId: string): Promise<Invoice> {
    const state = this.read();
    const index = state.invoices.findIndex(
      (invoice) => invoice.id === invoiceId,
    );
    if (index < 0) throw new Error("Invoice not found.");
    const current = state.invoices[index];
    if (current.status !== "draft") {
      throw new Error("Only draft invoices can be issued.");
    }
    const project = (await this.listInvoiceProjects()).find(
      (item) => item.id === current.projectId,
    );
    const snapshot = browserInvoiceSnapshot(project?.clientId ?? "");
    if (!snapshot.sellerName.trim()) {
      throw new Error(
        "An issued invoice needs a seller display or business name. Open Settings → Invoice profile and add one.",
      );
    }
    const updated: Invoice = {
      ...current,
      projectName: project?.name ?? current.projectName,
      clientName: project?.clientName ?? current.clientName,
      billToEmail: snapshot.billToEmail,
      billToAddress: snapshot.billToAddress,
      sellerName: snapshot.sellerName,
      sellerEmail: snapshot.sellerEmail,
      sellerAddress: snapshot.sellerAddress,
      sellerLogoPath: snapshot.sellerLogoPath,
      paymentInstructions:
        current.paymentInstructions ?? snapshot.paymentInstructions,
      status: effectiveInvoiceStatus({ ...current, status: "issued" }),
      updatedAt: nowIso(),
    };
    state.invoices[index] = updated;
    this.write(state);
    return updated;
  }

  async voidInvoice(invoiceId: string): Promise<Invoice> {
    const state = this.read();
    const index = state.invoices.findIndex(
      (invoice) => invoice.id === invoiceId,
    );
    if (index < 0) throw new Error("Invoice not found.");
    const current = state.invoices[index];
    const status = effectiveInvoiceStatus(current);
    if (!["issued", "partially-paid", "overdue"].includes(status)) {
      throw new Error(
        "Only issued, partially paid, or overdue invoices can be voided.",
      );
    }
    const updated: Invoice = {
      ...current,
      status: "void",
      updatedAt: nowIso(),
    };
    state.invoices[index] = updated;
    this.write(state);
    return updated;
  }

  async recordInvoicePayment(
    invoiceId: string,
    input: RecordInvoicePaymentInput,
  ): Promise<Invoice> {
    const normalized = normalizePaymentInput(input);
    const state = this.read();
    const index = state.invoices.findIndex(
      (invoice) => invoice.id === invoiceId,
    );
    if (index < 0) throw new Error("Invoice not found.");
    const current = state.invoices[index];
    if (current.status === "void" || current.status === "draft") {
      throw new Error("Payments can only be added to issued invoices.");
    }
    const balance = invoiceTotals(current).balanceDueMinor;
    if (
      normalized.amountMinor > balance &&
      !normalized.allowOverpayment
    ) {
      throw new Error(
        "Payment exceeds the remaining balance; confirm the overpayment first.",
      );
    }
    const updated: Invoice = {
      ...current,
      payments: [
        ...current.payments,
        {
          id: uid("payment"),
          amountMinor: normalized.amountMinor,
          receivedDate: normalized.receivedDate,
          note: normalized.note ?? null,
          createdAt: nowIso(),
        },
      ],
      updatedAt: nowIso(),
    };
    updated.status = effectiveInvoiceStatus(updated);
    state.invoices[index] = updated;
    this.write(state);
    return updated;
  }

  async listPersonalLoans(): Promise<PersonalLoan[]> {
    return this.read().loans.sort((a, b) =>
      b.createdAt.localeCompare(a.createdAt),
    );
  }

  async createPersonalLoan(
    input: CreatePersonalLoanInput,
  ): Promise<PersonalLoan> {
    const normalized = normalizeLoanInput(input);
    const timestamp = nowIso();
    const loan: PersonalLoan = {
      id: uid("loan"),
      operator: normalized.operator,
      description: normalized.description ?? null,
      loanDate: normalized.loanDate,
      firstPaymentDate: normalized.firstPaymentDate,
      installmentCount: normalized.installmentCount,
      frequency: normalized.frequency,
      paymentDays: normalized.paymentDays ?? null,
      installments: normalized.dueDates.map((dueDate, index) => ({
        id: uid("installment"),
        installmentNumber: index + 1,
        dueDate,
        paid: false,
        paidDate: null,
      })),
      createdAt: timestamp,
      updatedAt: timestamp,
    };
    const state = this.read();
    state.loans.unshift(loan);
    this.write(state);
    return loan;
  }

  async setInstallmentPaid(
    input: SetInstallmentPaidInput,
  ): Promise<PersonalLoan> {
    const state = this.read();
    const loanIndex = state.loans.findIndex(
      (loan) => loan.id === input.loanId,
    );
    if (loanIndex < 0) throw new Error("Personal loan not found.");
    const loan = state.loans[loanIndex];
    const installmentIndex = loan.installments.findIndex(
      (installment) => installment.id === input.installmentId,
    );
    if (installmentIndex < 0) throw new Error("Installment not found.");
    const installments = [...loan.installments];
    const paidDate = input.paid
      ? normalizeIsoDate(input.paidDate ?? localIsoDay())
      : null;
    if (paidDate && paidDate < loan.loanDate) {
      throw new Error("The paid date cannot be earlier than the loan date.");
    }
    if (paidDate && paidDate > localIsoDay()) {
      throw new Error("The paid date cannot be in the future.");
    }
    installments[installmentIndex] = {
      ...installments[installmentIndex],
      paid: input.paid,
      paidDate,
    };
    const updated = { ...loan, installments, updatedAt: nowIso() };
    state.loans[loanIndex] = updated;
    this.write(state);
    return updated;
  }

  async updateInstallmentDueDate(
    input: UpdateInstallmentDueDateInput,
  ): Promise<PersonalLoan> {
    const state = this.read();
    const loanIndex = state.loans.findIndex(
      (loan) => loan.id === input.loanId,
    );
    if (loanIndex < 0) throw new Error("Personal loan not found.");
    const loan = state.loans[loanIndex];
    const installmentIndex = loan.installments.findIndex(
      (installment) => installment.id === input.installmentId,
    );
    if (installmentIndex < 0) throw new Error("Installment not found.");
    const dueDate = normalizeIsoDate(input.dueDate);
    const previous = loan.installments[installmentIndex - 1];
    const next = loan.installments[installmentIndex + 1];
    if (dueDate < loan.loanDate) {
      throw new Error("The due date cannot be earlier than the loan date.");
    }
    if (previous && dueDate < previous.dueDate) {
      throw new Error(
        "The due date cannot be earlier than the previous installment.",
      );
    }
    if (next && dueDate > next.dueDate) {
      throw new Error(
        "The due date cannot be later than the next installment.",
      );
    }
    const installments = [...loan.installments];
    installments[installmentIndex] = {
      ...installments[installmentIndex],
      dueDate,
    };
    const updated: PersonalLoan = {
      ...loan,
      firstPaymentDate:
        installmentIndex === 0 ? dueDate : loan.firstPaymentDate,
      installments,
      updatedAt: nowIso(),
    };
    state.loans[loanIndex] = updated;
    this.write(state);
    return updated;
  }
}

class TauriMoneyService implements MoneyService {
  constructor(private readonly invoke: Invoke) {}

  async listInvoiceProjects(): Promise<InvoiceProjectOption[]> {
    const [clients, projects] = await Promise.all([
      this.invoke<unknown[]>(MONEY_COMMANDS.listClients),
      this.invoke<unknown[]>(MONEY_COMMANDS.listProjects),
    ]);
    return projectOptionsFromValues(clients, projects);
  }

  async listInvoices(): Promise<Invoice[]> {
    const rows = await this.invoke<unknown[]>(
      MONEY_COMMANDS.listInvoices,
      moneyCommandArgs.listInvoices(),
    );
    return rows.map(parseInvoice);
  }

  async createInvoice(input: CreateInvoiceInput): Promise<Invoice> {
    const normalized = normalizeInvoiceInput(input);
    const backendInput = invoiceBackendPayload(normalized);
    const row = await this.invoke<unknown>(
      MONEY_COMMANDS.createInvoice,
      moneyCommandArgs.createInvoice(backendInput),
    );
    return parseInvoice(row);
  }

  async updateDraftInvoice(
    input: UpdateDraftInvoiceInput,
  ): Promise<Invoice> {
    const normalized = normalizeInvoiceInput({ ...input, status: "draft" });
    const backendInput = {
      ...invoiceBackendPayload(normalized),
      invoiceId: input.invoiceId,
    };
    const row = await this.invoke<unknown>(
      MONEY_COMMANDS.updateDraftInvoice,
      moneyCommandArgs.updateDraftInvoice(backendInput),
    );
    return parseInvoice(row);
  }

  async issueDraftInvoice(invoiceId: string): Promise<Invoice> {
    const row = await this.invoke<unknown>(
      MONEY_COMMANDS.issueDraftInvoice,
      moneyCommandArgs.issueDraftInvoice(invoiceId),
    );
    return parseInvoice(row);
  }

  async voidInvoice(invoiceId: string): Promise<Invoice> {
    const row = await this.invoke<unknown>(
      MONEY_COMMANDS.voidInvoice,
      moneyCommandArgs.voidInvoice(invoiceId),
    );
    return parseInvoice(row);
  }

  async recordInvoicePayment(
    invoiceId: string,
    input: RecordInvoicePaymentInput,
  ): Promise<Invoice> {
    const normalized = normalizePaymentInput(input);
    const backendInput: Record<string, unknown> = {
      invoiceId,
      amountMinor: normalized.amountMinor,
      paidAt: normalized.receivedDate,
      notes: normalized.note,
      allowOverpayment: normalized.allowOverpayment,
    };
    const row = await this.invoke<unknown>(
      MONEY_COMMANDS.recordInvoicePayment,
      moneyCommandArgs.recordInvoicePayment(backendInput),
    );
    return parseInvoice(row);
  }

  async listPersonalLoans(): Promise<PersonalLoan[]> {
    const rows = await this.invoke<unknown[]>(
      MONEY_COMMANDS.listPersonalLoans,
      moneyCommandArgs.listPersonalLoans(),
    );
    return rows.map(parsePersonalLoan);
  }

  async createPersonalLoan(
    input: CreatePersonalLoanInput,
  ): Promise<PersonalLoan> {
    const normalized = normalizeLoanInput(input);
    const backendFrequency: Record<LoanFrequency, string> = {
      monthly: "monthly",
      weekly: "weekly",
      "every-two-weeks": "biweekly",
      "twice-monthly": "twice_monthly",
      custom: "custom",
    };
    const backendInput: Record<string, unknown> = {
      operatorName: normalized.operator,
      description: normalized.description,
      loanDate: normalized.loanDate,
      firstPaymentDate: normalized.firstPaymentDate,
      paymentCount: normalized.installmentCount,
      frequency: backendFrequency[normalized.frequency],
      firstMonthlyDay:
        normalized.frequency === "twice-monthly"
          ? normalized.paymentDays?.[0]
          : undefined,
      secondMonthlyDay:
        normalized.frequency === "twice-monthly"
          ? normalized.paymentDays?.[1]
          : undefined,
      customDueDates:
        normalized.frequency === "custom"
          ? normalized.dueDates
          : undefined,
    };
    const row = await this.invoke<unknown>(
      MONEY_COMMANDS.createPersonalLoan,
      moneyCommandArgs.createPersonalLoan(backendInput),
    );
    return parsePersonalLoan(row);
  }

  async setInstallmentPaid(
    input: SetInstallmentPaidInput,
  ): Promise<PersonalLoan> {
    const row = await this.invoke<unknown>(
      MONEY_COMMANDS.setLoanInstallmentPaid,
      moneyCommandArgs.setLoanInstallmentPaid(
        input.installmentId,
        input.paid,
        input.paidDate,
      ),
    );
    return parsePersonalLoan(row);
  }

  async updateInstallmentDueDate(
    input: UpdateInstallmentDueDateInput,
  ): Promise<PersonalLoan> {
    const row = await this.invoke<unknown>(
      MONEY_COMMANDS.updateLoanInstallmentDueDate,
      moneyCommandArgs.updateLoanInstallmentDueDate(
        input.installmentId,
        input.dueDate,
      ),
    );
    return parsePersonalLoan(row);
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

export function createMoneyService(): MoneyService {
  const invoke = resolveTauriInvoke();
  return invoke
    ? new TauriMoneyService(invoke)
    : new BrowserMoneyService();
}

export const moneyDateUtils = {
  localIsoDay,
};
