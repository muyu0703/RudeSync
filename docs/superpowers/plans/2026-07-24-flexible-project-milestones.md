# Flexible Project Milestones Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the fixed kickoff/completion (50/50) project model with a flexible list of explicitly-priced milestones (any number, template-seeded, addable anytime), each tracking its billing status via a linked invoice.

**Architecture:** New normalized `project_milestones` table (migration 3) + a nullable `invoices.milestone_id` link. Rust `domain.rs` manages milestone rows per project (project total = SUM of amounts); `finance.rs` links each invoice to one milestone, enforces one active invoice per milestone, and derives status. Frontend gets a pure `milestonePlans.ts` template module, a milestone list editor in `ProjectForm`, a milestone picker in the invoice form, and status chips in the project detail.

**Tech Stack:** Tauri 2, Rust + rusqlite (SQLite), Svelte 5 + TypeScript, `node --test`.

## Global Constraints

- Money is always integer **minor units**; never floating point. Per-currency only — never sum across currencies.
- All persisted rows use UUID PK + `created_at`/`updated_at`/nullable `deleted_at` (UTC RFC3339 via `utc_now()`), matching existing tables.
- Milestone `kind` ∈ `kickoff | completion | phase | weekly | additional | custom` (cosmetic tag only).
- Milestone `label` ≤ 80 chars; `amount_minor` ≥ 0.
- **Project total = SUM(milestone amount_minor).** `quoted_total_minor` is kept only as an optional reference; it no longer drives anything.
- **One active (non-void) invoice per milestone.** Statuses: `not-invoiced | invoiced | paid`.
- **Invoice `milestone_kind` CHECK is NOT widened.** It stays `kickoff|completion|custom`; when billing a milestone whose kind is `phase|weekly|additional|custom`, snapshot `milestone_kind='custom'` (the real name lives in `milestone_label`). This avoids an invoices-table rebuild. (Refines spec §4.3.)
- Migration is non-destructive: existing `kickoff_*`/`completion_*` columns are left in place, unused.
- Tests run `.ts` directly via `node --test` with explicit `.ts` import extensions; Rust tests via `cargo test` (cargo needs PATH: `export PATH="$(cygpath -u "$USERPROFILE/.cargo/bin"):$PATH"`).

---

## Phase 1 — Schema + backend project milestones

### Task 1: Migration 3 — `project_milestones` table + `invoices.milestone_id` + data backfill

**Files:**
- Create: `src-tauri/migrations/0003_flexible_milestones.sql`
- Modify: `src-tauri/src/lib.rs` (register the migration)

**Interfaces:**
- Produces: table `project_milestones(id, project_id, label, amount_minor, kind, sort_order, created_at, updated_at, deleted_at)` and column `invoices.milestone_id`.

- [ ] **Step 1: Write the migration SQL**

Create `src-tauri/migrations/0003_flexible_milestones.sql`:

