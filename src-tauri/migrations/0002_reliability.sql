-- Backup reliability and audit metadata.
--
-- Automatic backups are tracked by local calendar day, while attempts and
-- failures are persisted so the UI can surface problems and the worker can
-- retry with a measured backoff. `backup_kind` keeps manual and pre-restore
-- safety copies outside automatic-retention semantics.

-- Columns for this migration are added conditionally by the migration runner
-- before this SQL executes. SQLite has no `ADD COLUMN IF NOT EXISTS`, and the
-- conditional runner keeps upgrades safe for development databases that may
-- already contain a compatibility-added column.

UPDATE invoices
SET project_name = (
    SELECT projects.name
    FROM projects
    WHERE projects.id = invoices.project_id
)
WHERE project_name IS NULL OR TRIM(project_name) = '';

UPDATE personal_loans
SET first_monthly_day = CAST(SUBSTR(first_payment_date, 9, 2) AS INTEGER)
WHERE frequency = 'twice_monthly' AND first_monthly_day IS NULL;
