use std::{
    fs,
    path::PathBuf,
    sync::{Mutex, MutexGuard},
    thread,
    time::Duration,
};

use chrono::{DateTime, Datelike, Days, Local, NaiveDate, SecondsFormat, Utc, Weekday};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, State,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use thiserror::Error;
use uuid::Uuid;

mod csv_export;
mod domain;
mod finance;
mod invoice_pdf;
mod notifications;
mod settings;

const INITIAL_MIGRATION: &str = include_str!("../migrations/0001_initial.sql");
const RELIABILITY_MIGRATION: &str = include_str!("../migrations/0002_reliability.sql");
const MILESTONES_MIGRATION: &str = include_str!("../migrations/0003_flexible_milestones.sql");

struct Database {
    connection: Mutex<Connection>,
    path: PathBuf,
}

impl Database {
    fn open(path: PathBuf) -> Result<Self, AppError> {
        let mut connection = Connection::open(&path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;

        apply_migrations(&mut connection)?;
        seed_defaults(&connection)?;

        Ok(Self {
            connection: Mutex::new(connection),
            path,
        })
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, AppError> {
        self.connection
            .lock()
            .map_err(|_| AppError::State("The local database lock was poisoned.".into()))
    }
}

#[derive(Debug, Error)]
enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("File system error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    State(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    default_currency: String,
    date_format: String,
    week_starts_on: i64,
    theme: String,
    accent_color: String,
    close_to_tray: bool,
    autostart_enabled: bool,
    autostart_registered: bool,
    backup_directory: Option<String>,
    backup_retention_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapPayload {
    app_name: &'static str,
    app_version: String,
    database_path: String,
    today: String,
    settings: AppSettings,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SubtaskSummary {
    id: String,
    title: String,
    completed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Task {
    id: String,
    title: String,
    notes: Option<String>,
    planned_date: Option<String>,
    due_date: Option<String>,
    reminder_at: Option<String>,
    priority: String,
    category: Option<String>,
    parent_task_id: Option<String>,
    project_id: Option<String>,
    recurrence: String,
    recurrence_rule: Option<String>,
    subtasks: Vec<SubtaskSummary>,
    status: String,
    is_completed: bool,
    completed_at: Option<String>,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateTaskInput {
    title: String,
    notes: Option<String>,
    planned_date: Option<String>,
    due_date: Option<String>,
    reminder_at: Option<String>,
    priority: Option<String>,
    category: Option<String>,
    parent_task_id: Option<String>,
    project_id: Option<String>,
    recurrence: Option<String>,
    recurrence_rule: Option<String>,
}

#[derive(Debug, Default)]
enum Patch<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}

impl<'de, T> Deserialize<'de> for Patch<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match Option::<T>::deserialize(deserializer)? {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct UpdateTaskInput {
    title: Option<String>,
    notes: Patch<String>,
    planned_date: Patch<String>,
    due_date: Patch<String>,
    reminder_at: Patch<String>,
    priority: Option<String>,
    category: Patch<String>,
    parent_task_id: Patch<String>,
    project_id: Patch<String>,
    recurrence: Option<String>,
    recurrence_rule: Patch<String>,
    status: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskListFilter {
    from_date: Option<String>,
    to_date: Option<String>,
    include_completed: Option<bool>,
    include_deleted: Option<bool>,
}

#[tauri::command]
fn bootstrap_app(
    app: AppHandle,
    database: State<'_, Database>,
) -> Result<BootstrapPayload, AppError> {
    let connection = database.lock()?;

    let mut settings = connection.query_row(
        "SELECT
            default_currency,
            date_format,
            week_starts_on,
            theme,
            accent_color,
            close_to_tray,
            autostart_enabled
         FROM app_settings
         WHERE id = 1",
        [],
        |row| {
            Ok(AppSettings {
                default_currency: row.get(0)?,
                date_format: row.get(1)?,
                week_starts_on: row.get(2)?,
                theme: row.get(3)?,
                accent_color: row.get(4)?,
                close_to_tray: row.get(5)?,
                autostart_enabled: row.get(6)?,
                autostart_registered: false,
                backup_directory: None,
                backup_retention_count: 30,
            })
        },
    )?;

    let backup: (Option<String>, i64) = connection.query_row(
        "SELECT directory_path, retention_count
         FROM backup_settings
         WHERE id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    settings.backup_directory = backup.0;
    settings.backup_retention_count = backup.1;
    settings.autostart_registered = app.autolaunch().is_enabled().unwrap_or(false);

    Ok(BootstrapPayload {
        app_name: "RudeSync",
        app_version: app.package_info().version.to_string(),
        database_path: database.path.to_string_lossy().into_owned(),
        today: Local::now().date_naive().format("%Y-%m-%d").to_string(),
        settings,
    })
}

#[tauri::command]
fn create_task(database: State<'_, Database>, input: CreateTaskInput) -> Result<Task, AppError> {
    let title = required_trimmed(input.title, "Task title", 240)?;
    let notes = optional_trimmed(input.notes, 20_000, "Task notes")?;
    let category = optional_trimmed(input.category, 80, "Task category")?;
    let parent_task_id = optional_trimmed(input.parent_task_id, 64, "Parent task ID")?;
    let project_id = optional_trimmed(input.project_id, 64, "Project ID")?;
    let recurrence_rule = optional_trimmed(
        input
            .recurrence_rule
            .or_else(|| recurrence_shorthand_to_rule(input.recurrence)),
        1_000,
        "Recurrence rule",
    )?;
    let priority = validate_priority(input.priority.as_deref().unwrap_or("none"))?;
    let planned_date = validate_optional_date(input.planned_date, "plannedDate")?;
    let due_date = validate_optional_date(input.due_date, "dueDate")?;
    let reminder_at = validate_optional_timestamp(input.reminder_at, "reminderAt")?;

    if let (Some(planned), Some(due)) = (&planned_date, &due_date) {
        if due < planned {
            return Err(AppError::InvalidInput(
                "dueDate cannot be earlier than plannedDate.".into(),
            ));
        }
    }

    let id = Uuid::new_v4().to_string();
    let now = utc_now();
    let mut connection = database.lock()?;
    let transaction = connection.transaction()?;

    ensure_active_parent(&transaction, parent_task_id.as_deref())?;
    ensure_active_project(&transaction, project_id.as_deref())?;

    transaction.execute(
        "INSERT INTO tasks (
            id,
            parent_task_id,
            project_id,
            title,
            notes,
            planned_date,
            due_date,
            priority,
            category,
            status,
            completed_at,
            created_at,
            updated_at,
            deleted_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'open', NULL, ?10, ?10, NULL)",
        params![
            id,
            parent_task_id,
            project_id,
            title,
            notes,
            planned_date,
            due_date,
            priority,
            category,
            now,
        ],
    )?;

    if let Some(remind_at) = reminder_at {
        transaction.execute(
            "INSERT INTO task_reminders (
                id,
                task_id,
                remind_at,
                delivered_at,
                snoozed_until,
                created_at,
                updated_at,
                deleted_at
            ) VALUES (?1, ?2, ?3, NULL, NULL, ?4, ?4, NULL)",
            params![Uuid::new_v4().to_string(), id, remind_at, now],
        )?;
    }

    if let Some(rule) = recurrence_rule {
        transaction.execute(
            "INSERT INTO task_recurrences (
                id,
                task_id,
                rule,
                frequency,
                interval_count,
                next_occurrence_date,
                ends_on,
                occurrence_limit,
                created_at,
                updated_at,
                deleted_at
            ) VALUES (?1, ?2, ?3, ?4, 1, NULL, NULL, NULL, ?5, ?5, NULL)",
            params![
                Uuid::new_v4().to_string(),
                id,
                rule,
                infer_recurrence_frequency(&rule),
                now,
            ],
        )?;
    }

    transaction.commit()?;
    load_task(&connection, &id)
}

#[tauri::command]
fn list_tasks(
    database: State<'_, Database>,
    filter: Option<TaskListFilter>,
) -> Result<Vec<Task>, AppError> {
    let filter = filter.unwrap_or_default();
    let from_date = validate_optional_date(filter.from_date, "fromDate")?;
    let to_date = validate_optional_date(filter.to_date, "toDate")?;

    if let (Some(from), Some(to)) = (&from_date, &to_date) {
        if to < from {
            return Err(AppError::InvalidInput(
                "toDate cannot be earlier than fromDate.".into(),
            ));
        }
    }

    let include_completed = filter.include_completed.unwrap_or(true);
    let include_deleted = filter.include_deleted.unwrap_or(false);
    let connection = database.lock()?;
    let mut statement = connection.prepare(&format!(
        "{TASK_SELECT}
         WHERE (
            ?1 IS NULL
            OR COALESCE(t.planned_date, t.due_date) >= ?1
         )
         AND (
            ?2 IS NULL
            OR COALESCE(t.planned_date, t.due_date) <= ?2
         )
         AND (?3 = 1 OR t.status <> 'completed')
         AND (?4 = 1 OR t.deleted_at IS NULL)
         AND t.parent_task_id IS NULL
         ORDER BY
            CASE t.status
                WHEN 'open' THEN 0
                WHEN 'completed' THEN 1
                ELSE 2
            END,
            COALESCE(t.planned_date, t.due_date, '9999-12-31') ASC,
            CASE t.priority
                WHEN 'urgent' THEN 0
                WHEN 'high' THEN 1
                WHEN 'medium' THEN 2
                WHEN 'low' THEN 3
                ELSE 4
            END,
            t.created_at DESC"
    ))?;

    let rows = statement.query_map(
        params![from_date, to_date, include_completed, include_deleted],
        task_from_row,
    )?;

    let mut tasks = rows.collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    for task in &mut tasks {
        task.subtasks = load_subtasks(&connection, &task.id)?;
    }
    Ok(tasks)
}

#[tauri::command]
fn toggle_task(database: State<'_, Database>, task_id: String) -> Result<Task, AppError> {
    let task_id = required_trimmed(task_id, "Task ID", 64)?;
    let mut connection = database.lock()?;
    let transaction = connection.transaction()?;
    let status: Option<String> = transaction
        .query_row(
            "SELECT status
             FROM tasks
             WHERE id = ?1 AND deleted_at IS NULL",
            params![task_id],
            |row| row.get(0),
        )
        .optional()?;
    let status = status.ok_or_else(|| AppError::NotFound("Task not found.".into()))?;
    let completed = status != "completed";

    update_task_completion(&transaction, &task_id, completed)?;
    transaction.commit()?;
    load_task(&connection, &task_id)
}

#[tauri::command]
fn set_task_completed(
    database: State<'_, Database>,
    task_id: String,
    completed: bool,
) -> Result<Task, AppError> {
    let task_id = required_trimmed(task_id, "Task ID", 64)?;
    let mut connection = database.lock()?;
    let transaction = connection.transaction()?;

    update_task_completion(&transaction, &task_id, completed)?;
    transaction.commit()?;
    load_task(&connection, &task_id)
}

#[tauri::command]
fn delete_task(database: State<'_, Database>, task_id: String) -> Result<(), AppError> {
    let task_id = required_trimmed(task_id, "Task ID", 64)?;
    let mut connection = database.lock()?;
    let transaction = connection.transaction()?;
    let now = utc_now();

    let affected = transaction.execute(
        "UPDATE tasks
             SET deleted_at = ?2, updated_at = ?2
         WHERE (id = ?1 OR parent_task_id = ?1) AND deleted_at IS NULL",
        params![task_id, now],
    )?;
    if affected == 0 {
        return Err(AppError::NotFound("Task not found.".into()));
    }
    transaction.execute(
        "UPDATE task_reminders
             SET deleted_at = ?2, updated_at = ?2
         WHERE task_id = ?1 AND deleted_at IS NULL",
        params![task_id, now],
    )?;
    transaction.execute(
        "UPDATE task_recurrences
             SET deleted_at = ?2, updated_at = ?2
         WHERE task_id = ?1 AND deleted_at IS NULL",
        params![task_id, now],
    )?;
    transaction.commit()?;
    Ok(())
}

#[tauri::command]
fn update_task(
    database: State<'_, Database>,
    id: String,
    input: UpdateTaskInput,
) -> Result<Task, AppError> {
    let task_id = required_trimmed(id, "Task ID", 64)?;
    let mut connection = database.lock()?;
    let existing = load_task(&connection, &task_id)?;

    let title = match input.title {
        Some(title) => required_trimmed(title, "Task title", 240)?,
        None => existing.title,
    };
    let notes = match input.notes {
        Patch::Value(notes) => optional_trimmed(Some(notes), 20_000, "Task notes")?,
        Patch::Null => None,
        Patch::Missing => existing.notes,
    };
    let category = match input.category {
        Patch::Value(category) => optional_trimmed(Some(category), 80, "Task category")?,
        Patch::Null => None,
        Patch::Missing => existing.category,
    };
    let parent_task_id = match input.parent_task_id {
        Patch::Value(parent_task_id) => {
            optional_trimmed(Some(parent_task_id), 64, "Parent task ID")?
        }
        Patch::Null => None,
        Patch::Missing => existing.parent_task_id,
    };
    let project_id = match input.project_id {
        Patch::Value(project_id) => optional_trimmed(Some(project_id), 64, "Project ID")?,
        Patch::Null => None,
        Patch::Missing => existing.project_id,
    };
    let planned_date = match input.planned_date {
        Patch::Value(planned_date) => validate_optional_date(Some(planned_date), "plannedDate")?,
        Patch::Null => None,
        Patch::Missing => existing.planned_date,
    };
    let due_date = match input.due_date {
        Patch::Value(due_date) => validate_optional_date(Some(due_date), "dueDate")?,
        Patch::Null => None,
        Patch::Missing => existing.due_date,
    };
    let priority = match input.priority {
        Some(priority) => validate_priority(&priority)?,
        None => existing.priority,
    };
    let status = match input.status.as_deref() {
        Some("open") => "open".to_owned(),
        Some("completed") => "completed".to_owned(),
        Some(_) => {
            return Err(AppError::InvalidInput(
                "status must be open or completed.".into(),
            ))
        }
        None => existing.status,
    };

    if let (Some(planned), Some(due)) = (&planned_date, &due_date) {
        if due < planned {
            return Err(AppError::InvalidInput(
                "dueDate cannot be earlier than plannedDate.".into(),
            ));
        }
    }
    if parent_task_id.as_deref() == Some(task_id.as_str()) {
        return Err(AppError::InvalidInput(
            "A task cannot be its own parent.".into(),
        ));
    }

    let (has_reminder_patch, reminder_patch) = match input.reminder_at {
        Patch::Missing => (false, None),
        Patch::Null => (true, None),
        Patch::Value(value) => (
            true,
            validate_optional_timestamp(Some(value), "reminderAt")?,
        ),
    };
    let recurrence_input = match input.recurrence_rule {
        Patch::Missing => match input.recurrence {
            Some(value) if value.trim().eq_ignore_ascii_case("none") => Patch::Null,
            Some(value) => Patch::Value(value),
            None => Patch::Missing,
        },
        patch => patch,
    };
    let (has_recurrence_patch, recurrence_patch) = match recurrence_input {
        Patch::Missing => (false, None),
        Patch::Null => (true, None),
        Patch::Value(value) => (
            true,
            optional_trimmed(Some(value), 1_000, "Recurrence rule")?,
        ),
    };

    let now = utc_now();
    let transaction = connection.transaction()?;
    ensure_active_parent(&transaction, parent_task_id.as_deref())?;
    ensure_active_project(&transaction, project_id.as_deref())?;

    let completed_at = if status == "completed" {
        existing.completed_at.or_else(|| Some(now.clone()))
    } else {
        None
    };
    transaction.execute(
        "UPDATE tasks
         SET parent_task_id = ?2,
             project_id = ?3,
             title = ?4,
             notes = ?5,
             planned_date = ?6,
             due_date = ?7,
             priority = ?8,
             category = ?9,
             status = ?10,
             completed_at = ?11,
             updated_at = ?12
         WHERE id = ?1 AND deleted_at IS NULL",
        params![
            task_id,
            parent_task_id,
            project_id,
            title,
            notes,
            planned_date,
            due_date,
            priority,
            category,
            status,
            completed_at,
            now,
        ],
    )?;

    if has_reminder_patch {
        transaction.execute(
            "UPDATE task_reminders
             SET deleted_at = ?2, updated_at = ?2
             WHERE task_id = ?1 AND deleted_at IS NULL",
            params![task_id, now],
        )?;
        if let Some(remind_at) = reminder_patch {
            transaction.execute(
                "INSERT INTO task_reminders (
                    id, task_id, remind_at, delivered_at, snoozed_until,
                    created_at, updated_at, deleted_at
                 ) VALUES (?1, ?2, ?3, NULL, NULL, ?4, ?4, NULL)",
                params![Uuid::new_v4().to_string(), task_id, remind_at, now],
            )?;
        }
    }

    if has_recurrence_patch {
        transaction.execute(
            "UPDATE task_recurrences
             SET deleted_at = ?2, updated_at = ?2
             WHERE task_id = ?1 AND deleted_at IS NULL",
            params![task_id, now],
        )?;
        if let Some(rule) = recurrence_patch {
            transaction.execute(
                "INSERT INTO task_recurrences (
                    id, task_id, rule, frequency, interval_count,
                    next_occurrence_date, ends_on, occurrence_limit,
                    created_at, updated_at, deleted_at
                 ) VALUES (?1, ?2, ?3, ?4, 1, NULL, NULL, NULL, ?5, ?5, NULL)",
                params![
                    Uuid::new_v4().to_string(),
                    task_id,
                    rule,
                    infer_recurrence_frequency(&rule),
                    now,
                ],
            )?;
        }
    }

    transaction.commit()?;
    load_task(&connection, &task_id)
}

const TASK_SELECT: &str = "
    SELECT
        t.id,
        t.title,
        t.notes,
        t.planned_date,
        t.due_date,
        (
            SELECT r.remind_at
            FROM task_reminders r
            WHERE r.task_id = t.id AND r.deleted_at IS NULL
            ORDER BY r.remind_at ASC
            LIMIT 1
        ) AS reminder_at,
        t.priority,
        t.category,
        t.parent_task_id,
        t.project_id,
        (
            SELECT recurrence.rule
            FROM task_recurrences recurrence
            WHERE recurrence.task_id = t.id
              AND recurrence.deleted_at IS NULL
            LIMIT 1
        ) AS recurrence_rule,
        t.status,
        t.completed_at,
        t.created_at,
        t.updated_at,
        t.deleted_at
    FROM tasks t
";

fn load_task(connection: &Connection, task_id: &str) -> Result<Task, AppError> {
    let mut task = connection
        .query_row(
            &format!("{TASK_SELECT} WHERE t.id = ?1"),
            params![task_id],
            task_from_row,
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("Task not found.".into()))?;
    task.subtasks = load_subtasks(connection, task_id)?;
    Ok(task)
}

fn task_from_row(row: &Row<'_>) -> rusqlite::Result<Task> {
    let status: String = row.get(11)?;
    let recurrence_rule: Option<String> = row.get(10)?;
    let recurrence = recurrence_label(recurrence_rule.as_deref()).to_owned();
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        notes: row.get(2)?,
        planned_date: row.get(3)?,
        due_date: row.get(4)?,
        reminder_at: row.get(5)?,
        priority: row.get(6)?,
        category: row.get(7)?,
        parent_task_id: row.get(8)?,
        project_id: row.get(9)?,
        recurrence,
        recurrence_rule,
        subtasks: Vec::new(),
        status: status.clone(),
        is_completed: status == "completed",
        completed_at: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
        deleted_at: row.get(15)?,
    })
}

fn load_subtasks(
    connection: &Connection,
    parent_task_id: &str,
) -> Result<Vec<SubtaskSummary>, AppError> {
    let mut statement = connection.prepare(
        "SELECT id, title, status
         FROM tasks
         WHERE parent_task_id = ?1 AND deleted_at IS NULL
         ORDER BY
            CASE status WHEN 'open' THEN 0 ELSE 1 END,
            created_at ASC",
    )?;
    let rows = statement.query_map(params![parent_task_id], |row| {
        let status: String = row.get(2)?;
        Ok(SubtaskSummary {
            id: row.get(0)?,
            title: row.get(1)?,
            completed: status == "completed",
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

fn update_task_completion(
    connection: &Connection,
    task_id: &str,
    completed: bool,
) -> Result<(), AppError> {
    let now = utc_now();
    let was_completed: Option<bool> = connection
        .query_row(
            "SELECT status = 'completed'
             FROM tasks
             WHERE id = ?1 AND deleted_at IS NULL",
            params![task_id],
            |row| row.get(0),
        )
        .optional()?;
    let affected = if completed {
        connection.execute(
            "UPDATE tasks
             SET status = 'completed', completed_at = ?2, updated_at = ?2
             WHERE id = ?1 AND deleted_at IS NULL",
            params![task_id, now],
        )?
    } else {
        connection.execute(
            "UPDATE tasks
             SET status = 'open', completed_at = NULL, updated_at = ?2
             WHERE id = ?1 AND deleted_at IS NULL",
            params![task_id, now],
        )?
    };

    if affected == 0 {
        return Err(AppError::NotFound("Task not found.".into()));
    }
    if completed && was_completed == Some(false) {
        create_next_recurrence(connection, task_id, &now)?;
    }
    Ok(())
}

#[derive(Debug)]
struct RecurringTaskSource {
    parent_task_id: Option<String>,
    project_id: Option<String>,
    title: String,
    notes: Option<String>,
    planned_date: Option<String>,
    due_date: Option<String>,
    priority: String,
    category: Option<String>,
}

fn create_next_recurrence(
    connection: &Connection,
    task_id: &str,
    now: &str,
) -> Result<(), AppError> {
    let recurrence: Option<(String, String, Option<String>)> = connection
        .query_row(
            "SELECT id, rule, next_occurrence_date
             FROM task_recurrences
             WHERE task_id = ?1 AND deleted_at IS NULL
             LIMIT 1",
            params![task_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((recurrence_id, rule, already_generated)) = recurrence else {
        return Ok(());
    };
    if already_generated.is_some() {
        return Ok(());
    }

    let source: RecurringTaskSource = connection.query_row(
        "SELECT parent_task_id, project_id, title, notes, planned_date,
                due_date, priority, category
         FROM tasks
         WHERE id = ?1 AND deleted_at IS NULL",
        params![task_id],
        |row| {
            Ok(RecurringTaskSource {
                parent_task_id: row.get(0)?,
                project_id: row.get(1)?,
                title: row.get(2)?,
                notes: row.get(3)?,
                planned_date: row.get(4)?,
                due_date: row.get(5)?,
                priority: row.get(6)?,
                category: row.get(7)?,
            })
        },
    )?;
    if source.parent_task_id.is_some() {
        return Ok(());
    }

    let base = source
        .planned_date
        .as_deref()
        .or(source.due_date.as_deref())
        .map(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| AppError::State("A recurring task contains an invalid date.".into()))?
        .unwrap_or_else(|| Local::now().date_naive());
    let Some(next_base) = next_recurrence_date(base, &rule) else {
        // Free-form custom rules remain stored and editable, but cannot be
        // advanced safely without a deterministic schedule.
        return Ok(());
    };
    let shift_days = (next_base - base).num_days();
    let shift = |value: Option<&str>| -> Result<Option<String>, AppError> {
        value
            .map(|date| {
                NaiveDate::parse_from_str(date, "%Y-%m-%d")
                    .map_err(|_| {
                        AppError::State("A recurring task contains an invalid date.".into())
                    })
                    .and_then(|parsed| {
                        parsed
                            .checked_add_signed(chrono::Duration::days(shift_days))
                            .ok_or_else(|| {
                                AppError::State("The next recurring date is out of range.".into())
                            })
                    })
                    .map(|date| date.format("%Y-%m-%d").to_string())
            })
            .transpose()
    };
    let mut next_planned = shift(source.planned_date.as_deref())?;
    let next_due = shift(source.due_date.as_deref())?;
    if source.planned_date.is_none() && source.due_date.is_none() {
        next_planned = Some(next_base.format("%Y-%m-%d").to_string());
    }

    let next_task_id = Uuid::new_v4().to_string();
    connection.execute(
        "INSERT INTO tasks (
            id, parent_task_id, project_id, title, notes, planned_date,
            due_date, priority, category, status, completed_at,
            created_at, updated_at, deleted_at
         ) VALUES (
            ?1, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'open', NULL,
            ?9, ?9, NULL
         )",
        params![
            next_task_id,
            source.project_id,
            source.title,
            source.notes,
            next_planned,
            next_due,
            source.priority,
            source.category,
            now
        ],
    )?;

    connection.execute(
        "INSERT INTO task_recurrences (
            id, task_id, rule, frequency, interval_count,
            next_occurrence_date, ends_on, occurrence_limit,
            created_at, updated_at, deleted_at
         ) VALUES (?1, ?2, ?3, ?4, 1, NULL, NULL, NULL, ?5, ?5, NULL)",
        params![
            Uuid::new_v4().to_string(),
            next_task_id,
            rule,
            infer_recurrence_frequency(&rule),
            now
        ],
    )?;
    connection.execute(
        "UPDATE task_recurrences
         SET next_occurrence_date = ?2, updated_at = ?3
         WHERE id = ?1",
        params![recurrence_id, next_base.format("%Y-%m-%d").to_string(), now],
    )?;

    let reminder: Option<String> = connection
        .query_row(
            "SELECT remind_at
             FROM task_reminders
             WHERE task_id = ?1 AND deleted_at IS NULL
             ORDER BY remind_at
             LIMIT 1",
            params![task_id],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(reminder) = reminder {
        if let Ok(parsed) = DateTime::parse_from_rfc3339(&reminder) {
            if let Some(next_reminder) =
                parsed.checked_add_signed(chrono::Duration::days(shift_days))
            {
                connection.execute(
                    "INSERT INTO task_reminders (
                        id, task_id, remind_at, delivered_at, snoozed_until,
                        created_at, updated_at, deleted_at
                     ) VALUES (?1, ?2, ?3, NULL, NULL, ?4, ?4, NULL)",
                    params![
                        Uuid::new_v4().to_string(),
                        next_task_id,
                        next_reminder
                            .with_timezone(&Utc)
                            .to_rfc3339_opts(SecondsFormat::Millis, true),
                        now
                    ],
                )?;
            }
        }
    }

    let mut subtasks = connection.prepare(
        "SELECT title
         FROM tasks
         WHERE parent_task_id = ?1 AND deleted_at IS NULL
         ORDER BY created_at",
    )?;
    let titles = subtasks
        .query_map(params![task_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(subtasks);
    for title in titles {
        connection.execute(
            "INSERT INTO tasks (
                id, parent_task_id, project_id, title, notes, planned_date,
                due_date, priority, category, status, completed_at,
                created_at, updated_at, deleted_at
             ) VALUES (?1, ?2, ?3, ?4, NULL, NULL, NULL, 'none', NULL,
                       'open', NULL, ?5, ?5, NULL)",
            params![
                Uuid::new_v4().to_string(),
                next_task_id,
                source.project_id,
                title,
                now
            ],
        )?;
    }
    Ok(())
}

fn next_recurrence_date(base: NaiveDate, rule: &str) -> Option<NaiveDate> {
    let normalized = rule.trim().to_ascii_lowercase();
    let interval = recurrence_interval(&normalized);
    if normalized == "daily" || normalized.contains("freq=daily") {
        return base.checked_add_days(Days::new(interval));
    }
    if normalized == "weekdays" {
        let mut next = base.checked_add_days(Days::new(1))?;
        while matches!(next.weekday(), Weekday::Sat | Weekday::Sun) {
            next = next.checked_add_days(Days::new(1))?;
        }
        return Some(next);
    }
    if normalized == "weekly" || normalized.contains("freq=weekly") {
        let selected_days = recurrence_weekdays(&normalized);
        if !selected_days.is_empty() {
            let base_index = u64::from(base.weekday().num_days_from_monday());
            let search_days = interval.checked_mul(7)?.checked_add(7)?;
            for delta in 1..=search_days {
                let week_offset = base_index.checked_add(delta)?.checked_div(7)?;
                if week_offset != 0 && week_offset % interval != 0 {
                    continue;
                }
                let candidate = base.checked_add_days(Days::new(delta))?;
                if selected_days.contains(&candidate.weekday()) {
                    return Some(candidate);
                }
            }
            return None;
        }
        return base.checked_add_days(Days::new(interval.checked_mul(7)?));
    }
    if normalized == "monthly" || normalized.contains("freq=monthly") {
        let interval = i32::try_from(interval).ok()?;
        let total_month = base
            .year()
            .checked_mul(12)?
            .checked_add(i32::try_from(base.month0()).ok()?)?
            .checked_add(interval)?;
        let year = total_month.div_euclid(12);
        let month = u32::try_from(total_month.rem_euclid(12)).ok()? + 1;
        let first = NaiveDate::from_ymd_opt(year, month, 1)?;
        let following = if month == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1)?
        } else {
            NaiveDate::from_ymd_opt(year, month + 1, 1)?
        };
        let last_day = following.pred_opt()?.day();
        return NaiveDate::from_ymd_opt(year, month, base.day().min(last_day)).or(Some(first));
    }
    None
}

fn recurrence_weekdays(rule: &str) -> Vec<Weekday> {
    let Some(value) = rule.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        key.eq_ignore_ascii_case("BYDAY").then_some(value)
    }) else {
        return Vec::new();
    };
    value
        .split(',')
        .filter_map(|day| match day.trim().to_ascii_uppercase().as_str() {
            "MO" => Some(Weekday::Mon),
            "TU" => Some(Weekday::Tue),
            "WE" => Some(Weekday::Wed),
            "TH" => Some(Weekday::Thu),
            "FR" => Some(Weekday::Fri),
            "SA" => Some(Weekday::Sat),
            "SU" => Some(Weekday::Sun),
            _ => None,
        })
        .collect()
}

fn recurrence_interval(rule: &str) -> u64 {
    rule.split(';')
        .find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            key.eq_ignore_ascii_case("INTERVAL")
                .then(|| value.parse::<u64>().ok())
                .flatten()
        })
        .filter(|value| (1..=365).contains(value))
        .unwrap_or(1)
}

fn ensure_active_parent(connection: &Connection, task_id: Option<&str>) -> Result<(), AppError> {
    let Some(task_id) = task_id else {
        return Ok(());
    };
    let exists: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM tasks WHERE id = ?1 AND deleted_at IS NULL
        )",
        params![task_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(AppError::InvalidInput(
            "parentTaskId does not identify an active task.".into(),
        ));
    }
    Ok(())
}

