-- RudeSync initial schema.
-- All identifiers are application-generated UUIDs. Dates use YYYY-MM-DD and
-- timestamps use UTC RFC 3339 text so they remain portable across SQLite,
-- exports, and a future sync service.

CREATE TABLE app_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    default_currency TEXT NOT NULL DEFAULT 'USD'
        CHECK (length(default_currency) = 3),
    default_invoice_term_days INTEGER NOT NULL DEFAULT 14
        CHECK (default_invoice_term_days IN (0, 7, 14, 30)),
    date_format TEXT NOT NULL DEFAULT 'MMMM d, yyyy',
    week_starts_on INTEGER NOT NULL DEFAULT 1
        CHECK (week_starts_on BETWEEN 0 AND 6),
    theme TEXT NOT NULL DEFAULT 'dark'
        CHECK (theme IN ('dark', 'system')),
    accent_color TEXT NOT NULL DEFAULT '#22C55E',
    close_to_tray INTEGER NOT NULL DEFAULT 1
        CHECK (close_to_tray IN (0, 1)),
    autostart_enabled INTEGER NOT NULL DEFAULT 1
        CHECK (autostart_enabled IN (0, 1)),
    notifications_enabled INTEGER NOT NULL DEFAULT 1
        CHECK (notifications_enabled IN (0, 1)),
    loan_reminder_offsets_minutes TEXT NOT NULL DEFAULT '[10080,1440,0]',
    invoice_reminder_offsets_minutes TEXT NOT NULL DEFAULT '[4320,0]',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE invoice_profiles (
    id TEXT PRIMARY KEY,
    profile_name TEXT NOT NULL,
    display_name TEXT NOT NULL,
    business_name TEXT,
    email TEXT,
    address TEXT,
    logo_path TEXT,
    payment_instructions TEXT,
    is_default INTEGER NOT NULL DEFAULT 0
        CHECK (is_default IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE UNIQUE INDEX invoice_profiles_one_default
    ON invoice_profiles(is_default)
    WHERE is_default = 1 AND deleted_at IS NULL;

CREATE TABLE clients (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    company_name TEXT,
    email TEXT,
    billing_address TEXT,
    currency TEXT NOT NULL DEFAULT 'USD'
        CHECK (length(currency) = 3),
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE INDEX clients_active_name
    ON clients(name COLLATE NOCASE)
    WHERE deleted_at IS NULL;

CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    client_id TEXT REFERENCES clients(id),
    name TEXT NOT NULL,
    description TEXT,
    urls_json TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('draft', 'active', 'completed', 'archived')),
    currency TEXT NOT NULL DEFAULT 'USD'
        CHECK (length(currency) = 3),
    quoted_total_minor INTEGER NOT NULL DEFAULT 0
        CHECK (quoted_total_minor >= 0),
    kickoff_percent_basis_points INTEGER NOT NULL DEFAULT 5000
        CHECK (kickoff_percent_basis_points BETWEEN 0 AND 10000),
    kickoff_label TEXT NOT NULL DEFAULT 'Kickoff',
    completion_percent_basis_points INTEGER NOT NULL DEFAULT 5000
        CHECK (completion_percent_basis_points BETWEEN 0 AND 10000),
    completion_label TEXT NOT NULL DEFAULT 'Completion',
    start_date TEXT,
    due_date TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT,
    CHECK (
        kickoff_percent_basis_points + completion_percent_basis_points = 10000
    )
);

CREATE INDEX projects_active_client
    ON projects(client_id, status)
    WHERE deleted_at IS NULL;

CREATE TABLE work_entries (
    id TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id),
    title TEXT NOT NULL,
    details TEXT,
    work_date TEXT NOT NULL,
    urls_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE INDEX work_entries_active_date
    ON work_entries(work_date DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX work_entries_active_project
    ON work_entries(project_id, work_date DESC)
    WHERE deleted_at IS NULL;

CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    parent_task_id TEXT REFERENCES tasks(id),
    project_id TEXT REFERENCES projects(id),
    title TEXT NOT NULL,
    notes TEXT,
    planned_date TEXT,
    due_date TEXT,
    priority TEXT NOT NULL DEFAULT 'none'
        CHECK (priority IN ('none', 'low', 'medium', 'high', 'urgent')),
    category TEXT,
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'completed', 'cancelled')),
    completed_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT,
    CHECK (parent_task_id IS NULL OR parent_task_id <> id),
    CHECK (
        (status = 'completed' AND completed_at IS NOT NULL)
        OR (status <> 'completed')
    )
);