```sql
-- Flexible project milestones: a project owns an ordered list of priced milestones.
CREATE TABLE project_milestones (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    label TEXT NOT NULL,
    amount_minor INTEGER NOT NULL DEFAULT 0
        CHECK (amount_minor >= 0),
    kind TEXT NOT NULL DEFAULT 'custom'
        CHECK (kind IN ('kickoff', 'completion', 'phase', 'weekly', 'additional', 'custom')),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE INDEX project_milestones_by_project
    ON project_milestones(project_id)
    WHERE deleted_at IS NULL;

-- Link an invoice to the milestone it bills (nullable; legacy/ad-hoc invoices stay NULL).
ALTER TABLE invoices ADD COLUMN milestone_id TEXT REFERENCES project_milestones(id);

-- Backfill: convert each existing project's kickoff/completion columns into two rows.
-- completion gets the remainder so the two always re-sum to quoted_total_minor exactly.
INSERT INTO project_milestones (id, project_id, label, amount_minor, kind, sort_order, created_at, updated_at, deleted_at)
SELECT
    lower(hex(randomblob(16))),
    p.id,
    p.kickoff_label,
    CAST(p.quoted_total_minor * p.kickoff_percent_basis_points / 10000 AS INTEGER),
    'kickoff',
    0,
    p.created_at,
    p.updated_at,
    NULL
FROM projects p
WHERE p.deleted_at IS NULL;

INSERT INTO project_milestones (id, project_id, label, amount_minor, kind, sort_order, created_at, updated_at, deleted_at)
SELECT
    lower(hex(randomblob(16))),
    p.id,
    p.completion_label,
    p.quoted_total_minor - CAST(p.quoted_total_minor * p.kickoff_percent_basis_points / 10000 AS INTEGER),
    'completion',
    1,
    p.created_at,
    p.updated_at,
    NULL
FROM projects p
WHERE p.deleted_at IS NULL;

-- Best-effort link existing issued invoices to the matching new milestone by kind.
UPDATE invoices
SET milestone_id = (
    SELECT m.id FROM project_milestones m
    WHERE m.project_id = invoices.project_id
      AND m.kind = invoices.milestone_kind
      AND m.deleted_at IS NULL
    LIMIT 1
)
WHERE milestone_id IS NULL
  AND project_id IS NOT NULL
  AND milestone_kind IN ('kickoff', 'completion');
```

- [ ] **Step 2: Register the migration in `lib.rs`**

In `src-tauri/src/lib.rs`, next to the existing migration consts (~line 29-30):

```rust
const MILESTONES_MIGRATION: &str = include_str!("../migrations/0003_flexible_milestones.sql");
```

Then in `apply_migrations` (~line 1282), extend the array:

```rust
    let migrations = [
        (1_i64, INITIAL_MIGRATION),
        (2_i64, RELIABILITY_MIGRATION),
        (3_i64, MILESTONES_MIGRATION),
    ];
```

- [ ] **Step 3: Update the migration count test**

In `src-tauri/src/lib.rs`, the test `initial_migration_and_defaults_apply_to_empty_database` asserts `migration_version == 2`. Change that assertion to `3`.

- [ ] **Step 4: Add a Rust test for the backfill**

This test must prove the migration converts **pre-existing** legacy projects, so it
applies migration 1, inserts legacy projects, and only then runs the remaining
migrations. It follows the existing partial-migration test pattern in this file
(see `reliability_migration_tolerates_compatibility_columns_added_early`).

Add to the `#[cfg(test)]` module in `src-tauri/src/lib.rs`:

```rust
    #[test]
    fn milestones_migration_backfills_existing_projects() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at TEXT NOT NULL
                );",
            )
            .unwrap();
        connection.execute_batch(INITIAL_MIGRATION).unwrap();
        let now = utc_now();
        connection
            .execute(
                "INSERT INTO schema_migrations(version, applied_at) VALUES (1, ?1)",
                params![now],
            )
            .unwrap();
        // Two legacy projects created under the old two-column milestone model.
        // The second uses an odd total + 33.33% split to expose rounding drift.
        connection
            .execute(
                "INSERT INTO projects (
                    id, name, urls_json, status, currency, quoted_total_minor,
                    kickoff_percent_basis_points, kickoff_label,
                    completion_percent_basis_points, completion_label,
                    created_at, updated_at
                 ) VALUES
                 ('legacy-1','Even Split','[]','active','USD',300000,3000,'Deposit',7000,'Final',?1,?1),
                 ('legacy-2','Odd Split','[]','active','USD',100001,3333,'Kickoff',6667,'Completion',?1,?1)",
                params![now],
            )
            .unwrap();

        apply_migrations(&mut connection).unwrap();

        let mut statement = connection
            .prepare(
                "SELECT label, amount_minor, kind, sort_order
                 FROM project_milestones
                 WHERE project_id = ?1 AND deleted_at IS NULL
                 ORDER BY sort_order ASC",
            )
            .unwrap();
        let load = |statement: &mut rusqlite::Statement<'_>, project_id: &str| {
            statement
                .query_map(params![project_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };

        let first = load(&mut statement, "legacy-1");
        assert_eq!(first.len(), 2);
        assert_eq!(first[0], ("Deposit".into(), 90_000, "kickoff".into(), 0));
        assert_eq!(first[1], ("Final".into(), 210_000, "completion".into(), 1));

        // Completion takes the remainder, so an odd total re-sums exactly.
        let second = load(&mut statement, "legacy-2");
        assert_eq!(second.len(), 2);
        assert_eq!(second[0].1, 33_330);
        assert_eq!(second[1].1, 66_671);
        assert_eq!(second[0].1 + second[1].1, 100_001);
    }
```

