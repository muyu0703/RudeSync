use chrono::{DateTime, Datelike, Duration as ChronoDuration, Local, Utc, Weekday};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_notification::NotificationExt;
use uuid::Uuid;

use super::{utc_now, AppError, Database};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReminderItem {
    id: String,
    kind: String,
    title: String,
    scheduled_at: Option<String>,
    duration_minutes: Option<i64>,
    remaining_minutes: Option<i64>,
    status: String,
    recurrence_rule: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateAlarmInput {
    title: String,
    scheduled_at: String,
    recurrence_rule: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateTimerInput {
    title: String,
    minutes: i64,
}

pub(crate) fn ensure_schema(database: &Database) -> Result<(), AppError> {
    let connection = database.lock()?;
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS reminder_center (
            id TEXT PRIMARY KEY NOT NULL,
            kind TEXT NOT NULL CHECK(kind IN ('alarm','timer')),
            title TEXT NOT NULL,
            scheduled_at TEXT,
            duration_minutes INTEGER,
            remaining_minutes INTEGER,
            status TEXT NOT NULL CHECK(status IN ('active','paused','ringing')),
            recurrence_rule TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            deleted_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_reminder_center_due
            ON reminder_center(status, scheduled_at, deleted_at);"
    )?;
    Ok(())
}

fn parse_utc(value: &str) -> Result<DateTime<Utc>, AppError> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| AppError::InvalidInput("提醒时间格式无效。".into()))
}

fn item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReminderItem> {
    Ok(ReminderItem {
        id: row.get(0)?,
        kind: row.get(1)?,
        title: row.get(2)?,
        scheduled_at: row.get(3)?,
        duration_minutes: row.get(4)?,
        remaining_minutes: row.get(5)?,
        status: row.get(6)?,
        recurrence_rule: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn load_item(database: &Database, id: &str) -> Result<ReminderItem, AppError> {
    let connection = database.lock()?;
    connection
        .query_row(
            "SELECT id, kind, title, scheduled_at, duration_minutes,
                    remaining_minutes, status, recurrence_rule, created_at, updated_at
             FROM reminder_center
             WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
            item_from_row,
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("提醒不存在。".into()))
}

#[tauri::command]
pub(crate) fn list_reminder_center(database: State<'_, Database>) -> Result<Vec<ReminderItem>, AppError> {
    let connection = database.lock()?;
    let mut statement = connection.prepare(
        "SELECT id, kind, title, scheduled_at, duration_minutes,
                remaining_minutes, status, recurrence_rule, created_at, updated_at
         FROM reminder_center
         WHERE deleted_at IS NULL
         ORDER BY
           CASE status WHEN 'ringing' THEN 0 WHEN 'active' THEN 1 ELSE 2 END,
           COALESCE(scheduled_at, '9999-12-31T23:59:59Z') ASC,
           created_at DESC"
    )?;
    let rows = statement.query_map([], item_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

#[tauri::command]
pub(crate) fn create_alarm(
    database: State<'_, Database>,
    input: CreateAlarmInput,
) -> Result<ReminderItem, AppError> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(AppError::InvalidInput("请填写提醒事项。".into()));
    }
    let scheduled = parse_utc(&input.scheduled_at)?;
    if scheduled <= Utc::now() {
        return Err(AppError::InvalidInput("闹钟时间必须晚于当前时间。".into()));
    }
    let rule = input.recurrence_rule
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty() && v != "none");
    let id = Uuid::new_v4().to_string();
    let now = utc_now();
    let connection = database.lock()?;
    connection.execute(
        "INSERT INTO reminder_center (
            id, kind, title, scheduled_at, duration_minutes, remaining_minutes,
            status, recurrence_rule, created_at, updated_at, deleted_at
         ) VALUES (?1, 'alarm', ?2, ?3, NULL, NULL, 'active', ?4, ?5, ?5, NULL)",
        params![id, title, scheduled.to_rfc3339(), rule, now],
    )?;
    drop(connection);
    load_item(&database, &id)
}

#[tauri::command]
pub(crate) fn create_timer(
    database: State<'_, Database>,
    input: CreateTimerInput,
) -> Result<ReminderItem, AppError> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(AppError::InvalidInput("请填写提醒事项。".into()));
    }
    if input.minutes < 1 || input.minutes > 525_600 {
        return Err(AppError::InvalidInput("倒计时必须在 1 分钟到 365 天之间。".into()));
    }
    let end = Utc::now() + ChronoDuration::minutes(input.minutes);
    let id = Uuid::new_v4().to_string();
    let now = utc_now();
    let connection = database.lock()?;
    connection.execute(
        "INSERT INTO reminder_center (
            id, kind, title, scheduled_at, duration_minutes, remaining_minutes,
            status, recurrence_rule, created_at, updated_at, deleted_at
         ) VALUES (?1, 'timer', ?2, ?3, ?4, ?4, 'active', NULL, ?5, ?5, NULL)",
        params![id, title, end.to_rfc3339(), input.minutes, now],
    )?;
    drop(connection);
    load_item(&database, &id)
}