CREATE INDEX tasks_active_schedule
    ON tasks(status, planned_date, due_date)
    WHERE deleted_at IS NULL;

CREATE INDEX tasks_active_parent
    ON tasks(parent_task_id, status)
    WHERE deleted_at IS NULL;

CREATE INDEX tasks_active_project
    ON tasks(project_id, status)
    WHERE deleted_at IS NULL;

CREATE TABLE task_recurrences (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    rule TEXT NOT NULL,
    frequency TEXT
        CHECK (
            frequency IS NULL
            OR frequency IN ('daily', 'weekly', 'monthly', 'yearly', 'custom')
        ),
    interval_count INTEGER NOT NULL DEFAULT 1
        CHECK (interval_count > 0),
    next_occurrence_date TEXT,
    ends_on TEXT,
    occurrence_limit INTEGER
        CHECK (occurrence_limit IS NULL OR occurrence_limit > 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE UNIQUE INDEX task_recurrences_one_active_rule
    ON task_recurrences(task_id)
    WHERE deleted_at IS NULL;

CREATE TABLE task_reminders (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    remind_at TEXT NOT NULL,
    delivered_at TEXT,
    snoozed_until TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE INDEX task_reminders_pending
    ON task_reminders(remind_at)
    WHERE delivered_at IS NULL AND deleted_at IS NULL;

CREATE TABLE invoice_sequences (
    issue_date TEXT PRIMARY KEY,
    last_value INTEGER NOT NULL DEFAULT 0
        CHECK (last_value >= 0),
    updated_at TEXT NOT NULL
);

CREATE TABLE invoices (
    id TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id),
    client_id TEXT NOT NULL REFERENCES clients(id),
    invoice_profile_id TEXT REFERENCES invoice_profiles(id),
    invoice_number TEXT NOT NULL UNIQUE,
    milestone_kind TEXT NOT NULL DEFAULT 'custom'
        CHECK (milestone_kind IN ('kickoff', 'completion', 'custom')),
    milestone_label TEXT,
    milestone_percent_basis_points INTEGER
        CHECK (
            milestone_percent_basis_points IS NULL
            OR milestone_percent_basis_points BETWEEN 0 AND 10000
        ),
    issue_date TEXT NOT NULL,
    due_date TEXT NOT NULL,
    payment_term_days INTEGER
        CHECK (
            payment_term_days IS NULL
            OR payment_term_days IN (0, 7, 14, 30)
        ),
    currency TEXT NOT NULL DEFAULT 'USD'
        CHECK (length(currency) = 3),
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (
            status IN (
                'draft',
                'issued',
                'partially_paid',
                'paid',
                'overdue',
                'void'
            )
        ),
    subtotal_minor INTEGER NOT NULL DEFAULT 0
        CHECK (subtotal_minor >= 0),
    discount_total_minor INTEGER NOT NULL DEFAULT 0
        CHECK (discount_total_minor >= 0),
    tax_total_minor INTEGER NOT NULL DEFAULT 0
        CHECK (tax_total_minor >= 0),
    total_minor INTEGER NOT NULL DEFAULT 0
        CHECK (total_minor >= 0),
    notes TEXT,
    payment_instructions TEXT,
    bill_to_name TEXT NOT NULL,
    bill_to_email TEXT,
    bill_to_address TEXT,
    seller_name TEXT NOT NULL,
    seller_email TEXT,
    seller_address TEXT,
    seller_logo_path TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE INDEX invoices_active_due
    ON invoices(status, due_date)
    WHERE deleted_at IS NULL;

CREATE INDEX invoices_active_project
    ON invoices(project_id, issue_date DESC)
    WHERE deleted_at IS NULL;

CREATE TABLE invoice_items (
    id TEXT PRIMARY KEY,
    invoice_id TEXT NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    description TEXT NOT NULL,
    quantity_millis INTEGER NOT NULL DEFAULT 1000
        CHECK (quantity_millis > 0),
    unit_price_minor INTEGER NOT NULL DEFAULT 0
        CHECK (unit_price_minor >= 0),
    line_total_minor INTEGER NOT NULL DEFAULT 0
        CHECK (line_total_minor >= 0),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE INDEX invoice_items_active_invoice
    ON invoice_items(invoice_id, sort_order)
    WHERE deleted_at IS NULL;

CREATE TABLE invoice_adjustments (
    id TEXT PRIMARY KEY,
    invoice_id TEXT NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    kind TEXT NOT NULL
        CHECK (kind IN ('tax', 'discount')),
    label TEXT NOT NULL,
    calculation_type TEXT NOT NULL
        CHECK (calculation_type IN ('percentage', 'fixed')),
    rate_basis_points INTEGER
        CHECK (
            rate_basis_points IS NULL
            OR rate_basis_points BETWEEN 0 AND 10000
        ),
    amount_minor INTEGER NOT NULL DEFAULT 0
        CHECK (amount_minor >= 0),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE INDEX invoice_adjustments_active_invoice
    ON invoice_adjustments(invoice_id, sort_order)
    WHERE deleted_at IS NULL;

CREATE TABLE invoice_payments (
    id TEXT PRIMARY KEY,
    invoice_id TEXT NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    amount_minor INTEGER NOT NULL
        CHECK (amount_minor > 0),
    paid_at TEXT NOT NULL,
    payment_method TEXT,
    reference TEXT,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE INDEX invoice_payments_active_invoice
    ON invoice_payments(invoice_id, paid_at)
    WHERE deleted_at IS NULL;

CREATE TABLE personal_loans (
    id TEXT PRIMARY KEY,
    operator_name TEXT NOT NULL,
    description TEXT,
    loan_date TEXT NOT NULL,
    first_payment_date TEXT NOT NULL,
    payment_count INTEGER NOT NULL
        CHECK (payment_count > 0),
    frequency TEXT NOT NULL
        CHECK (
            frequency IN (
                'monthly',
                'weekly',
                'biweekly',
                'twice_monthly',
                'custom'
            )
        ),
    second_monthly_day INTEGER
        CHECK (
            second_monthly_day IS NULL
            OR second_monthly_day BETWEEN 1 AND 31
        ),
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'paid', 'archived')),
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE INDEX personal_loans_active_status
    ON personal_loans(status, first_payment_date)
    WHERE deleted_at IS NULL;

CREATE TABLE loan_installments (
    id TEXT PRIMARY KEY,
    loan_id TEXT NOT NULL REFERENCES personal_loans(id) ON DELETE CASCADE,
    installment_number INTEGER NOT NULL
        CHECK (installment_number > 0),
    due_date TEXT NOT NULL,
    is_paid INTEGER NOT NULL DEFAULT 0
        CHECK (is_paid IN (0, 1)),
    paid_at TEXT,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT,
    CHECK (
        (is_paid = 1 AND paid_at IS NOT NULL)
        OR (is_paid = 0)
    )
);

CREATE UNIQUE INDEX loan_installments_active_number
    ON loan_installments(loan_id, installment_number)
    WHERE deleted_at IS NULL;

CREATE INDEX loan_installments_pending_due
    ON loan_installments(due_date)
    WHERE is_paid = 0 AND deleted_at IS NULL;

CREATE TABLE notification_deliveries (
    id TEXT PRIMARY KEY,
    notification_kind TEXT NOT NULL
        CHECK (notification_kind IN ('invoice_due', 'loan_due')),
    entity_id TEXT NOT NULL,
    reminder_offset_days INTEGER NOT NULL,
    delivered_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE (notification_kind, entity_id, reminder_offset_days)
);

CREATE TABLE backup_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    directory_path TEXT,
    is_enabled INTEGER NOT NULL DEFAULT 1
        CHECK (is_enabled IN (0, 1)),
    interval_hours INTEGER NOT NULL DEFAULT 24
        CHECK (interval_hours > 0),
    retention_count INTEGER NOT NULL DEFAULT 30
        CHECK (retention_count > 0),
    last_success_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE backup_runs (
    id TEXT PRIMARY KEY,
    destination_path TEXT NOT NULL,
    file_name TEXT NOT NULL,
    status TEXT NOT NULL
        CHECK (status IN ('started', 'completed', 'failed')),
    size_bytes INTEGER
        CHECK (size_bytes IS NULL OR size_bytes >= 0),
    checksum_sha256 TEXT,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX backup_runs_recent
    ON backup_runs(started_at DESC);
