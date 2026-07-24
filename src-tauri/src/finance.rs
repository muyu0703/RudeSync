use chrono::{Datelike, Duration, Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use super::{
    optional_trimmed, required_trimmed, utc_now, validate_optional_date, AppError, Database,
};

const MAX_INVOICE_ITEMS: usize = 100;
const MAX_INVOICE_ADJUSTMENTS: usize = 20;
const MAX_LOAN_INSTALLMENTS: i64 = 240;
const MAX_INVOICE_SEQUENCE: i64 = 9_999;

pub(crate) fn ensure_finance_schema_compatibility(connection: &Connection) -> Result<(), AppError> {
    if !table_has_column(connection, "personal_loans", "first_monthly_day")? {
        connection.execute(
            "ALTER TABLE personal_loans
             ADD COLUMN first_monthly_day INTEGER
             CHECK (
                first_monthly_day IS NULL
                OR first_monthly_day BETWEEN 1 AND 31
             )",
            [],
        )?;
    }
    if !table_has_column(connection, "invoices", "project_name")? {
        connection.execute("ALTER TABLE invoices ADD COLUMN project_name TEXT", [])?;
    }

    // Existing rows predate these snapshots. Backfill the best information
    // still available once, then all newly-created records remain immutable.
    connection.execute(
        "UPDATE invoices
         SET project_name = (
            SELECT project.name FROM projects project
            WHERE project.id = invoices.project_id
         )
         WHERE project_name IS NULL OR TRIM(project_name) = ''",
        [],
    )?;
    connection.execute(
        "UPDATE personal_loans
         SET first_monthly_day = CAST(SUBSTR(first_payment_date, 9, 2) AS INTEGER)
         WHERE frequency = 'twice_monthly'
           AND first_monthly_day IS NULL",
        [],
    )?;
    Ok(())
}

fn table_has_column(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
) -> Result<bool, AppError> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for stored_column in columns {
        if stored_column? == column_name {
            return Ok(true);
        }
    }
    Ok(false)
}