fn remaining_minutes(item: &ReminderItem) -> i64 {
    if item.status == "paused" {
        return item.remaining_minutes.unwrap_or(item.duration_minutes.unwrap_or(0)).max(0);
    }
    let Some(at) = item.scheduled_at.as_deref() else { return 0; };
    let Ok(end) = parse_utc(at) else { return 0; };
    let seconds = (end - Utc::now()).num_seconds().max(0);
    (seconds + 59) / 60
}

#[tauri::command]
pub(crate) fn pause_timer(database: State<'_, Database>, id: String) -> Result<ReminderItem, AppError> {
    let item = load_item(&database, &id)?;
    if item.kind != "timer" || item.status != "active" {
        return Ok(item);
    }
    let remaining = remaining_minutes(&item).max(1);
    let now = utc_now();
    let connection = database.lock()?;
    connection.execute(
        "UPDATE reminder_center
         SET status='paused', remaining_minutes=?2, scheduled_at=NULL, updated_at=?3
         WHERE id=?1 AND deleted_at IS NULL",
        params![id, remaining, now],
    )?;
    drop(connection);
    load_item(&database, &id)
}

#[tauri::command]
pub(crate) fn resume_timer(database: State<'_, Database>, id: String) -> Result<ReminderItem, AppError> {
    let item = load_item(&database, &id)?;
    if item.kind != "timer" || item.status != "paused" {
        return Ok(item);
    }
    let remaining = item.remaining_minutes.unwrap_or(1).max(1);
    let end = Utc::now() + ChronoDuration::minutes(remaining);
    let now = utc_now();
    let connection = database.lock()?;
    connection.execute(
        "UPDATE reminder_center
         SET status='active', scheduled_at=?2, updated_at=?3
         WHERE id=?1 AND deleted_at IS NULL",
        params![id, end.to_rfc3339(), now],
    )?;
    drop(connection);
    load_item(&database, &id)
}

#[tauri::command]
pub(crate) fn reset_timer(database: State<'_, Database>, id: String) -> Result<ReminderItem, AppError> {
    let item = load_item(&database, &id)?;
    if item.kind != "timer" {
        return Ok(item);
    }
    let duration = item.duration_minutes.unwrap_or(1).max(1);
    let end = Utc::now() + ChronoDuration::minutes(duration);
    let now = utc_now();
    let connection = database.lock()?;
    connection.execute(
        "UPDATE reminder_center
         SET status='active', scheduled_at=?2, remaining_minutes=?3, updated_at=?4
         WHERE id=?1 AND deleted_at IS NULL",
        params![id, end.to_rfc3339(), duration, now],
    )?;
    drop(connection);
    load_item(&database, &id)
}

