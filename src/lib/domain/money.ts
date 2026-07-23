/**
 * Monetary values are integer minor units (for example, USD cents).
 * Decimal percentages and quantities are accepted as strings so calculations
 * never depend on binary floating-point arithmetic.
 */
export type DecimalString = string;

export interface InvoiceLineCalculationInput {
  readonly unitPriceMinor: number;
  readonly quantity: DecimalString;
}

export type InvoiceDiscount =
  | {
      readonly kind: "fixed";
      readonly amountMinor: number;
    }
  | {
      readonly kind: "percentage";
      /** "10" means 10%; "7.25" means 7.25%. */
      readonly percentage: DecimalString;
    };

export interface InvoiceTotalsInput {
  readonly lineItems: readonly InvoiceLineCalculationInput[];
  readonly discount?: InvoiceDiscount | null;
  /** "10" means 10%; "7.25" means 7.25%. */
  readonly taxPercentage?: DecimalString | null;
  readonly paymentsMinor?: readonly number[];
}

export interface InvoiceSubtotalTotalsInput {
  readonly subtotalMinor: number;
  readonly discount?: InvoiceDiscount | null;
  readonly taxPercentage?: DecimalString | null;
  readonly paymentsMinor?: readonly number[];
}

export interface InvoiceTotals {
  readonly subtotalMinor: number;
  readonly discountMinor: number;
  readonly discountedSubtotalMinor: number;
  readonly taxMinor: number;
  readonly totalMinor: number;
  readonly paidMinor: number;
  readonly balanceDueMinor: number;
  readonly creditMinor: number;
}

interface ParsedDecimal {
  readonly numerator: bigint;
  readonly denominator: bigint;
}

// The app runs on Windows 11 WebView2 (and modern WKWebView in a future macOS
// build), both of which support BigInt. Declaring the constructor locally lets
// this exact-arithmetic module coexist with the repository's older TS lib
// target without changing global compiler settings.
declare function BigInt(value: number | string): bigint;

const UNSIGNED_DECIMAL_PATTERN = /^(?:0|[1-9]\d*)(?:\.(\d+))?$/;
const MAX_SAFE_BIGINT = BigInt(Number.MAX_SAFE_INTEGER);
const BIGINT_ZERO = BigInt(0);
const BIGINT_TWO = BigInt(2);
const BIGINT_TEN = BigInt(10);
const BIGINT_ONE_HUNDRED = BigInt(100);

export function sumMinorUnits(amounts: readonly number[]): number {
  let total = BIGINT_ZERO;
  for (const amount of amounts) {
    assertNonnegativeMinorUnits(amount, "amount");
    total += BigInt(amount);
  }
  return toSafeMinorUnits(total, "minor-unit sum");
}

export function calculateLineItemTotal(
  unitPriceMinor: number,
  quantity: DecimalString,
): number {
  assertNonnegativeMinorUnits(unitPriceMinor, "unitPriceMinor");
  const parsedQuantity = parseUnsignedDecimal(quantity, "quantity");
  if (parsedQuantity.numerator === BIGINT_ZERO) {
    throw new RangeError("quantity must be greater than zero.");
  }

  return toSafeMinorUnits(
    roundRatio(
      BigInt(unitPriceMinor) * parsedQuantity.numerator,
      parsedQuantity.denominator,
    ),
    "line-item total",
  );
}

export function calculatePercentageOfMinorUnits(
  amountMinor: number,
  percentage: DecimalString,
): number {
  assertNonnegativeMinorUnits(amountMinor, "amountMinor");
  const parsedPercentage = parseUnsignedDecimal(percentage, "percentage");
  return toSafeMinorUnits(
    roundRatio(
      BigInt(amountMinor) * parsedPercentage.numerator,
      parsedPercentage.denominator * BIGINT_ONE_HUNDRED,
    ),
    "percentage amount",
  );
}

export function calculateInvoiceTotals(
  input: InvoiceTotalsInput,
): InvoiceTotals {
  if (input.lineItems.length === 0) {
    throw new RangeError("An invoice must contain at least one line item.");
  }

  const subtotalMinor = sumMinorUnits(
    input.lineItems.map((lineItem) =>
      calculateLineItemTotal(lineItem.unitPriceMinor, lineItem.quantity),
    ),
  );

  return calculateInvoiceTotalsFromSubtotal({
    subtotalMinor,
    discount: input.discount,
    taxPercentage: input.taxPercentage,
    paymentsMinor: input.paymentsMinor,
  });
}

