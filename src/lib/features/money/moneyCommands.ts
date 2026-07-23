/**
 * The Money UI intentionally keeps all assumed Rust command names here.
 * Adjusting the backend contract should only require changing this map and,
 * when necessary, the small argument builders below.
 */
export const MONEY_COMMANDS = {
  listClients: "list_clients",
  listProjects: "list_projects",
  listInvoices: "list_invoices",
  createInvoice: "create_invoice",
  updateDraftInvoice: "update_draft_invoice",
  issueDraftInvoice: "issue_draft_invoice",
  voidInvoice: "void_invoice",
  recordInvoicePayment: "record_invoice_payment",
  listPersonalLoans: "list_personal_loans",
  createPersonalLoan: "create_personal_loan",
  setLoanInstallmentPaid: "set_loan_installment_paid",
  updateLoanInstallmentDueDate: "update_loan_installment_due_date",
} as const;

export const moneyCommandArgs = {
  listInvoices: () => ({
    filter: { includeDeleted: false },
  }),
  createInvoice: (input: Record<string, unknown>) => ({ input }),
  updateDraftInvoice: (input: Record<string, unknown>) => ({ input }),
  issueDraftInvoice: (invoiceId: string) => ({ invoiceId }),
  voidInvoice: (invoiceId: string) => ({ invoiceId, confirmed: true }),
  recordInvoicePayment: (input: Record<string, unknown>) => ({ input }),
  listPersonalLoans: () => ({ includeDeleted: false }),
  createPersonalLoan: (input: Record<string, unknown>) => ({ input }),
  setLoanInstallmentPaid: (
    installmentId: string,
    paid: boolean,
    paidDate?: string | null,
  ) => ({
    installmentId,
    paid,
    paidDate,
  }),
  updateLoanInstallmentDueDate: (
    installmentId: string,
    dueDate: string,
  ) => ({ installmentId, dueDate }),
};