fn ensure_active_project(
    connection: &Connection,
    project_id: Option<&str>,
) -> Result<(), AppError> {
    let Some(project_id) = project_id else {
        return Ok(());
    };
    let exists: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM projects WHERE id = ?1 AND deleted_at IS NULL
        )",
        params![project_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(AppError::InvalidInput(
            "projectId does not identify an active project.".into(),
        ));
    }
    Ok(())
}

fn required_trimmed(value: String, field: &str, max_chars: usize) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput(format!("{field} is required.")));
    }
    if trimmed.chars().count() > max_chars {
        return Err(AppError::InvalidInput(format!(
            "{field} must be no longer than {max_chars} characters."
        )));
    }
    Ok(trimmed.to_owned())
}

fn optional_trimmed(
    value: Option<String>,
    max_chars: usize,
    field: &str,
) -> Result<Option<String>, AppError> {
    value
        .map(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            if trimmed.chars().count() > max_chars {
                return Err(AppError::InvalidInput(format!(
                    "{field} must be no longer than {max_chars} characters."
                )));
            }
            Ok(Some(trimmed.to_owned()))
        })
        .transpose()
        .map(Option::flatten)
}

fn validate_priority(priority: &str) -> Result<String, AppError> {
    match priority.trim().to_ascii_lowercase().as_str() {
        "none" => Ok("none".into()),
        "low" => Ok("low".into()),
        "medium" => Ok("medium".into()),
        "high" => Ok("high".into()),
        "urgent" => Ok("urgent".into()),
        _ => Err(AppError::InvalidInput(
            "priority must be none, low, medium, high, or urgent.".into(),
        )),
    }
}

