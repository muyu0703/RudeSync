import {
  addDays,
  compareIsoDates,
  normalizeIsoDate,
  type IsoDate,
} from "./date.ts";

export const STANDARD_INVOICE_DUE_DAYS = [0, 7, 14, 30] as const;

export type StandardInvoiceDueDays =
  (typeof STANDARD_INVOICE_DUE_DAYS)[number];

export type InvoiceDueTerm =
  | StandardInvoiceDueDays
  | {
      readonly kind: "custom";
      readonly dueDate: string;
    };

export type InvoiceNumber =
  `INV-${number}-${number}-${number}-${string}`;

export interface ParsedInvoiceNumber {
  readonly issueDate: IsoDate;
  readonly sequence: number;
}

const INVOICE_NUMBER_PATTERN = /^INV-(\d{4}-\d{2}-\d{2})-(\d{4})$/;
const MIN_INVOICE_SEQUENCE = 1;
const MAX_INVOICE_SEQUENCE = 9999;

export function formatInvoiceNumber(
  issueDate: string,
  sequence: number,
): InvoiceNumber {
  const date = normalizeIsoDate(issueDate);
  assertInvoiceSequence(sequence);
  return `INV-${date}-${String(sequence).padStart(4, "0")}` as InvoiceNumber;
}

export function parseInvoiceNumber(value: string): ParsedInvoiceNumber {
  const match = INVOICE_NUMBER_PATTERN.exec(value);
  if (!match) {
    throw new RangeError(
      `Invalid invoice number "${value}". Expected INV-YYYY-MM-DD-####.`,
    );
  }

  const issueDate = normalizeIsoDate(match[1]);
  const sequence = Number(match[2]);
  assertInvoiceSequence(sequence);
  return { issueDate, sequence };
}

export function tryParseInvoiceNumber(
  value: string,
): ParsedInvoiceNumber | null {
  try {
    return parseInvoiceNumber(value);
  } catch {
    return null;
  }
}

/**
 * Finds the next monotonically increasing sequence for an issue date.
 *
 * Persistence code should reserve the returned number in the same database
 * transaction used to create/update the draft so concurrent callers cannot
 * claim the same sequence.
 */
export function nextInvoiceSequence(
  issueDate: string,
  existingNumbers: readonly string[],
): number {
  const date = normalizeIsoDate(issueDate);
  let highestSequence = 0;

  for (const value of existingNumbers) {
    const parsed = tryParseInvoiceNumber(value);
    if (parsed?.issueDate === date && parsed.sequence > highestSequence) {
      highestSequence = parsed.sequence;
    }
  }

  const nextSequence = highestSequence + 1;
  assertInvoiceSequence(nextSequence);
  return nextSequence;
}

export function nextInvoiceNumber(
  issueDate: string,
  existingNumbers: readonly string[],
): InvoiceNumber {
  return formatInvoiceNumber(
    issueDate,
    nextInvoiceSequence(issueDate, existingNumbers),
  );
}

export function calculateInvoiceDueDate(
  issueDate: string,
  term: InvoiceDueTerm,
): IsoDate {
  const normalizedIssueDate = normalizeIsoDate(issueDate);

  if (typeof term === "number") {
    if (!STANDARD_INVOICE_DUE_DAYS.includes(term as StandardInvoiceDueDays)) {
      throw new RangeError(
        `Unsupported invoice due term "${term}". Use 0, 7, 14, 30, or a custom date.`,
      );
    }
    return addDays(normalizedIssueDate, term);
  }

  if (term.kind !== "custom") {
    throw new TypeError("Invalid invoice due term.");
  }

  const customDueDate = normalizeIsoDate(term.dueDate);
  if (compareIsoDates(customDueDate, normalizedIssueDate) < 0) {
    throw new RangeError("A custom due date cannot be before the issue date.");
  }
  return customDueDate;
}

function assertInvoiceSequence(sequence: number): void {
  if (
    !Number.isSafeInteger(sequence) ||
    sequence < MIN_INVOICE_SEQUENCE ||
    sequence > MAX_INVOICE_SEQUENCE
  ) {
    throw new RangeError(
      `Invoice sequence must be an integer from ${MIN_INVOICE_SEQUENCE} through ${MAX_INVOICE_SEQUENCE}; received ${sequence}.`,
    );
  }
}
