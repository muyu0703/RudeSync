import type {
  InvoiceDiscount,
  InvoiceTotals,
} from "../../domain/money.ts";

export type InvoiceStatus =
  | "draft"
  | "issued"
  | "partially-paid"
  | "paid"
  | "overdue"
  | "void";

export type InvoiceTermKind =
  | "immediate"
  | "7-days"
  | "14-days"
  | "30-days"
  | "custom";

export type LoanFrequency =
  | "monthly"
  | "weekly"
  | "every-two-weeks"
  | "twice-monthly"
  | "custom";

export type MilestoneStatus = "not-invoiced" | "invoiced" | "paid";

export interface InvoiceProjectMilestone {
  id: string;
  label: string;
  amountMinor: number;
  kind: string;
  sortOrder: number;
  status: MilestoneStatus;
}

export interface InvoiceProjectOption {
  id: string;
  name: string;
  clientId: string;
  clientName: string;
  currency: string;
  quotedTotalMinor: number;
  milestones: InvoiceProjectMilestone[];
}

export interface InvoiceLineItem {
  id: string;
  description: string;
  quantity: string;
  unitPriceMinor: number;
}

export interface InvoicePayment {
  id: string;
  amountMinor: number;
  receivedDate: string;
  note: string | null;
  createdAt: string;
}

export interface Invoice {
  id: string;
  number: string;
  projectId: string;
  projectName: string;
  clientName: string;
  billToEmail: string | null;
  billToAddress: string | null;
  sellerName: string;
  sellerEmail: string | null;
  sellerAddress: string | null;
  sellerLogoPath: string | null;
  milestoneId: string | null;
  milestoneLabel: string | null;
  milestoneKind: string;
  milestonePercentBasisPoints: number | null;
  issueDate: string;
  dueDate: string;
  termKind: InvoiceTermKind;
  currency: string;
  lineItems: InvoiceLineItem[];
  discount: InvoiceDiscount | null;
  taxPercentage: string | null;
  notes: string | null;
  paymentInstructions: string | null;
  status: InvoiceStatus;
  payments: InvoicePayment[];
  createdAt: string;
  updatedAt: string;
}

export interface CreateInvoiceInput {
  projectId: string;
  clientId: string;
  projectName: string;
  clientName: string;
  milestoneId?: string | null;
  milestoneKind?: string | null;
  milestonePercentBasisPoints?: number | null;
  milestoneLabel?: string | null;
  issueDate: string;
  dueDate: string;
  termKind: InvoiceTermKind;
  currency: string;
  lineItems: Array<Omit<InvoiceLineItem, "id">>;
  discount?: InvoiceDiscount | null;
  taxPercentage?: string | null;
  notes?: string | null;
  paymentInstructions?: string | null;
  status: Extract<InvoiceStatus, "draft" | "issued">;
}

export interface RecordInvoicePaymentInput {
  amountMinor: number;
  receivedDate: string;
  note?: string | null;
  allowOverpayment?: boolean;
}

export interface UpdateDraftInvoiceInput
  extends Omit<CreateInvoiceInput, "status"> {
  invoiceId: string;
}

export interface LoanInstallment {
  id: string;
  installmentNumber: number;
  dueDate: string;
  paid: boolean;
  paidDate: string | null;
}

export interface PersonalLoan {
  id: string;
  operator: string;
  description: string | null;
  loanDate: string;
  firstPaymentDate: string;
  installmentCount: number;
  frequency: LoanFrequency;
  paymentDays: [number, number] | null;
  installments: LoanInstallment[];
  createdAt: string;
  updatedAt: string;
}

export interface CreatePersonalLoanInput {
  operator: string;
  description?: string | null;
  loanDate: string;
  firstPaymentDate: string;
  installmentCount: number;
  frequency: LoanFrequency;
  paymentDays?: [number, number] | null;
  dueDates: string[];
}

export interface SetInstallmentPaidInput {
  loanId: string;
  installmentId: string;
  paid: boolean;
  paidDate?: string | null;
}

export interface UpdateInstallmentDueDateInput {
  loanId: string;
  installmentId: string;
  dueDate: string;
}

export interface MoneyService {
  listInvoiceProjects(): Promise<InvoiceProjectOption[]>;
  listInvoices(): Promise<Invoice[]>;
  createInvoice(input: CreateInvoiceInput): Promise<Invoice>;
  updateDraftInvoice(input: UpdateDraftInvoiceInput): Promise<Invoice>;
  issueDraftInvoice(invoiceId: string): Promise<Invoice>;
  voidInvoice(invoiceId: string): Promise<Invoice>;
  /** Discard a draft outright, releasing the milestone it holds. */
  deleteDraftInvoice(invoiceId: string): Promise<void>;
  recordInvoicePayment(
    invoiceId: string,
    input: RecordInvoicePaymentInput,
  ): Promise<Invoice>;
  listPersonalLoans(): Promise<PersonalLoan[]>;
  createPersonalLoan(
    input: CreatePersonalLoanInput,
  ): Promise<PersonalLoan>;
  setInstallmentPaid(input: SetInstallmentPaidInput): Promise<PersonalLoan>;
  updateInstallmentDueDate(
    input: UpdateInstallmentDueDateInput,
  ): Promise<PersonalLoan>;
}

export interface InvoiceWithTotals {
  invoice: Invoice;
  totals: InvoiceTotals;
}

export interface InvoiceExportDetail {
  invoice: Invoice;
  format: "pdf" | "print";
}
