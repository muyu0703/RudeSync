import {
  addDays,
  addMonthsClamped,
  compareIsoDates,
  dateForDayOfMonth,
  normalizeIsoDate,
  parseIsoDate,
  type IsoDate,
} from "./date.ts";

export type EveryTwoWeeksFrequency = "biweekly" | "every-two-weeks";

export type LoanScheduleInput =
  | {
      readonly frequency: "monthly" | "weekly" | EveryTwoWeeksFrequency;
      readonly firstPaymentDate: string;
      readonly installmentCount: number;
    }
  | {
      readonly frequency: "twice-monthly";
      readonly firstPaymentDate: string;
      readonly installmentCount: number;
      readonly paymentDays: readonly [number, number];
    }
  | {
      readonly frequency: "custom";
      readonly firstPaymentDate: string;
      readonly dueDates: readonly string[];
    };

export interface GeneratedLoanInstallment {
  readonly installmentNumber: number;
  readonly dueDate: IsoDate;
}

const MAX_INSTALLMENTS = 10_000;

export function generateLoanDueDates(
  input: LoanScheduleInput,
): readonly IsoDate[] {
  const firstPaymentDate = normalizeIsoDate(input.firstPaymentDate);

  switch (input.frequency) {
    case "monthly": {
      assertInstallmentCount(input.installmentCount);
      const anchorDay = parseIsoDate(firstPaymentDate).day;
      return Array.from({ length: input.installmentCount }, (_, index) =>
        addMonthsClamped(firstPaymentDate, index, anchorDay),
      );
    }

    case "weekly":
      assertInstallmentCount(input.installmentCount);
      return generateFixedDaySchedule(
        firstPaymentDate,
        input.installmentCount,
        7,
      );

    case "biweekly":
    case "every-two-weeks":
      assertInstallmentCount(input.installmentCount);
      return generateFixedDaySchedule(
        firstPaymentDate,
        input.installmentCount,
        14,
      );

    case "twice-monthly":
      return generateTwiceMonthlySchedule(
        firstPaymentDate,
        input.installmentCount,
        input.paymentDays,
      );

    case "custom":
      return validateCustomSchedule(firstPaymentDate, input.dueDates);

    default:
      return assertNever(input);
  }
}

export function generateLoanSchedule(
  input: LoanScheduleInput,
): readonly GeneratedLoanInstallment[] {
  return generateLoanDueDates(input).map((dueDate, index) => ({
    installmentNumber: index + 1,
    dueDate,
  }));
}

function generateFixedDaySchedule(
  firstPaymentDate: IsoDate,
  installmentCount: number,
  intervalDays: number,
): readonly IsoDate[] {
  return Array.from({ length: installmentCount }, (_, index) =>
    addDays(firstPaymentDate, index * intervalDays),
  );
}

function generateTwiceMonthlySchedule(
  firstPaymentDate: IsoDate,
  installmentCount: number,
  paymentDays: readonly [number, number],
): readonly IsoDate[] {
  assertInstallmentCount(installmentCount);
  const [firstDay, secondDay] = paymentDays;
  assertPaymentDay(firstDay, "paymentDays[0]");
  assertPaymentDay(secondDay, "paymentDays[1]");
  if (firstDay === secondDay) {
    throw new RangeError("Twice-monthly payment days must differ.");
  }

  const firstDateParts = parseIsoDate(firstPaymentDate);
  const firstMonthCandidates = monthlyCandidates(
    firstDateParts.year,
    firstDateParts.month,
    paymentDays,
  );
  if (!firstMonthCandidates.includes(firstPaymentDate)) {
    throw new RangeError(
      "The first payment date must match one of the selected twice-monthly payment days (after month-end clamping).",
    );
  }

  const dueDates: IsoDate[] = [];
  let monthOffset = 0;

  while (dueDates.length < installmentCount) {
    const monthDate = addMonthsClamped(firstPaymentDate, monthOffset, 1);
    const { year, month } = parseIsoDate(monthDate);
    const candidates = monthlyCandidates(year, month, paymentDays);

    for (const candidate of candidates) {
      if (
        compareIsoDates(candidate, firstPaymentDate) >= 0 &&
        dueDates.length < installmentCount
      ) {
        dueDates.push(candidate);
      }
    }

    monthOffset += 1;
  }

  return dueDates;
}

function monthlyCandidates(
  year: number,
  month: number,
  paymentDays: readonly [number, number],
): IsoDate[] {
  // Do not deduplicate clamped dates: two distinct scheduled installments can
  // legitimately land on the same final day of a short month.
  return paymentDays
    .map((paymentDay) => dateForDayOfMonth(year, month, paymentDay))
    .sort(compareIsoDates);
}

function validateCustomSchedule(
  firstPaymentDate: IsoDate,
  dueDates: readonly string[],
): readonly IsoDate[] {
  if (dueDates.length === 0) {
    throw new RangeError("A custom schedule must contain at least one due date.");
  }
  if (dueDates.length > MAX_INSTALLMENTS) {
    throw new RangeError(
      `A schedule cannot contain more than ${MAX_INSTALLMENTS} installments.`,
    );
  }

  const normalizedDates = dueDates.map(normalizeIsoDate);
  if (normalizedDates[0] !== firstPaymentDate) {
    throw new RangeError(
      "The first custom due date must equal the explicit first payment date.",
    );
  }

  for (let index = 1; index < normalizedDates.length; index += 1) {
    if (compareIsoDates(normalizedDates[index - 1], normalizedDates[index]) >= 0) {
      throw new RangeError(
        "Custom due dates must be unique and in chronological order.",
      );
    }
  }

  return normalizedDates;
}

function assertInstallmentCount(value: number): void {
  if (
    !Number.isSafeInteger(value) ||
    value < 1 ||
    value > MAX_INSTALLMENTS
  ) {
    throw new RangeError(
      `installmentCount must be an integer from 1 through ${MAX_INSTALLMENTS}; received ${value}.`,
    );
  }
}

function assertPaymentDay(value: number, label: string): void {
  if (!Number.isSafeInteger(value) || value < 1 || value > 31) {
    throw new RangeError(
      `${label} must be an integer from 1 through 31; received ${value}.`,
    );
  }
}

function assertNever(value: never): never {
  throw new TypeError(`Unsupported loan frequency: ${String(value)}`);
}