fn validate_optional_date(value: Option<String>, field: &str) -> Result<Option<String>, AppError> {
    value
        .map(|value| {
            let value = value.trim();
            NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .map_err(|_| AppError::InvalidInput(format!("{field} must use YYYY-MM-DD.")))?;
            Ok(value.to_owned())
        })
        .transpose()
}

fn validate_optional_timestamp(
    value: Option<String>,
    field: &str,
) -> Result<Option<String>, AppError> {
    value
        .map(|value| {
            let parsed = DateTime::parse_from_rfc3339(value.trim()).map_err(|_| {
                AppError::InvalidInput(format!("{field} must be an RFC 3339 timestamp."))
            })?;
            Ok(parsed
                .with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Millis, true))
        })
        .transpose()
}

fn infer_recurrence_frequency(rule: &str) -> Option<&'static str> {
    let rule = rule.to_ascii_uppercase();
    if rule.contains("FREQ=DAILY") {
        Some("daily")
    } else if rule.contains("FREQ=WEEKLY") {
        Some("weekly")
    } else if rule.contains("FREQ=MONTHLY") {
        Some("monthly")
    } else if rule.contains("FREQ=YEARLY") {
        Some("yearly")
    } else {
        Some("custom")
    }
}

fn recurrence_shorthand_to_rule(recurrence: Option<String>) -> Option<String> {
    recurrence.filter(|value| !value.trim().eq_ignore_ascii_case("none"))
}

