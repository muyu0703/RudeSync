use std::{
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, Row};
use serde::Serialize;
use tauri::State as TauriState;

use super::{AppError, Database};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CsvExportResult {
    destination_path: String,
    row_count: usize,
}

#[tauri::command]
pub(crate) fn export_csv(
    database: TauriState<'_, Database>,
    export_kind: String,
    destination: String,
) -> Result<CsvExportResult, AppError> {
    let destination = validate_destination(&destination)?;
    let (headers, rows) = {
        let connection = database.lock()?;
        match export_kind.trim() {
            "tasks" => export_tasks(&connection)?,
            "completed-work" => export_completed_work(&connection)?,
            "invoices-payments" => export_invoices_and_payments(&connection)?,
            "loan-schedules" => export_loan_schedules(&connection)?,
            _ => return Err(AppError::InvalidInput("Unknown CSV export type.".into())),
        }
    };

    let contents = render_csv(headers, &rows);
    fs::write(&destination, contents.as_bytes())?;

    Ok(CsvExportResult {
        destination_path: destination.to_string_lossy().into_owned(),
        row_count: rows.len(),
    })
}

fn export_tasks(
    connection: &Connection,
) -> Result<(&'static [&'static str], Vec<Vec<String>>), AppError> {
    const HEADERS: &[&str] = &[
        "ID",
        "Title",
        "Status",
        "Priority",
        "Category",
        "Planned date",
        "Deadline",
        "Reminder",
        "Project",
        "Notes",
        "Created at",
        "Completed at",
    ];
    let mut statement = connection.prepare(
        "SELECT
            t.id, t.title, t.status, t.priority, COALESCE(t.category, ''),
            COALESCE(t.planned_date, ''), COALESCE(t.due_date, ''),
            COALESCE(r.remind_at, ''), COALESCE(p.name, ''),
            COALESCE(t.notes, ''), t.created_at, COALESCE(t.completed_at, '')
         FROM tasks t
         LEFT JOIN projects p
           ON p.id = t.project_id AND p.deleted_at IS NULL
         LEFT JOIN task_reminders r
           ON r.task_id = t.id AND r.deleted_at IS NULL
         WHERE t.deleted_at IS NULL
         ORDER BY COALESCE(t.planned_date, t.due_date, '9999-12-31'), t.created_at",
    )?;
    Ok((HEADERS, collect_text_rows(&mut statement, HEADERS.len())?))
}

fn export_completed_work(
    connection: &Connection,
) -> Result<(&'static [&'static str], Vec<Vec<String>>), AppError> {
    const HEADERS: &[&str] = &[
        "ID",
        "Completed date",
        "Title",
        "Client",
        "Project",
        "Details",
        "URLs",
        "Created at",
    ];
    let mut statement = connection.prepare(
        "SELECT
            w.id, w.work_date, w.title, COALESCE(c.name, ''),
            COALESCE(p.name, ''), COALESCE(w.details, ''), w.urls_json,
            w.created_at
         FROM work_entries w
         LEFT JOIN projects p
           ON p.id = w.project_id AND p.deleted_at IS NULL
         LEFT JOIN clients c
           ON c.id = p.client_id AND c.deleted_at IS NULL
         WHERE w.deleted_at IS NULL
         ORDER BY w.work_date DESC, w.created_at DESC",
    )?;
    Ok((HEADERS, collect_text_rows(&mut statement, HEADERS.len())?))
}

fn export_invoices_and_payments(
    connection: &Connection,
) -> Result<(&'static [&'static str], Vec<Vec<String>>), AppError> {
    const HEADERS: &[&str] = &[
        "Invoice ID",
        "Invoice number",
        "Status",
        "Client",
        "Project",
        "Issue date",
        "Due date",
        "Currency",
        "Subtotal (minor units)",
        "Discount (minor units)",
        "Tax (minor units)",
        "Total (minor units)",
        "Payment ID",
        "Payment date",
        "Payment amount (minor units)",
        "Payment reference",
        "Payment notes",
    ];
    let mut statement = connection.prepare(
        "SELECT
            i.id, i.invoice_number, i.status, i.bill_to_name,
            COALESCE(i.project_name, p.name, ''), i.issue_date, i.due_date,
            i.currency, CAST(i.subtotal_minor AS TEXT),
            CAST(i.discount_total_minor AS TEXT), CAST(i.tax_total_minor AS TEXT),
            CAST(i.total_minor AS TEXT), COALESCE(pay.id, ''),
            COALESCE(pay.paid_at, ''),
            CASE WHEN pay.id IS NULL THEN '' ELSE CAST(pay.amount_minor AS TEXT) END,
            COALESCE(pay.reference, ''), COALESCE(pay.notes, '')
         FROM invoices i
         LEFT JOIN projects p
           ON p.id = i.project_id
         LEFT JOIN invoice_payments pay
           ON pay.invoice_id = i.id AND pay.deleted_at IS NULL
         WHERE i.deleted_at IS NULL
         ORDER BY i.issue_date DESC, i.invoice_number DESC, pay.paid_at",
    )?;
    Ok((HEADERS, collect_text_rows(&mut statement, HEADERS.len())?))
}