export function calculateInvoiceTotalsFromSubtotal(
  input: InvoiceSubtotalTotalsInput,
): InvoiceTotals {
  assertNonnegativeMinorUnits(input.subtotalMinor, "subtotalMinor");

  const discountMinor = calculateDiscount(
    input.subtotalMinor,
    input.discount ?? null,
  );
  const discountedSubtotalMinor = input.subtotalMinor - discountMinor;
  const taxMinor =
    input.taxPercentage == null
      ? 0
      : calculatePercentageOfMinorUnits(
          discountedSubtotalMinor,
          input.taxPercentage,
        );
  const totalMinor = checkedAdd(
    discountedSubtotalMinor,
    taxMinor,
    "invoice total",
  );
  const paidMinor = sumMinorUnits(input.paymentsMinor ?? []);

  return {
    subtotalMinor: input.subtotalMinor,
    discountMinor,
    discountedSubtotalMinor,
    taxMinor,
    totalMinor,
    paidMinor,
    balanceDueMinor: Math.max(totalMinor - paidMinor, 0),
    creditMinor: Math.max(paidMinor - totalMinor, 0),
  };
}

function calculateDiscount(
  subtotalMinor: number,
  discount: InvoiceDiscount | null,
): number {
  if (discount == null) return 0;

  if (discount.kind === "fixed") {
    assertNonnegativeMinorUnits(discount.amountMinor, "discount.amountMinor");
    if (discount.amountMinor > subtotalMinor) {
      throw new RangeError("A fixed discount cannot exceed the subtotal.");
    }
    return discount.amountMinor;
  }

  if (discount.kind === "percentage") {
    const percentage = parseUnsignedDecimal(
      discount.percentage,
      "discount.percentage",
    );
    if (
      percentage.numerator >
      percentage.denominator * BIGINT_ONE_HUNDRED
    ) {
      throw new RangeError("A percentage discount cannot exceed 100%.");
    }
    return calculatePercentageOfMinorUnits(
      subtotalMinor,
      discount.percentage,
    );
  }

  return assertNever(discount);
}

function parseUnsignedDecimal(
  value: DecimalString,
  label: string,
): ParsedDecimal {
  if (typeof value !== "string") {
    throw new TypeError(
      `${label} must be a decimal string so it can be calculated exactly.`,
    );
  }

  const normalized = value.trim();
  const match = UNSIGNED_DECIMAL_PATTERN.exec(normalized);
  if (!match) {
    throw new RangeError(
      `${label} must be a non-negative decimal without an exponent; received "${value}".`,
    );
  }

  const fraction = match[1] ?? "";
  const digits = normalized.replace(".", "");
  return {
    numerator: BigInt(digits),
    denominator: powerOfTen(fraction.length),
  };
}

/**
 * Rounds a non-negative rational to the nearest integer, with exact halves up.
 */
function roundRatio(numerator: bigint, denominator: bigint): bigint {
  if (numerator < BIGINT_ZERO || denominator <= BIGINT_ZERO) {
    throw new RangeError("roundRatio expects a non-negative value.");
  }
  return (
    (numerator * BIGINT_TWO + denominator) /
    (denominator * BIGINT_TWO)
  );
}

function checkedAdd(left: number, right: number, label: string): number {
  return toSafeMinorUnits(BigInt(left) + BigInt(right), label);
}

function toSafeMinorUnits(value: bigint, label: string): number {
  if (value < BIGINT_ZERO || value > MAX_SAFE_BIGINT) {
    throw new RangeError(`${label} exceeds the safe integer range.`);
  }
  return Number(value);
}

function powerOfTen(exponent: number): bigint {
  let result = BigInt(1);
  for (let index = 0; index < exponent; index += 1) {
    result *= BIGINT_TEN;
  }
  return result;
}

function assertNonnegativeMinorUnits(value: number, label: string): void {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new TypeError(
      `${label} must be a non-negative safe integer in minor units; received ${value}.`,
    );
  }
}

function assertNever(value: never): never {
  throw new TypeError(`Unsupported discount: ${String(value)}`);
}
