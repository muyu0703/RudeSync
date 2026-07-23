use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime};
use rusqlite::params;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;
use uuid::Uuid;

use super::{utc_now, AppError, Database};

const INVOICE_OFFSETS_DAYS: [i64; 2] = [3, 0];
const LOAN_OFFSETS_DAYS: [i64; 3] = [7, 1, 0];
const DUE_REMINDER_HOUR: u32 = 9;

#[derive(Debug)]
struct InvoiceNotice {
    id: String,
    number: String,
    client: String,
    due_date: NaiveDate,
}

#[derive(Debug)]
struct LoanNotice {
    installment_id: String,
    operator: String,
    installment_number: i64,
    due_date: NaiveDate,
}

pub(crate) fn dispatch_due_notifications(app: &AppHandle) -> Result<(), AppError> {
    let database = app.state::<Database>();
    if !notifications_enabled(&database)? {
        return Ok(());
    }

    dispatch_task_reminders(app, &database)?;
    let now_local = Local::now().naive_local();
    dispatch_invoice_reminders(app, &database, now_local)?;
    dispatch_loan_reminders(app, &database, now_local)?;
    Ok(())
}

fn notifications_enabled(database: &Database) -> Result<bool, AppError> {
    let connection = database.lock()?;
    Ok(connection
        .query_row(
            "SELECT notifications_enabled FROM app_settings WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(true))
}

fn dispatch_task_reminders(app: &AppHandle, database: &Database) -> Result<(), AppError> {
    let now = utc_now();
    let reminders = {
        let connection = database.lock()?;
        let mut statement = connection.prepare(
            "SELECT reminder.id, task.title
             FROM task_reminders reminder
             JOIN tasks task ON task.id = reminder.task_id
             WHERE reminder.deleted_at IS NULL
               AND reminder.delivered_at IS NULL
               AND task.deleted_at IS NULL
               AND task.status = 'open'
               AND COALESCE(reminder.snoozed_until, reminder.remind_at) <= ?1",
        )?;
        let rows = statement
            .query_map(params![now], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };

    for (reminder_id, title) in reminders {
        show_notification(app, "RudeSync task", &title)?;
        let connection = database.lock()?;
        connection.execute(
            "UPDATE task_reminders
             SET delivered_at = ?2, updated_at = ?2
             WHERE id = ?1 AND delivered_at IS NULL",
            params![reminder_id, utc_now()],
        )?;
    }
    Ok(())
}

fn dispatch_invoice_reminders(
    app: &AppHandle,
    database: &Database,
    now_local: NaiveDateTime,
) -> Result<(), AppError> {
    let invoices = {
        let connection = database.lock()?;
        let mut statement = connection.prepare(
            "SELECT id, invoice_number, bill_to_name, due_date
             FROM invoices
             WHERE deleted_at IS NULL
               AND status IN ('issued', 'partially_paid', 'overdue')
             ORDER BY due_date, invoice_number",
        )?;
        let rows = statement
            .query_map([], |row| {
                let due_date: String = row.get(3)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    due_date,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };

    for (id, number, client, due_date) in invoices {
        let due_date = parse_stored_date(&due_date, "invoice due date")?;
        let notice = InvoiceNotice {
            id,
            number,
            client,
            due_date,
        };
        let pending = pending_offsets(
            database,
            "invoice_due",
            &notice.id,
            notice.due_date,
            &INVOICE_OFFSETS_DAYS,
            now_local,
        )?;
        let Some(selected_offset) = pending.iter().copied().min() else {
            continue;
        };

        let body = format!(
            "{} for {} {}.",
            notice.number,
            notice.client,
            due_timing(notice.due_date, now_local.date(), selected_offset)
        );
        show_notification(app, "RudeSync invoice", &body)?;
        mark_offsets_delivered(database, "invoice_due", &notice.id, &pending)?;
    }
    Ok(())
}

fn dispatch_loan_reminders(
    app: &AppHandle,
    database: &Database,
    now_local: NaiveDateTime,
) -> Result<(), AppError> {
    let installments = {
        let connection = database.lock()?;
        let mut statement = connection.prepare(
            "SELECT installment.id, loan.operator_name,
                    installment.installment_number, installment.due_date
             FROM loan_installments installment
             JOIN personal_loans loan ON loan.id = installment.loan_id
             WHERE installment.deleted_at IS NULL
               AND installment.is_paid = 0
               AND loan.deleted_at IS NULL
               AND loan.status = 'active'
             ORDER BY installment.due_date, loan.operator_name,
                      installment.installment_number",
        )?;
        let rows = statement
            .query_map([], |row| {
                let due_date: String = row.get(3)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    due_date,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };

    for (installment_id, operator, installment_number, due_date) in installments {
        let due_date = parse_stored_date(&due_date, "loan installment due date")?;
        let notice = LoanNotice {
            installment_id,
            operator,
            installment_number,
            due_date,
        };
        let pending = pending_offsets(
            database,
            "loan_due",
            &notice.installment_id,
            notice.due_date,
            &LOAN_OFFSETS_DAYS,
            now_local,
        )?;
        let Some(selected_offset) = pending.iter().copied().min() else {
            continue;
        };

        let body = format!(
            "{} installment #{} {}.",
            notice.operator,
            notice.installment_number,
            due_timing(notice.due_date, now_local.date(), selected_offset)
        );
        show_notification(app, "RudeSync personal loan", &body)?;
        mark_offsets_delivered(database, "loan_due", &notice.installment_id, &pending)?;
    }
    Ok(())
}

fn pending_offsets(
    database: &Database,
    kind: &str,
    entity_id: &str,
    due_date: NaiveDate,
    offsets: &[i64],
    now_local: NaiveDateTime,
) -> Result<Vec<i64>, AppError> {
    let connection = database.lock()?;
    let mut pending = Vec::new();
    for offset in offsets {
        if reminder_time(due_date, *offset) > now_local {
            continue;
        }
        let delivered = connection.query_row(
            "SELECT EXISTS (
                SELECT 1
                FROM notification_deliveries
                WHERE notification_kind = ?1
                  AND entity_id = ?2
                  AND reminder_offset_days = ?3
             )",
            params![kind, entity_id, offset],
            |row| row.get::<_, bool>(0),
        )?;
        if !delivered {
            pending.push(*offset);
        }
    }
    Ok(pending)
}

fn mark_offsets_delivered(
    database: &Database,
    kind: &str,
    entity_id: &str,
    offsets: &[i64],
) -> Result<(), AppError> {
    let now = utc_now();
    let mut connection = database.lock()?;
    let transaction = connection.transaction()?;
    for offset in offsets {
        transaction.execute(
            "INSERT OR IGNORE INTO notification_deliveries (
                id, notification_kind, entity_id, reminder_offset_days,
                delivered_at, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![Uuid::new_v4().to_string(), kind, entity_id, offset, now],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

fn show_notification(app: &AppHandle, title: &str, body: &str) -> Result<(), AppError> {
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|error| AppError::State(format!("Windows notification failed: {error}")))?;
    Ok(())
}

fn reminder_time(due_date: NaiveDate, offset_days: i64) -> NaiveDateTime {
    let reminder_date = due_date
        .checked_sub_signed(chrono::Duration::days(offset_days))
        .unwrap_or(NaiveDate::MIN);
    reminder_date.and_time(
        NaiveTime::from_hms_opt(DUE_REMINDER_HOUR, 0, 0).expect("the fixed reminder time is valid"),
    )
}

fn due_timing(due_date: NaiveDate, today: NaiveDate, selected_offset: i64) -> String {
    if due_date < today {
        return format!("was due {}", due_date.format("%B %-d, %Y"));
    }
    if due_date == today {
        return "is due today".into();
    }
    if selected_offset == 1 {
        return "is due tomorrow".into();
    }
    let days = (due_date - today).num_days();
    format!("is due in {days} days")
}

fn parse_stored_date(value: &str, label: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| AppError::State(format!("Stored {label} is invalid.")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(value: &str) -> NaiveDate {
        NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn due_reminders_wait_until_nine_local_time() {
        let due = date("2026-08-13");
        assert_eq!(
            reminder_time(due, 0),
            date("2026-08-13").and_hms_opt(9, 0, 0).unwrap()
        );
        assert_eq!(
            reminder_time(due, 7),
            date("2026-08-06").and_hms_opt(9, 0, 0).unwrap()
        );
    }

    #[test]
    fn catch_up_wording_uses_current_due_relationship() {
        assert_eq!(
            due_timing(date("2026-08-13"), date("2026-08-14"), 0),
            "was due August 13, 2026"
        );
        assert_eq!(
            due_timing(date("2026-08-13"), date("2026-08-13"), 0),
            "is due today"
        );
        assert_eq!(
            due_timing(date("2026-08-14"), date("2026-08-13"), 1),
            "is due tomorrow"
        );
    }
}