fn export_loan_schedules(
    connection: &Connection,
) -> Result<(&'static [&'static str], Vec<Vec<String>>), AppError> {
    const HEADERS: &[&str] = &[
        "Loan ID",
        "Operator",
        "Description",
        "Loan date",
        "First payment date",
        "Frequency",
        "Installment",
        "Due date",
        "Paid",
        "Paid at",
        "Notes",
    ];
    let mut statement = connection.prepare(
        "SELECT
            l.id, l.operator_name, COALESCE(l.description, ''), l.loan_date,
            l.first_payment_date, l.frequency,
            CAST(i.installment_number AS TEXT), i.due_date,
            CASE i.is_paid WHEN 1 THEN 'Yes' ELSE 'No' END,
            COALESCE(i.paid_at, ''), COALESCE(i.notes, '')
         FROM personal_loans l
         JOIN loan_installments i
           ON i.loan_id = l.id AND i.deleted_at IS NULL
         WHERE l.deleted_at IS NULL
         ORDER BY l.operator_name COLLATE NOCASE, i.installment_number",
    )?;
    Ok((HEADERS, collect_text_rows(&mut statement, HEADERS.len())?))
}

fn collect_text_rows(
    statement: &mut rusqlite::Statement<'_>,
    column_count: usize,
) -> Result<Vec<Vec<String>>, AppError> {
    let mut query = statement.query([])?;
    let mut output = Vec::new();
    while let Some(row) = query.next()? {
        output.push(text_row(row, column_count)?);
    }
    Ok(output)
}

fn text_row(row: &Row<'_>, column_count: usize) -> Result<Vec<String>, rusqlite::Error> {
    let mut values = Vec::with_capacity(column_count);
    for index in 0..column_count {
        values.push(row.get::<_, String>(index)?);
    }
    Ok(values)
}

fn validate_destination(destination: &str) -> Result<PathBuf, AppError> {
    let trimmed = destination.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput(
            "Choose a destination for the CSV file.".into(),
        ));
    }
    let path = PathBuf::from(trimmed);
    if path
        .extension()
        .and_then(|value| value.to_str())
        .map_or(true, |value| !value.eq_ignore_ascii_case("csv"))
    {
        return Err(AppError::InvalidInput(
            "CSV exports must use a .csv file extension.".into(),
        ));
    }
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    if parent.as_os_str().is_empty() || !parent.is_dir() {
        return Err(AppError::InvalidInput(
            "The CSV destination folder does not exist.".into(),
        ));
    }
    if path.is_dir() {
        return Err(AppError::InvalidInput(
            "The CSV destination must be a file.".into(),
        ));
    }
    Ok(path)
}

fn render_csv(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut output = String::from("\u{feff}");
    append_csv_row(&mut output, headers.iter().copied());
    for row in rows {
        append_csv_row(&mut output, row.iter().map(String::as_str));
    }
    output
}

fn append_csv_row<'a>(output: &mut String, values: impl IntoIterator<Item = &'a str>) {
    let mut first = true;
    for value in values {
        if !first {
            output.push(',');
        }
        first = false;
        let safe_value = protect_spreadsheet_formula(value);
        output.push('"');
        output.push_str(&safe_value.replace('"', "\"\""));
        output.push('"');
    }
    output.push_str("\r\n");
}

fn protect_spreadsheet_formula(value: &str) -> String {
    let trimmed = value.trim_start();
    if trimmed.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_escapes_quotes_commas_and_line_breaks() {
        let csv = render_csv(
            &["Name", "Notes"],
            &[vec![
                "Client, Inc.".into(),
                "Said \"yes\"\nthen paid".into(),
            ]],
        );
        assert_eq!(
            csv,
            "\u{feff}\"Name\",\"Notes\"\r\n\"Client, Inc.\",\"Said \"\"yes\"\"\nthen paid\"\r\n"
        );
    }

    #[test]
    fn csv_neutralizes_spreadsheet_formulas() {
        let csv = render_csv(&["Value"], &[vec!["=HYPERLINK(\"bad\")".into()]]);
        assert!(csv.contains("\"'=HYPERLINK(\"\"bad\"\")\""));
    }

    #[test]
    fn every_export_query_runs_against_the_current_schema() {
        let mut connection = Connection::open_in_memory().unwrap();
        crate::apply_migrations(&mut connection).unwrap();
        crate::seed_defaults(&connection).unwrap();

        assert!(export_tasks(&connection).unwrap().1.is_empty());
        assert!(export_completed_work(&connection).unwrap().1.is_empty());
        assert!(export_invoices_and_payments(&connection)
            .unwrap()
            .1
            .is_empty());
        assert!(export_loan_schedules(&connection).unwrap().1.is_empty());
    }
}