- [ ] **Step 5: Run tests**

Run: `cd src-tauri && export PATH="$(cygpath -u "$USERPROFILE/.cargo/bin"):$PATH" && cargo test migration`
Expected: PASS — migration applies, version is 3, backfill sum test passes.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/migrations/0003_flexible_milestones.sql src-tauri/src/lib.rs
git commit -m "feat(milestones): add project_milestones table and migration"
```

---

### Task 2: Backend — project milestone CRUD (create/update/load with milestone rows)

**Files:**
- Modify: `src-tauri/src/domain.rs` (structs, `validate_project_input`, `create_project`, `update_project_in_connection`, `load_project`, `project_from_row`)

**Interfaces:**
- Consumes: `project_milestones` table (Task 1).
- Produces: `Project.milestones: Vec<Milestone>` (serialized camelCase) and accepts `CreateProjectInput.milestones: Vec<MilestoneInput>`. `Milestone { id, label, amount_minor, kind, sort_order, status }` where `status` is derived (Task 4 fills real status; Task 2 sets `"not-invoiced"` as a placeholder value until Task 4).

Read `src-tauri/src/domain.rs` project section first (structs near line 386, `validate_project_input` ~402-451, `create_project` ~281-321, `update_project_in_connection` ~333-384, `load_project`/`project_from_row` ~453-497). Apply these exact changes:

- [ ] **Step 1: Add milestone structs**

After the existing `Project`/`CreateProjectInput` structs in `domain.rs`, add:

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Milestone {
    id: String,
    label: String,
    amount_minor: i64,
    kind: String,
    sort_order: i64,
    status: String, // "not-invoiced" | "invoiced" | "paid"
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MilestoneInput {
    id: Option<String>,
    label: String,
    amount_minor: i64,
    kind: Option<String>,
    sort_order: Option<i64>,
}
```

- [ ] **Step 2: Add `milestones` to `Project` and `CreateProjectInput`**

Add `milestones: Vec<Milestone>` to the `Project` struct and `milestones: Option<Vec<MilestoneInput>>` to `CreateProjectInput`. Add a validated field `milestones: Vec<ValidatedMilestone>` to `ValidatedProject` (define `struct ValidatedMilestone { id: Option<String>, label: String, amount_minor: i64, kind: String, sort_order: i64 }`). Leave the existing `kickoff_*`/`completion_*` `ValidatedProject` fields in place (still written for back-compat defaults).

- [ ] **Step 3: Validate milestones in `validate_project_input`**

In `validate_project_input`, after the existing validation, build the milestone list:

```rust
    let allowed_kinds = ["kickoff", "completion", "phase", "weekly", "additional", "custom"];
    let mut milestones = Vec::new();
    for (index, m) in input.milestones.unwrap_or_default().into_iter().enumerate() {
        let label = required_trimmed(m.label, "Milestone label", 80)?;
        if m.amount_minor < 0 {
            return Err(AppError::InvalidInput("Milestone amount cannot be negative.".into()));
        }
        let kind = m.kind.unwrap_or_else(|| "custom".into());
        if !allowed_kinds.contains(&kind.as_str()) {
            return Err(AppError::InvalidInput("Unknown milestone kind.".into()));
        }
        let id = match m.id {
            Some(id) => Some(required_trimmed(id, "Milestone ID", 64)?),
            None => None,
        };
        milestones.push(ValidatedMilestone {
            id,
            label,
            amount_minor: m.amount_minor,
            kind,
            sort_order: m.sort_order.unwrap_or(index as i64),
        });
    }
```