// ---------------------------------------------------------------------------
// Invoices

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct InvoiceListFilter {
    include_deleted: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateInvoiceInput {
    project_id: String,
    client_id: String,
    milestone_id: Option<String>,
    milestone_kind: String,
    milestone_percent_basis_points: Option<i64>,
    milestone_label: Option<String>,
    issue_date: String,
    payment_term_days: Option<i64>,
    custom_due_date: Option<String>,
    currency: String,
    status: Option<String>,
    notes: Option<String>,
    payment_instructions: Option<String>,
    items: Vec<CreateInvoiceItemInput>,
    adjustments: Vec<CreateInvoiceAdjustmentInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateDraftInvoiceInput {
    invoice_id: String,
    project_id: String,
    client_id: String,
    milestone_kind: String,
    milestone_percent_basis_points: Option<i64>,
    milestone_label: Option<String>,
    issue_date: String,
    payment_term_days: Option<i64>,
    custom_due_date: Option<String>,
    currency: String,
    notes: Option<String>,
    payment_instructions: Option<String>,
    items: Vec<CreateInvoiceItemInput>,
    adjustments: Vec<CreateInvoiceAdjustmentInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateInvoiceItemInput {
    description: String,
    quantity_millis: i64,
    unit_price_minor: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateInvoiceAdjustmentInput {
    kind: String,
    label: String,
    calculation_type: String,
    rate_basis_points: Option<i64>,
    amount_minor: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecordInvoicePaymentInput {
    invoice_id: String,
    amount_minor: i64,
    paid_at: String,
    notes: Option<String>,
    allow_overpayment: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Invoice {
    id: String,
    number: String,
    project_id: String,
    project_name: String,
    client_name: String,
    bill_to_email: Option<String>,
    bill_to_address: Option<String>,
    seller_name: String,
    seller_email: Option<String>,
    seller_address: Option<String>,
    seller_logo_path: Option<String>,
    milestone_label: Option<String>,
    milestone_kind: String,
    milestone_percent_basis_points: Option<i64>,
    issue_date: String,
    due_date: String,
    term_kind: String,
    currency: String,
    line_items: Vec<InvoiceLineItem>,
    adjustments: Vec<InvoiceAdjustment>,
    payments: Vec<InvoicePayment>,
    notes: Option<String>,
    payment_instructions: Option<String>,
    status: String,
    subtotal_minor: i64,
    discount_total_minor: i64,
    tax_total_minor: i64,
    total_minor: i64,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InvoiceLineItem {
    id: String,
    description: String,
    quantity_millis: i64,
    unit_price_minor: i64,
    line_total_minor: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InvoiceAdjustment {
    id: String,
    kind: String,
    label: String,
    calculation_type: String,
    rate_basis_points: Option<i64>,
    amount_minor: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InvoicePayment {
    id: String,
    amount_minor: i64,
    received_date: String,
    note: Option<String>,
    created_at: String,
}

struct ProjectInvoiceContext {
    project_name: String,
    client_id: String,
    project_currency: String,
    kickoff_percent_basis_points: i64,
    completion_percent_basis_points: i64,
    kickoff_label: String,
    completion_label: String,
    client_name: String,
    client_email: Option<String>,
    billing_address: Option<String>,
}

struct SellerSnapshot {
    profile_id: Option<String>,
    name: String,
    email: Option<String>,
    address: Option<String>,
    logo_path: Option<String>,
    payment_instructions: Option<String>,
}

struct ValidatedInvoiceItem {
    description: String,
    quantity_millis: i64,
    unit_price_minor: i64,
    line_total_minor: i64,
}

struct ValidatedInvoiceAdjustment {
    kind: String,
    label: String,
    calculation_type: String,
    rate_basis_points: Option<i64>,
    amount_minor: i64,
}

struct InvoiceAmounts {
    subtotal_minor: i64,
    discount_total_minor: i64,
    tax_total_minor: i64,
    total_minor: i64,
}

#[tauri::command]
pub(crate) fn list_invoices(
    database: State<'_, Database>,
    filter: Option<InvoiceListFilter>,
) -> Result<Vec<Invoice>, AppError> {
    let include_deleted = filter.unwrap_or_default().include_deleted.unwrap_or(false);
    let connection = database.lock()?;
    ensure_finance_schema_compatibility(&connection)?;
    let mut statement = connection.prepare(
        "SELECT
            i.id,
            i.invoice_number,
            i.project_id,
            COALESCE(NULLIF(TRIM(i.project_name), ''), p.name, 'Deleted project'),
            i.bill_to_name,
            i.bill_to_email,
            i.bill_to_address,
            i.seller_name,
            i.seller_email,
            i.seller_address,
            i.seller_logo_path,
            i.milestone_label,
            i.milestone_kind,
            i.milestone_percent_basis_points,
            i.issue_date,
            i.due_date,
            i.payment_term_days,
            i.currency,
            i.status,
            i.subtotal_minor,
            i.discount_total_minor,
            i.tax_total_minor,
            i.total_minor,
            i.notes,
            i.payment_instructions,
            i.created_at,
            i.updated_at,
            i.deleted_at
         FROM invoices i
         LEFT JOIN projects p ON p.id = i.project_id
         WHERE (?1 = 1 OR i.deleted_at IS NULL)
         ORDER BY i.issue_date DESC, i.created_at DESC",
    )?;
    let rows = statement.query_map(params![include_deleted], invoice_from_row)?;
    let mut invoices = rows.collect::<Result<Vec<_>, _>>()?;
    drop(statement);

    for invoice in &mut invoices {
        populate_invoice_children_and_status(&connection, invoice)?;
    }
    Ok(invoices)
}

#[tauri::command]
pub(crate) fn create_invoice(
    database: State<'_, Database>,
    input: CreateInvoiceInput,
) -> Result<Invoice, AppError> {
    create_invoice_with_database(database.inner(), input)
}

fn create_invoice_with_database(
    database: &Database,
    input: CreateInvoiceInput,
) -> Result<Invoice, AppError> {
    let project_id = required_trimmed(input.project_id, "Project ID", 64)?;
    let client_id = required_trimmed(input.client_id, "Client ID", 64)?;
    let issue_date = required_date(input.issue_date, "issueDate")?;
    let currency = validate_currency(&input.currency)?;
    let milestone_kind = validate_milestone_kind(&input.milestone_kind)?;
    let milestone_id = optional_trimmed(input.milestone_id, 64, "Milestone ID")?;
    let status = validate_new_invoice_status(input.status.as_deref().unwrap_or("issued"))?;
    let notes = optional_trimmed(input.notes, 20_000, "Invoice notes")?;
    let requested_payment_instructions =
        optional_trimmed(input.payment_instructions, 20_000, "Payment instructions")?;
    let (due_date, payment_term_days, _term_kind) =
        calculate_invoice_due_date(issue_date, input.payment_term_days, input.custom_due_date)?;
    let (items, adjustments, amounts) = validate_invoice_contents(input.items, input.adjustments)?;

    let mut connection = database.lock()?;
    ensure_finance_schema_compatibility(&connection)?;
    let transaction = connection.transaction()?;
    let context = load_project_invoice_context(&transaction, &project_id)?;
    if context.client_id != client_id {
        return Err(AppError::InvalidInput(
            "clientId does not belong to the selected project.".into(),
        ));
    }
    if context.project_currency != currency {
        return Err(AppError::InvalidInput(format!(
            "Invoice currency must match the project's {} currency.",
            context.project_currency
        )));
    }

    let milestone_percent_basis_points = resolve_milestone_percent(
        &milestone_kind,
        input.milestone_percent_basis_points,
        &context,
    )?;
    let milestone_label =
        optional_trimmed(input.milestone_label, 240, "Milestone label")?.or_else(|| {
            match milestone_kind.as_str() {
                "kickoff" => Some(context.kickoff_label.clone()),
                "completion" => Some(context.completion_label.clone()),
                _ => None,
            }
        });

    // If a flexible milestone was selected, its label/kind snapshot overrides
    // the legacy kickoff/completion-derived values above, and we enforce that
    // a milestone can have at most one active (non-void, non-deleted) invoice.
    //
    // `project_id` is a required (non-Option) field on CreateInvoiceInput and is
    // validated above via `required_trimmed` plus `load_project_invoice_context`
    // (which errors out if the project does not exist), so there is no "invoice
    // with no project" case to special-case here: project_id is always a real,
    // non-empty project id by this point. The lookup below is scoped to that
    // project id explicitly, so a milestone belonging to a different project
    // never resolves, regardless of SQL NULL-comparison semantics.
    let (milestone_kind, milestone_label, milestone_percent_basis_points) = if let Some(
        milestone_id,
    ) = milestone_id.as_deref()
    {
        let milestone: Option<(String, String, i64)> = transaction
            .query_row(
                "SELECT label, kind, amount_minor FROM project_milestones
                 WHERE id = ?1 AND project_id = ?2 AND deleted_at IS NULL",
                params![milestone_id, project_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let (label, kind, _amount_minor) = milestone.ok_or_else(|| {
            AppError::NotFound(
                "Milestone not found for this project. It may belong to a different project."
                    .into(),
            )
        })?;

        let active: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM invoices WHERE milestone_id = ?1 AND deleted_at IS NULL AND status <> 'void'",
            params![milestone_id],
            |row| row.get(0),
        )?;
        if active > 0 {
            return Err(AppError::InvalidInput(
                "This milestone already has an active invoice. Void it first to re-invoice."
                    .into(),
            ));
        }

        let mapped_kind = match kind.as_str() {
            "kickoff" | "completion" => kind,
            _ => "custom".to_string(),
        };
        // Flexible milestones are amount-based, not percentage-based: a
        // linked milestone overrides any legacy percent input so the
        // persisted value never contradicts the resolved milestone.
        (mapped_kind, Some(label), None)
    } else {
        (milestone_kind, milestone_label, milestone_percent_basis_points)
    };

    let seller = load_seller_snapshot(&transaction)?;
    if status == "issued" && seller.name.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "An issued invoice needs a seller display or business name. Open Settings → Invoice profile and add one."
                .into(),
        ));
    }
    let payment_instructions =
        requested_payment_instructions.or_else(|| seller.payment_instructions.clone());
    let invoice_number = reserve_invoice_number(&transaction, issue_date)?;
    let id = Uuid::new_v4().to_string();
    let now = utc_now();
    let effective_status = effective_invoice_status(
        &status,
        amounts.total_minor,
        0,
        due_date,
        Local::now().date_naive(),
    )?;

    transaction.execute(
        "INSERT INTO invoices (
            id,
            project_id,
            client_id,
            invoice_profile_id,
            invoice_number,
            milestone_id,
            milestone_kind,
            milestone_percent_basis_points,
            milestone_label,
            issue_date,
            due_date,
            payment_term_days,
            currency,
            status,
            subtotal_minor,
            discount_total_minor,
            tax_total_minor,
            total_minor,
            notes,
            payment_instructions,
            project_name,
            bill_to_name,
            bill_to_email,
            bill_to_address,
            seller_name,
            seller_email,
            seller_address,
            seller_logo_path,
            created_at,
            updated_at,
            deleted_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
            ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
            ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?29, NULL
         )",
        params![
            id,
            project_id,
            client_id,
            seller.profile_id,
            invoice_number,
            milestone_id,
            milestone_kind,
            milestone_percent_basis_points,
            milestone_label,
            issue_date.format("%Y-%m-%d").to_string(),
            due_date.format("%Y-%m-%d").to_string(),
            payment_term_days,
            currency,
            effective_status,
            amounts.subtotal_minor,
            amounts.discount_total_minor,
            amounts.tax_total_minor,
            amounts.total_minor,
            notes,
            payment_instructions,
            context.project_name,
            context.client_name,
            context.client_email,
            context.billing_address,
            seller.name,
            seller.email,
            seller.address,
            seller.logo_path,
            now,
        ],
    )?;

    for (index, item) in items.iter().enumerate() {
        transaction.execute(
            "INSERT INTO invoice_items (
                id,
                invoice_id,
                description,
                quantity_millis,
                unit_price_minor,
                line_total_minor,
                sort_order,
                created_at,
                updated_at,
                deleted_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, NULL)",
            params![
                Uuid::new_v4().to_string(),
                id,
                item.description,
                item.quantity_millis,
                item.unit_price_minor,
                item.line_total_minor,
                index as i64,
                now,
            ],
        )?;
    }

    for (index, adjustment) in adjustments.iter().enumerate() {
        transaction.execute(
            "INSERT INTO invoice_adjustments (
                id,
                invoice_id,
                kind,
                label,
                calculation_type,
                rate_basis_points,
                amount_minor,
                sort_order,
                created_at,
                updated_at,
                deleted_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, NULL)",
            params![
                Uuid::new_v4().to_string(),
                id,
                adjustment.kind,
                adjustment.label,
                adjustment.calculation_type,
                adjustment.rate_basis_points,
                adjustment.amount_minor,
                index as i64,
                now,
            ],
        )?;
    }

    transaction.commit()?;
    load_invoice(&connection, &id, false)
}

#[tauri::command]
pub(crate) fn update_draft_invoice(
    database: State<'_, Database>,
    input: UpdateDraftInvoiceInput,
) -> Result<Invoice, AppError> {
    update_draft_invoice_with_database(database.inner(), input)
}

fn update_draft_invoice_with_database(
    database: &Database,
    input: UpdateDraftInvoiceInput,
) -> Result<Invoice, AppError> {
    let invoice_id = required_trimmed(input.invoice_id, "Invoice ID", 64)?;
    let project_id = required_trimmed(input.project_id, "Project ID", 64)?;
    let client_id = required_trimmed(input.client_id, "Client ID", 64)?;
    let issue_date = required_date(input.issue_date, "issueDate")?;
    let currency = validate_currency(&input.currency)?;
    let milestone_kind = validate_milestone_kind(&input.milestone_kind)?;
    let notes = optional_trimmed(input.notes, 20_000, "Invoice notes")?;
    let requested_payment_instructions =
        optional_trimmed(input.payment_instructions, 20_000, "Payment instructions")?;
    let (due_date, payment_term_days, _term_kind) =
        calculate_invoice_due_date(issue_date, input.payment_term_days, input.custom_due_date)?;
    let (items, adjustments, amounts) = validate_invoice_contents(input.items, input.adjustments)?;

    let mut connection = database.lock()?;
    ensure_finance_schema_compatibility(&connection)?;
    let transaction = connection.transaction()?;
    let current: Option<(String, String, String)> = transaction
        .query_row(
            "SELECT status, issue_date, invoice_number
             FROM invoices
             WHERE id = ?1 AND deleted_at IS NULL",
            params![invoice_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let (current_status, current_issue_date, current_number) =
        current.ok_or_else(|| AppError::NotFound("Invoice not found.".into()))?;
    if current_status != "draft" {
        return Err(AppError::InvalidInput(
            "Only draft invoices can be edited. Issued and void invoices are immutable.".into(),
        ));
    }

    let context = load_project_invoice_context(&transaction, &project_id)?;
    if context.client_id != client_id {
        return Err(AppError::InvalidInput(
            "clientId does not belong to the selected project.".into(),
        ));
    }
    if context.project_currency != currency {
        return Err(AppError::InvalidInput(format!(
            "Invoice currency must match the project's {} currency.",
            context.project_currency
        )));
    }
    let milestone_percent_basis_points = resolve_milestone_percent(
        &milestone_kind,
        input.milestone_percent_basis_points,
        &context,
    )?;
    let milestone_label =
        optional_trimmed(input.milestone_label, 240, "Milestone label")?.or_else(|| {
            match milestone_kind.as_str() {
                "kickoff" => Some(context.kickoff_label.clone()),
                "completion" => Some(context.completion_label.clone()),
                _ => None,
            }
        });
    let seller = load_seller_snapshot(&transaction)?;
    let payment_instructions =
        requested_payment_instructions.or_else(|| seller.payment_instructions.clone());
    let current_issue_date = parse_stored_date(&current_issue_date, "invoice issue date")?;
    let invoice_number = if current_issue_date == issue_date {
        current_number
    } else {
        reserve_invoice_number(&transaction, issue_date)?
    };
    let issue_date = issue_date.format("%Y-%m-%d").to_string();
    let due_date = due_date.format("%Y-%m-%d").to_string();
    let now = utc_now();

    let affected = transaction.execute(
        "UPDATE invoices
         SET project_id = ?2,
             client_id = ?3,
             invoice_profile_id = ?4,
             invoice_number = ?5,
             milestone_kind = ?6,
             milestone_percent_basis_points = ?7,
             milestone_label = ?8,
             issue_date = ?9,
             due_date = ?10,
             payment_term_days = ?11,
             currency = ?12,
             status = 'draft',
             subtotal_minor = ?13,
             discount_total_minor = ?14,
             tax_total_minor = ?15,
             total_minor = ?16,
             notes = ?17,
             payment_instructions = ?18,
             project_name = ?19,
             bill_to_name = ?20,
             bill_to_email = ?21,
             bill_to_address = ?22,
             seller_name = ?23,
             seller_email = ?24,
             seller_address = ?25,
             seller_logo_path = ?26,
             updated_at = ?27
         WHERE id = ?1 AND status = 'draft' AND deleted_at IS NULL",
        params![
            invoice_id,
            project_id,
            client_id,
            seller.profile_id,
            invoice_number,
            milestone_kind,
            milestone_percent_basis_points,
            milestone_label,
            issue_date,
            due_date,
            payment_term_days,
            currency,
            amounts.subtotal_minor,
            amounts.discount_total_minor,
            amounts.tax_total_minor,
            amounts.total_minor,
            notes,
            payment_instructions,
            context.project_name,
            context.client_name,
            context.client_email,
            context.billing_address,
            seller.name,
            seller.email,
            seller.address,
            seller.logo_path,
            now,
        ],
    )?;
    if affected == 0 {
        return Err(AppError::State(
            "The draft changed before it could be saved. Reload and try again.".into(),
        ));
    }

    transaction.execute(
        "UPDATE invoice_items
         SET deleted_at = ?2, updated_at = ?2
         WHERE invoice_id = ?1 AND deleted_at IS NULL",
        params![invoice_id, now],
    )?;
    transaction.execute(
        "UPDATE invoice_adjustments
         SET deleted_at = ?2, updated_at = ?2
         WHERE invoice_id = ?1 AND deleted_at IS NULL",
        params![invoice_id, now],
    )?;
    for (index, item) in items.iter().enumerate() {
        transaction.execute(
            "INSERT INTO invoice_items (
                id, invoice_id, description, quantity_millis, unit_price_minor,
                line_total_minor, sort_order, created_at, updated_at, deleted_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, NULL)",
            params![
                Uuid::new_v4().to_string(),
                invoice_id,
                item.description,
                item.quantity_millis,
                item.unit_price_minor,
                item.line_total_minor,
                index as i64,
                now,
            ],
        )?;
    }
    for (index, adjustment) in adjustments.iter().enumerate() {
        transaction.execute(
            "INSERT INTO invoice_adjustments (
                id, invoice_id, kind, label, calculation_type,
                rate_basis_points, amount_minor, sort_order, created_at,
                updated_at, deleted_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, NULL)",
            params![
                Uuid::new_v4().to_string(),
                invoice_id,
                adjustment.kind,
                adjustment.label,
                adjustment.calculation_type,
                adjustment.rate_basis_points,
                adjustment.amount_minor,
                index as i64,
                now,
            ],
        )?;
    }

    transaction.commit()?;
    load_invoice(&connection, &invoice_id, false)
}

#[tauri::command]
pub(crate) fn issue_draft_invoice(
    database: State<'_, Database>,
    invoice_id: String,
) -> Result<Invoice, AppError> {
    issue_draft_invoice_with_database(database.inner(), invoice_id)
}

fn issue_draft_invoice_with_database(
    database: &Database,
    invoice_id: String,
) -> Result<Invoice, AppError> {
    let invoice_id = required_trimmed(invoice_id, "Invoice ID", 64)?;
    let mut connection = database.lock()?;
    ensure_finance_schema_compatibility(&connection)?;
    let transaction = connection.transaction()?;
    let target: Option<(String, i64, String, String, String, Option<String>)> = transaction
        .query_row(
            "SELECT status, total_minor, due_date, project_id, currency, payment_instructions
             FROM invoices
             WHERE id = ?1 AND deleted_at IS NULL",
            params![invoice_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()?;
    let (current_status, total_minor, due_date, project_id, currency, current_instructions) =
        target.ok_or_else(|| AppError::NotFound("Invoice not found.".into()))?;
    if current_status != "draft" {
        return Err(AppError::InvalidInput(
            "Only draft invoices can be issued.".into(),
        ));
    }

    let context = load_project_invoice_context(&transaction, &project_id)?;
    if context.project_currency != currency {
        return Err(AppError::InvalidInput(
            "The project currency changed after this draft was created. Edit the draft before issuing it."
                .into(),
        ));
    }
    let seller = load_seller_snapshot(&transaction)?;
    if seller.name.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "An issued invoice needs a seller display or business name. Open Settings → Invoice profile and add one."
                .into(),
        ));
    }
    let due_date_value = parse_stored_date(&due_date, "invoice due date")?;
    let status = effective_invoice_status(
        "issued",
        total_minor,
        0,
        due_date_value,
        Local::now().date_naive(),
    )?;
    let payment_instructions = current_instructions
        .and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        })
        .or_else(|| seller.payment_instructions.clone());
    let now = utc_now();
    let affected = transaction.execute(
        "UPDATE invoices
         SET invoice_profile_id = ?2,
             status = ?3,
             payment_instructions = ?4,
             project_name = ?5,
             bill_to_name = ?6,
             bill_to_email = ?7,
             bill_to_address = ?8,
             seller_name = ?9,
             seller_email = ?10,
             seller_address = ?11,
             seller_logo_path = ?12,
             updated_at = ?13
         WHERE id = ?1 AND status = 'draft' AND deleted_at IS NULL",
        params![
            invoice_id,
            seller.profile_id,
            status,
            payment_instructions,
            context.project_name,
            context.client_name,
            context.client_email,
            context.billing_address,
            seller.name,
            seller.email,
            seller.address,
            seller.logo_path,
            now,
        ],
    )?;
    if affected == 0 {
        return Err(AppError::State(
            "The draft changed before it could be issued. Reload and try again.".into(),
        ));
    }
    transaction.commit()?;
    load_invoice(&connection, &invoice_id, false)
}

#[tauri::command]
pub(crate) fn void_invoice(
    database: State<'_, Database>,
    invoice_id: String,
    confirmed: bool,
) -> Result<Invoice, AppError> {
    void_invoice_with_database(database.inner(), invoice_id, confirmed)
}

fn void_invoice_with_database(
    database: &Database,
    invoice_id: String,
    confirmed: bool,
) -> Result<Invoice, AppError> {
    if !confirmed {
        return Err(AppError::InvalidInput(
            "Voiding an invoice requires explicit confirmation.".into(),
        ));
    }
    let invoice_id = required_trimmed(invoice_id, "Invoice ID", 64)?;
    let mut connection = database.lock()?;
    ensure_finance_schema_compatibility(&connection)?;
    let transaction = connection.transaction()?;
    let current_status: Option<String> = transaction
        .query_row(
            "SELECT status
             FROM invoices
             WHERE id = ?1 AND deleted_at IS NULL",
            params![invoice_id],
            |row| row.get(0),
        )
        .optional()?;
    let current_status =
        current_status.ok_or_else(|| AppError::NotFound("Invoice not found.".into()))?;
    if !matches!(
        current_status.as_str(),
        "issued" | "partially_paid" | "overdue"
    ) {
        return Err(AppError::InvalidInput(
            "Only issued, partially paid, or overdue invoices can be voided.".into(),
        ));
    }
    let now = utc_now();
    transaction.execute(
        "UPDATE invoices
         SET status = 'void', updated_at = ?2
         WHERE id = ?1 AND deleted_at IS NULL",
        params![invoice_id, now],
    )?;
    transaction.commit()?;
    load_invoice(&connection, &invoice_id, false)
}

#[tauri::command]
pub(crate) fn record_invoice_payment(
    database: State<'_, Database>,
    input: RecordInvoicePaymentInput,
) -> Result<Invoice, AppError> {
    let invoice_id = required_trimmed(input.invoice_id, "Invoice ID", 64)?;
    if input.amount_minor <= 0 {
        return Err(AppError::InvalidInput(
            "amountMinor must be greater than zero.".into(),
        ));
    }
    let paid_at = required_date(input.paid_at, "paidAt")?;
    let notes = optional_trimmed(input.notes, 20_000, "Payment notes")?;
    let mut connection = database.lock()?;
    ensure_finance_schema_compatibility(&connection)?;
    let transaction = connection.transaction()?;

    let target: Option<(i64, String, String)> = transaction
        .query_row(
            "SELECT total_minor, status, due_date
             FROM invoices
             WHERE id = ?1 AND deleted_at IS NULL",
            params![invoice_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let (total_minor, current_status, due_date) =
        target.ok_or_else(|| AppError::NotFound("Invoice not found.".into()))?;
    if matches!(current_status.as_str(), "draft" | "void") {
        return Err(AppError::InvalidInput(
            "Payments can only be recorded against issued invoices.".into(),
        ));
    }

    let paid_minor: i64 = transaction.query_row(
        "SELECT COALESCE(SUM(amount_minor), 0)
         FROM invoice_payments
         WHERE invoice_id = ?1 AND deleted_at IS NULL",
        params![invoice_id],
        |row| row.get(0),
    )?;
    let remaining_minor = total_minor.saturating_sub(paid_minor).max(0);
    if input.amount_minor > remaining_minor && !input.allow_overpayment.unwrap_or(false) {
        return Err(AppError::InvalidInput(format!(
            "Payment exceeds the remaining balance of {remaining_minor} minor units; confirm the overpayment first."
        )));
    }

    let now = utc_now();
    transaction.execute(
        "INSERT INTO invoice_payments (
            id,
            invoice_id,
            amount_minor,
            paid_at,
            payment_method,
            reference,
            notes,
            created_at,
            updated_at,
            deleted_at
         ) VALUES (?1, ?2, ?3, ?4, NULL, NULL, ?5, ?6, ?6, NULL)",
        params![
            Uuid::new_v4().to_string(),
            invoice_id,
            input.amount_minor,
            paid_at.format("%Y-%m-%d").to_string(),
            notes,
            now,
        ],
    )?;

    let due_date = parse_stored_date(&due_date, "invoice due date")?;
    let new_paid_minor = paid_minor.checked_add(input.amount_minor).ok_or_else(|| {
        AppError::InvalidInput("The total paid amount exceeds the supported range.".into())
    })?;
    let status = effective_invoice_status(
        &current_status,
        total_minor,
        new_paid_minor,
        due_date,
        Local::now().date_naive(),
    )?;
    transaction.execute(
        "UPDATE invoices
         SET status = ?2, updated_at = ?3
         WHERE id = ?1 AND deleted_at IS NULL",
        params![invoice_id, status, now],
    )?;
    transaction.commit()?;
    load_invoice(&connection, &invoice_id, false)
}

fn load_invoice(
    connection: &Connection,
    id: &str,
    include_deleted: bool,
) -> Result<Invoice, AppError> {
    let mut invoice = connection
        .query_row(
            "SELECT
                i.id,
                i.invoice_number,
                i.project_id,
                COALESCE(NULLIF(TRIM(i.project_name), ''), p.name, 'Deleted project'),
                i.bill_to_name,
                i.bill_to_email,
                i.bill_to_address,
                i.seller_name,
                i.seller_email,
                i.seller_address,
                i.seller_logo_path,
                i.milestone_label,
                i.milestone_kind,
                i.milestone_percent_basis_points,
                i.issue_date,
                i.due_date,
                i.payment_term_days,
                i.currency,
                i.status,
                i.subtotal_minor,
                i.discount_total_minor,
                i.tax_total_minor,
                i.total_minor,
                i.notes,
                i.payment_instructions,
                i.created_at,
                i.updated_at,
                i.deleted_at
             FROM invoices i
             LEFT JOIN projects p ON p.id = i.project_id
             WHERE i.id = ?1 AND (?2 = 1 OR i.deleted_at IS NULL)",
            params![id, include_deleted],
            invoice_from_row,
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("Invoice not found.".into()))?;
    populate_invoice_children_and_status(connection, &mut invoice)?;
    Ok(invoice)
}

fn invoice_from_row(row: &Row<'_>) -> rusqlite::Result<Invoice> {
    let payment_term_days: Option<i64> = row.get(16)?;
    Ok(Invoice {
        id: row.get(0)?,
        number: row.get(1)?,
        project_id: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        project_name: row.get(3)?,
        client_name: row.get(4)?,
        bill_to_email: row.get(5)?,
        bill_to_address: row.get(6)?,
        seller_name: row.get(7)?,
        seller_email: row.get(8)?,
        seller_address: row.get(9)?,
        seller_logo_path: row.get(10)?,
        milestone_label: row.get(11)?,
        milestone_kind: row.get(12)?,
        milestone_percent_basis_points: row.get(13)?,
        issue_date: row.get(14)?,
        due_date: row.get(15)?,
        term_kind: term_kind_for_days(payment_term_days).to_owned(),
        currency: row.get(17)?,
        status: row.get(18)?,
        subtotal_minor: row.get(19)?,
        discount_total_minor: row.get(20)?,
        tax_total_minor: row.get(21)?,
        total_minor: row.get(22)?,
        notes: row.get(23)?,
        payment_instructions: row.get(24)?,
        created_at: row.get(25)?,
        updated_at: row.get(26)?,
        deleted_at: row.get(27)?,
        line_items: Vec::new(),
        adjustments: Vec::new(),
        payments: Vec::new(),
    })
}

fn populate_invoice_children_and_status(
    connection: &Connection,
    invoice: &mut Invoice,
) -> Result<(), AppError> {
    invoice.line_items = load_invoice_items(connection, &invoice.id)?;
    invoice.adjustments = load_invoice_adjustments(connection, &invoice.id)?;
    invoice.payments = load_invoice_payments(connection, &invoice.id)?;
    let paid_minor = checked_sum(
        invoice.payments.iter().map(|payment| payment.amount_minor),
        "invoice payments",
    )?;
    let due_date = parse_stored_date(&invoice.due_date, "invoice due date")?;
    invoice.status = effective_invoice_status(
        &invoice.status,
        invoice.total_minor,
        paid_minor,
        due_date,
        Local::now().date_naive(),
    )?;
    Ok(())
}

fn load_invoice_items(
    connection: &Connection,
    invoice_id: &str,
) -> Result<Vec<InvoiceLineItem>, AppError> {
    let mut statement = connection.prepare(
        "SELECT id, description, quantity_millis, unit_price_minor, line_total_minor
         FROM invoice_items
         WHERE invoice_id = ?1 AND deleted_at IS NULL
         ORDER BY sort_order ASC, created_at ASC",
    )?;
    let rows = statement.query_map(params![invoice_id], |row| {
        Ok(InvoiceLineItem {
            id: row.get(0)?,
            description: row.get(1)?,
            quantity_millis: row.get(2)?,
            unit_price_minor: row.get(3)?,
            line_total_minor: row.get(4)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

fn load_invoice_adjustments(
    connection: &Connection,
    invoice_id: &str,
) -> Result<Vec<InvoiceAdjustment>, AppError> {
    let mut statement = connection.prepare(
        "SELECT id, kind, label, calculation_type, rate_basis_points, amount_minor
         FROM invoice_adjustments
         WHERE invoice_id = ?1 AND deleted_at IS NULL
         ORDER BY sort_order ASC, created_at ASC",
    )?;
    let rows = statement.query_map(params![invoice_id], |row| {
        Ok(InvoiceAdjustment {
            id: row.get(0)?,
            kind: row.get(1)?,
            label: row.get(2)?,
            calculation_type: row.get(3)?,
            rate_basis_points: row.get(4)?,
            amount_minor: row.get(5)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

fn load_invoice_payments(
    connection: &Connection,
    invoice_id: &str,
) -> Result<Vec<InvoicePayment>, AppError> {
    let mut statement = connection.prepare(
        "SELECT id, amount_minor, paid_at, notes, created_at
         FROM invoice_payments
         WHERE invoice_id = ?1 AND deleted_at IS NULL
         ORDER BY paid_at ASC, created_at ASC",
    )?;
    let rows = statement.query_map(params![invoice_id], |row| {
        let paid_at: String = row.get(2)?;
        Ok(InvoicePayment {
            id: row.get(0)?,
            amount_minor: row.get(1)?,
            received_date: date_portion(&paid_at),
            note: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

fn load_project_invoice_context(
    connection: &Connection,
    project_id: &str,
) -> Result<ProjectInvoiceContext, AppError> {
    connection
        .query_row(
            "SELECT
                p.name,
                p.client_id,
                p.currency,
                p.kickoff_percent_basis_points,
                p.completion_percent_basis_points,
                p.kickoff_label,
                p.completion_label,
                COALESCE(NULLIF(TRIM(c.company_name), ''), c.name),
                c.email,
                c.billing_address
             FROM projects p
             JOIN clients c ON c.id = p.client_id
             WHERE p.id = ?1
               AND p.deleted_at IS NULL
               AND c.deleted_at IS NULL
               AND p.status <> 'archived'",
            params![project_id],
            |row| {
                Ok(ProjectInvoiceContext {
                    project_name: row.get(0)?,
                    client_id: row.get(1)?,
                    project_currency: row.get(2)?,
                    kickoff_percent_basis_points: row.get(3)?,
                    completion_percent_basis_points: row.get(4)?,
                    kickoff_label: row.get(5)?,
                    completion_label: row.get(6)?,
                    client_name: row.get(7)?,
                    client_email: row.get(8)?,
                    billing_address: row.get(9)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| {
            AppError::InvalidInput(
                "projectId must identify a non-archived project with an active client.".into(),
            )
        })
}

fn load_seller_snapshot(connection: &Connection) -> Result<SellerSnapshot, AppError> {
    let profile: Option<SellerSnapshot> = connection
        .query_row(
            "SELECT
                id,
                COALESCE(
                    NULLIF(TRIM(business_name), ''),
                    NULLIF(TRIM(display_name), ''),
                    ''
                ),
                email,
                address,
                logo_path,
                payment_instructions
             FROM invoice_profiles
             WHERE is_default = 1 AND deleted_at IS NULL
             LIMIT 1",
            [],
            |row| {
                Ok(SellerSnapshot {
                    profile_id: Some(row.get(0)?),
                    name: row.get(1)?,
                    email: row.get(2)?,
                    address: row.get(3)?,
                    logo_path: row.get(4)?,
                    payment_instructions: row.get(5)?,
                })
            },
        )
        .optional()?;
    Ok(profile.unwrap_or_else(|| SellerSnapshot {
        profile_id: None,
        name: String::new(),
        email: None,
        address: None,
        logo_path: None,
        payment_instructions: None,
    }))
}

fn reserve_invoice_number(
    transaction: &Transaction<'_>,
    issue_date: NaiveDate,
) -> Result<String, AppError> {
    let issue_date = issue_date.format("%Y-%m-%d").to_string();
    let now = utc_now();
    let sequence: i64 = transaction.query_row(
        "INSERT INTO invoice_sequences (issue_date, last_value, updated_at)
         VALUES (?1, 1, ?2)
         ON CONFLICT(issue_date) DO UPDATE
         SET last_value = invoice_sequences.last_value + 1,
             updated_at = excluded.updated_at
         RETURNING last_value",
        params![issue_date, now],
        |row| row.get(0),
    )?;
    if !(1..=MAX_INVOICE_SEQUENCE).contains(&sequence) {
        return Err(AppError::InvalidInput(format!(
            "No invoice numbers remain for {issue_date}; the daily sequence is limited to 9999."
        )));
    }
    Ok(format_invoice_number(&issue_date, sequence))
}

fn format_invoice_number(issue_date: &str, sequence: i64) -> String {
    format!("INV-{issue_date}-{sequence:04}")
}

fn validate_invoice_contents(
    item_inputs: Vec<CreateInvoiceItemInput>,
    adjustment_inputs: Vec<CreateInvoiceAdjustmentInput>,
) -> Result<
    (
        Vec<ValidatedInvoiceItem>,
        Vec<ValidatedInvoiceAdjustment>,
        InvoiceAmounts,
    ),
    AppError,
> {
    if item_inputs.is_empty() {
        return Err(AppError::InvalidInput(
            "An invoice must contain at least one line item.".into(),
        ));
    }
    if item_inputs.len() > MAX_INVOICE_ITEMS {
        return Err(AppError::InvalidInput(format!(
            "An invoice can contain at most {MAX_INVOICE_ITEMS} line items."
        )));
    }
    if adjustment_inputs.len() > MAX_INVOICE_ADJUSTMENTS {
        return Err(AppError::InvalidInput(format!(
            "An invoice can contain at most {MAX_INVOICE_ADJUSTMENTS} adjustments."
        )));
    }

    let mut items = Vec::with_capacity(item_inputs.len());
    let mut subtotal_minor = 0_i64;
    for input in item_inputs {
        let description = required_trimmed(input.description, "Line item description", 1_000)?;
        if input.quantity_millis <= 0 {
            return Err(AppError::InvalidInput(
                "quantityMillis must be greater than zero.".into(),
            ));
        }
        if input.unit_price_minor < 0 {
            return Err(AppError::InvalidInput(
                "unitPriceMinor cannot be negative.".into(),
            ));
        }
        let line_total_minor = rounded_product_ratio(
            input.unit_price_minor,
            input.quantity_millis,
            1_000,
            "line item total",
        )?;
        subtotal_minor = subtotal_minor
            .checked_add(line_total_minor)
            .ok_or_else(|| {
                AppError::InvalidInput("Invoice subtotal exceeds the supported range.".into())
            })?;
        items.push(ValidatedInvoiceItem {
            description,
            quantity_millis: input.quantity_millis,
            unit_price_minor: input.unit_price_minor,
            line_total_minor,
        });
    }

    let mut adjustments = Vec::with_capacity(adjustment_inputs.len());
    for input in adjustment_inputs {
        let kind = validate_adjustment_kind(&input.kind)?;
        let calculation_type = validate_calculation_type(&input.calculation_type)?;
        let label = required_trimmed(input.label, "Adjustment label", 240)?;
        let (rate_basis_points, amount_minor) = match calculation_type.as_str() {
            "percentage" => {
                let rate = input.rate_basis_points.ok_or_else(|| {
                    AppError::InvalidInput(
                        "rateBasisPoints is required for percentage adjustments.".into(),
                    )
                })?;
                if !(0..=10_000).contains(&rate) {
                    return Err(AppError::InvalidInput(
                        "rateBasisPoints must be between 0 and 10000.".into(),
                    ));
                }
                (Some(rate), 0)
            }
            "fixed" => {
                let amount = input.amount_minor.ok_or_else(|| {
                    AppError::InvalidInput("amountMinor is required for fixed adjustments.".into())
                })?;
                if amount < 0 {
                    return Err(AppError::InvalidInput(
                        "Adjustment amountMinor cannot be negative.".into(),
                    ));
                }
                (None, amount)
            }
            _ => unreachable!("calculation type was validated"),
        };
        adjustments.push(ValidatedInvoiceAdjustment {
            kind,
            label,
            calculation_type,
            rate_basis_points,
            amount_minor,
        });
    }

    let mut discount_total_minor = 0_i64;
    for adjustment in adjustments
        .iter_mut()
        .filter(|adjustment| adjustment.kind == "discount")
    {
        if let Some(rate) = adjustment.rate_basis_points {
            adjustment.amount_minor =
                rounded_product_ratio(subtotal_minor, rate, 10_000, "discount")?;
        }
        discount_total_minor = discount_total_minor
            .checked_add(adjustment.amount_minor)
            .ok_or_else(|| {
                AppError::InvalidInput("Invoice discounts exceed the supported range.".into())
            })?;
    }
    if discount_total_minor > subtotal_minor {
        return Err(AppError::InvalidInput(
            "Combined discounts cannot exceed the invoice subtotal.".into(),
        ));
    }

    let taxable_minor = subtotal_minor - discount_total_minor;
    let mut tax_total_minor = 0_i64;
    for adjustment in adjustments
        .iter_mut()
        .filter(|adjustment| adjustment.kind == "tax")
    {
        if let Some(rate) = adjustment.rate_basis_points {
            adjustment.amount_minor = rounded_product_ratio(taxable_minor, rate, 10_000, "tax")?;
        }
        tax_total_minor = tax_total_minor
            .checked_add(adjustment.amount_minor)
            .ok_or_else(|| {
                AppError::InvalidInput("Invoice taxes exceed the supported range.".into())
            })?;
    }
    let total_minor = taxable_minor.checked_add(tax_total_minor).ok_or_else(|| {
        AppError::InvalidInput("Invoice total exceeds the supported range.".into())
    })?;

    Ok((
        items,
        adjustments,
        InvoiceAmounts {
            subtotal_minor,
            discount_total_minor,
            tax_total_minor,
            total_minor,
        },
    ))
}

fn rounded_product_ratio(
    left: i64,
    right: i64,
    divisor: i64,
    label: &str,
) -> Result<i64, AppError> {
    if left < 0 || right < 0 || divisor <= 0 {
        return Err(AppError::InvalidInput(format!(
            "{label} cannot be calculated from negative values."
        )));
    }
    let product = i128::from(left)
        .checked_mul(i128::from(right))
        .ok_or_else(|| AppError::InvalidInput(format!("{label} exceeds the supported range.")))?;
    let rounded = product
        .checked_add(i128::from(divisor / 2))
        .ok_or_else(|| AppError::InvalidInput(format!("{label} exceeds the supported range.")))?
        / i128::from(divisor);
    i64::try_from(rounded)
        .map_err(|_| AppError::InvalidInput(format!("{label} exceeds the supported range.")))
}

fn checked_sum(values: impl IntoIterator<Item = i64>, label: &str) -> Result<i64, AppError> {
    values.into_iter().try_fold(0_i64, |sum, value| {
        sum.checked_add(value).ok_or_else(|| {
            AppError::State(format!(
                "Stored {label} exceed the supported integer range."
            ))
        })
    })
}

fn calculate_invoice_due_date(
    issue_date: NaiveDate,
    payment_term_days: Option<i64>,
    custom_due_date: Option<String>,
) -> Result<(NaiveDate, Option<i64>, &'static str), AppError> {
    match payment_term_days {
        Some(days) => {
            if !matches!(days, 0 | 7 | 14 | 30) {
                return Err(AppError::InvalidInput(
                    "paymentTermDays must be 0, 7, 14, or 30.".into(),
                ));
            }
            if custom_due_date.is_some() {
                return Err(AppError::InvalidInput(
                    "customDueDate cannot be combined with paymentTermDays.".into(),
                ));
            }
            let due_date = issue_date
                .checked_add_signed(Duration::days(days))
                .ok_or_else(|| {
                    AppError::InvalidInput(
                        "The calculated invoice due date is out of range.".into(),
                    )
                })?;
            Ok((due_date, Some(days), term_kind_for_days(Some(days))))
        }
        None => {
            let custom_due_date = custom_due_date.ok_or_else(|| {
                AppError::InvalidInput(
                    "Provide paymentTermDays or an explicit customDueDate.".into(),
                )
            })?;
            let due_date = required_date(custom_due_date, "customDueDate")?;
            if due_date < issue_date {
                return Err(AppError::InvalidInput(
                    "customDueDate cannot be earlier than issueDate.".into(),
                ));
            }
            Ok((due_date, None, "custom"))
        }
    }
}

fn term_kind_for_days(days: Option<i64>) -> &'static str {
    match days {
        Some(0) => "immediate",
        Some(7) => "7-days",
        Some(14) => "14-days",
        Some(30) => "30-days",
        _ => "custom",
    }
}

fn effective_invoice_status(
    stored_status: &str,
    total_minor: i64,
    paid_minor: i64,
    due_date: NaiveDate,
    today: NaiveDate,
) -> Result<String, AppError> {
    if matches!(stored_status, "draft" | "void") {
        return Ok(stored_status.to_owned());
    }
    if total_minor < 0 || paid_minor < 0 {
        return Err(AppError::State(
            "Stored invoice totals or payments are inconsistent.".into(),
        ));
    }
    if paid_minor >= total_minor {
        Ok("paid".into())
    } else if due_date < today {
        Ok("overdue".into())
    } else if paid_minor > 0 {
        Ok("partially_paid".into())
    } else {
        Ok("issued".into())
    }
}

fn validate_currency(value: &str) -> Result<String, AppError> {
    let currency = value.trim().to_ascii_uppercase();
    if currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(AppError::InvalidInput(
            "currency must be a three-letter ISO-style code such as USD.".into(),
        ));
    }
    Ok(currency)
}

fn validate_milestone_kind(value: &str) -> Result<String, AppError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "kickoff" => Ok("kickoff".into()),
        "completion" => Ok("completion".into()),
        "custom" => Ok("custom".into()),
        _ => Err(AppError::InvalidInput(
            "milestoneKind must be kickoff, completion, or custom.".into(),
        )),
    }
}

fn resolve_milestone_percent(
    kind: &str,
    requested: Option<i64>,
    context: &ProjectInvoiceContext,
) -> Result<Option<i64>, AppError> {
    let value = requested.or(match kind {
        "kickoff" => Some(context.kickoff_percent_basis_points),
        "completion" => Some(context.completion_percent_basis_points),
        _ => None,
    });
    if value.is_some_and(|value| !(0..=10_000).contains(&value)) {
        return Err(AppError::InvalidInput(
            "milestonePercentBasisPoints must be between 0 and 10000.".into(),
        ));
    }
    Ok(value)
}

fn validate_new_invoice_status(value: &str) -> Result<String, AppError> {
    match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "draft" => Ok("draft".into()),
        "issued" => Ok("issued".into()),
        _ => Err(AppError::InvalidInput(
            "A new invoice status must be draft or issued. Use the confirmed void action for an existing issued invoice."
                .into(),
        )),
    }
}

fn validate_adjustment_kind(value: &str) -> Result<String, AppError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "discount" => Ok("discount".into()),
        "tax" => Ok("tax".into()),
        _ => Err(AppError::InvalidInput(
            "Adjustment kind must be discount or tax.".into(),
        )),
    }
}

fn validate_calculation_type(value: &str) -> Result<String, AppError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "percentage" => Ok("percentage".into()),
        "fixed" => Ok("fixed".into()),
        _ => Err(AppError::InvalidInput(
            "Adjustment calculationType must be percentage or fixed.".into(),
        )),
    }
}

// ---------------------------------------------------------------------------
// Personal loans

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreatePersonalLoanInput {
    operator_name: String,
    description: Option<String>,
    loan_date: String,
    first_payment_date: String,
    payment_count: i64,
    frequency: String,
    first_monthly_day: Option<i64>,
    second_monthly_day: Option<i64>,
    custom_due_dates: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PersonalLoan {
    id: String,
    operator: String,
    description: Option<String>,
    loan_date: String,
    first_payment_date: String,
    installment_count: i64,
    frequency: String,
    payment_days: Option<[u32; 2]>,
    installments: Vec<LoanInstallment>,
    status: String,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoanInstallment {
    id: String,
    installment_number: i64,
    due_date: String,
    paid: bool,
    paid_date: Option<String>,
}

struct PersonalLoanHeader {
    id: String,
    operator: String,
    description: Option<String>,
    loan_date: String,
    first_payment_date: String,
    installment_count: i64,
    frequency: String,
    first_monthly_day: Option<i64>,
    second_monthly_day: Option<i64>,
    status: String,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

#[tauri::command]
pub(crate) fn list_personal_loans(
    database: State<'_, Database>,
    include_deleted: Option<bool>,
) -> Result<Vec<PersonalLoan>, AppError> {
    let connection = database.lock()?;
    ensure_finance_schema_compatibility(&connection)?;
    let mut statement = connection.prepare(
        "SELECT
            id,
            operator_name,
            description,
            loan_date,
            first_payment_date,
            payment_count,
            frequency,
            first_monthly_day,
            second_monthly_day,
            status,
            created_at,
            updated_at,
            deleted_at
         FROM personal_loans
         WHERE (?1 = 1 OR deleted_at IS NULL)
         ORDER BY created_at DESC",
    )?;
    let rows = statement.query_map(
        params![include_deleted.unwrap_or(false)],
        personal_loan_header_from_row,
    )?;
    let headers = rows.collect::<Result<Vec<_>, _>>()?;
    drop(statement);

    headers
        .into_iter()
        .map(|header| personal_loan_from_header(&connection, header))
        .collect()
}

#[tauri::command]
pub(crate) fn create_personal_loan(
    database: State<'_, Database>,
    input: CreatePersonalLoanInput,
) -> Result<PersonalLoan, AppError> {
    create_personal_loan_with_database(database.inner(), input)
}

fn create_personal_loan_with_database(
    database: &Database,
    input: CreatePersonalLoanInput,
) -> Result<PersonalLoan, AppError> {
    let operator = required_trimmed(input.operator_name, "Loan operator", 240)?;
    let description = optional_trimmed(input.description, 20_000, "Loan description")?;
    let loan_date = required_date(input.loan_date, "loanDate")?;
    let first_payment_date = required_date(input.first_payment_date, "firstPaymentDate")?;
    if first_payment_date < loan_date {
        return Err(AppError::InvalidInput(
            "firstPaymentDate cannot be earlier than loanDate.".into(),
        ));
    }
    if !(1..=MAX_LOAN_INSTALLMENTS).contains(&input.payment_count) {
        return Err(AppError::InvalidInput(format!(
            "paymentCount must be between 1 and {MAX_LOAN_INSTALLMENTS}."
        )));
    }
    let frequency = validate_loan_frequency(&input.frequency)?;
    let due_dates = generate_loan_schedule(
        first_payment_date,
        input.payment_count,
        &frequency,
        input.first_monthly_day,
        input.second_monthly_day,
        input.custom_due_dates.as_deref(),
    )?;

    let id = Uuid::new_v4().to_string();
    let now = utc_now();
    let mut connection = database.lock()?;
    ensure_finance_schema_compatibility(&connection)?;
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO personal_loans (
            id,
            operator_name,
            description,
            loan_date,
            first_payment_date,
            payment_count,
            frequency,
            first_monthly_day,
            second_monthly_day,
            status,
            notes,
            created_at,
            updated_at,
            deleted_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'active', NULL, ?10, ?10, NULL)",
        params![
            id,
            operator,
            description,
            loan_date.format("%Y-%m-%d").to_string(),
            first_payment_date.format("%Y-%m-%d").to_string(),
            input.payment_count,
            frequency,
            input.first_monthly_day,
            input.second_monthly_day,
            now,
        ],
    )?;
    for (index, due_date) in due_dates.iter().enumerate() {
        transaction.execute(
            "INSERT INTO loan_installments (
                id,
                loan_id,
                installment_number,
                due_date,
                is_paid,
                paid_at,
                notes,
                created_at,
                updated_at,
                deleted_at
             ) VALUES (?1, ?2, ?3, ?4, 0, NULL, NULL, ?5, ?5, NULL)",
            params![
                Uuid::new_v4().to_string(),
                id,
                index as i64 + 1,
                due_date.format("%Y-%m-%d").to_string(),
                now,
            ],
        )?;
    }
    transaction.commit()?;
    load_personal_loan(&connection, &id, false)
}

#[tauri::command]
pub(crate) fn update_loan_installment_due_date(
    database: State<'_, Database>,
    installment_id: String,
    due_date: String,
) -> Result<PersonalLoan, AppError> {
    update_loan_installment_due_date_with_database(database.inner(), installment_id, due_date)
}

fn update_loan_installment_due_date_with_database(
    database: &Database,
    installment_id: String,
    due_date: String,
) -> Result<PersonalLoan, AppError> {
    let installment_id = required_trimmed(installment_id, "Installment ID", 64)?;
    let due_date = required_date(due_date, "dueDate")?;
    let mut connection = database.lock()?;
    ensure_finance_schema_compatibility(&connection)?;
    let transaction = connection.transaction()?;
    let target: Option<(String, i64, String)> = transaction
        .query_row(
            "SELECT li.loan_id, li.installment_number, l.loan_date
             FROM loan_installments li
             JOIN personal_loans l ON l.id = li.loan_id
             WHERE li.id = ?1
               AND li.deleted_at IS NULL
               AND l.deleted_at IS NULL",
            params![installment_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let (loan_id, installment_number, loan_date) =
        target.ok_or_else(|| AppError::NotFound("Loan installment not found.".into()))?;
    let loan_date = parse_stored_date(&loan_date, "loan date")?;
    if due_date < loan_date {
        return Err(AppError::InvalidInput(
            "An installment due date cannot be earlier than the loan date.".into(),
        ));
    }
    let previous_due_date: Option<String> = transaction
        .query_row(
            "SELECT due_date
             FROM loan_installments
             WHERE loan_id = ?1
               AND installment_number < ?2
               AND deleted_at IS NULL
             ORDER BY installment_number DESC
             LIMIT 1",
            params![loan_id, installment_number],
            |row| row.get(0),
        )
        .optional()?;
    let next_due_date: Option<String> = transaction
        .query_row(
            "SELECT due_date
             FROM loan_installments
             WHERE loan_id = ?1
               AND installment_number > ?2
               AND deleted_at IS NULL
             ORDER BY installment_number ASC
             LIMIT 1",
            params![loan_id, installment_number],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(previous) = previous_due_date {
        let previous = parse_stored_date(&previous, "previous installment due date")?;
        if due_date < previous {
            return Err(AppError::InvalidInput(
                "The corrected due date cannot be earlier than the previous installment.".into(),
            ));
        }
    }
    if let Some(next) = next_due_date {
        let next = parse_stored_date(&next, "next installment due date")?;
        if due_date > next {
            return Err(AppError::InvalidInput(
                "The corrected due date cannot be later than the next installment.".into(),
            ));
        }
    }

    let due_date = due_date.format("%Y-%m-%d").to_string();
    let now = utc_now();
    transaction.execute(
        "UPDATE loan_installments
         SET due_date = ?2, updated_at = ?3
         WHERE id = ?1 AND deleted_at IS NULL",
        params![installment_id, due_date, now],
    )?;
    if installment_number == 1 {
        transaction.execute(
            "UPDATE personal_loans
             SET first_payment_date = ?2, updated_at = ?3
             WHERE id = ?1 AND deleted_at IS NULL",
            params![loan_id, due_date, now],
        )?;
    } else {
        transaction.execute(
            "UPDATE personal_loans
             SET updated_at = ?2
             WHERE id = ?1 AND deleted_at IS NULL",
            params![loan_id, now],
        )?;
    }
    transaction.commit()?;
    load_personal_loan(&connection, &loan_id, false)
}

#[tauri::command]
pub(crate) fn set_loan_installment_paid(
    database: State<'_, Database>,
    installment_id: String,
    paid: bool,
    paid_date: Option<String>,
) -> Result<PersonalLoan, AppError> {
    set_loan_installment_paid_with_database(database.inner(), installment_id, paid, paid_date)
}

fn set_loan_installment_paid_with_database(
    database: &Database,
    installment_id: String,
    paid: bool,
    paid_date: Option<String>,
) -> Result<PersonalLoan, AppError> {
    let installment_id = required_trimmed(installment_id, "Installment ID", 64)?;
    let mut connection = database.lock()?;
    ensure_finance_schema_compatibility(&connection)?;
    let transaction = connection.transaction()?;
    let target: Option<(String, String)> = transaction
        .query_row(
            "SELECT li.loan_id, l.loan_date
             FROM loan_installments li
             JOIN personal_loans l ON l.id = li.loan_id
             WHERE li.id = ?1
               AND li.deleted_at IS NULL
               AND l.deleted_at IS NULL",
            params![installment_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (loan_id, loan_date) =
        target.ok_or_else(|| AppError::NotFound("Loan installment not found.".into()))?;
    let now = utc_now();
    let paid_at = if paid {
        let paid_date = match paid_date {
            Some(value) if !value.trim().is_empty() => required_date(value, "paidDate")?,
            _ => Local::now().date_naive(),
        };
        let loan_date = parse_stored_date(&loan_date, "loan date")?;
        if paid_date < loan_date {
            return Err(AppError::InvalidInput(
                "paidDate cannot be earlier than the loan date.".into(),
            ));
        }
        if paid_date > Local::now().date_naive() {
            return Err(AppError::InvalidInput(
                "paidDate cannot be in the future.".into(),
            ));
        }
        Some(paid_date.format("%Y-%m-%d").to_string())
    } else {
        None
    };
    transaction.execute(
        "UPDATE loan_installments
         SET is_paid = ?2, paid_at = ?3, updated_at = ?4
         WHERE id = ?1 AND deleted_at IS NULL",
        params![installment_id, paid, paid_at, now],
    )?;
    let all_paid: bool = transaction.query_row(
        "SELECT NOT EXISTS (
            SELECT 1
            FROM loan_installments
            WHERE loan_id = ?1 AND is_paid = 0 AND deleted_at IS NULL
         )",
        params![loan_id],
        |row| row.get(0),
    )?;
    let status = if all_paid { "paid" } else { "active" };
    transaction.execute(
        "UPDATE personal_loans
         SET status = ?2, updated_at = ?3
         WHERE id = ?1 AND deleted_at IS NULL",
        params![loan_id, status, now],
    )?;
    transaction.commit()?;
    load_personal_loan(&connection, &loan_id, false)
}

fn load_personal_loan(
    connection: &Connection,
    id: &str,
    include_deleted: bool,
) -> Result<PersonalLoan, AppError> {
    let header = connection
        .query_row(
            "SELECT
                id,
                operator_name,
                description,
                loan_date,
                first_payment_date,
                payment_count,
                frequency,
                first_monthly_day,
                second_monthly_day,
                status,
                created_at,
                updated_at,
                deleted_at
             FROM personal_loans
             WHERE id = ?1 AND (?2 = 1 OR deleted_at IS NULL)",
            params![id, include_deleted],
            personal_loan_header_from_row,
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("Personal loan not found.".into()))?;
    personal_loan_from_header(connection, header)
}

fn personal_loan_header_from_row(row: &Row<'_>) -> rusqlite::Result<PersonalLoanHeader> {
    Ok(PersonalLoanHeader {
        id: row.get(0)?,
        operator: row.get(1)?,
        description: row.get(2)?,
        loan_date: row.get(3)?,
        first_payment_date: row.get(4)?,
        installment_count: row.get(5)?,
        frequency: row.get(6)?,
        first_monthly_day: row.get(7)?,
        second_monthly_day: row.get(8)?,
        status: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        deleted_at: row.get(12)?,
    })
}

fn personal_loan_from_header(
    connection: &Connection,
    header: PersonalLoanHeader,
) -> Result<PersonalLoan, AppError> {
    let installments = load_loan_installments(connection, &header.id)?;
    let payment_days = if header.frequency == "twice_monthly" {
        let first = parse_stored_date(&header.first_payment_date, "first payment date")?;
        let first_day = header
            .first_monthly_day
            .and_then(|day| u32::try_from(day).ok())
            .unwrap_or_else(|| first.day());
        header
            .second_monthly_day
            .and_then(|second| u32::try_from(second).ok())
            .map(|second| [first_day, second])
    } else {
        None
    };
    Ok(PersonalLoan {
        id: header.id,
        operator: header.operator,
        description: header.description,
        loan_date: header.loan_date,
        first_payment_date: header.first_payment_date,
        installment_count: header.installment_count,
        frequency: header.frequency,
        payment_days,
        installments,
        status: header.status,
        created_at: header.created_at,
        updated_at: header.updated_at,
        deleted_at: header.deleted_at,
    })
}

fn load_loan_installments(
    connection: &Connection,
    loan_id: &str,
) -> Result<Vec<LoanInstallment>, AppError> {
    let mut statement = connection.prepare(
        "SELECT id, installment_number, due_date, is_paid, paid_at
         FROM loan_installments
         WHERE loan_id = ?1 AND deleted_at IS NULL
         ORDER BY installment_number ASC",
    )?;
    let rows = statement.query_map(params![loan_id], |row| {
        let paid_at: Option<String> = row.get(4)?;
        Ok(LoanInstallment {
            id: row.get(0)?,
            installment_number: row.get(1)?,
            due_date: row.get(2)?,
            paid: row.get(3)?,
            paid_date: paid_at.map(|value| date_portion(&value)),
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

fn validate_loan_frequency(value: &str) -> Result<String, AppError> {
    match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "monthly" => Ok("monthly".into()),
        "weekly" => Ok("weekly".into()),
        "biweekly" | "every_two_weeks" => Ok("biweekly".into()),
        "twice_monthly" => Ok("twice_monthly".into()),
        "custom" => Ok("custom".into()),
        _ => Err(AppError::InvalidInput(
            "frequency must be monthly, weekly, biweekly, twice_monthly, or custom.".into(),
        )),
    }
}

fn generate_loan_schedule(
    first_payment_date: NaiveDate,
    payment_count: i64,
    frequency: &str,
    first_monthly_day: Option<i64>,
    second_monthly_day: Option<i64>,
    custom_due_dates: Option<&[String]>,
) -> Result<Vec<NaiveDate>, AppError> {
    if !(1..=MAX_LOAN_INSTALLMENTS).contains(&payment_count) {
        return Err(AppError::InvalidInput(format!(
            "paymentCount must be between 1 and {MAX_LOAN_INSTALLMENTS}."
        )));
    }

    match frequency {
        "monthly" => {
            reject_schedule_extras(first_monthly_day, second_monthly_day, custom_due_dates)?;
            (0..payment_count)
                .map(|offset| {
                    add_months_clamped(first_payment_date, offset, first_payment_date.day())
                })
                .collect()
        }
        "weekly" => {
            reject_schedule_extras(first_monthly_day, second_monthly_day, custom_due_dates)?;
            fixed_day_schedule(first_payment_date, payment_count, 7)
        }
        "biweekly" => {
            reject_schedule_extras(first_monthly_day, second_monthly_day, custom_due_dates)?;
            fixed_day_schedule(first_payment_date, payment_count, 14)
        }
        "twice_monthly" => {
            if custom_due_dates.is_some() {
                return Err(AppError::InvalidInput(
                    "customDueDates can only be used with custom frequency.".into(),
                ));
            }
            let first_day =
                first_monthly_day.unwrap_or_else(|| i64::from(first_payment_date.day()));
            let second_day = second_monthly_day.ok_or_else(|| {
                AppError::InvalidInput(
                    "secondMonthlyDay is required for twice_monthly frequency.".into(),
                )
            })?;
            if !(1..=31).contains(&first_day) {
                return Err(AppError::InvalidInput(
                    "firstMonthlyDay must be between 1 and 31.".into(),
                ));
            }
            if !(1..=31).contains(&second_day) {
                return Err(AppError::InvalidInput(
                    "secondMonthlyDay must be between 1 and 31.".into(),
                ));
            }
            if first_day == second_day {
                return Err(AppError::InvalidInput(
                    "The two twice-monthly payment days must differ.".into(),
                ));
            }

            let first_month_candidates = [
                date_for_day_of_month(
                    first_payment_date.year(),
                    first_payment_date.month(),
                    first_day as u32,
                )?,
                date_for_day_of_month(
                    first_payment_date.year(),
                    first_payment_date.month(),
                    second_day as u32,
                )?,
            ];
            if !first_month_candidates.contains(&first_payment_date) {
                return Err(AppError::InvalidInput(
                    "firstPaymentDate must match one of the selected twice-monthly payment days after month-end clamping."
                        .into(),
                ));
            }

            twice_monthly_schedule(
                first_payment_date,
                payment_count,
                first_day as u32,
                second_day as u32,
            )
        }
        "custom" => {
            if first_monthly_day.is_some() || second_monthly_day.is_some() {
                return Err(AppError::InvalidInput(
                    "firstMonthlyDay and secondMonthlyDay can only be used with twice_monthly frequency."
                        .into(),
                ));
            }
            validate_custom_schedule(first_payment_date, payment_count, custom_due_dates)
        }
        _ => Err(AppError::InvalidInput("Unsupported loan frequency.".into())),
    }
}

fn reject_schedule_extras(
    first_monthly_day: Option<i64>,
    second_monthly_day: Option<i64>,
    custom_due_dates: Option<&[String]>,
) -> Result<(), AppError> {
    if first_monthly_day.is_some() || second_monthly_day.is_some() || custom_due_dates.is_some() {
        return Err(AppError::InvalidInput(
            "firstMonthlyDay, secondMonthlyDay, and customDueDates do not apply to this frequency."
                .into(),
        ));
    }
    Ok(())
}

fn fixed_day_schedule(
    first_payment_date: NaiveDate,
    payment_count: i64,
    interval_days: i64,
) -> Result<Vec<NaiveDate>, AppError> {
    (0..payment_count)
        .map(|index| {
            let offset = index.checked_mul(interval_days).ok_or_else(|| {
                AppError::InvalidInput("Loan schedule exceeds the supported date range.".into())
            })?;
            first_payment_date
                .checked_add_signed(Duration::days(offset))
                .ok_or_else(|| {
                    AppError::InvalidInput("Loan schedule exceeds the supported date range.".into())
                })
        })
        .collect()
}

fn twice_monthly_schedule(
    first_payment_date: NaiveDate,
    payment_count: i64,
    first_day: u32,
    second_day: u32,
) -> Result<Vec<NaiveDate>, AppError> {
    let mut due_dates = Vec::with_capacity(payment_count as usize);
    let mut month_offset = 0_i64;
    while due_dates.len() < payment_count as usize {
        let month = add_months_clamped(first_payment_date, month_offset, 1)?;
        let mut candidates = [
            date_for_day_of_month(month.year(), month.month(), first_day)?,
            date_for_day_of_month(month.year(), month.month(), second_day)?,
        ];
        candidates.sort_unstable();
        for candidate in candidates {
            if candidate >= first_payment_date && due_dates.len() < payment_count as usize {
                due_dates.push(candidate);
            }
        }
        month_offset = month_offset.checked_add(1).ok_or_else(|| {
            AppError::InvalidInput("Loan schedule exceeds the supported date range.".into())
        })?;
    }
    Ok(due_dates)
}

fn validate_custom_schedule(
    first_payment_date: NaiveDate,
    payment_count: i64,
    custom_due_dates: Option<&[String]>,
) -> Result<Vec<NaiveDate>, AppError> {
    let dates = custom_due_dates.ok_or_else(|| {
        AppError::InvalidInput("customDueDates is required for custom frequency.".into())
    })?;
    if dates.len() != payment_count as usize {
        return Err(AppError::InvalidInput(
            "customDueDates must contain exactly paymentCount dates.".into(),
        ));
    }
    let parsed = dates
        .iter()
        .map(|date| required_date(date.clone(), "customDueDates"))
        .collect::<Result<Vec<_>, _>>()?;
    if parsed.first().copied() != Some(first_payment_date) {
        return Err(AppError::InvalidInput(
            "The first custom due date must equal firstPaymentDate.".into(),
        ));
    }
    if parsed.windows(2).any(|dates| dates[0] >= dates[1]) {
        return Err(AppError::InvalidInput(
            "Custom due dates must be unique and in chronological order.".into(),
        ));
    }
    Ok(parsed)
}

fn add_months_clamped(
    date: NaiveDate,
    offset_months: i64,
    preferred_day: u32,
) -> Result<NaiveDate, AppError> {
    let base_month = i64::from(date.year())
        .checked_mul(12)
        .and_then(|value| value.checked_add(i64::from(date.month0())))
        .ok_or_else(|| {
            AppError::InvalidInput("Loan schedule exceeds the supported date range.".into())
        })?;
    let target_month = base_month.checked_add(offset_months).ok_or_else(|| {
        AppError::InvalidInput("Loan schedule exceeds the supported date range.".into())
    })?;
    let year = i32::try_from(target_month.div_euclid(12)).map_err(|_| {
        AppError::InvalidInput("Loan schedule exceeds the supported date range.".into())
    })?;
    let month = u32::try_from(target_month.rem_euclid(12) + 1).map_err(|_| {
        AppError::InvalidInput("Loan schedule exceeds the supported date range.".into())
    })?;
    date_for_day_of_month(year, month, preferred_day)
}

fn date_for_day_of_month(year: i32, month: u32, preferred_day: u32) -> Result<NaiveDate, AppError> {
    if !(1..=31).contains(&preferred_day) {
        return Err(AppError::InvalidInput(
            "Payment days must be between 1 and 31.".into(),
        ));
    }
    for day in (1..=preferred_day).rev() {
        if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
            return Ok(date);
        }
    }
    Err(AppError::InvalidInput(
        "Loan schedule exceeds the supported date range.".into(),
    ))
}

// ---------------------------------------------------------------------------
// Shared helpers

fn required_date(value: String, field: &str) -> Result<NaiveDate, AppError> {
    let normalized = validate_optional_date(Some(value), field)?
        .ok_or_else(|| AppError::InvalidInput(format!("{field} is required.")))?;
    parse_stored_date(&normalized, field)
}

fn parse_stored_date(value: &str, field: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| AppError::State(format!("Stored {field} is not a valid YYYY-MM-DD date.")))
}

fn date_portion(value: &str) -> String {
    value.get(..10).unwrap_or(value).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Mutex};

    const TEST_CLIENT_ID: &str = "client-1";
    const TEST_PROJECT_ID: &str = "project-1";

    fn finance_test_database(seller_name: &str) -> Database {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        crate::apply_migrations(&mut connection).unwrap();
        crate::seed_defaults(&connection).unwrap();

        let now = utc_now();
        connection
            .execute(
                "INSERT INTO invoice_profiles (
                    id, profile_name, display_name, email, address,
                    payment_instructions, is_default, created_at, updated_at
                 ) VALUES (
                    'profile-1', 'Default', ?1, 'seller@example.test',
                    'Seller address', 'Bank transfer', 1, ?2, ?2
                 )",
                params![seller_name, now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO clients (
                    id, name, company_name, email, billing_address,
                    currency, created_at, updated_at
                 ) VALUES (
                    ?1, 'Client contact', 'Client company', 'client@example.test',
                    'Client address', 'USD', ?2, ?2
                 )",
                params![TEST_CLIENT_ID, now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO projects (
                    id, client_id, name, currency, quoted_total_minor,
                    kickoff_percent_basis_points, kickoff_label,
                    completion_percent_basis_points, completion_label,
                    created_at, updated_at
                 ) VALUES (
                    ?1, ?2, 'Website build', 'USD', 100000,
                    5000, 'Kickoff', 5000, 'Completion', ?3, ?3
                 )",
                params![TEST_PROJECT_ID, TEST_CLIENT_ID, now],
            )
            .unwrap();

        Database {
            connection: Mutex::new(connection),
            path: PathBuf::from(":memory:"),
        }
    }

    fn invoice_input(status: &str, issue_date: &str, amount_minor: i64) -> CreateInvoiceInput {
        CreateInvoiceInput {
            project_id: TEST_PROJECT_ID.into(),
            client_id: TEST_CLIENT_ID.into(),
            milestone_id: None,
            milestone_kind: "kickoff".into(),
            milestone_percent_basis_points: None,
            milestone_label: None,
            issue_date: issue_date.into(),
            payment_term_days: Some(14),
            custom_due_date: None,
            currency: "USD".into(),
            status: Some(status.into()),
            notes: Some("Original notes".into()),
            payment_instructions: None,
            items: vec![line("Initial milestone", 1_000, amount_minor)],
            adjustments: Vec::new(),
        }
    }

    fn draft_update_input(
        invoice_id: &str,
        issue_date: &str,
        amount_minor: i64,
    ) -> UpdateDraftInvoiceInput {
        UpdateDraftInvoiceInput {
            invoice_id: invoice_id.into(),
            project_id: TEST_PROJECT_ID.into(),
            client_id: TEST_CLIENT_ID.into(),
            milestone_kind: "completion".into(),
            milestone_percent_basis_points: None,
            milestone_label: Some("Final delivery".into()),
            issue_date: issue_date.into(),
            payment_term_days: Some(30),
            custom_due_date: None,
            currency: "USD".into(),
            notes: Some("Updated notes".into()),
            payment_instructions: Some("Updated payment instructions".into()),
            items: vec![line("Completed website", 1_000, amount_minor)],
            adjustments: vec![adjustment("discount", "fixed", None, Some(500))],
        }
    }

    fn line(
        description: &str,
        quantity_millis: i64,
        unit_price_minor: i64,
    ) -> CreateInvoiceItemInput {
        CreateInvoiceItemInput {
            description: description.into(),
            quantity_millis,
            unit_price_minor,
        }
    }

    fn adjustment(
        kind: &str,
        calculation_type: &str,
        rate_basis_points: Option<i64>,
        amount_minor: Option<i64>,
    ) -> CreateInvoiceAdjustmentInput {
        CreateInvoiceAdjustmentInput {
            kind: kind.into(),
            label: kind.into(),
            calculation_type: calculation_type.into(),
            rate_basis_points,
            amount_minor,
        }
    }

    #[test]
    fn draft_invoice_can_be_rewritten_issued_and_voided_without_losing_snapshots() {
        let database = finance_test_database("Original seller");
        let created =
            create_invoice_with_database(&database, invoice_input("draft", "2099-01-01", 10_000))
                .unwrap();
        let original_number = created.number.clone();
        let original_item_id = created.line_items[0].id.clone();
        assert_eq!(created.status, "draft");
        assert!(matches!(
            create_invoice_with_database(&database, invoice_input("void", "2099-01-02", 10_000)),
            Err(AppError::InvalidInput(_))
        ));
        assert!(matches!(
            void_invoice_with_database(&database, created.id.clone(), true),
            Err(AppError::InvalidInput(_))
        ));

        {
            let connection = database.lock().unwrap();
            connection
                .execute(
                    "UPDATE projects SET name = 'Updated project' WHERE id = ?1",
                    params![TEST_PROJECT_ID],
                )
                .unwrap();
            connection
                .execute(
                    "UPDATE clients
                     SET company_name = 'Updated client', email = 'updated-client@example.test'
                     WHERE id = ?1",
                    params![TEST_CLIENT_ID],
                )
                .unwrap();
            connection
                .execute(
                    "UPDATE invoice_profiles
                     SET business_name = 'Updated studio'
                     WHERE id = 'profile-1'",
                    [],
                )
                .unwrap();
        }

        let updated = update_draft_invoice_with_database(
            &database,
            draft_update_input(&created.id, "2099-01-01", 20_000),
        )
        .unwrap();
        assert_eq!(updated.number, original_number);
        assert_eq!(updated.status, "draft");
        assert_eq!(updated.project_name, "Updated project");
        assert_eq!(updated.client_name, "Updated client");
        assert_eq!(updated.seller_name, "Updated studio");
        assert_eq!(updated.milestone_kind, "completion");
        assert_eq!(updated.milestone_percent_basis_points, Some(5_000));
        assert_eq!(updated.milestone_label.as_deref(), Some("Final delivery"));
        assert_eq!(updated.line_items.len(), 1);
        assert_eq!(updated.line_items[0].description, "Completed website");
        assert_ne!(updated.line_items[0].id, original_item_id);
        assert_eq!(updated.subtotal_minor, 20_000);
        assert_eq!(updated.discount_total_minor, 500);
        assert_eq!(updated.total_minor, 19_500);
        assert_eq!(updated.term_kind, "30-days");

        {
            let connection = database.lock().unwrap();
            let deleted_original_items: i64 = connection
                .query_row(
                    "SELECT COUNT(*)
                     FROM invoice_items
                     WHERE id = ?1 AND deleted_at IS NOT NULL",
                    params![original_item_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(deleted_original_items, 1);
        }

        let renumbered = update_draft_invoice_with_database(
            &database,
            draft_update_input(&created.id, "2099-02-01", 25_000),
        )
        .unwrap();
        assert_eq!(renumbered.number, "INV-2099-02-01-0001");
        assert_ne!(renumbered.number, original_number);

        {
            let connection = database.lock().unwrap();
            connection
                .execute(
                    "UPDATE projects SET name = 'Issued project' WHERE id = ?1",
                    params![TEST_PROJECT_ID],
                )
                .unwrap();
            connection
                .execute(
                    "UPDATE clients SET company_name = 'Issued client' WHERE id = ?1",
                    params![TEST_CLIENT_ID],
                )
                .unwrap();
            connection
                .execute(
                    "UPDATE invoice_profiles
                     SET business_name = 'Issued studio'
                     WHERE id = 'profile-1'",
                    [],
                )
                .unwrap();
        }

        let issued = issue_draft_invoice_with_database(&database, created.id.clone()).unwrap();
        assert_eq!(issued.status, "issued");
        assert_eq!(issued.project_name, "Issued project");
        assert_eq!(issued.client_name, "Issued client");
        assert_eq!(issued.seller_name, "Issued studio");
        assert!(matches!(
            update_draft_invoice_with_database(
                &database,
                draft_update_input(&created.id, "2099-02-01", 30_000)
            ),
            Err(AppError::InvalidInput(_))
        ));
        assert!(matches!(
            void_invoice_with_database(&database, created.id.clone(), false),
            Err(AppError::InvalidInput(_))
        ));

        {
            let connection = database.lock().unwrap();
            connection
                .execute(
                    "UPDATE projects SET name = 'Later project' WHERE id = ?1",
                    params![TEST_PROJECT_ID],
                )
                .unwrap();
            connection
                .execute(
                    "UPDATE clients SET company_name = 'Later client' WHERE id = ?1",
                    params![TEST_CLIENT_ID],
                )
                .unwrap();
            connection
                .execute(
                    "UPDATE invoice_profiles
                     SET business_name = 'Later studio'
                     WHERE id = 'profile-1'",
                    [],
                )
                .unwrap();
            let reloaded = load_invoice(&connection, &created.id, false).unwrap();
            assert_eq!(reloaded.project_name, "Issued project");
            assert_eq!(reloaded.client_name, "Issued client");
            assert_eq!(reloaded.seller_name, "Issued studio");
        }

        let voided = void_invoice_with_database(&database, created.id.clone(), true).unwrap();
        assert_eq!(voided.status, "void");
        assert!(matches!(
            issue_draft_invoice_with_database(&database, created.id),
            Err(AppError::InvalidInput(_))
        ));
    }

    fn insert_project_milestone(
        database: &Database,
        id: &str,
        kind: &str,
        label: &str,
        amount_minor: i64,
    ) {
        let connection = database.lock().unwrap();
        let now = utc_now();
        connection
            .execute(
                "INSERT INTO project_milestones (
                    id, project_id, label, amount_minor, kind, sort_order,
                    created_at, updated_at, deleted_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?6, NULL)",
                params![id, TEST_PROJECT_ID, label, amount_minor, kind, now],
            )
            .unwrap();
    }

    #[test]
    fn create_invoice_links_milestone_and_maps_non_billable_kind_to_custom() {
        let database = finance_test_database("Milestone seller");
        insert_project_milestone(&database, "milestone-1", "phase", "Phase 1", 20_000);

        let mut input = invoice_input("issued", "2099-01-01", 20_000);
        input.milestone_id = Some("milestone-1".into());
        let created = create_invoice_with_database(&database, input).unwrap();

        assert_eq!(created.milestone_kind, "custom");
        assert_eq!(created.milestone_label.as_deref(), Some("Phase 1"));

        let connection = database.lock().unwrap();
        let stored_milestone_id: Option<String> = connection
            .query_row(
                "SELECT milestone_id FROM invoices WHERE id = ?1",
                params![created.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored_milestone_id.as_deref(), Some("milestone-1"));
    }

    #[test]
    fn create_invoice_rejects_a_second_active_invoice_for_the_same_milestone() {
        let database = finance_test_database("Milestone seller");
        insert_project_milestone(&database, "milestone-1", "kickoff", "Kickoff", 20_000);

        let mut first_input = invoice_input("issued", "2099-01-01", 20_000);
        first_input.milestone_id = Some("milestone-1".into());
        let created = create_invoice_with_database(&database, first_input).unwrap();
        assert_eq!(created.milestone_kind, "kickoff");

        let mut second_input = invoice_input("issued", "2099-01-02", 5_000);
        second_input.milestone_id = Some("milestone-1".into());
        assert!(matches!(
            create_invoice_with_database(&database, second_input),
            Err(AppError::InvalidInput(_))
        ));

        // Voiding the first invoice frees the milestone up for re-invoicing.
        void_invoice_with_database(&database, created.id.clone(), true).unwrap();

        let mut third_input = invoice_input("issued", "2099-01-03", 5_000);
        third_input.milestone_id = Some("milestone-1".into());
        let recreated = create_invoice_with_database(&database, third_input).unwrap();
        assert_eq!(recreated.milestone_kind, "kickoff");
        assert_eq!(recreated.milestone_label.as_deref(), Some("Kickoff"));
    }

    #[test]
    fn create_invoice_rejects_milestone_from_another_project() {
        let database = finance_test_database("Milestone seller");
        insert_project_milestone(&database, "milestone-a", "phase", "Project A phase", 20_000);

        const OTHER_PROJECT_ID: &str = "project-2";
        {
            let connection = database.lock().unwrap();
            let now = utc_now();
            connection
                .execute(
                    "INSERT INTO projects (
                        id, client_id, name, currency, quoted_total_minor,
                        kickoff_percent_basis_points, kickoff_label,
                        completion_percent_basis_points, completion_label,
                        created_at, updated_at
                     ) VALUES (
                        ?1, ?2, 'Other project', 'USD', 100000,
                        5000, 'Kickoff', 5000, 'Completion', ?3, ?3
                     )",
                    params![OTHER_PROJECT_ID, TEST_CLIENT_ID, now],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO project_milestones (
                        id, project_id, label, amount_minor, kind, sort_order,
                        created_at, updated_at, deleted_at
                     ) VALUES ('milestone-b', ?1, 'Project B phase', 15000, 'phase', 0, ?2, ?2, NULL)",
                    params![OTHER_PROJECT_ID, now],
                )
                .unwrap();
        }

        // Invoice targets TEST_PROJECT_ID ("project A") but references the
        // milestone that belongs to OTHER_PROJECT_ID ("project B"). This must
        // be rejected rather than silently cross-linking billing across
        // projects (and, transitively, across currencies).
        let mut input = invoice_input("issued", "2099-01-01", 10_000);
        input.milestone_id = Some("milestone-b".into());

        assert!(matches!(
            create_invoice_with_database(&database, input),
            Err(AppError::NotFound(_))
        ));

        let connection = database.lock().unwrap();
        let invoice_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM invoices", [], |row| row.get(0))
            .unwrap();
        assert_eq!(invoice_count, 0);
    }

    #[test]
    fn create_invoice_rejects_unknown_milestone_id() {
        let database = finance_test_database("Milestone seller");
        let mut input = invoice_input("issued", "2099-01-01", 10_000);
        input.milestone_id = Some("does-not-exist".into());
        assert!(matches!(
            create_invoice_with_database(&database, input),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn issuing_a_draft_requires_a_seller_identity() {
        let database = finance_test_database("");
        let draft =
            create_invoice_with_database(&database, invoice_input("draft", "2099-01-01", 10_000))
                .unwrap();

        assert!(matches!(
            issue_draft_invoice_with_database(&database, draft.id.clone()),
            Err(AppError::InvalidInput(_))
        ));

        {
            let connection = database.lock().unwrap();
            connection
                .execute(
                    "UPDATE invoice_profiles
                     SET display_name = 'Configured seller'
                     WHERE id = 'profile-1'",
                    [],
                )
                .unwrap();
        }
        let issued = issue_draft_invoice_with_database(&database, draft.id).unwrap();
        assert_eq!(issued.status, "issued");
        assert_eq!(issued.seller_name, "Configured seller");
    }

    #[test]
    fn installment_corrections_preserve_order_and_explicit_payment_dates() {
        let database = finance_test_database("Seller");
        let loan = create_personal_loan_with_database(
            &database,
            CreatePersonalLoanInput {
                operator_name: "Personal lender".into(),
                description: Some("Three payments".into()),
                loan_date: "2020-01-01".into(),
                first_payment_date: "2020-02-10".into(),
                payment_count: 3,
                frequency: "monthly".into(),
                first_monthly_day: None,
                second_monthly_day: None,
                custom_due_dates: None,
            },
        )
        .unwrap();
        let first_id = loan.installments[0].id.clone();
        let second_id = loan.installments[1].id.clone();
        let third_id = loan.installments[2].id.clone();

        let corrected = update_loan_installment_due_date_with_database(
            &database,
            second_id.clone(),
            "2020-03-15".into(),
        )
        .unwrap();
        assert_eq!(corrected.installments[1].due_date, "2020-03-15");
        assert!(matches!(
            update_loan_installment_due_date_with_database(
                &database,
                second_id.clone(),
                "2020-04-11".into()
            ),
            Err(AppError::InvalidInput(_))
        ));

        let corrected_first = update_loan_installment_due_date_with_database(
            &database,
            first_id.clone(),
            "2020-02-12".into(),
        )
        .unwrap();
        assert_eq!(corrected_first.first_payment_date, "2020-02-12");
        assert_eq!(corrected_first.installments[0].due_date, "2020-02-12");
        assert!(matches!(
            update_loan_installment_due_date_with_database(
                &database,
                first_id.clone(),
                "2019-12-31".into()
            ),
            Err(AppError::InvalidInput(_))
        ));

        let partly_paid = set_loan_installment_paid_with_database(
            &database,
            first_id,
            true,
            Some("2020-02-13".into()),
        )
        .unwrap();
        assert_eq!(
            partly_paid.installments[0].paid_date.as_deref(),
            Some("2020-02-13")
        );
        assert_eq!(partly_paid.status, "active");

        set_loan_installment_paid_with_database(
            &database,
            second_id.clone(),
            true,
            Some("2020-03-01".into()),
        )
        .unwrap();
        let fully_paid = set_loan_installment_paid_with_database(
            &database,
            third_id,
            true,
            Some("2020-04-01".into()),
        )
        .unwrap();
        assert_eq!(fully_paid.status, "paid");
        assert!(fully_paid
            .installments
            .iter()
            .all(|installment| installment.paid));

        let reopened = set_loan_installment_paid_with_database(
            &database,
            second_id.clone(),
            false,
            Some("2999-01-01".into()),
        )
        .unwrap();
        assert_eq!(reopened.status, "active");
        assert!(!reopened.installments[1].paid);
        assert_eq!(reopened.installments[1].paid_date, None);

        let future = (Local::now().date_naive() + Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        assert!(matches!(
            set_loan_installment_paid_with_database(
                &database,
                second_id.clone(),
                true,
                Some(future)
            ),
            Err(AppError::InvalidInput(_))
        ));
        assert!(matches!(
            set_loan_installment_paid_with_database(
                &database,
                second_id,
                true,
                Some("2019-12-31".into())
            ),
            Err(AppError::InvalidInput(_))
        ));
    }

    #[test]
    fn invoice_number_uses_exact_date_and_four_digit_sequence() {
        assert_eq!(
            format_invoice_number("2026-07-23", 1),
            "INV-2026-07-23-0001"
        );
        assert_eq!(
            format_invoice_number("2026-07-23", 42),
            "INV-2026-07-23-0042"
        );
    }

    #[test]
    fn finance_schema_compatibility_adds_and_backfills_snapshot_columns() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE projects (id TEXT PRIMARY KEY, name TEXT NOT NULL);
                 CREATE TABLE invoices (
                    id TEXT PRIMARY KEY,
                    project_id TEXT
                 );
                 CREATE TABLE personal_loans (
                    id TEXT PRIMARY KEY,
                    frequency TEXT NOT NULL,
                    first_payment_date TEXT NOT NULL
                 );
                 INSERT INTO projects (id, name) VALUES ('project-1', 'Snapshot project');
                 INSERT INTO invoices (id, project_id) VALUES ('invoice-1', 'project-1');
                 INSERT INTO personal_loans (id, frequency, first_payment_date)
                 VALUES ('loan-1', 'twice_monthly', '2027-02-28');",
            )
            .unwrap();

        ensure_finance_schema_compatibility(&connection).unwrap();
        ensure_finance_schema_compatibility(&connection).unwrap();

        let project_name: String = connection
            .query_row(
                "SELECT project_name FROM invoices WHERE id = 'invoice-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let first_monthly_day: i64 = connection
            .query_row(
                "SELECT first_monthly_day FROM personal_loans WHERE id = 'loan-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(project_name, "Snapshot project");
        assert_eq!(first_monthly_day, 28);
    }

    #[test]
    fn invoice_calculation_applies_discount_before_tax() {
        let (_, calculated, totals) = validate_invoice_contents(
            vec![line("Project", 1_000, 10_000)],
            vec![
                adjustment("discount", "percentage", Some(1_000), None),
                adjustment("tax", "percentage", Some(1_000), None),
            ],
        )
        .unwrap();
        assert_eq!(totals.subtotal_minor, 10_000);
        assert_eq!(totals.discount_total_minor, 1_000);
        assert_eq!(totals.tax_total_minor, 900);
        assert_eq!(totals.total_minor, 9_900);
        assert_eq!(calculated[0].amount_minor, 1_000);
        assert_eq!(calculated[1].amount_minor, 900);
    }

    #[test]
    fn invoice_calculation_rounds_half_up_and_rejects_excess_discount() {
        let (_, _, totals) = validate_invoice_contents(vec![line("Half", 500, 1)], vec![]).unwrap();
        assert_eq!(totals.subtotal_minor, 1);
        assert!(validate_invoice_contents(
            vec![line("Project", 1_000, 100)],
            vec![adjustment("discount", "fixed", None, Some(101))]
        )
        .is_err());
    }

    #[test]
    fn standard_and_custom_invoice_terms_are_validated() {
        let issue = NaiveDate::from_ymd_opt(2026, 7, 23).unwrap();
        let (due, days, kind) = calculate_invoice_due_date(issue, Some(30), None).unwrap();
        assert_eq!(due, NaiveDate::from_ymd_opt(2026, 8, 22).unwrap());
        assert_eq!(days, Some(30));
        assert_eq!(kind, "30-days");
        assert!(calculate_invoice_due_date(issue, None, Some("2026-07-22".into())).is_err());
    }

    #[test]
    fn status_preserves_draft_and_void_and_derives_payment_state() {
        let today = NaiveDate::from_ymd_opt(2026, 7, 23).unwrap();
        let future = NaiveDate::from_ymd_opt(2026, 8, 1).unwrap();
        let past = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        assert_eq!(
            effective_invoice_status("draft", 100, 100, past, today).unwrap(),
            "draft"
        );
        assert_eq!(
            effective_invoice_status("void", 100, 0, future, today).unwrap(),
            "void"
        );
        assert_eq!(
            effective_invoice_status("issued", 100, 50, future, today).unwrap(),
            "partially_paid"
        );
        assert_eq!(
            effective_invoice_status("issued", 100, 50, past, today).unwrap(),
            "overdue"
        );
        assert_eq!(
            effective_invoice_status("issued", 100, 100, past, today).unwrap(),
            "paid"
        );
        assert_eq!(
            effective_invoice_status("issued", 100, 150, past, today).unwrap(),
            "paid"
        );
    }

    #[test]
    fn monthly_schedule_retains_original_day_after_clamping() {
        let first = NaiveDate::from_ymd_opt(2027, 1, 31).unwrap();
        let dates = generate_loan_schedule(first, 4, "monthly", None, None, None).unwrap();
        assert_eq!(
            dates,
            vec![
                NaiveDate::from_ymd_opt(2027, 1, 31).unwrap(),
                NaiveDate::from_ymd_opt(2027, 2, 28).unwrap(),
                NaiveDate::from_ymd_opt(2027, 3, 31).unwrap(),
                NaiveDate::from_ymd_opt(2027, 4, 30).unwrap(),
            ]
        );
    }

    #[test]
    fn weekly_and_biweekly_schedules_start_on_explicit_first_date() {
        let first = NaiveDate::from_ymd_opt(2026, 8, 13).unwrap();
        let weekly = generate_loan_schedule(first, 3, "weekly", None, None, None).unwrap();
        let biweekly = generate_loan_schedule(first, 3, "biweekly", None, None, None).unwrap();
        assert_eq!(weekly[1], NaiveDate::from_ymd_opt(2026, 8, 20).unwrap());
        assert_eq!(biweekly[2], NaiveDate::from_ymd_opt(2026, 9, 10).unwrap());
    }

    #[test]
    fn twice_monthly_schedule_clamps_each_selected_day() {
        let first = NaiveDate::from_ymd_opt(2027, 1, 15).unwrap();
        let dates =
            generate_loan_schedule(first, 5, "twice_monthly", Some(15), Some(31), None).unwrap();
        assert_eq!(
            dates,
            vec![
                NaiveDate::from_ymd_opt(2027, 1, 15).unwrap(),
                NaiveDate::from_ymd_opt(2027, 1, 31).unwrap(),
                NaiveDate::from_ymd_opt(2027, 2, 15).unwrap(),
                NaiveDate::from_ymd_opt(2027, 2, 28).unwrap(),
                NaiveDate::from_ymd_opt(2027, 3, 15).unwrap(),
            ]
        );
    }

    #[test]
    fn twice_monthly_schedule_preserves_selected_day_after_first_date_is_clamped() {
        let first = NaiveDate::from_ymd_opt(2027, 2, 28).unwrap();
        let dates =
            generate_loan_schedule(first, 5, "twice_monthly", Some(31), Some(15), None).unwrap();
        assert_eq!(
            dates,
            vec![
                NaiveDate::from_ymd_opt(2027, 2, 28).unwrap(),
                NaiveDate::from_ymd_opt(2027, 3, 15).unwrap(),
                NaiveDate::from_ymd_opt(2027, 3, 31).unwrap(),
                NaiveDate::from_ymd_opt(2027, 4, 15).unwrap(),
                NaiveDate::from_ymd_opt(2027, 4, 30).unwrap(),
            ]
        );
    }

    #[test]
    fn custom_schedule_must_match_count_first_date_and_order() {
        let first = NaiveDate::from_ymd_opt(2026, 8, 13).unwrap();
        let valid = vec![
            "2026-08-13".to_owned(),
            "2026-09-02".to_owned(),
            "2026-11-30".to_owned(),
        ];
        assert_eq!(
            generate_loan_schedule(first, 3, "custom", None, None, Some(&valid))
                .unwrap()
                .len(),
            3
        );
        assert!(generate_loan_schedule(first, 2, "custom", None, None, Some(&valid)).is_err());
        let unordered = vec!["2026-08-13".to_owned(), "2026-08-13".to_owned()];
        assert!(generate_loan_schedule(first, 2, "custom", None, None, Some(&unordered)).is_err());
    }
}
