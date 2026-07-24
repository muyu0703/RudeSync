# Flexible Project Milestones — Design

Status: Approved scope, ready for implementation plan
Date: 2026-07-24
Feature area: Work (projects) + Money (invoices)
Platform: Windows 11 (Tauri 2). Keep platform-specific code isolated for a future macOS build.

## 1. Goal

Replace the rigid two-milestone model (fixed `kickoff` + `completion` summing to
100%) with a flexible, per-project **milestone plan**: any number of milestones,
each with its own explicit amount, created from quick-start templates or by hand,
addable at any time (including out-of-scope "additional feature" work after the
project has started). Each milestone tracks its billing status (not invoiced /
invoiced / paid) by linking to the invoice that bills it, giving the project a
real billing dashboard.

## 2. Confirmed decisions (fixed inputs)

- **Data model:** a normalized `project_milestones` table; invoices gain a
  nullable `milestone_id` link. (Chosen over storing milestones as JSON, which
  can't cleanly support per-milestone invoiced/paid status.)
- **Amounts:** each milestone has an **explicit amount** in integer minor units.
  The **project total = SUM(milestone amounts)**. The existing
  `quoted_total_minor` is retained only as an optional "original quote"
  reference; it no longer drives percentages.
- **Templates (frontend conveniences that seed ordinary milestone rows):**
  Kickoff + Completion (classic), Even weekly, Phase-by-phase, Custom/empty.
- **Additional feature:** just an "Add milestone" action (kind `additional`)
  available anytime, including after invoices exist. It raises the project total
  and never recalculates existing invoices.
- **Status tracking:** each milestone shows Not invoiced → Invoiced → Paid,
  derived from its linked invoice.
- **One active invoice per milestone:** a milestone is billed by at most one
  non-void invoice. To re-bill, void the existing invoice first.

## 3. Current state (what changes)

- `projects` table stores milestones as fixed columns: `quoted_total_minor`,
  `kickoff_percent_basis_points`, `kickoff_label`, `completion_percent_basis_points`,
  `completion_label`, with a CHECK that the two percents total 10000.
- `invoices` table stores a milestone **snapshot**: `milestone_kind`
  (`kickoff|completion|custom`), `milestone_label`, `milestone_percent_basis_points`
  — but no link to a specific milestone.
- Rust `domain.rs` `create/update project` validates exactly two milestones via
  `validate_milestone_split`. Frontend `ProjectMilestone` is
  `{ kind, label, percentBasisPoints }` and the UI edits exactly two.
- `MoneyView` invoice form picks a milestone from the project's two milestones.

## 4. Data model

### 4.1 New table `project_milestones`

One row per milestone (mirrors the app's existing table conventions — UUID PK,
timestamps, soft delete):

- `id TEXT PRIMARY KEY` (UUID)
- `project_id TEXT NOT NULL` → `projects(id)`
- `label TEXT NOT NULL` (≤ 80 chars)
- `amount_minor INTEGER NOT NULL CHECK (amount_minor >= 0)`
- `kind TEXT NOT NULL DEFAULT 'custom'` CHECK in
  (`kickoff`,`completion`,`phase`,`weekly`,`additional`,`custom`) — cosmetic tag
  only (icon/label); does not constrain behavior
- `sort_order INTEGER NOT NULL DEFAULT 0`
- `created_at`, `updated_at`, `deleted_at` (nullable) — UTC RFC3339, matching
  other tables

### 4.2 `invoices.milestone_id`

Add `milestone_id TEXT` (nullable) → `project_milestones(id)`. The existing
snapshot columns (`milestone_label`, and `milestone_kind` widened — see 4.3) are
kept so issued invoices/PDFs remain immutable and self-contained.

### 4.3 Widen `invoices.milestone_kind`

The CHECK currently allows only `kickoff|completion|custom`. Widen it to the same
set as milestone `kind` (add `phase`,`weekly`,`additional`) so an invoice can
snapshot any milestone's kind. `milestone_percent_basis_points` becomes purely
optional/informational (nullable already).

### 4.4 Retired, not dropped

The `projects.kickoff_*`/`completion_*` columns and their 100% CHECK are left in
place (SQLite makes column drops costly) but are **no longer read or written** by
the new code. `quoted_total_minor` stays as the optional reference quote.

## 5. Migration (existing data)

A new numbered SQL migration:

1. Create `project_milestones` and add `invoices.milestone_id`; recreate the
   `invoices` table (or use SQLite's supported path) to widen the
   `milestone_kind` CHECK.
2. For every existing non-deleted project, insert two milestone rows preserving
   current values exactly:
   - kickoff: `label = kickoff_label`, `amount_minor = round(quoted_total_minor ×
     kickoff_percent_basis_points ÷ 10000)`, `kind = 'kickoff'`, `sort_order = 0`.
   - completion: the remaining amount (`quoted_total_minor − kickoff amount`, so
     the two always re-sum to the original total with no rounding drift),
     `kind = 'completion'`, `sort_order = 1`.
3. Best-effort link existing issued invoices to the new milestone rows by
   `(project_id, milestone_kind)`; leave `milestone_id` NULL where ambiguous.

Migration is deterministic and non-destructive: existing totals and issued
invoices are unchanged in value.

## 6. Backend (`domain.rs`, `finance.rs`)

- **Project create/update:** accept `milestones: [{ id?, label, amountMinor,
  kind, sortOrder }]`. Replace the two-column read/write with milestone-row
  management inside the existing transaction (insert new, update existing by id,
  soft-delete removed ones). Drop `validate_milestone_split`; validate instead:
  label non-empty ≤ 80, `amountMinor ≥ 0`, at least zero milestones allowed
  (empty plan is valid). Project total is derived (`SUM(amount_minor)`), not
  stored as the source of truth.
- **Load project:** return `milestones` ordered by `sort_order, created_at`, each
  with a derived `status` (see 7).
- **Create invoice:** accept a `milestoneId`; snapshot `milestone_label` and
  `milestone_kind` from that milestone, seed the invoice's first line item from
  the milestone (`amount_minor`), and set `invoices.milestone_id`. Reject if the
  milestone already has an active (non-void) invoice (enforces "one active
  invoice per milestone") with a clear error.
- **Milestone status query:** a helper that, given a milestone, finds its latest
  non-void linked invoice and reports status.

No new money math: amounts are integer minor units end to end; project totals are
per-currency and never mix currencies (unchanged rule).

## 7. Milestone status derivation

For a milestone, consider its linked invoices where `deleted_at IS NULL` and
`status <> 'void'`:

- **Not invoiced** — none exist.
- **Paid** — the linked invoice's balance due is 0 (status `paid`).
- **Invoiced** — a linked invoice exists but isn't fully paid (draft, issued,
  partially paid, or overdue).

Because only one active invoice per milestone is allowed, this is unambiguous.
Project detail also shows **remaining to invoice** = SUM(amount of milestones
whose status is Not invoiced).

## 8. Frontend

### 8.1 Types (`work/types.ts`)

- `MilestoneKind = "kickoff" | "completion" | "phase" | "weekly" | "additional" | "custom"`.
- `ProjectMilestone = { id, label, amountMinor, kind, sortOrder, status }` where
  `status: "not-invoiced" | "invoiced" | "paid"` (read-only, backend-derived).
- `CreateProjectInput.milestones` becomes the editable list (id optional for new
  rows). Remove percent-basis-point fields from the project input.

### 8.2 Plan editor (`ProjectForm.svelte`)

A milestone list editor: rows with label + amount, add/remove, reorder
(sort_order), and a **template picker** that seeds rows:
- Kickoff + Completion → 2 rows split from an entered amount (default 50/50).
- Even weekly → prompt weeks + amount → N rows "Week 1..N".
- Phase-by-phase → seed "Phase 1"/"Phase 2"; add more.
- Custom → empty.
A live "Plan total" shows SUM(amounts). The pure template/derivation logic lives
in a small, unit-tested module (see 9), not inline in the component.

### 8.3 Invoice form (`MoneyView.svelte`)

The milestone picker lists the selected project's milestones with amount +
status; choosing one seeds the invoice amount and links `milestoneId`. Milestones
that already have an active invoice are shown disabled with their status.

### 8.4 Project detail (`WorkView.svelte`)

Show each milestone with amount and a status chip (Not invoiced / Invoiced /
Paid), the derived project total, and "remaining to invoice".

## 9. Components and boundaries

- **`milestonePlans.ts` (new, pure, unit-tested):** template generators
  (`kickoffCompletion(totalMinor)`, `evenWeekly(weeks, amountMinor)`,
  `phases(n)`), plan total (`planTotal(milestones)`), and reorder helpers. No I/O.
- **`ProjectForm.svelte`:** milestone list UI on top of `milestonePlans.ts`.
- **Rust `project_milestones` module concerns** stay within `domain.rs`
  (project) and `finance.rs` (invoice link + status), following existing file
  organization.

## 10. Testing

- **Rust:** milestone CRUD within project create/update; project total = sum;
  invoice creation links milestone + snapshots label/amount; "one active invoice
  per milestone" rejection; status derivation (not-invoiced / invoiced / paid;
  void ignored so re-invoice works); migration produces two rows whose amounts
  re-sum to the original total (rounding handled by giving completion the
  remainder).
- **Frontend (`node --test`):** `milestonePlans.ts` — weekly generates the right
  count and amounts, phases seed correctly, `planTotal` sums, reorder preserves
  data.
- **Manual:** create a project with each template, add an "additional feature"
  after issuing an invoice, invoice a milestone, mark it paid, confirm the status
  chips and "remaining to invoice" update; confirm existing (pre-migration)
  projects still show their original two milestones and totals.

## 11. Non-goals (v1 of this feature)

- No splitting one milestone across multiple invoices (one active invoice per
  milestone).
- No automatic due-date scheduling from a weekly template (weekly seeds amounts
  and labels only; dates stay per-invoice as today).
- No percentage-based milestone entry (amounts only; a template may compute an
  even split, but storage is amounts).
- No reordering of already-issued invoices or retroactive recompute of issued
  invoices when the plan changes.
- Not dropping the retired `kickoff_*`/`completion_*` columns in this migration.

## 12. Open risks (resolve in planning)

- SQLite CHECK-widening on `invoices.milestone_kind` requires a table rebuild
  (create new, copy, drop, rename) within the migration — verify against the
  existing migration style and foreign keys.
- Confirm the exact "balance due = 0 → paid" signal reused from the existing
  invoice-totals logic so status derivation matches the Money view.