Add `milestones` to the returned `ValidatedProject`. Keep `quoted_total_minor` accepted as-is (optional reference); no longer require the 100% split — leave `validate_milestone_split` unused (or delete it and its test `milestone_split_must_total_one_hundred_percent`; deleting is cleaner — do that and remove the now-dead `kickoff_percent_basis_points`/`completion_percent_basis_points` reads by defaulting them to 5000/5000 in `ValidatedProject`).

- [ ] **Step 4: Add a milestone-sync helper**

Add to `domain.rs`:

```rust
fn sync_project_milestones(
    connection: &Connection,
    project_id: &str,
    milestones: &[ValidatedMilestone],
    now: &str,
) -> Result<(), AppError> {
    // Soft-delete milestones no longer present.
    let keep_ids: Vec<String> = milestones.iter().filter_map(|m| m.id.clone()).collect();
    let placeholders = keep_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "UPDATE project_milestones SET deleted_at = ?, updated_at = ? \
         WHERE project_id = ? AND deleted_at IS NULL \
         AND ({} )",
        if keep_ids.is_empty() { "1=1".into() } else { format!("id NOT IN ({placeholders})") }
    );
    let mut args: Vec<&dyn rusqlite::ToSql> = vec![&now, &now, &project_id];
    for id in &keep_ids { args.push(id); }
    connection.execute(&sql, args.as_slice())?;

    for m in milestones {
        match &m.id {
            Some(id) => {
                connection.execute(
                    "UPDATE project_milestones SET label=?2, amount_minor=?3, kind=?4, sort_order=?5, updated_at=?6 \
                     WHERE id=?1 AND project_id=?7 AND deleted_at IS NULL",
                    params![id, m.label, m.amount_minor, m.kind, m.sort_order, now, project_id],
                )?;
            }
            None => {
                connection.execute(
                    "INSERT INTO project_milestones (id, project_id, label, amount_minor, kind, sort_order, created_at, updated_at) \
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?7)",
                    params![Uuid::new_v4().to_string(), project_id, m.label, m.amount_minor, m.kind, m.sort_order, now],
                )?;
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 5: Call the helper in create/update**

In `create_project`, after the `INSERT INTO projects` succeeds (before `load_project`), add:
`sync_project_milestones(&connection, &id, &values.milestones, &now)?;`
In `update_project_in_connection`, after the `UPDATE projects` (before `load_project`), add the same call with the update's `id`/`now`.

- [ ] **Step 6: Load milestones in `load_project`**

Add a loader and call it in `project_from_row`'s caller. Since `project_from_row` has only the row, load milestones in `load_project`/`list_projects` after building each `Project`:

```rust
fn load_project_milestones(connection: &Connection, project_id: &str) -> Result<Vec<Milestone>, AppError> {
    let mut stmt = connection.prepare(
        "SELECT id, label, amount_minor, kind, sort_order FROM project_milestones \
         WHERE project_id = ?1 AND deleted_at IS NULL ORDER BY sort_order ASC, created_at ASC",
    )?;
    let rows = stmt.query_map(params![project_id], |row| {
        Ok(Milestone {
            id: row.get(0)?,
            label: row.get(1)?,
            amount_minor: row.get(2)?,
            kind: row.get(3)?,
            sort_order: row.get(4)?,
            status: "not-invoiced".to_string(), // real status wired in Task 4
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}
```

In `load_project`, after `.ok_or_else(...)?` produces the `Project`, set `project.milestones = load_project_milestones(connection, id)?;` and return it. In `list_projects`, after collecting the rows, loop and populate each project's milestones the same way. Initialize `milestones: Vec::new()` in `project_from_row`.

- [ ] **Step 7: Rust tests**

Add tests to `domain.rs` `#[cfg(test)]`: creating a project with three milestones persists three rows; updating to remove one soft-deletes it; `load_project` returns them ordered; project total helper (SUM) equals the sum. Use the in-memory DB pattern already used by other domain tests.

- [ ] **Step 8: Run + commit**

Run: `cd src-tauri && cargo test project`
Expected: PASS.
```bash
git add src-tauri/src/domain.rs
git commit -m "feat(milestones): manage flexible milestone rows on projects"
```

---

## Phase 2 — Invoice ↔ milestone link + status

### Task 3: Backend — link invoice to milestone + enforce one active invoice

**Files:**
- Modify: `src-tauri/src/finance.rs` (`create_invoice` and its input struct)

**Interfaces:**
- Consumes: `project_milestones`, `invoices.milestone_id` (Task 1).
- Produces: invoices created with `milestone_id` set and `milestone_kind` mapped to `kickoff|completion|custom`.

Read `finance.rs` `create_invoice` and its input struct first. Apply:

- [ ] **Step 1: Accept `milestoneId` in the create-invoice input**

Add `milestone_id: Option<String>` to the create-invoice input struct (camelCase `milestoneId`).

- [ ] **Step 2: Snapshot + link + enforce single active invoice**

In `create_invoice`, before inserting the invoice: if `milestone_id` is `Some(id)`, load that milestone (`SELECT label, kind, amount_minor FROM project_milestones WHERE id=?1 AND deleted_at IS NULL`). Reject if not found. Then reject if it already has an active invoice:

```rust
let active: i64 = connection.query_row(
    "SELECT COUNT(*) FROM invoices WHERE milestone_id = ?1 AND deleted_at IS NULL AND status <> 'void'",
    params![milestone_id],
    |r| r.get(0),
)?;
if active > 0 {
    return Err(AppError::InvalidInput(
        "This milestone already has an active invoice. Void it first to re-invoice.".into(),
    ));
}
```

Snapshot: `milestone_label = milestone.label`; `milestone_kind = if kind in ("kickoff","completion") { kind } else { "custom" }`; and set the invoice's `milestone_id` column on INSERT. (Confirm the invoice INSERT column list and add `milestone_id`.)

- [ ] **Step 3: Rust tests**

Test that creating an invoice for a milestone sets `milestone_id` and maps a `phase` kind to `custom`; a second create for the same milestone errors; after voiding the first, a new one succeeds.

- [ ] **Step 4: Run + commit**

Run: `cd src-tauri && cargo test invoice`
```bash
git add src-tauri/src/finance.rs
git commit -m "feat(milestones): link invoices to milestones, one active per milestone"
```

---

### Task 4: Backend — derive milestone status

**Files:**
- Modify: `src-tauri/src/domain.rs` (`load_project_milestones` status), `src-tauri/src/finance.rs` (a status helper if the balance logic lives there)

**Interfaces:**
- Consumes: `invoices` (status + payments), `invoices.milestone_id`.
- Produces: real `Milestone.status` (`not-invoiced|invoiced|paid`).

- [ ] **Step 1: Status query**

Replace the placeholder status in `load_project_milestones` with a derived value. For each milestone id, run:

```sql
SELECT status FROM invoices
WHERE milestone_id = ?1 AND deleted_at IS NULL AND status <> 'void'
ORDER BY created_at DESC LIMIT 1
```

Map: no row → `"not-invoiced"`; invoice `status = 'paid'` → `"paid"`; any other non-void status → `"invoiced"`. (The invoices table already maintains `status` including `paid`/`partially_paid`; reuse it rather than recomputing balances.)

- [ ] **Step 2: Rust test**

Seed a project + milestone + invoice; assert status transitions: no invoice → not-invoiced; issued → invoiced; paid → paid; void → not-invoiced again.

- [ ] **Step 3: Run + commit**

Run: `cd src-tauri && cargo test status`
```bash
git add src-tauri/src/domain.rs src-tauri/src/finance.rs
git commit -m "feat(milestones): derive per-milestone billing status"
```

---

## Phase 3 — Frontend plan editor

### Task 5: Frontend types + pure template module (unit-tested)

**Files:**
- Modify: `src/lib/features/work/types.ts`
- Create: `src/lib/features/work/milestonePlans.ts`
- Test: `tests/work/milestonePlans.test.ts`

**Interfaces:**
- Produces:
  - Types: `MilestoneKind`, `ProjectMilestone { id?: string; label: string; amountMinor: number; kind: MilestoneKind; sortOrder: number; status?: "not-invoiced"|"invoiced"|"paid" }`.
  - `milestonePlans.ts`: `kickoffCompletion(totalMinor: number): ProjectMilestone[]`, `evenWeekly(weeks: number, amountMinor: number): ProjectMilestone[]`, `phases(count: number): ProjectMilestone[]`, `planTotalMinor(ms: ProjectMilestone[]): number`.

- [ ] **Step 1: Update types**

In `src/lib/features/work/types.ts` replace the `MilestoneKind`/`ProjectMilestone` definitions:

```ts
export type MilestoneKind =
  | "kickoff"
  | "completion"
  | "phase"
  | "weekly"
  | "additional"
  | "custom";

export type MilestoneStatus = "not-invoiced" | "invoiced" | "paid";

export interface ProjectMilestone {
  id?: string;
  label: string;
  amountMinor: number;
  kind: MilestoneKind;
  sortOrder: number;
  status?: MilestoneStatus;
}
```

Leave `Project.milestones: ProjectMilestone[]` and `CreateProjectInput.milestones: ProjectMilestone[]` as-is (they already reference `ProjectMilestone`).

- [ ] **Step 2: Write the failing test**

Create `tests/work/milestonePlans.test.ts`:

```ts
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  kickoffCompletion,
  evenWeekly,
  phases,
  planTotalMinor,
} from "../../src/lib/features/work/milestonePlans.ts";

test("kickoffCompletion splits 50/50 with remainder on completion", () => {
  const ms = kickoffCompletion(300001);
  assert.equal(ms.length, 2);
  assert.equal(ms[0].amountMinor + ms[1].amountMinor, 300001);
  assert.equal(ms[0].kind, "kickoff");
  assert.equal(ms[1].kind, "completion");
});

test("evenWeekly generates N weekly milestones with the amount", () => {
  const ms = evenWeekly(4, 25000);
  assert.equal(ms.length, 4);
  assert.deepEqual(ms.map((m) => m.amountMinor), [25000, 25000, 25000, 25000]);
  assert.equal(ms[0].label, "Week 1");
  assert.equal(ms[3].kind, "weekly");
});

test("phases seeds `count` phase milestones", () => {
  const ms = phases(3);
  assert.equal(ms.length, 3);
  assert.equal(ms[0].label, "Phase 1");
  assert.equal(ms[2].kind, "phase");
});

test("planTotalMinor sums amounts", () => {
  assert.equal(planTotalMinor(evenWeekly(3, 10000)), 30000);
});
```

- [ ] **Step 3: Run to verify it fails**

Run: `node --test tests/work/milestonePlans.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 4: Implement the module**

Create `src/lib/features/work/milestonePlans.ts`:

```ts
import type { ProjectMilestone } from "./types.ts";

export function kickoffCompletion(totalMinor: number): ProjectMilestone[] {
  const kickoff = Math.floor(totalMinor / 2);
  return [
    { label: "Kickoff", amountMinor: kickoff, kind: "kickoff", sortOrder: 0 },
    { label: "Completion", amountMinor: totalMinor - kickoff, kind: "completion", sortOrder: 1 },
  ];
}

export function evenWeekly(weeks: number, amountMinor: number): ProjectMilestone[] {
  const count = Math.max(0, Math.floor(weeks));
  return Array.from({ length: count }, (_, i) => ({
    label: `Week ${i + 1}`,
    amountMinor,
    kind: "weekly" as const,
    sortOrder: i,
  }));
}

export function phases(count: number): ProjectMilestone[] {
  const n = Math.max(0, Math.floor(count));
  return Array.from({ length: n }, (_, i) => ({
    label: `Phase ${i + 1}`,
    amountMinor: 0,
    kind: "phase" as const,
    sortOrder: i,
  }));
}

export function planTotalMinor(ms: ProjectMilestone[]): number {
  return ms.reduce((sum, m) => sum + (m.amountMinor || 0), 0);
}
```

- [ ] **Step 5: Run to verify pass**

Run: `node --test tests/work/milestonePlans.test.ts`
Expected: PASS — 4 tests.

- [ ] **Step 6: Commit**

```bash
git add src/lib/features/work/types.ts src/lib/features/work/milestonePlans.ts tests/work/milestonePlans.test.ts
git commit -m "feat(milestones): add milestone types and template generators"
```

---

### Task 6: Frontend — milestone plan editor in ProjectForm

**Files:**
- Modify: `src/lib/features/work/components/ProjectForm.svelte`
- Modify: `src/lib/features/work/workService.ts` (map milestones to/from backend)

**Interfaces:**
- Consumes: `milestonePlans.ts`, `ProjectMilestone` (Task 5).
- Produces: `CreateProjectInput.milestones` populated from the editor.

Read the current `ProjectForm.svelte` and `workService.ts` project mapping first.

- [ ] **Step 1: workService mapping**

In `workService.ts`, ensure the project create/update payload sends `milestones` (array of `{ id?, label, amountMinor, kind, sortOrder }`) and that the parsed `Project` reads `milestones` from the backend (camelCase already). Remove any kickoff/completion percent mapping. Verify with the existing project parse/normalize helpers.

- [ ] **Step 2: Editor UI**

Replace the two fixed kickoff/completion inputs in `ProjectForm.svelte` with a milestone list: each row has a label input and an amount input (minor units via the existing money input helper used elsewhere in the form), plus remove/reorder buttons. Above the list, a template `<select>`:
- "Kickoff + Completion" → `kickoffCompletion(currentTotalOrZero)`
- "Even weekly" → prompt weeks + amount (two small inputs) → `evenWeekly(...)`
- "Phase-by-phase" → `phases(2)` (then user adds more)
- "Custom / empty" → `[]`
Show a live "Plan total" = `planTotalMinor(milestones)` formatted with the existing money formatter. Bind the milestone array into `CreateProjectInput.milestones` on submit. An "+ Add milestone" button appends `{ label: "", amountMinor: 0, kind: "custom", sortOrder: milestones.length }`.

- [ ] **Step 3: Type-check**

Run: `npm run check`
Expected: 0 errors.

- [ ] **Step 4: Commit**

```bash
git add src/lib/features/work/components/ProjectForm.svelte src/lib/features/work/workService.ts
git commit -m "feat(milestones): milestone plan editor with templates in ProjectForm"
```

---

## Phase 4 — Invoicing UI + status

### Task 7: Frontend — invoice milestone picker

**Files:**
- Modify: `src/lib/features/money/MoneyView.svelte`
- Modify: `src/lib/features/money/moneyService.ts` / `moneyCommands.ts` (pass `milestoneId`)

**Interfaces:**
- Consumes: project `milestones` with `status`; backend create-invoice `milestoneId` (Task 3).

Read the current invoice-creation section of `MoneyView.svelte` and the money service create-invoice call first.

- [ ] **Step 1: Picker UI**

Replace the kickoff/completion milestone select with one that lists `selectedProject.milestones` showing `label — amount (status)`. Selecting one sets `invoiceMilestoneId`, seeds the first line item's amount from the milestone `amountMinor`, and sets the milestone label. Milestones whose `status !== "not-invoiced"` render disabled with their status text.

- [ ] **Step 2: Pass `milestoneId` through**

In the money service create-invoice call, include `milestoneId: invoiceMilestoneId` in the payload (matching Task 3's `milestone_id`).

- [ ] **Step 3: Type-check + commit**

Run: `npm run check` → 0 errors.
```bash
git add src/lib/features/money/MoneyView.svelte src/lib/features/money/moneyService.ts src/lib/features/money/moneyCommands.ts
git commit -m "feat(milestones): invoice milestone picker with status"
```

---

### Task 8: Frontend — milestone status in project detail

**Files:**
- Modify: `src/lib/features/work/WorkView.svelte`

**Interfaces:**
- Consumes: project `milestones` with `status` and `amountMinor`.

Read the current project-detail rendering in `WorkView.svelte` first.

- [ ] **Step 1: Render milestones + status**

In the project detail, list each milestone with its label, formatted amount, and a status chip (Not invoiced / Invoiced / Paid) using existing chip/label styles. Show the derived **project total** = sum of milestone amounts and **remaining to invoice** = sum of `amountMinor` where `status === "not-invoiced"`, both via the existing money formatter.

- [ ] **Step 2: Type-check + commit**

Run: `npm run check` → 0 errors.
```bash
git add src/lib/features/work/WorkView.svelte
git commit -m "feat(milestones): show milestone status and remaining-to-invoice"
```

---

## Self-Review

**Spec coverage:**
- New `project_milestones` table + `invoices.milestone_id` → Task 1. ✓
- Explicit amounts, total = sum → Tasks 2, 5, 8. ✓
- Templates (kickoff/completion, weekly, phase, custom) → Tasks 5, 6. ✓
- Additional feature anytime → Task 6 ("+ Add milestone"; update path in Task 2 allows adding post-creation). ✓
- Status per milestone (not-invoiced/invoiced/paid) → Task 4, surfaced in Tasks 7, 8. ✓
- One active invoice per milestone → Task 3. ✓
- Migration converting existing kickoff/completion, non-destructive → Task 1. ✓
- Retired columns left in place → Task 1/2 (not dropped). ✓
- Legacy invoice best-effort link → Task 1 Step 1 UPDATE. ✓
- PDF/snapshot unaffected (kind mapped to custom, label snapshotted) → Task 3, Global Constraints. ✓

**Placeholder scan:** Tasks 2/3/4/6/7/8 instruct reading the existing large files (domain.rs, finance.rs, the Svelte forms) before editing — the exact structs, SQL, mappings, and helper code to add are given in full; the "read first" is codebase-fidelity (locating insertion points in big files), matching how the widget plan handled `App.svelte`. No TBD/TODO.

**Type consistency:** `Milestone`/`MilestoneInput` (Rust) ↔ `ProjectMilestone` (TS) fields align (`label`, `amountMinor`/`amount_minor`, `kind`, `sortOrder`/`sort_order`, `status`). `status` values `not-invoiced|invoiced|paid` consistent across Task 4 (Rust) and Task 5 (TS). Create-invoice `milestoneId` ↔ `milestone_id` consistent Tasks 3/7.

## Open Risks (verify during implementation)

- `sync_project_milestones` builds a dynamic `IN (...)` list — verify the rusqlite `ToSql` slice binding compiles; if awkward, delete-then-reinsert-by-diff is an acceptable alternative that keeps ids stable for kept rows.
- Confirm the invoice INSERT column list in `finance.rs` so `milestone_id` is added in the right position, and that `create_invoice` still seeds a line item when a milestone is chosen.
- Confirm the money-input and money-formatter helpers used in ProjectForm/MoneyView so the milestone amount fields match existing minor-unit handling.