fn next_alarm_time(current: DateTime<Utc>, rule: &str) -> Option<DateTime<Utc>> {
    let local = current.with_timezone(&Local);
    if rule.eq_ignore_ascii_case("daily") {
        return Some((local + ChronoDuration::days(1)).with_timezone(&Utc));
    }
    if rule.eq_ignore_ascii_case("weekdays") {
        let mut next = local + ChronoDuration::days(1);
        while matches!(next.weekday(), Weekday::Sat | Weekday::Sun) {
            next += ChronoDuration::days(1);
        }
        return Some(next.with_timezone(&Utc));
    }
    if let Some(cap) = rule.strip_prefix("FREQ=DAILY;INTERVAL=") {
        if let Ok(days) = cap.parse::<i64>() {
            return Some((local + ChronoDuration::days(days.max(1))).with_timezone(&Utc));
        }
    }
    None
}

#[tauri::command]
pub(crate) fn acknowledge_reminder(
    database: State<'_, Database>,
    id: String,
) -> Result<(), AppError> {
    let item = load_item(&database, &id)?;
    let now = utc_now();
    let connection = database.lock()?;
    if item.kind == "alarm" {
        let next = item
            .recurrence_rule
            .as_deref()
            .and_then(|rule| item.scheduled_at.as_deref().and_then(|at| parse_utc(at).ok().and_then(|dt| next_alarm_time(dt, rule))));
        if let Some(next) = next {
            connection.execute(
                "UPDATE reminder_center
                 SET status='active', scheduled_at=?2, updated_at=?3
                 WHERE id=?1 AND deleted_at IS NULL",
                params![id, next.to_rfc3339(), now],
            )?;
        } else {
            connection.execute(
                "UPDATE reminder_center SET deleted_at=?2, updated_at=?2 WHERE id=?1",
                params![id, now],
            )?;
        }
    } else {
        connection.execute(
            "UPDATE reminder_center SET deleted_at=?2, updated_at=?2 WHERE id=?1",
            params![id, now],
        )?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn snooze_reminder(
    database: State<'_, Database>,
    id: String,
    minutes: i64,
) -> Result<ReminderItem, AppError> {
    if minutes < 1 || minutes > 1440 {
        return Err(AppError::InvalidInput("稍后提醒时间必须在 1 到 1440 分钟之间。".into()));
    }
    let end = Utc::now() + ChronoDuration::minutes(minutes);
    let now = utc_now();
    let connection = database.lock()?;
    connection.execute(
        "UPDATE reminder_center
         SET status='active', scheduled_at=?2, updated_at=?3
         WHERE id=?1 AND deleted_at IS NULL",
        params![id, end.to_rfc3339(), now],
    )?;
    drop(connection);
    load_item(&database, &id)
}

#[tauri::command]
pub(crate) fn delete_reminder_center(database: State<'_, Database>, id: String) -> Result<(), AppError> {
    let connection = database.lock()?;
    let changed = connection.execute(
        "UPDATE reminder_center SET deleted_at=?2, updated_at=?2 WHERE id=?1 AND deleted_at IS NULL",
        params![id, utc_now()],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound("提醒不存在。".into()));
    }
    Ok(())
}

pub(crate) fn dispatch_due(app: &AppHandle) -> Result<(), AppError> {
    let database = app.state::<Database>();
    let now = Utc::now();
    let due = {
        let connection = database.lock()?;
        let mut statement = connection.prepare(
            "SELECT id, kind, title
             FROM reminder_center
             WHERE deleted_at IS NULL
               AND status='active'
               AND scheduled_at IS NOT NULL
               AND scheduled_at <= ?1"
        )?;
        let rows = statement
            .query_map(params![now.to_rfc3339()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };

    for (id, kind, title) in due {
        {
            let connection = database.lock()?;
            connection.execute(
                "UPDATE reminder_center
                 SET status='ringing', remaining_minutes=0, updated_at=?2
                 WHERE id=?1 AND status='active'",
                params![id, utc_now()],
            )?;
        }

        let notice_title = if kind == "alarm" { "定时闹钟" } else { "倒计时结束" };
        let _ = app.notification().builder().title(notice_title).body(&title).show();

        if let Some(window) = app.get_webview_window("task-widget") {
            let _ = window.show();
        }
    }
    Ok(())
}
