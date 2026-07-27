# Dashboard Layout System Implementation Plan (Cycle 1 of 2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild every RudeSync screen as a card-grid dashboard built from five shared primitives, on the "Deep forest" palette, replacing ~3,500 lines of per-screen scoped CSS.

**Architecture:** Extend the existing design tokens in `src/app.css` to the Deep forest surface ramp. Add one pure, unit-tested module (`stats.ts`) that derives dashboard numbers from data already loaded client-side. Add five presentational Svelte primitives. Then convert one screen per task, deleting that screen's scoped CSS as it starts composing primitives.

**Tech Stack:** Svelte 5, TypeScript, Vite, Tauri 2, `node --test`.

## Global Constraints

- Money is always integer **minor units**; never floating point. Per-currency only — **never sum across currencies**.
- Palette is fixed: ground `#070d0a`, cards `#0d1712`, raised `#142019`, overlay `#1a2820`. Accent `#3ddc84`, danger `#ff6961`, amber `#e3b341`.
- Semantic colour: accent = positive, danger = overdue, amber = due soon / outstanding, neutral = informational. **Colour is never the only carrier of meaning** — every status colour is paired with a text label.
- Type scale is whole pixels only (11, 12, 13, 15, 17, 20, 24, 28). Weights only 400/500/600/700. Radii only `--radius-control` 6px, `--radius-panel` 10px, `--radius-sheet` 14px, `--radius-pill`.
- Numbers displayed in stat cards and money columns use `font-variant-numeric: tabular-nums`.
- No navigation, route, IA, copy, backend, schema or command changes.
- No charts this cycle. Chart slots render an empty placeholder region only.
- The floating task widget (`src/widget/**`) is out of scope — do not touch it.
- `src/lib/features/money/moneyService.ts` **cannot be imported by node tests** (TypeScript parameter-property syntax breaks node's type-stripping). Test-facing modules may only import from `src/lib/domain/**` and `src/lib/**/types.ts`, which are verified node-importable.
- Svelte scopes styles per component instance: sibling selectors (`.a + .a`) never match across instances. Rules that span instances belong in `src/app.css`.
- Tests run `.ts` directly: `npm test` runs `node --test "tests/**/*.test.ts"`. Use explicit `.ts` import extensions.
- `npm run check` must report **0 errors** at the end of every task. Four pre-existing a11y warnings in `src/widget/TaskWidget.svelte` are expected and must remain untouched.

---

### Task 1: Deep forest palette

**Files:**
- Modify: `src/app.css` (the `--surface-*` block in `:root`, and the `body.has-window-effect` rule)

**Interfaces:**
- Produces: the surface ramp every later task builds on.

- [ ] **Step 1: Replace the surface ramp**

In `src/app.css`, inside `:root`, replace the four `--surface-*` declarations with:

```css
  /* Deep forest: the ground reads green, but stays dark enough that the
     emerald accent still functions as a signal rather than blending in. */
  --surface-window: #070d0a;
  --surface-content: #0d1712;
  --surface-raised: #142019;
  --surface-overlay: #1a2820;
```

Leave the legacy aliases (`--surface-0`, `--surface-1`, `--surface-2`) exactly as they are — they already point at the variables above.

- [ ] **Step 2: Simplify the sidebar tint**

The window-effect experiment is over. Replace the whole `body.has-window-effect` block with:

```css
/* Window translucency was removed: WebView2 renders over the DWM backdrop and
   making the window transparent produced artifacts. Depth comes from the
   surface ramp and elevation instead. */
body.has-window-effect .sidebar { background: rgb(0 0 0 / 0.22); }
```

- [ ] **Step 3: Verify**

Run: `npm run check`
Expected: `0 ERRORS`, 4 warnings (all in `src/widget/TaskWidget.svelte`).

Run: `npm run build`
Expected: build succeeds.

- [ ] **Step 4: Commit**

```bash
git add src/app.css
git commit -m "feat(ui): adopt the deep forest surface ramp"
```

---

### Task 2: Dashboard stat derivation (pure, TDD)

**Files:**
- Create: `src/lib/features/dashboard/stats.ts`
- Test: `tests/dashboard/stats.test.ts`

**Interfaces:**
- Consumes: `Task` from `src/lib/types.ts`; `Invoice` from `src/lib/features/money/types.ts`; `calculateInvoiceTotals` from `src/lib/domain/money.ts`.
- Produces (later tasks call these exact signatures):
  - `interface CurrencyAmount { currency: string; amountMinor: number }`
  - `openTaskCount(tasks: Task[]): number`
  - `completedTodayCount(tasks: Task[], today: string): number`
  - `overdueTaskCount(tasks: Task[], today: string): number`
  - `overdueInvoiceCount(invoices: Invoice[], today: string): number`
  - `outstandingByCurrency(invoices: Invoice[]): CurrencyAmount[]`
  - `receivedInMonth(invoices: Invoice[], today: string): CurrencyAmount[]`

Dates are ISO `YYYY-MM-DD` strings and compare lexicographically; do not construct `Date` objects.

- [ ] **Step 1: Write the failing test**

Create `tests/dashboard/stats.test.ts`:

```ts
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  openTaskCount,
  completedTodayCount,
  overdueTaskCount,
  overdueInvoiceCount,
  outstandingByCurrency,
  receivedInMonth,
} from "../../src/lib/features/dashboard/stats.ts";
import type { Task } from "../../src/lib/types.ts";
import type { Invoice } from "../../src/lib/features/money/types.ts";

function task(partial: Partial<Task> & { id: string }): Task {
  return {
    title: "Task",
    notes: null,
    plannedDate: null,
    dueDate: null,
    reminderAt: null,
    priority: "none",
    category: null,
    projectId: null,
    recurrence: "none",
    subtasks: [],
    status: "open",
    completedAt: null,
    createdAt: "2026-07-01T00:00:00.000Z",
    updatedAt: "2026-07-01T00:00:00.000Z",
    ...partial,
  } as Task;
}

function invoice(partial: Partial<Invoice> & { id: string }): Invoice {
  return {
    number: "INV-2026-07-01-0001",
    projectId: "p1",
    projectName: "Project",
    clientName: "Client",
    billToEmail: null,
    billToAddress: null,
    sellerName: "Seller",
    sellerEmail: null,
    sellerAddress: null,
    sellerLogoPath: null,
    milestoneId: null,
    milestoneLabel: null,
    milestoneKind: "custom",
    milestonePercentBasisPoints: null,
    issueDate: "2026-07-01",
    dueDate: "2026-07-15",
    termKind: "14-days",
    currency: "USD",
    lineItems: [
      { id: "l1", description: "Work", quantity: "1", unitPriceMinor: 100000 },
    ],
    discount: null,
    taxPercentage: null,
    notes: null,
    paymentInstructions: null,
    status: "issued",
    payments: [],
    createdAt: "2026-07-01T00:00:00.000Z",
    updatedAt: "2026-07-01T00:00:00.000Z",
    ...partial,
  } as Invoice;
}

test("openTaskCount counts only open tasks", () => {
  assert.equal(
    openTaskCount([
      task({ id: "1" }),
      task({ id: "2", status: "completed" }),
      task({ id: "3" }),
    ]),
    2,
  );
});

test("completedTodayCount matches the completion date only", () => {
  const tasks = [
    task({ id: "1", status: "completed", completedAt: "2026-07-28T09:00:00.000Z" }),
    task({ id: "2", status: "completed", completedAt: "2026-07-27T23:00:00.000Z" }),
    task({ id: "3" }),
  ];
  assert.equal(completedTodayCount(tasks, "2026-07-28"), 1);
});

test("overdueTaskCount ignores completed and undated tasks", () => {
  const tasks = [
    task({ id: "1", dueDate: "2026-07-20" }),
    task({ id: "2", dueDate: "2026-07-20", status: "completed" }),
    task({ id: "3", dueDate: "2026-07-28" }),
    task({ id: "4" }),
  ];
  assert.equal(overdueTaskCount(tasks, "2026-07-28"), 1);
});

test("overdueInvoiceCount ignores paid, draft and void invoices", () => {
  const invoices = [
    invoice({ id: "a", dueDate: "2026-07-20" }),
    invoice({ id: "b", dueDate: "2026-07-20", status: "draft" }),
    invoice({ id: "c", dueDate: "2026-07-20", status: "void" }),
    invoice({
      id: "d",
      dueDate: "2026-07-20",
      payments: [
        { id: "p", amountMinor: 100000, receivedDate: "2026-07-10", note: null, createdAt: "" },
      ],
    }),
    invoice({ id: "e", dueDate: "2026-07-30" }),
  ];
  assert.equal(overdueInvoiceCount(invoices, "2026-07-28"), 1);
});

test("outstandingByCurrency groups per currency and never merges them", () => {
  const result = outstandingByCurrency([
    invoice({ id: "a" }),
    invoice({ id: "b", currency: "EUR" }),
    invoice({ id: "c", status: "void" }),
  ]);
  assert.deepEqual(result, [
    { currency: "EUR", amountMinor: 100000 },
    { currency: "USD", amountMinor: 100000 },
  ]);
});

test("receivedInMonth sums payments inside the current month only", () => {
  const result = receivedInMonth(
    [
      invoice({
        id: "a",
        payments: [
          { id: "p1", amountMinor: 30000, receivedDate: "2026-07-05", note: null, createdAt: "" },
          { id: "p2", amountMinor: 20000, receivedDate: "2026-06-30", note: null, createdAt: "" },
        ],
      }),
    ],
    "2026-07-28",
  );
  assert.deepEqual(result, [{ currency: "USD", amountMinor: 30000 }]);
});

test("empty input returns zeroes, not errors", () => {
  assert.equal(openTaskCount([]), 0);
  assert.deepEqual(outstandingByCurrency([]), []);
  assert.deepEqual(receivedInMonth([], "2026-07-28"), []);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test tests/dashboard/stats.test.ts`
Expected: FAIL — cannot find module `../../src/lib/features/dashboard/stats.ts`.

- [ ] **Step 3: Write the implementation**

Create `src/lib/features/dashboard/stats.ts`:

```ts
import { calculateInvoiceTotals } from "../../domain/money.ts";
import type { Task } from "../../types.ts";
import type { Invoice } from "../money/types.ts";

export interface CurrencyAmount {
  currency: string;
  amountMinor: number;
}

/** Invoices that no longer represent money owed. */
function isInactive(invoice: Invoice): boolean {
  return invoice.status === "draft" || invoice.status === "void";
}

/**
 * `calculateInvoiceTotals` throws on an invoice with no line items, which a
 * dashboard must never do. Treat that as nothing owed.
 */
function balanceDueMinor(invoice: Invoice): number {
  if (invoice.lineItems.length === 0) return 0;
  return calculateInvoiceTotals({
    lineItems: invoice.lineItems,
    discount: invoice.discount,
    taxPercentage: invoice.taxPercentage,
    paymentsMinor: invoice.payments.map((payment) => payment.amountMinor),
  }).balanceDueMinor;
}

function sortByCurrency(totals: Map<string, number>): CurrencyAmount[] {
  return [...totals.entries()]
    .filter(([, amountMinor]) => amountMinor > 0)
    .map(([currency, amountMinor]) => ({ currency, amountMinor }))
    .sort((a, b) => a.currency.localeCompare(b.currency));
}

export function openTaskCount(tasks: Task[]): number {
  return tasks.filter((task) => task.status !== "completed").length;
}

export function completedTodayCount(tasks: Task[], today: string): number {
  return tasks.filter(
    (task) =>
      task.status === "completed" &&
      (task.completedAt ?? "").slice(0, 10) === today,
  ).length;
}

export function overdueTaskCount(tasks: Task[], today: string): number {
  return tasks.filter(
    (task) =>
      task.status !== "completed" &&
      task.dueDate !== null &&
      task.dueDate < today,
  ).length;
}

export function overdueInvoiceCount(invoices: Invoice[], today: string): number {
  return invoices.filter(
    (invoice) =>
      !isInactive(invoice) &&
      invoice.dueDate < today &&
      balanceDueMinor(invoice) > 0,
  ).length;
}

export function outstandingByCurrency(invoices: Invoice[]): CurrencyAmount[] {
  const totals = new Map<string, number>();
  for (const invoice of invoices) {
    if (isInactive(invoice)) continue;
    const balance = balanceDueMinor(invoice);
    if (balance <= 0) continue;
    totals.set(invoice.currency, (totals.get(invoice.currency) ?? 0) + balance);
  }
  return sortByCurrency(totals);
}

export function receivedInMonth(
  invoices: Invoice[],
  today: string,
): CurrencyAmount[] {
  const month = today.slice(0, 7);
  const totals = new Map<string, number>();
  for (const invoice of invoices) {
    if (invoice.status === "void") continue;
    for (const payment of invoice.payments) {
      if (payment.receivedDate.slice(0, 7) !== month) continue;
      totals.set(
        invoice.currency,
        (totals.get(invoice.currency) ?? 0) + payment.amountMinor,
      );
    }
  }
  return sortByCurrency(totals);
}
```

- [ ] **Step 4: Run tests**

Run: `node --test tests/dashboard/stats.test.ts`
Expected: PASS — 7 tests.

Run: `npm test`
Expected: all tests pass (24 existing + 7 new = 31).

- [ ] **Step 5: Commit**

```bash
git add src/lib/features/dashboard/stats.ts tests/dashboard/stats.test.ts
git commit -m "feat(dashboard): derive stat values from loaded data"
```

---

### Task 3: The five layout primitives

**Files:**
- Create: `src/lib/components/Card.svelte`
- Create: `src/lib/components/StatCard.svelte`
- Create: `src/lib/components/StatRow.svelte`
- Create: `src/lib/components/SectionHeader.svelte`
- Create: `src/lib/components/MeterBar.svelte`

**Interfaces:**
- Consumes: `Icon` from `src/lib/components/Icon.svelte`.
- Produces (every screen task composes these):
  - `Card` — props `padded?: boolean` (default `true`); slots `default`, `header`
  - `StatCard` — props `label: string`, `value: string`, `detail?: string`, `tone?: "neutral" | "positive" | "warning" | "danger"`, `icon?: IconName`
  - `StatRow` — no props; slot `default`
  - `SectionHeader` — props `title: string`, `subtext?: string`; slot `actions`
  - `MeterBar` — props `label: string`, `value: number`, `max: number`, `detail?: string`, `tone?: "neutral" | "positive" | "warning" | "danger"`

`IconName` is the union already declared by `export let name` in `src/lib/components/Icon.svelte`. Valid values include `today, tasks, work, briefcase, money, review, settings, plus, search, bell, chevron-right, calendar, check, clock, invoice, loan, arrow-up-right, more, link, spark, tray, database, palette, shield, keyboard, circle, x`.

- [ ] **Step 1: Create `Card.svelte`**

```svelte
<script lang="ts">
  export let padded = true;
</script>

<section class="card" class:padded>
  {#if $$slots.header}
    <header class="card-header"><slot name="header" /></header>
  {/if}
  <slot />
</section>

<style>
  .card {
    min-width: 0;
    background: var(--surface-content);
    border-radius: var(--radius-panel);
    box-shadow: var(--shadow-raised);
  }
  .padded { padding: var(--space-4); }
  .card-header {
    margin: calc(var(--space-4) * -1) calc(var(--space-4) * -1) var(--space-4);
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--separator);
  }
  .card:not(.padded) > .card-header { margin: 0; }
</style>
```

- [ ] **Step 2: Create `StatCard.svelte`**

```svelte
<script lang="ts">
  import Icon from "./Icon.svelte";

  export let label: string;
  export let value: string;
  export let detail: string | null = null;
  export let tone: "neutral" | "positive" | "warning" | "danger" = "neutral";
  export let icon: string | null = null;
</script>

<article class="stat" data-tone={tone}>
  <div class="stat-head">
    {#if icon}
      <span class="stat-icon"><Icon name={icon} size={14} /></span>
    {/if}
    <span class="stat-label">{label}</span>
  </div>
  <div class="stat-value">{value}</div>
  {#if detail}<div class="stat-detail">{detail}</div>{/if}
</article>

<style>
  .stat {
    min-width: 0;
    padding: var(--space-4);
    background: var(--surface-content);
    border-radius: var(--radius-panel);
    box-shadow: var(--shadow-raised);
  }
  .stat-head { display: flex; align-items: center; gap: var(--space-2); margin-bottom: var(--space-3); }
  .stat-icon {
    display: grid;
    width: 24px;
    height: 24px;
    flex: 0 0 auto;
    place-items: center;
    border-radius: var(--radius-control);
  }
  .stat-label { color: var(--text-secondary); font-size: var(--text-12); }
  .stat-value {
    font-size: var(--text-24);
    font-weight: var(--weight-semibold);
    letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums;
  }
  .stat-detail { margin-top: var(--space-1); color: var(--text-tertiary); font-size: var(--text-11); }

  /* Tone colours the value and the icon tile. It never changes layout, and the
     label always states the meaning in words so colour is not the only signal. */
  [data-tone="positive"] .stat-icon { color: var(--accent); background: var(--accent-fill); }
  [data-tone="positive"] .stat-value { color: var(--accent); }
  [data-tone="warning"] .stat-icon { color: var(--amber); background: var(--amber-fill); }
  [data-tone="danger"] .stat-icon { color: var(--danger); background: var(--danger-fill); }
  [data-tone="danger"] .stat-value { color: var(--danger); }
  [data-tone="neutral"] .stat-icon { color: var(--text-secondary); background: var(--surface-raised); }
</style>
```

- [ ] **Step 3: Create `StatRow.svelte`**

```svelte
<div class="stat-row"><slot /></div>

<style>
  .stat-row {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--space-3);
    max-width: 1120px;
    margin: 0 auto var(--space-4);
  }
  @media (max-width: 1020px) {
    .stat-row { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
  @media (max-width: 780px) {
    .stat-row { grid-template-columns: 1fr; }
  }
</style>
```

- [ ] **Step 4: Create `SectionHeader.svelte`**

```svelte
<script lang="ts">
  export let title: string;
  export let subtext: string | null = null;
</script>

<div class="section-header">
  <div class="section-copy">
    <h2>{title}</h2>
    {#if subtext}<p>{subtext}</p>{/if}
  </div>
  {#if $$slots.actions}
    <div class="section-actions"><slot name="actions" /></div>
  {/if}
</div>

<style>
  .section-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-4);
    min-width: 0;
  }
  .section-copy { min-width: 0; }
  h2 { font-size: var(--text-15); font-weight: var(--weight-semibold); }
  p { margin-top: 2px; color: var(--text-tertiary); font-size: var(--text-11); }
  .section-actions { display: flex; align-items: center; gap: var(--space-2); flex: 0 0 auto; }
</style>
```

- [ ] **Step 5: Create `MeterBar.svelte`**

```svelte
<script lang="ts">
  export let label: string;
  export let value: number;
  export let max: number;
  export let detail: string | null = null;
  export let tone: "neutral" | "positive" | "warning" | "danger" = "positive";

  $: percent = max > 0 ? Math.min(100, Math.max(0, (value / max) * 100)) : 0;
</script>

<div class="meter" data-tone={tone}>
  <div class="meter-head">
    <span class="meter-label">{label}</span>
    {#if detail}<span class="meter-detail">{detail}</span>{/if}
  </div>
  <div
    class="meter-track"
    role="progressbar"
    aria-label={label}
    aria-valuenow={value}
    aria-valuemin={0}
    aria-valuemax={max}
  >
    <i style={`width:${percent}%`}></i>
  </div>
</div>

<style>
  .meter { min-width: 0; }
  .meter-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-2);
    margin-bottom: var(--space-1);
  }
  .meter-label { font-size: var(--text-12); }
  .meter-detail {
    color: var(--text-tertiary);
    font-size: var(--text-11);
    font-variant-numeric: tabular-nums;
  }
  .meter-track {
    height: 5px;
    overflow: hidden;
    background: var(--surface-raised);
    border-radius: var(--radius-pill);
  }
  .meter-track i {
    display: block;
    height: 100%;
    background: var(--accent);
    border-radius: inherit;
    transition: width var(--duration) var(--ease);
  }
  [data-tone="warning"] .meter-track i { background: var(--amber); }
  [data-tone="danger"] .meter-track i { background: var(--danger); }
  [data-tone="neutral"] .meter-track i { background: var(--text-tertiary); }
</style>
```

- [ ] **Step 6: Verify**

Run: `npm run check`
Expected: `0 ERRORS`. Svelte may report the new components as unused until Task 4 imports them; that is not an error.

- [ ] **Step 7: Commit**

```bash
git add src/lib/components/Card.svelte src/lib/components/StatCard.svelte src/lib/components/StatRow.svelte src/lib/components/SectionHeader.svelte src/lib/components/MeterBar.svelte
git commit -m "feat(ui): add dashboard layout primitives"
```

---

### Task 4: Convert the Today screen

This is the pattern-setting task. Later screens copy its approach, so it gets the closest review.

**Files:**
- Modify: `src/App.svelte` (the `{#if active === "today"}` block, around lines 668-798, and the `<script>` imports)
- Modify: `src/app.css` (delete rules that the primitives now own)

**Interfaces:**
- Consumes: all five primitives from Task 3; `openTaskCount`, `completedTodayCount`, `overdueTaskCount`, `overdueInvoiceCount`, `outstandingByCurrency`, `receivedInMonth` from `src/lib/features/dashboard/stats.ts` (Task 2).

READ `src/App.svelte` fully before editing. It already loads `tasks`, `reviewInvoices` (all invoices), and has `formatMoney`-style helpers in the money feature; if no money formatter is in scope, format with the existing pattern used elsewhere in the file rather than inventing a new one.

- [ ] **Step 1: Import the primitives and stats**

Add to `src/App.svelte`'s `<script>`, beside the existing component imports:

```ts
  import Card from "./lib/components/Card.svelte";
  import StatCard from "./lib/components/StatCard.svelte";
  import StatRow from "./lib/components/StatRow.svelte";
  import SectionHeader from "./lib/components/SectionHeader.svelte";
  import MeterBar from "./lib/components/MeterBar.svelte";
  import {
    openTaskCount,
    completedTodayCount,
    overdueTaskCount,
    overdueInvoiceCount,
    outstandingByCurrency,
    receivedInMonth,
  } from "./lib/features/dashboard/stats.ts";
```

- [ ] **Step 2: Derive the four stat values**

Add reactive statements near the other `$:` declarations. `todayIso` is the existing ISO string for today used by the file (reuse whatever it is called there; do not add a second source of today):

```ts
  $: statOpen = openTaskCount(tasks);
  $: statDoneToday = completedTodayCount(tasks, todayIso);
  $: statOverdue = overdueTaskCount(tasks, todayIso) + overdueInvoiceCount(reviewInvoices, todayIso);
  $: statOutstanding = outstandingByCurrency(reviewInvoices);
  $: statReceived = receivedInMonth(reviewInvoices, todayIso);
```

For the two currency-grouped stats, display the **first** entry as the value and, when more than one currency is present, put the remainder in the `detail` line (e.g. `+ EUR 400.00`). Never add amounts across currencies.

- [ ] **Step 3: Replace the Today markup with the dashboard composition**

Inside `{#if active === "today"}`, replace the existing intro/grid markup with:

```svelte
      <StatRow>
        <StatCard
          icon="check"
          label="Open tasks"
          value={String(statOpen)}
          detail={`${statDoneToday} done today`}
          tone="neutral"
        />
        <StatCard
          icon="clock"
          label="Overdue"
          value={String(statOverdue)}
          detail={statOverdue === 0 ? "Nothing late" : "Tasks and invoices"}
          tone={statOverdue > 0 ? "danger" : "neutral"}
        />
        <StatCard
          icon="invoice"
          label="Outstanding"
          value={statOutstanding.length ? formatStatMoney(statOutstanding[0]) : "None"}
          detail={statOutstanding.length > 1 ? extraCurrencies(statOutstanding) : "Invoiced, not yet paid"}
          tone={statOutstanding.length ? "warning" : "neutral"}
        />
        <StatCard
          icon="arrow-up-right"
          label="Received"
          value={statReceived.length ? formatStatMoney(statReceived[0]) : "None"}
          detail={statReceived.length > 1 ? extraCurrencies(statReceived) : "This month"}
          tone={statReceived.length ? "positive" : "neutral"}
        />
      </StatRow>
```

Add the two small helpers to the `<script>`:

```ts
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
```

- [ ] **Step 4: Keep the existing quick-add, then compose the panels**

Leave the quick-add row exactly as it is. Convert the two-column area so each panel is a `Card` whose `header` slot holds a `SectionHeader`, e.g.:

```svelte
        <Card padded={false}>
          <SectionHeader slot="header" title="Today's tasks" subtext="Due or planned for today" />
          <!-- existing task list markup, unchanged -->
        </Card>
```

Add a reserved chart slot in the right-hand stack:

```svelte
        <Card>
          <SectionHeader slot="header" title="Earnings trend" />
          <div class="chart-slot" aria-hidden="true"></div>
        </Card>
```

- [ ] **Step 5: Add the chart-slot style to `src/app.css`**

```css
/* Reserved for Cycle 2. Renders as a quiet empty region, never a broken chart. */
.chart-slot {
  height: 96px;
  border: 1px dashed var(--separator-strong);
  border-radius: var(--radius-control);
}
```

- [ ] **Step 6: Delete the superseded Today rules from `src/app.css`**

Remove the rules the primitives now own: `.today-intro`, `.streak-chip`, `.today-grid`, `.panel`, `.panel-header`, `.metric-panel`, `.metric-heading`, `.metric-icon`, `.metric-value`, `.side-stack`, `.summary-card`, `.summary-grid`, and `.progress-track`. Do **not** delete `.task-list`, `.task-row` separator rules, `.quick-add`, `.empty-state`, or any rule still referenced by a screen that has not been converted yet — check with `grep -rn "class=\"[^\"]*<name>" src` before removing each one.

- [ ] **Step 7: Verify**

Run: `npm run check` → `0 ERRORS`.
Run: `npm test` → all pass.
Run: `npm run build` → succeeds.

- [ ] **Step 8: Commit**

```bash
git add src/App.svelte src/app.css
git commit -m "feat(ui): rebuild Today as a dashboard"
```

---

### Task 5: Convert the Tasks screen

**Files:**
- Modify: `src/App.svelte` (the `{:else if active === "tasks"}` block, around lines 799-845)
- Modify: `src/app.css` (delete superseded rules)

**Interfaces:**
- Consumes: `Card`, `SectionHeader` (Task 3).

No stat row on this screen.

- [ ] **Step 1: Compose the screen**

Wrap the filter toolbar and task list so the list lives in a `Card` with a `SectionHeader` in its `header` slot. Keep the existing `.segmented` filter control and `.inline-search` exactly as they are — move them into the header's `actions` slot:

```svelte
        <Card padded={false}>
          <SectionHeader slot="header" title="All tasks" subtext={taskFilterSubtext}>
            <svelte:fragment slot="actions">
              <!-- existing segmented filter + inline search markup, unchanged -->
            </svelte:fragment>
          </SectionHeader>
          <!-- existing task list markup, unchanged -->
        </Card>
```

Add `taskFilterSubtext` to the `<script>` as a reactive string describing the active filter, e.g.:

```ts
  $: taskFilterSubtext = `${visibleTasks.length} shown`;
```

Use whatever the file already calls the filtered task array; do not introduce a second filtering path.

- [ ] **Step 2: Delete superseded rules**

Remove `.section-toolbar` and `.full-panel` from `src/app.css` **only if** `grep -rn "section-toolbar\|full-panel" src` shows no remaining users.

- [ ] **Step 3: Verify**

Run: `npm run check` → `0 ERRORS`. Run: `npm test` → pass. Run: `npm run build` → succeeds.

- [ ] **Step 4: Commit**

```bash
git add src/App.svelte src/app.css
git commit -m "feat(ui): rebuild Tasks on the card layout"
```

---

### Task 6: Convert the Work screen

**Files:**
- Modify: `src/lib/features/work/WorkView.svelte` (markup and its 1213-line `<style>` block)

**Interfaces:**
- Consumes: `Card`, `StatCard`, `StatRow`, `SectionHeader`, `MeterBar` (Task 3); `planTotalMinor` from `src/lib/features/work/milestonePlans.ts`.

READ `WorkView.svelte` fully first. It is the largest file in the project; work through it section by section.

- [ ] **Step 1: Add the stat row**

Two stats, derived from data the view already has:

```svelte
  <StatRow>
    <StatCard icon="briefcase" label="Active projects" value={String(activeProjectCount)} detail="In flight" tone="neutral" />
    <StatCard icon="invoice" label="Remaining to invoice" value={remainingToInvoiceLabel} detail="Across active projects" tone="warning" />
  </StatRow>
```

Derive both from the existing project list. `remainingToInvoiceLabel` sums `amountMinor` of milestones whose `status === "not-invoiced"`, grouped per currency; render the first currency and never sum across currencies. Reuse the file's existing money formatter.

- [ ] **Step 2: Convert project cards**

Each project becomes a `Card` with a `SectionHeader` (title = project name, subtext = client name) and a `MeterBar` per milestone:

```svelte
      <MeterBar
        label={milestone.label}
        value={milestone.status === "not-invoiced" ? 0 : 1}
        max={1}
        detail={milestoneStatusLabel(milestone.status)}
        tone={milestone.status === "paid" ? "positive" : milestone.status === "invoiced" ? "warning" : "neutral"}
      />
```

Add `milestoneStatusLabel` returning `"Paid"`, `"Invoiced"` or `"Not invoiced"` so the status is stated in words, not only colour.

- [ ] **Step 3: Delete the superseded scoped CSS**

Delete every rule in `WorkView.svelte`'s `<style>` block that the primitives now provide (its own card, panel, header, badge, meter and grid rules). Keep only rules for markup unique to this screen that no primitive covers. `npm run check` reports unused selectors, which is the signal for what else can go.

- [ ] **Step 4: Verify**

Run: `npm run check` → `0 ERRORS` and no unused-selector warnings in `WorkView.svelte`.
Run: `npm test` → pass. Run: `npm run build` → succeeds.

- [ ] **Step 5: Commit**

```bash
git add src/lib/features/work/WorkView.svelte
git commit -m "feat(ui): rebuild Work on the card layout"
```

---

### Task 7: Convert the Money screen

**Files:**
- Modify: `src/lib/features/money/MoneyView.svelte` (markup and its 178-line `<style>` block)

**Interfaces:**
- Consumes: `Card`, `StatCard`, `StatRow`, `SectionHeader` (Task 3); `outstandingByCurrency`, `receivedInMonth`, `overdueInvoiceCount` from `stats.ts` (Task 2).

- [ ] **Step 1: Add the stat row**

```svelte
  <StatRow>
    <StatCard
      icon="invoice"
      label="Outstanding"
      value={moneyOutstanding.length ? formatMoney(moneyOutstanding[0].amountMinor, moneyOutstanding[0].currency) : "None"}
      detail="Invoiced, not yet paid"
      tone={moneyOutstanding.length ? "warning" : "neutral"}
    />
    <StatCard
      icon="arrow-up-right"
      label="Received"
      value={moneyReceived.length ? formatMoney(moneyReceived[0].amountMinor, moneyReceived[0].currency) : "None"}
      detail="This month"
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
```

Declare the three values reactively, using the view's existing `invoices` array
and its existing `formatMoney(minor, currency)` helper:

```ts
  $: moneyOutstanding = outstandingByCurrency(invoices);
  $: moneyReceived = receivedInMonth(invoices, today);
  $: overdueInvoices = overdueInvoiceCount(invoices, today);
```

Reuse the file's existing `today` ISO string; do not introduce a second one.
Render the first currency only. When more than one currency is present, append
the rest to `detail` rather than adding them together.

- [ ] **Step 2: Convert the invoice and payment panels**

Each becomes a `Card` with a `SectionHeader` in the `header` slot; existing filters move into the `actions` slot. Add one reserved chart slot card:

```svelte
      <Card>
        <SectionHeader slot="header" title="Invoiced vs received" />
        <div class="chart-slot" aria-hidden="true"></div>
      </Card>
```

- [ ] **Step 3: Delete the superseded scoped CSS**, guided by unused-selector warnings.

- [ ] **Step 4: Verify**

Run: `npm run check` → `0 ERRORS`. Run: `npm test` → pass. Run: `npm run build` → succeeds.

- [ ] **Step 5: Commit**

```bash
git add src/lib/features/money/MoneyView.svelte
git commit -m "feat(ui): rebuild Money on the card layout"
```

---

### Task 8: Convert the Review screen

**Files:**
- Modify: `src/App.svelte` (the `{:else if active === "review"}` block, around lines 853-900)
- Modify: `src/app.css` (delete `.review-hero`, `.review-score` once unused)

**Interfaces:**
- Consumes: `Card`, `StatCard`, `StatRow`, `SectionHeader`, `MeterBar` (Task 3).

This screen is the most chart-heavy in Cycle 2, so it reserves the most slots.

- [ ] **Step 1: Replace the review hero with a stat row**

```svelte
      <StatRow>
        <StatCard icon="check" label="Tasks done" value={String(completedThisWeek.length)} detail="This week" tone="positive" />
        <StatCard icon="work" label="Work recorded" value={String(reviewWorkEntries.length)} detail="Entries logged" tone="neutral" />
        <StatCard
          icon="arrow-up-right"
          label="Received"
          value={reviewReceived.length ? formatStatMoney(reviewReceived[0]) : "None"}
          detail="This month"
          tone={reviewReceived.length ? "positive" : "neutral"}
        />
        <StatCard icon="loan" label="Loan payments due" value={String(upcomingInstallmentCount)} detail="Next 30 days" tone="warning" />
      </StatRow>
```

Use the arrays the screen already loads (`completedThisWeek`, `reviewWorkEntries`, `reviewInvoices`, `reviewLoans`) and the `formatStatMoney` helper added to `App.svelte` in Task 4:

```ts
  $: reviewReceived = receivedInMonth(reviewInvoices, todayIso);
  $: upcomingInstallmentCount = reviewLoans
    .flatMap((loan) => loan.installments)
    .filter(
      (installment) =>
        !installment.paid &&
        installment.dueDate >= todayIso &&
        installment.dueDate <= addDays(todayIso, 30),
    ).length;
```

`addDays(date: string, offsetDays: number)` is exported from `src/lib/domain/date.ts`; import it if it is not already in scope. `LoanInstallment` has the fields `{ id, installmentNumber, dueDate, paid, paidDate }`.

- [ ] **Step 2: Weekly progress as a MeterBar**

Replace the percentage figure with:

```svelte
      <MeterBar
        label="Weekly progress"
        value={completedThisWeek.length}
        max={weeklyProgressTotal}
        detail={`${completedThisWeek.length} of ${weeklyProgressTotal} done`}
        tone="positive"
      />
```

- [ ] **Step 3: Add two reserved chart slots** in `Card`s titled "Completion trend" and "Earnings by month", each containing `<div class="chart-slot" aria-hidden="true"></div>`.

- [ ] **Step 4: Delete `.review-hero` and `.review-score`** from `src/app.css` once `grep -rn "review-hero\|review-score" src` is clean.

- [ ] **Step 5: Verify**

Run: `npm run check` → `0 ERRORS`. Run: `npm test` → pass. Run: `npm run build` → succeeds.

- [ ] **Step 6: Commit**

```bash
git add src/App.svelte src/app.css
git commit -m "feat(ui): rebuild Review on the card layout"
```

---

### Task 9: Convert Settings, forms and dialogs

**Files:**
- Modify: `src/lib/features/settings/SettingsView.svelte` (697-line `<style>`)
- Modify: `src/lib/features/settings/DataExports.svelte`
- Modify: `src/lib/features/tasks/TaskDialog.svelte`
- Modify: `src/lib/features/work/components/ProjectForm.svelte`
- Modify: `src/lib/features/work/components/ClientForm.svelte`
- Modify: `src/lib/features/work/components/WorkEntryForm.svelte`
- Modify: `src/lib/features/work/components/WorkDialog.svelte`
- Modify: `src/app.css` (shared form/dialog rules)

**Interfaces:**
- Consumes: `Card`, `SectionHeader` (Task 3).

- [ ] **Step 1: Settings**

Each settings group becomes a `Card` with a `SectionHeader`. No stat row. Delete the scoped rules the primitives replace.

- [ ] **Step 2: Unify form controls in `src/app.css`**

The four forms each restyle the same inputs. Add one shared set of rules and delete the per-file duplicates:

```css
/* Shared form controls. Individual forms must not restyle these. */
.field-label { display: block; margin-bottom: var(--space-1); color: var(--text-secondary); font-size: var(--text-12); }
.field-input,
.field-textarea {
  width: 100%;
  padding: var(--space-2) var(--space-3);
  color: var(--text-primary);
  font: inherit;
  font-size: var(--text-13);
  background: var(--surface-window);
  border: 1px solid var(--separator-strong);
  border-radius: var(--radius-control);
}
.field-input { height: 32px; }
.field-textarea { min-height: 64px; line-height: 1.5; resize: vertical; }
.field-input:focus-visible,
.field-textarea:focus-visible { border-color: var(--accent); outline: none; }
.field-hint { margin-top: var(--space-1); color: var(--text-tertiary); font-size: var(--text-11); }
.field-error { margin-top: var(--space-1); color: var(--danger); font-size: var(--text-11); }
```

Apply these classes in each form and delete that form's duplicate input styling. Keep every existing label, hint, validation message and `required` attribute exactly as it is — this is a styling change, not a forms change.

- [ ] **Step 3: Dialogs**

Give `TaskDialog` and `WorkDialog` the sheet treatment already used by `.setup-dialog`: `--surface-overlay`, `--radius-sheet`, `--shadow-sheet`. Delete their duplicate scoped rules.

- [ ] **Step 4: Verify**

Run: `npm run check` → `0 ERRORS`, and no unused-selector warnings in any converted file.
Run: `npm test` → all pass.
Run: `npm run build` → succeeds.

- [ ] **Step 5: Commit**

```bash
git add src/lib/features/settings src/lib/features/tasks/TaskDialog.svelte src/lib/features/work/components src/app.css
git commit -m "feat(ui): rebuild Settings, forms and dialogs on the card layout"
```

---

## Self-Review

**Spec coverage:**
- Deep forest palette → Task 1 ✓
- Stat derivation, per-currency, integer minor units → Task 2 ✓
- Five primitives with the spec's exact props → Task 3 ✓
- Today stat row (tasks / overdue / outstanding / received) → Task 4 ✓
- Stat rows on Work, Money, Review → Tasks 6, 7, 8 ✓
- Tasks and Settings without stat rows → Tasks 5, 9 ✓
- Sidebar unchanged → no task touches it ✓
- Reserved chart slots → Tasks 4, 7, 8 (`.chart-slot`) ✓
- Semantic colour paired with a text label → Task 3 (`StatCard` label + detail), Task 6 (`milestoneStatusLabel`) ✓
- ~3,500 lines of scoped CSS deleted → Tasks 4-9 each delete their screen's rules ✓
- Typography foundation retained → no task alters the type scale ✓
- Widget untouched → stated in Global Constraints ✓

**Placeholder scan:** No TBD/TODO. Tasks 4-9 instruct reading the large existing files before editing and give the exact composition to apply; the `…` in Tasks 7 and 8 stat values refer to values computed by the named Task 2 helpers with the file's existing formatter, which the preceding step specifies.

**Type consistency:** `CurrencyAmount { currency, amountMinor }` is used identically in Tasks 2 and 4. Primitive prop names (`label`, `value`, `detail`, `tone`, `icon`, `title`, `subtext`, `max`, `padded`) match between Task 3's definitions and every consuming task. `tone` union is the same four values throughout. Stats function names match between Task 2 and Tasks 4, 7, 8.

## Open Risks

- `App.svelte` already holds Today, Tasks and Review; Tasks 4, 5 and 8 all modify it. Run them in order and re-read the file at the start of each.
- Deleting shared rules from `src/app.css` while later screens still use them would break those screens mid-plan. Every deletion step requires a `grep` check first; when in doubt, defer the deletion to Task 9.
- `todayIso` in Task 4 must reuse the file's existing today value. Introducing a second source of "today" would make stats disagree with the lists beside them.