fn recurrence_label(rule: Option<&str>) -> &'static str {
    let Some(rule) = rule else {
        return "none";
    };
    if rule.to_ascii_uppercase().contains("INTERVAL=") && recurrence_interval(rule) > 1 {
        return "custom";
    }
    match rule.trim().to_ascii_lowercase().as_str() {
        "daily" => "daily",
        "weekdays" => "weekdays",
        "weekly" => "weekly",
        "monthly" => "monthly",
        "custom" => "custom",
        _ => infer_recurrence_frequency(rule).unwrap_or("custom"),
    }
}

/// Every migration this build knows how to apply, in order.
const MIGRATIONS: [(i64, &str); 3] = [
    (1_i64, INITIAL_MIGRATION),
    (2_i64, RELIABILITY_MIGRATION),
    (3_i64, MILESTONES_MIGRATION),
];

/// Highest `schema_migrations.version` this build can produce or read.
/// Derived from [`MIGRATIONS`] so adding a migration cannot leave a stale
/// hardcoded ceiling behind (see backup restore validation in `settings.rs`).
pub(crate) const fn latest_schema_version() -> i64 {
    let mut latest = 0_i64;
    let mut index = 0;
    while index < MIGRATIONS.len() {
        let (version, _) = MIGRATIONS[index];
        if version > latest {
            latest = version;
        }
        index += 1;
    }
    latest
}

