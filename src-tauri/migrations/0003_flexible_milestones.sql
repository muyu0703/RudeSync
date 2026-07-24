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