fn apply_migrations(connection: &mut Connection) -> Result<(), AppError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
    )?;

    let current_version: i64 = connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;

    for (version, sql) in MIGRATIONS {
        if version <= current_version {
            continue;
        }
        let transaction = connection.transaction()?;
        if version == 2 {
            ensure_schema_compatibility(&transaction)?;
        }
        transaction.execute_batch(sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations(version, applied_at) VALUES (?1, ?2)",
            params![version, utc_now()],
        )?;
        transaction.commit()?;
    }
    ensure_schema_compatibility(connection)?;
    Ok(())
}

fn ensure_schema_compatibility(connection: &Connection) -> Result<(), AppError> {
    let additions = [
        (
            "app_settings",
            "default_invoice_term_days",
            "ALTER TABLE app_settings ADD COLUMN default_invoice_term_days
             INTEGER NOT NULL DEFAULT 14
             CHECK (default_invoice_term_days IN (0, 7, 14, 30))",
        ),
        (
            "app_settings",
            "notifications_enabled",
            "ALTER TABLE app_settings ADD COLUMN notifications_enabled
             INTEGER NOT NULL DEFAULT 1
             CHECK (notifications_enabled IN (0, 1))",
        ),
        (
            "projects",
            "kickoff_label",
            "ALTER TABLE projects ADD COLUMN kickoff_label
             TEXT NOT NULL DEFAULT 'Kickoff'",
        ),
        (
            "projects",
            "completion_label",
            "ALTER TABLE projects ADD COLUMN completion_label
             TEXT NOT NULL DEFAULT 'Completion'",
        ),
        (
            "projects",
            "urls_json",
            "ALTER TABLE projects ADD COLUMN urls_json
             TEXT NOT NULL DEFAULT '[]'",
        ),
        (
            "invoices",
            "milestone_label",
            "ALTER TABLE invoices ADD COLUMN milestone_label TEXT",
        ),
        (
            "backup_settings",
            "last_attempt_at",
            "ALTER TABLE backup_settings ADD COLUMN last_attempt_at TEXT",
        ),
        (
            "backup_settings",
            "last_attempt_local_date",
            "ALTER TABLE backup_settings ADD COLUMN last_attempt_local_date TEXT",
        ),
        (
            "backup_settings",
            "last_success_local_date",
            "ALTER TABLE backup_settings ADD COLUMN last_success_local_date TEXT",
        ),
        (
            "backup_settings",
            "last_error",
            "ALTER TABLE backup_settings ADD COLUMN last_error TEXT",
        ),
        (
            "backup_runs",
            "backup_kind",
            "ALTER TABLE backup_runs ADD COLUMN backup_kind
             TEXT NOT NULL DEFAULT 'automatic'
             CHECK (backup_kind IN ('automatic', 'manual', 'pre_restore'))",
        ),
        (
            "personal_loans",
            "first_monthly_day",
            "ALTER TABLE personal_loans ADD COLUMN first_monthly_day INTEGER
             CHECK (first_monthly_day IS NULL OR first_monthly_day BETWEEN 1 AND 31)",
        ),
        (
            "invoices",
            "project_name",
            "ALTER TABLE invoices ADD COLUMN project_name TEXT",
        ),
    ];
    for (table, column, sql) in additions {
        let pragma = format!("PRAGMA table_info({table})");
        let mut statement = connection.prepare(&pragma)?;
        let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
        let mut exists = false;
        for stored_column in columns {
            if stored_column? == column {
                exists = true;
                break;
            }
        }
        drop(statement);
        if !exists {
            connection.execute(sql, [])?;
        }
    }
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS notification_deliveries (
            id TEXT PRIMARY KEY,
            notification_kind TEXT NOT NULL
                CHECK (notification_kind IN ('invoice_due', 'loan_due')),
            entity_id TEXT NOT NULL,
            reminder_offset_days INTEGER NOT NULL,
            delivered_at TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE (notification_kind, entity_id, reminder_offset_days)
        );",
    )?;
    Ok(())
}

fn seed_defaults(connection: &Connection) -> Result<(), AppError> {
    let now = utc_now();
    connection.execute(
        "INSERT OR IGNORE INTO app_settings (
            id,
            default_currency,
            date_format,
            week_starts_on,
            theme,
            accent_color,
            close_to_tray,
            autostart_enabled,
            loan_reminder_offsets_minutes,
            invoice_reminder_offsets_minutes,
            created_at,
            updated_at
        ) VALUES (
            1,
            'USD',
            'MMMM d, yyyy',
            1,
            'dark',
            '#22C55E',
            1,
            1,
            '[10080,1440,0]',
            '[4320,0]',
            ?1,
            ?1
        )",
        params![now],
    )?;
    connection.execute(
        "INSERT OR IGNORE INTO backup_settings (
            id,
            directory_path,
            is_enabled,
            interval_hours,
            retention_count,
            last_success_at,
            created_at,
            updated_at
        ) VALUES (1, NULL, 1, 24, 30, NULL, ?1, ?1)",
        params![now],
    )?;
    Ok(())
}

fn utc_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn reveal_task_widget(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("task-widget") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn show_task_widget(app: AppHandle) {
    reveal_task_widget(&app);
}

#[tauri::command]
fn focus_main_window(app: AppHandle) {
    show_main_window(&app);
}

fn preference_enabled(app: &AppHandle, column: &str, fallback: bool) -> bool {
    // `column` is never supplied by IPC; only these two static call sites use
    // it. Keeping the small allowlist here prevents accidental SQL injection if
    // this helper is reused later.
    if !matches!(column, "close_to_tray" | "autostart_enabled") {
        return fallback;
    }

    let database = app.state::<Database>();
    let Ok(connection) = database.lock() else {
        return fallback;
    };
    connection
        .query_row(
            &format!("SELECT {column} FROM app_settings WHERE id = 1"),
            [],
            |row| row.get(0),
        )
        .unwrap_or(fallback)
}

fn tray_icon() -> Image<'static> {
    const SIZE: u32 = 32;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);

    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as i32 - 15;
            let dy = y as i32 - 15;
            let inside_circle = dx * dx + dy * dy <= 15 * 15;
            let vertical = (8..=11).contains(&x) && (7..=24).contains(&y);
            let upper_bar = (10..=21).contains(&x) && (7..=10).contains(&y);
            let middle_bar = (10..=20).contains(&x) && (14..=17).contains(&y);
            let bowl_edge = (19..=22).contains(&x) && (9..=15).contains(&y);
            let diagonal = (16..=24).contains(&y) && (x as i32 - (y as i32 - 7)).abs() <= 2;
            let is_mark = vertical || upper_bar || middle_bar || bowl_edge || diagonal;

            let pixel = if is_mark && inside_circle {
                [34, 197, 94, 255]
            } else if inside_circle {
                [8, 17, 13, 255]
            } else {
                [0, 0, 0, 0]
            };
            rgba.extend_from_slice(&pixel);
        }
    }

    Image::new_owned(rgba, SIZE, SIZE)
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let open_item = MenuItem::with_id(app, "open", "Open RudeSync", true, None::<&str>)?;
    let widget_item =
        MenuItem::with_id(app, "widget", "Show task widget", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_item, &widget_item, &quit_item])?;

    TrayIconBuilder::with_id("rudesync")
        .icon(tray_icon())
        .tooltip("RudeSync")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "widget" => reveal_task_widget(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn start_notification_worker(app: AppHandle) {
    thread::spawn(move || loop {
        let _ = notifications::dispatch_due_notifications(&app);
        let database = app.state::<Database>();
        let _ = settings::run_backup_if_due(&database);
        thread::sleep(Duration::from_secs(60));
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Single-instance must be the first plugin so a second launch can focus
        // the existing window before any other plugin performs startup work.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_denylist(&["main"])
                .build(),
        )
        .setup(|app| {
            let data_directory = app.path().app_data_dir()?;
            fs::create_dir_all(&data_directory)?;
            let database_path = data_directory.join("rudesync.sqlite3");
            let database = Database::open(database_path)?;
            app.manage(database);
            start_notification_worker(app.handle().clone());

            // The agreed default is to start with the computer. Do not register
            // a temporary debug binary. In packaged builds registration is
            // best-effort because OS policy may deny it; bootstrap_app reports
            // the actual registration state to the UI.
            #[cfg(not(debug_assertions))]
            {
                let wants_autostart = preference_enabled(app.handle(), "autostart_enabled", true);
                let autostart = app.autolaunch();
                let is_registered = autostart.is_enabled().unwrap_or(false);
                match (wants_autostart, is_registered) {
                    (true, false) => {
                        let _ = autostart.enable();
                    }
                    (false, true) => {
                        let _ = autostart.disable();
                    }
                    _ => {}
                }
            }

            setup_tray(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    if preference_enabled(window.app_handle(), "close_to_tray", true) {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap_app,
            create_task,
            list_tasks,
            toggle_task,
            set_task_completed,
            update_task,
            delete_task,
            show_task_widget,
            focus_main_window,
            csv_export::export_csv,
            domain::list_clients,
            domain::create_client,
            domain::update_client,
            domain::list_projects,
            domain::create_project,
            domain::update_project,
            domain::list_work_entries,
            domain::create_work_entry,
            domain::update_work_entry,
            finance::list_invoices,
            finance::create_invoice,
            finance::update_draft_invoice,
            finance::issue_draft_invoice,
            finance::void_invoice,
            finance::record_invoice_payment,
            finance::list_personal_loans,
            finance::create_personal_loan,
            finance::set_loan_installment_paid,
            finance::update_loan_installment_due_date,
            invoice_pdf::export_invoice_pdf,
            settings::get_settings,
            settings::update_settings,
            settings::get_invoice_profile,
            settings::update_invoice_profile,
            settings::run_backup,
            settings::run_manual_backup,
            settings::restore_backup
        ])
        .run(tauri::generate_context!())
        .expect("RudeSync failed to start");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_priority_is_normalized() {
        assert_eq!(validate_priority(" HIGH ").unwrap(), "high");
        assert!(validate_priority("critical").is_err());
    }

    #[test]
    fn date_validation_is_strict() {
        assert!(validate_optional_date(Some("2026-07-23".into()), "date").is_ok());
        assert!(validate_optional_date(Some("07/23/2026".into()), "date").is_err());
    }

    #[test]
    fn recurrence_frequency_is_inferred_from_rrule() {
        assert_eq!(
            infer_recurrence_frequency("FREQ=MONTHLY;INTERVAL=1"),
            Some("monthly")
        );
        assert_eq!(infer_recurrence_frequency("dates: custom"), Some("custom"));
    }

    #[test]
    fn recurring_dates_handle_weekends_and_month_end() {
        let friday = NaiveDate::from_ymd_opt(2026, 7, 24).unwrap();
        assert_eq!(
            next_recurrence_date(friday, "weekdays"),
            NaiveDate::from_ymd_opt(2026, 7, 27)
        );
        let month_end = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        assert_eq!(
            next_recurrence_date(month_end, "monthly"),
            NaiveDate::from_ymd_opt(2026, 2, 28)
        );
        assert_eq!(
            next_recurrence_date(month_end, "FREQ=MONTHLY;INTERVAL=2"),
            NaiveDate::from_ymd_opt(2026, 3, 31)
        );
        assert_eq!(recurrence_label(Some("FREQ=WEEKLY;INTERVAL=2")), "custom");
    }

    #[test]
    fn weekly_recurrence_advances_through_selected_weekdays() {
        let monday = NaiveDate::from_ymd_opt(2026, 7, 20).unwrap();
        let wednesday = NaiveDate::from_ymd_opt(2026, 7, 22).unwrap();
        assert_eq!(
            next_recurrence_date(monday, "FREQ=WEEKLY;INTERVAL=1;BYDAY=MO,WE"),
            Some(wednesday)
        );
        assert_eq!(
            next_recurrence_date(wednesday, "FREQ=WEEKLY;INTERVAL=1;BYDAY=MO,WE"),
            Some(NaiveDate::from_ymd_opt(2026, 7, 27).unwrap())
        );
        assert_eq!(
            next_recurrence_date(wednesday, "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO"),
            Some(NaiveDate::from_ymd_opt(2026, 8, 3).unwrap())
        );
    }

    #[test]
    fn task_patch_distinguishes_missing_from_explicit_null() {
        let input: UpdateTaskInput =
            serde_json::from_str(r#"{"notes":null,"recurrence":"weekly"}"#).unwrap();

        assert!(matches!(input.notes, Patch::Null));
        assert!(matches!(input.due_date, Patch::Missing));
        assert_eq!(
            recurrence_shorthand_to_rule(input.recurrence),
            Some("weekly".into())
        );
    }

    #[test]
    fn initial_migration_and_defaults_apply_to_empty_database() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        apply_migrations(&mut connection).unwrap();
        seed_defaults(&connection).unwrap();

        let migration_version: i64 = connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        let currency: String = connection
            .query_row(
                "SELECT default_currency FROM app_settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(migration_version, 3);
        assert_eq!(currency, "USD");
    }

    #[test]
    fn reliability_migration_tolerates_compatibility_columns_added_early() {
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
        connection
            .execute(
                "INSERT INTO schema_migrations(version, applied_at)
                 VALUES (1, ?1)",
                params![utc_now()],
            )
            .unwrap();
        connection
            .execute("ALTER TABLE invoices ADD COLUMN project_name TEXT", [])
            .unwrap();
        connection
            .execute("ALTER TABLE backup_settings ADD COLUMN last_error TEXT", [])
            .unwrap();

        apply_migrations(&mut connection).unwrap();

        let version: i64 = connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        let backup_kind_exists: bool = connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM pragma_table_info('backup_runs')
                    WHERE name = 'backup_kind'
                 )",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 3);
        assert!(backup_kind_exists);
    }

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
}
