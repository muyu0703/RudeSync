use std::{
    cmp::Reverse,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::{DateTime, Local, NaiveDateTime, Utc};
use rusqlite::{backup::Backup, params, Connection, OpenFlags, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;
use uuid::Uuid;

use super::{apply_migrations, seed_defaults, utc_now, AppError, Database};

const DEFAULT_INVOICE_TERM: InvoiceTerm = InvoiceTerm::FourteenDays;
const BACKUP_INTERVAL_HOURS: i64 = 24;
const MAX_BACKUP_RETENTION: i64 = 365;
const BACKUP_FILE_PREFIX: &str = "RudeSync-backup-";
const PRE_RESTORE_FILE_PREFIX: &str = "RudeSync-pre-restore-";
const BACKUP_FILE_SUFFIX: &str = ".db";
const AUTOMATIC_RETRY_MINUTES: i64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum InvoiceTerm {
    #[serde(rename = "immediate")]
    Immediate,
    #[serde(rename = "7-days")]
    SevenDays,
    #[serde(rename = "14-days")]
    FourteenDays,
    #[serde(rename = "30-days")]
    ThirtyDays,
}

impl InvoiceTerm {
    fn days(self) -> i64 {
        match self {
            Self::Immediate => 0,
            Self::SevenDays => 7,
            Self::FourteenDays => 14,
            Self::ThirtyDays => 30,
        }
    }

    fn from_database(value: i64) -> Self {
        match value {
            0 => Self::Immediate,
            7 => Self::SevenDays,
            30 => Self::ThirtyDays,
            _ => DEFAULT_INVOICE_TERM,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsResponse {
    default_currency: String,
    default_invoice_term: InvoiceTerm,
    date_format: String,
    week_starts_on: i64,
    autostart_enabled: bool,
    autostart_registered: bool,
    close_to_tray: bool,
    notifications_enabled: bool,
    backup_directory: Option<String>,
    backup_enabled: bool,
    backup_interval_hours: i64,
    backup_retention_count: i64,
    last_backup_at: Option<String>,
    last_backup_attempt_at: Option<String>,
    last_backup_error: Option<String>,
    backup_setup_required: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateSettingsInput {
    default_currency: String,
    default_invoice_term: InvoiceTerm,
    date_format: String,
    week_starts_on: i64,
    autostart_enabled: bool,
    #[serde(default)]
    autostart_registered: bool,
    close_to_tray: bool,
    notifications_enabled: bool,
    backup_directory: Option<String>,
    backup_enabled: bool,
    backup_interval_hours: i64,
    backup_retention_count: i64,
    #[serde(default)]
    last_backup_at: Option<String>,
    #[serde(default)]
    last_backup_attempt_at: Option<String>,
    #[serde(default)]
    last_backup_error: Option<String>,
    #[serde(default)]
    backup_setup_required: bool,
}

#[derive(Debug)]
struct ValidatedSettings {
    default_currency: String,
    default_invoice_term: InvoiceTerm,
    date_format: String,
    week_starts_on: i64,
    autostart_enabled: bool,
    close_to_tray: bool,
    notifications_enabled: bool,
    backup_directory: Option<String>,
    backup_enabled: bool,
    backup_retention_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InvoiceProfileResponse {
    id: Option<String>,
    profile_name: String,
    display_name: String,
    business_name: String,
    address: String,
    email: String,
    logo_path: String,
    payment_instructions: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateInvoiceProfileInput {
    id: Option<String>,
    profile_name: String,
    display_name: String,
    business_name: String,
    address: String,
    email: String,
    logo_path: String,
    payment_instructions: String,
}

#[derive(Debug)]
struct ValidatedInvoiceProfile {
    id: Option<String>,
    profile_name: String,
    display_name: String,
    business_name: String,
    address: String,
    email: String,
    logo_path: String,
    payment_instructions: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupResult {
    destination_path: String,
    file_name: String,
    completed_at: String,
    size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestoreResult {
    safety_backup_path: String,
    restored_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackupKind {
    Automatic,
    Manual,
    PreRestore,
}

impl BackupKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Manual => "manual",
            Self::PreRestore => "pre_restore",
        }
    }
}

#[tauri::command]
pub(crate) fn get_settings(
    app: AppHandle,
    database: State<'_, Database>,
) -> Result<SettingsResponse, AppError> {
    let mut settings = {
        let connection = database.lock()?;
        ensure_extended_settings_schema(&connection)?;
        load_settings(&connection)?
    };
    settings.autostart_registered = autostart_is_registered(&app);
    Ok(settings)
}

#[tauri::command]
pub(crate) fn update_settings(
    app: AppHandle,
    database: State<'_, Database>,
    input: UpdateSettingsInput,
) -> Result<SettingsResponse, AppError> {
    let validated = validate_settings(input)?;
    let now = utc_now();

    let mut settings = {
        let mut connection = database.lock()?;
        ensure_extended_settings_schema(&connection)?;
        let transaction = connection.transaction()?;

        let settings_updated = transaction.execute(
            "UPDATE app_settings
             SET default_currency = ?1,
                 default_invoice_term_days = ?2,
                 date_format = ?3,
                 week_starts_on = ?4,
                 close_to_tray = ?5,
                 autostart_enabled = ?6,
                 notifications_enabled = ?7,
                 updated_at = ?8
             WHERE id = 1",
            params![
                validated.default_currency,
                validated.default_invoice_term.days(),
                validated.date_format,
                validated.week_starts_on,
                validated.close_to_tray,
                validated.autostart_enabled,
                validated.notifications_enabled,
                now,
            ],
        )?;
        if settings_updated != 1 {
            return Err(AppError::State(
                "The application settings row is missing.".into(),
            ));
        }

        transaction.execute(
            "INSERT INTO backup_settings (
                id, directory_path, is_enabled, interval_hours,
                retention_count, last_success_at, created_at, updated_at
             ) VALUES (1, ?1, ?2, ?3, ?4, NULL, ?5, ?5)
             ON CONFLICT(id) DO UPDATE SET
                directory_path = excluded.directory_path,
                is_enabled = excluded.is_enabled,
                interval_hours = excluded.interval_hours,
                retention_count = excluded.retention_count,
                last_attempt_at = CASE
                    WHEN backup_settings.directory_path IS NOT excluded.directory_path
                    THEN NULL ELSE backup_settings.last_attempt_at END,
                last_attempt_local_date = CASE
                    WHEN backup_settings.directory_path IS NOT excluded.directory_path
                    THEN NULL ELSE backup_settings.last_attempt_local_date END,
                last_error = CASE
                    WHEN backup_settings.directory_path IS NOT excluded.directory_path
                    THEN NULL ELSE backup_settings.last_error END,
                updated_at = excluded.updated_at",
            params![
                validated.backup_directory,
                validated.backup_enabled,
                BACKUP_INTERVAL_HOURS,
                validated.backup_retention_count,
                now,
            ],
        )?;
        transaction.commit()?;
        load_settings(&connection)?
    };

    apply_autostart_preference(&app, settings.autostart_enabled);
    settings.autostart_registered = autostart_is_registered(&app);
    Ok(settings)
}

#[tauri::command]
pub(crate) fn get_invoice_profile(
    database: State<'_, Database>,
) -> Result<InvoiceProfileResponse, AppError> {
    let connection = database.lock()?;
    Ok(load_invoice_profile(&connection)?.unwrap_or_else(default_invoice_profile))
}

#[tauri::command]
pub(crate) fn update_invoice_profile(
    database: State<'_, Database>,
    input: UpdateInvoiceProfileInput,
) -> Result<InvoiceProfileResponse, AppError> {
    let validated = validate_invoice_profile(input)?;
    let now = utc_now();
    let mut connection = database.lock()?;
    let transaction = connection.transaction()?;

    let existing_default_id = transaction
        .query_row(
            "SELECT id
             FROM invoice_profiles
             WHERE is_default = 1 AND deleted_at IS NULL
             LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let profile_id = validated
        .id
        .or(existing_default_id)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Demote first so the partial unique index can never reject the upsert.
    transaction.execute(
        "UPDATE invoice_profiles
         SET is_default = 0, updated_at = ?2
         WHERE is_default = 1 AND id <> ?1 AND deleted_at IS NULL",
        params![profile_id, now],
    )?;
    transaction.execute(
        "INSERT INTO invoice_profiles (
            id, profile_name, display_name, business_name, email, address,
            logo_path, payment_instructions, is_default,
            created_at, updated_at, deleted_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9, NULL)
         ON CONFLICT(id) DO UPDATE SET
            profile_name = excluded.profile_name,
            display_name = excluded.display_name,
            business_name = excluded.business_name,
            email = excluded.email,
            address = excluded.address,
            logo_path = excluded.logo_path,
            payment_instructions = excluded.payment_instructions,
            is_default = 1,
            updated_at = excluded.updated_at,
            deleted_at = NULL",
        params![
            profile_id,
            validated.profile_name,
            validated.display_name,
            validated.business_name,
            validated.email,
            validated.address,
            validated.logo_path,
            validated.payment_instructions,
            now,
        ],
    )?;
    transaction.commit()?;

    load_invoice_profile(&connection)?.ok_or_else(|| {
        AppError::State("The invoice profile could not be loaded after saving.".into())
    })
}

#[tauri::command]
pub(crate) fn run_backup(database: State<'_, Database>) -> Result<BackupResult, AppError> {
    run_automatic_backup(&database)
}

#[tauri::command]
pub(crate) fn run_manual_backup(
    database: State<'_, Database>,
    destination_path: String,
) -> Result<BackupResult, AppError> {
    let destination = validate_manual_backup_destination(&destination_path)?;
    let canonical_directory = destination
        .parent()
        .ok_or_else(|| AppError::InvalidInput("Choose a destination folder.".into()))?
        .to_path_buf();
    let display_directory = canonical_directory.to_string_lossy().into_owned();
    let file_name = destination
        .file_name()
        .ok_or_else(|| AppError::InvalidInput("Choose a backup file name.".into()))?
        .to_string_lossy()
        .into_owned();

    let connection = database.lock()?;
    create_backup_run(
        &connection,
        &display_directory,
        &destination,
        &file_name,
        BackupKind::Manual,
    )
}

#[tauri::command]
pub(crate) fn restore_backup(
    database: State<'_, Database>,
    source_path: String,
) -> Result<RestoreResult, AppError> {
    let source_path = validate_restore_source(&source_path)?;
    let source = open_validated_backup(&source_path)?;
    let mut connection = database.lock()?;

    let active_path = fs::canonicalize(&database.path)?;
    if source_path == active_path {
        return Err(AppError::InvalidInput(
            "The active RudeSync database cannot be restored onto itself.".into(),
        ));
    }
    let active_directory = active_path
        .parent()
        .ok_or_else(|| AppError::State("The application data folder is unavailable.".into()))?
        .to_path_buf();

    let safety_directory = configured_backup_directory(&connection)
        .and_then(|path| validate_backup_directory(&path).ok())
        .unwrap_or_else(|| active_directory.clone());
    let safety_name =
        backup_file_name_with_prefix(PRE_RESTORE_FILE_PREFIX, Utc::now(), Uuid::new_v4());
    let safety_path = safety_directory.join(&safety_name);
    create_backup_run(
        &connection,
        &safety_directory.to_string_lossy(),
        &safety_path,
        &safety_name,
        BackupKind::PreRestore,
    )?;

    let replacement_name = format!(".rudesync-restore-{}.sqlite3", Uuid::new_v4().simple());
    let replacement_path = active_directory.join(replacement_name);
    if let Err(error) = create_sqlite_backup(&source, &replacement_path)
        .and_then(|_| validate_database_file(&replacement_path))
    {
        let _ = fs::remove_file(&replacement_path);
        return Err(error);
    }
    drop(source);

    connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    let placeholder = Connection::open_in_memory()?;
    let active_connection = std::mem::replace(&mut *connection, placeholder);
    if let Err((active_connection, error)) = active_connection.close() {
        *connection = active_connection;
        let _ = fs::remove_file(&replacement_path);
        return Err(AppError::Database(error));
    }
    if let Err(error) = remove_sqlite_sidecars(&active_path) {
        *connection = open_database_connection(&active_path)?;
        let _ = fs::remove_file(&replacement_path);
        return Err(error);
    }

    let rollback_path = active_directory.join(format!(
        ".rudesync-restore-rollback-{}.sqlite3",
        Uuid::new_v4().simple()
    ));
    if let Err(error) = atomic_replace_database(&replacement_path, &active_path, &rollback_path) {
        *connection = open_database_connection(&active_path)?;
        let _ = fs::remove_file(&replacement_path);
        return Err(error);
    }

    match open_database_connection(&active_path) {
        Ok(restored_connection) => {
            *connection = restored_connection;
            let _ = fs::remove_file(&rollback_path);
        }
        Err(restore_error) => {
            let _ = remove_sqlite_sidecars(&active_path);
            let failed_path = active_directory.join(format!(
                ".rudesync-restore-failed-{}.sqlite3",
                Uuid::new_v4().simple()
            ));
            let rollback_result =
                atomic_replace_database(&rollback_path, &active_path, &failed_path)
                    .and_then(|_| open_database_connection(&active_path));
            let _ = fs::remove_file(&failed_path);
            match rollback_result {
                Ok(original_connection) => *connection = original_connection,
                Err(rollback_error) => {
                    return Err(AppError::State(format!(
                        "Restore failed ({restore_error}); the automatic rollback also failed \
                         ({rollback_error}). Your pre-restore safety copy is {}.",
                        safety_path.display()
                    )));
                }
            }
            return Err(AppError::State(format!(
                "The selected backup could not be opened after replacement ({restore_error}). \
                 RudeSync rolled back to the original database."
            )));
        }
    }

    Ok(RestoreResult {
        safety_backup_path: safety_path.to_string_lossy().into_owned(),
        restored_at: utc_now(),
    })
}

pub(crate) fn run_backup_if_due(database: &Database) -> Result<Option<BackupResult>, AppError> {
    let now_utc = Utc::now();
    let today_local = Local::now().date_naive().format("%Y-%m-%d").to_string();
    let due = {
        let connection = database.lock()?;
        let configured: (
            Option<String>,
            bool,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = connection.query_row(
            "SELECT directory_path, is_enabled, last_success_local_date,
                    last_attempt_at, last_attempt_local_date
             FROM backup_settings
             WHERE id = 1",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )?;
        let has_directory = configured
            .0
            .as_deref()
            .is_some_and(|path| !path.trim().is_empty());
        automatic_backup_is_due(
            configured.1,
            has_directory,
            configured.2.as_deref(),
            configured.3.as_deref(),
            configured.4.as_deref(),
            &today_local,
            now_utc,
        )
    };
    if !due {
        return Ok(None);
    }
    run_automatic_backup(database).map(Some)
}

fn run_automatic_backup(database: &Database) -> Result<BackupResult, AppError> {
    let attempt_at = utc_now();
    let local_date = Local::now().date_naive().format("%Y-%m-%d").to_string();
    {
        let connection = database.lock()?;
        connection.execute(
            "UPDATE backup_settings
             SET last_attempt_at = ?1,
                 last_attempt_local_date = ?2,
                 last_error = NULL,
                 updated_at = ?1
             WHERE id = 1",
            params![attempt_at, local_date],
        )?;
    }

    let result = create_automatic_backup(database);
    let connection = database.lock()?;
    match &result {
        Ok(backup) => {
            connection.execute(
                "UPDATE backup_settings
                 SET last_success_at = ?1,
                     last_success_local_date = ?2,
                     last_error = NULL,
                     updated_at = ?1
                 WHERE id = 1",
                params![backup.completed_at, local_date],
            )?;
        }
        Err(error) => {
            connection.execute(
                "UPDATE backup_settings
                 SET last_error = ?1, updated_at = ?2
                 WHERE id = 1",
                params![error.to_string(), utc_now()],
            )?;
        }
    }
    drop(connection);
    result
}

fn create_automatic_backup(database: &Database) -> Result<BackupResult, AppError> {
    let connection = database.lock()?;
    let (configured_directory, retention_count) = connection.query_row(
        "SELECT directory_path, retention_count
         FROM backup_settings
         WHERE id = 1",
        [],
        |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i64>(1)?)),
    )?;
    let configured_directory = configured_directory
        .map(|value| nonempty_trimmed(value, 32_000, "Backup directory"))
        .transpose()?
        .flatten()
        .ok_or_else(|| AppError::InvalidInput("Choose a backup directory first.".into()))?;
    let canonical_directory = validate_backup_directory(&configured_directory)?;

    let file_name = backup_file_name(Utc::now(), Uuid::new_v4());
    let destination = canonical_directory.join(&file_name);
    ensure_direct_child(&canonical_directory, &destination)?;

    let result = create_backup_run(
        &connection,
        &configured_directory,
        &destination,
        &file_name,
        BackupKind::Automatic,
    )?;

    // Retention cleanup is deliberately direct-child-only and recognizes only
    // RudeSync's exact automatic timestamp-plus-UUID filename shape.
    let retention_count = retention_count.clamp(1, MAX_BACKUP_RETENTION) as usize;
    let _ = prune_backups(&canonical_directory, retention_count);
    Ok(result)
}

fn create_backup_run(
    connection: &Connection,
    display_directory: &str,
    destination: &Path,
    file_name: &str,
    kind: BackupKind,
) -> Result<BackupResult, AppError> {
    let started_at = utc_now();
    let run_id = Uuid::new_v4().to_string();
    let destination_directory = destination
        .parent()
        .ok_or_else(|| AppError::State("The backup destination has no parent folder.".into()))?;
    ensure_direct_child(destination_directory, destination)?;

    connection.execute(
        "INSERT INTO backup_runs (
            id, destination_path, file_name, status, size_bytes,
            checksum_sha256, started_at, completed_at, error_message,
            created_at, backup_kind
         ) VALUES (?1, ?2, ?3, 'started', NULL, NULL, ?4, NULL, NULL, ?4, ?5)",
        params![
            run_id,
            display_directory,
            file_name,
            started_at,
            kind.as_str(),
        ],
    )?;

    let size_bytes = match create_sqlite_backup(connection, destination) {
        Ok(size) => size,
        Err(error) => {
            let _ = fs::remove_file(destination);
            let _ = connection.execute(
                "UPDATE backup_runs
                 SET status = 'failed', completed_at = ?2, error_message = ?3
                 WHERE id = ?1",
                params![run_id, utc_now(), error.to_string()],
            );
            return Err(error);
        }
    };

    let completed_at = utc_now();
    let size_bytes_for_database = i64::try_from(size_bytes)
        .map_err(|_| AppError::State("The backup file is unexpectedly large.".into()))?;
    let completed_transaction = connection.unchecked_transaction()?;
    completed_transaction.execute(
        "UPDATE backup_runs
         SET status = 'completed',
             size_bytes = ?2,
             completed_at = ?3,
             error_message = NULL
         WHERE id = ?1",
        params![run_id, size_bytes_for_database, completed_at],
    )?;
    completed_transaction.commit()?;

    Ok(BackupResult {
        destination_path: display_directory.to_owned(),
        file_name: file_name.to_owned(),
        completed_at,
        size_bytes,
    })
}

fn load_settings(connection: &Connection) -> Result<SettingsResponse, AppError> {
    connection
        .query_row(
            "SELECT
                settings.default_currency,
                settings.default_invoice_term_days,
                settings.date_format,
                settings.week_starts_on,
                settings.autostart_enabled,
                settings.close_to_tray,
                settings.notifications_enabled,
                backup.directory_path,
                COALESCE(backup.is_enabled, 1),
                COALESCE(backup.interval_hours, 24),
                COALESCE(backup.retention_count, 30),
                backup.last_success_at,
                backup.last_attempt_at,
                backup.last_error
             FROM app_settings settings
             LEFT JOIN backup_settings backup ON backup.id = 1
             WHERE settings.id = 1",
            [],
            settings_from_row,
        )
        .map_err(AppError::from)
}

fn settings_from_row(row: &Row<'_>) -> rusqlite::Result<SettingsResponse> {
    let invoice_term: i64 = row.get(1)?;
    let backup_directory: Option<String> = row.get(7)?;
    let backup_enabled: bool = row.get(8)?;
    let backup_setup_required = backup_enabled
        && backup_directory
            .as_deref()
            .map_or(true, |path| path.trim().is_empty());
    Ok(SettingsResponse {
        default_currency: row.get(0)?,
        default_invoice_term: InvoiceTerm::from_database(invoice_term),
        date_format: row.get(2)?,
        week_starts_on: row.get(3)?,
        autostart_enabled: row.get(4)?,
        autostart_registered: false,
        close_to_tray: row.get(5)?,
        notifications_enabled: row.get(6)?,
        backup_directory,
        backup_enabled,
        backup_interval_hours: row.get(9)?,
        backup_retention_count: row.get(10)?,
        last_backup_at: row.get(11)?,
        last_backup_attempt_at: row.get(12)?,
        last_backup_error: row.get(13)?,
        backup_setup_required,
    })
}

fn validate_settings(input: UpdateSettingsInput) -> Result<ValidatedSettings, AppError> {
    let currency = input.default_currency.trim().to_ascii_uppercase();
    if currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err(AppError::InvalidInput(
            "defaultCurrency must be a three-letter currency code.".into(),
        ));
    }
    if !matches!(
        input.date_format.as_str(),
        "MMMM d, yyyy" | "MM/dd/yyyy" | "yyyy-MM-dd"
    ) {
        return Err(AppError::InvalidInput(
            "dateFormat is not supported.".into(),
        ));
    }
    if !matches!(input.week_starts_on, 0 | 1) {
        return Err(AppError::InvalidInput(
            "weekStartsOn must be 0 (Sunday) or 1 (Monday).".into(),
        ));
    }
    if input.backup_interval_hours != BACKUP_INTERVAL_HOURS {
        return Err(AppError::InvalidInput(
            "backupIntervalHours must be 24 in this version.".into(),
        ));
    }
    if !(1..=MAX_BACKUP_RETENTION).contains(&input.backup_retention_count) {
        return Err(AppError::InvalidInput(format!(
            "backupRetentionCount must be between 1 and {MAX_BACKUP_RETENTION}."
        )));
    }

    let backup_directory = input
        .backup_directory
        .map(|value| nonempty_trimmed(value, 32_000, "Backup directory"))
        .transpose()?
        .flatten();
    if let Some(directory) = backup_directory.as_deref() {
        if !Path::new(directory).is_absolute() {
            return Err(AppError::InvalidInput(
                "The backup directory must be an absolute path.".into(),
            ));
        }
    }

    // These are response-only fields. Reading them here makes that intent
    // explicit and prevents future accidental persistence of stale UI data.
    let _ = input.autostart_registered;
    let _ = input.last_backup_at;
    let _ = input.last_backup_attempt_at;
    let _ = input.last_backup_error;
    let _ = input.backup_setup_required;

    Ok(ValidatedSettings {
        default_currency: currency,
        default_invoice_term: input.default_invoice_term,
        date_format: input.date_format,
        week_starts_on: input.week_starts_on,
        autostart_enabled: input.autostart_enabled,
        close_to_tray: input.close_to_tray,
        notifications_enabled: input.notifications_enabled,
        backup_directory,
        backup_enabled: input.backup_enabled,
        backup_retention_count: input.backup_retention_count,
    })
}

fn validate_invoice_profile(
    input: UpdateInvoiceProfileInput,
) -> Result<ValidatedInvoiceProfile, AppError> {
    let id = input
        .id
        .map(|value| nonempty_trimmed(value, 64, "Invoice profile ID"))
        .transpose()?
        .flatten();
    let profile_name = trimmed_or_default(input.profile_name, "Default", 120, "Profile name")?;

    Ok(ValidatedInvoiceProfile {
        id,
        profile_name,
        display_name: trimmed(input.display_name, 240, "Display name")?,
        business_name: trimmed(input.business_name, 240, "Business name")?,
        address: trimmed(input.address, 4_000, "Address")?,
        email: trimmed(input.email, 240, "Email or contact details")?,
        logo_path: trimmed(input.logo_path, 32_000, "Logo path")?,
        payment_instructions: trimmed(input.payment_instructions, 4_000, "Payment instructions")?,
    })
}

fn trimmed(value: String, max_chars: usize, field: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.chars().count() > max_chars {
        return Err(AppError::InvalidInput(format!(
            "{field} must be no longer than {max_chars} characters."
        )));
    }
    Ok(value.to_owned())
}

fn nonempty_trimmed(
    value: String,
    max_chars: usize,
    field: &str,
) -> Result<Option<String>, AppError> {
    let value = trimmed(value, max_chars, field)?;
    Ok((!value.is_empty()).then_some(value))
}

fn trimmed_or_default(
    value: String,
    default: &str,
    max_chars: usize,
    field: &str,
) -> Result<String, AppError> {
    let value = trimmed(value, max_chars, field)?;
    Ok(if value.is_empty() {
        default.to_owned()
    } else {
        value
    })
}

fn load_invoice_profile(
    connection: &Connection,
) -> Result<Option<InvoiceProfileResponse>, AppError> {
    connection
        .query_row(
            "SELECT
                id,
                profile_name,
                display_name,
                COALESCE(business_name, ''),
                COALESCE(address, ''),
                COALESCE(email, ''),
                COALESCE(logo_path, ''),
                COALESCE(payment_instructions, '')
             FROM invoice_profiles
             WHERE deleted_at IS NULL
             ORDER BY is_default DESC, updated_at DESC
             LIMIT 1",
            [],
            |row| {
                Ok(InvoiceProfileResponse {
                    id: Some(row.get(0)?),
                    profile_name: row.get(1)?,
                    display_name: row.get(2)?,
                    business_name: row.get(3)?,
                    address: row.get(4)?,
                    email: row.get(5)?,
                    logo_path: row.get(6)?,
                    payment_instructions: row.get(7)?,
                })
            },
        )
        .optional()
        .map_err(AppError::from)
}

fn default_invoice_profile() -> InvoiceProfileResponse {
    InvoiceProfileResponse {
        id: None,
        profile_name: "Default".into(),
        display_name: String::new(),
        business_name: String::new(),
        address: String::new(),
        email: String::new(),
        logo_path: String::new(),
        payment_instructions: String::new(),
    }
}

fn ensure_extended_settings_schema(connection: &Connection) -> Result<(), AppError> {
    if !table_has_column(connection, "app_settings", "default_invoice_term_days")? {
        connection.execute(
            "ALTER TABLE app_settings
             ADD COLUMN default_invoice_term_days INTEGER NOT NULL DEFAULT 14
             CHECK (default_invoice_term_days IN (0, 7, 14, 30))",
            [],
        )?;
    }
    if !table_has_column(connection, "app_settings", "notifications_enabled")? {
        connection.execute(
            "ALTER TABLE app_settings
             ADD COLUMN notifications_enabled INTEGER NOT NULL DEFAULT 1
             CHECK (notifications_enabled IN (0, 1))",
            [],
        )?;
    }
    Ok(())
}

fn table_has_column(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
) -> Result<bool, AppError> {
    if table_name != "app_settings" {
        return Err(AppError::State("Unsupported settings table.".into()));
    }
    let mut statement = connection.prepare("PRAGMA table_info(app_settings)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == column_name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn autostart_is_registered(app: &AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

fn apply_autostart_preference(app: &AppHandle, enabled: bool) {
    let autostart = app.autolaunch();
    let registered = autostart.is_enabled().unwrap_or(false);
    match (enabled, registered) {
        (true, false) => {
            let _ = autostart.enable();
        }
        (false, true) => {
            let _ = autostart.disable();
        }
        _ => {}
    }
}

fn automatic_backup_is_due(
    enabled: bool,
    has_directory: bool,
    last_success_local_date: Option<&str>,
    last_attempt_at: Option<&str>,
    last_attempt_local_date: Option<&str>,
    today_local: &str,
    now_utc: DateTime<Utc>,
) -> bool {
    if !enabled || !has_directory || last_success_local_date == Some(today_local) {
        return false;
    }
    if last_attempt_local_date != Some(today_local) {
        return true;
    }
    last_attempt_at
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|attempt| now_utc.signed_duration_since(attempt.with_timezone(&Utc)))
        .map_or(true, |elapsed| {
            elapsed >= chrono::Duration::minutes(AUTOMATIC_RETRY_MINUTES)
        })
}

fn configured_backup_directory(connection: &Connection) -> Option<String> {
    connection
        .query_row(
            "SELECT directory_path FROM backup_settings WHERE id = 1",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
        .and_then(|path| {
            let trimmed = path.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        })
}

fn validate_backup_directory(configured_directory: &str) -> Result<PathBuf, AppError> {
    let path = Path::new(configured_directory);
    if !path.is_absolute() {
        return Err(AppError::InvalidInput(
            "The backup directory must be an absolute path.".into(),
        ));
    }
    let canonical = fs::canonicalize(path).map_err(|error| {
        AppError::InvalidInput(format!("The backup directory is unavailable: {error}"))
    })?;
    if !fs::metadata(&canonical)?.is_dir() {
        return Err(AppError::InvalidInput(
            "The selected backup path is not a directory.".into(),
        ));
    }
    if canonical.parent().is_none() {
        return Err(AppError::InvalidInput(
            "Choose a dedicated backup folder instead of a filesystem root.".into(),
        ));
    }
    Ok(canonical)
}

fn validate_manual_backup_destination(destination: &str) -> Result<PathBuf, AppError> {
    let path = PathBuf::from(destination.trim());
    if !path.is_absolute() {
        return Err(AppError::InvalidInput(
            "The manual backup destination must be an absolute path.".into(),
        ));
    }
    let file_name = path
        .file_name()
        .ok_or_else(|| AppError::InvalidInput("Choose a backup file name.".into()))?;
    let extension_is_supported = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("sqlite3") || extension.eq_ignore_ascii_case("db")
        });
    if !extension_is_supported {
        return Err(AppError::InvalidInput(
            "Manual backups must use a .sqlite3 or .db extension.".into(),
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| AppError::InvalidInput("Choose a destination folder.".into()))?;
    let canonical_parent = fs::canonicalize(parent).map_err(|error| {
        AppError::InvalidInput(format!("The backup destination is unavailable: {error}"))
    })?;
    if !fs::metadata(&canonical_parent)?.is_dir() {
        return Err(AppError::InvalidInput(
            "The backup destination parent must be a folder.".into(),
        ));
    }
    let canonical_destination = canonical_parent.join(file_name);
    ensure_direct_child(&canonical_parent, &canonical_destination)?;
    if canonical_destination.exists() {
        return Err(AppError::InvalidInput(
            "That backup file already exists. Choose a new file name.".into(),
        ));
    }
    Ok(canonical_destination)
}

fn validate_restore_source(source: &str) -> Result<PathBuf, AppError> {
    let path = PathBuf::from(source.trim());
    if !path.is_absolute() {
        return Err(AppError::InvalidInput(
            "The restore source must be an absolute path.".into(),
        ));
    }
    let canonical = fs::canonicalize(&path).map_err(|error| {
        AppError::InvalidInput(format!("The selected backup is unavailable: {error}"))
    })?;
    if !fs::metadata(&canonical)?.is_file() {
        return Err(AppError::InvalidInput(
            "The selected restore source is not a file.".into(),
        ));
    }
    Ok(canonical)
}

fn open_validated_backup(path: &Path) -> Result<Connection, AppError> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    validate_database_connection(&connection)?;
    Ok(connection)
}

fn validate_database_file(path: &Path) -> Result<(), AppError> {
    let connection = open_validated_backup(path)?;
    connection
        .close()
        .map_err(|(_, error)| AppError::Database(error))
}

fn validate_database_connection(connection: &Connection) -> Result<(), AppError> {
    let quick_check: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if quick_check != "ok" {
        return Err(AppError::InvalidInput(format!(
            "The selected backup failed SQLite verification: {quick_check}"
        )));
    }

    const REQUIRED_TABLES: [&str; 9] = [
        "schema_migrations",
        "app_settings",
        "backup_settings",
        "tasks",
        "clients",
        "projects",
        "invoices",
        "personal_loans",
        "loan_installments",
    ];
    for table in REQUIRED_TABLES {
        let exists: bool = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = ?1
             )",
            params![table],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::InvalidInput(format!(
                "The selected file is not a complete RudeSync backup (missing {table})."
            )));
        }
    }
    let version: i64 = connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;
    if version > 2 {
        return Err(AppError::InvalidInput(
            "This backup was created by a newer RudeSync database version.".into(),
        ));
    }
    Ok(())
}

fn open_database_connection(path: &Path) -> Result<Connection, AppError> {
    let mut connection = Connection::open(path)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    apply_migrations(&mut connection)?;
    seed_defaults(&connection)?;
    Ok(connection)
}

fn remove_sqlite_sidecars(database_path: &Path) -> Result<(), AppError> {
    let file_name = database_path
        .file_name()
        .ok_or_else(|| AppError::State("The database path has no file name.".into()))?
        .to_string_lossy();
    let directory = database_path
        .parent()
        .ok_or_else(|| AppError::State("The database path has no parent folder.".into()))?;
    for suffix in ["-wal", "-shm"] {
        let sidecar = directory.join(format!("{file_name}{suffix}"));
        ensure_direct_child(directory, &sidecar)?;
        match fs::remove_file(&sidecar) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(AppError::Io(error)),
        }
    }
    Ok(())
}

#[cfg(windows)]
fn atomic_replace_database(
    replacement: &Path,
    destination: &Path,
    rollback: &Path,
) -> Result<(), AppError> {
    use std::{ffi::c_void, os::windows::ffi::OsStrExt, ptr};

    #[link(name = "Kernel32")]
    extern "system" {
        fn ReplaceFileW(
            replaced_file_name: *const u16,
            replacement_file_name: *const u16,
            backup_file_name: *const u16,
            replace_flags: u32,
            exclude: *mut c_void,
            reserved: *mut c_void,
        ) -> i32;
    }

    let destination_wide = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let replacement_wide = replacement
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let rollback_wide = rollback
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    const REPLACEFILE_WRITE_THROUGH: u32 = 0x0000_0001;
    const REPLACEFILE_IGNORE_MERGE_ERRORS: u32 = 0x0000_0002;

    // SAFETY: all three paths are valid, null-terminated UTF-16 buffers that
    // live for the duration of the Win32 call. Both opaque pointer parameters
    // are documented as reserved and must be null.
    let replaced = unsafe {
        ReplaceFileW(
            destination_wide.as_ptr(),
            replacement_wide.as_ptr(),
            rollback_wide.as_ptr(),
            REPLACEFILE_WRITE_THROUGH | REPLACEFILE_IGNORE_MERGE_ERRORS,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    if replaced == 0 {
        return Err(AppError::Io(std::io::Error::last_os_error()));
    }
    Ok(())
}

#[cfg(not(windows))]
fn atomic_replace_database(
    replacement: &Path,
    destination: &Path,
    rollback: &Path,
) -> Result<(), AppError> {
    fs::rename(destination, rollback)?;
    if let Err(error) = fs::rename(replacement, destination) {
        let _ = fs::rename(rollback, destination);
        return Err(AppError::Io(error));
    }
    Ok(())
}

fn ensure_direct_child(directory: &Path, destination: &Path) -> Result<(), AppError> {
    if destination.parent() != Some(directory) {
        return Err(AppError::State(
            "The backup destination escaped the selected directory.".into(),
        ));
    }
    Ok(())
}

fn backup_file_name(timestamp: DateTime<Utc>, _id: Uuid) -> String {
    format!(
        "{BACKUP_FILE_PREFIX}{}{BACKUP_FILE_SUFFIX}",
        timestamp.with_timezone(&Local).format("%Y-%m-%d-%H%M%S")
    )
}

fn backup_file_name_with_prefix(prefix: &str, timestamp: DateTime<Utc>, id: Uuid) -> String {
    format!(
        "{prefix}{}-{}{BACKUP_FILE_SUFFIX}",
        timestamp.format("%Y-%m-%dT%H-%M-%S%.3fZ"),
        id.simple()
    )
}

fn parse_backup_file_name(file_name: &str) -> Option<NaiveDateTime> {
    let core = file_name
        .strip_prefix(BACKUP_FILE_PREFIX)?
        .strip_suffix(BACKUP_FILE_SUFFIX)?;
    NaiveDateTime::parse_from_str(core, "%Y-%m-%d-%H%M%S").ok()
}

fn create_sqlite_backup(source: &Connection, destination: &Path) -> Result<u64, AppError> {
    let reservation = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)?;
    drop(reservation);

    let mut target = Connection::open(destination)?;
    {
        let backup = Backup::new(source, &mut target)?;
        backup.run_to_completion(128, Duration::from_millis(5), None)?;
    }

    let quick_check: String = target.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if quick_check != "ok" {
        return Err(AppError::State(format!(
            "SQLite verification failed for the backup: {quick_check}"
        )));
    }
    target
        .close()
        .map_err(|(_, error)| AppError::Database(error))?;

    let size_bytes = fs::metadata(destination)?.len();
    if size_bytes == 0 {
        return Err(AppError::State(
            "SQLite produced an empty backup file.".into(),
        ));
    }
    Ok(size_bytes)
}

fn prune_backups(directory: &Path, retention_count: usize) -> Result<Vec<PathBuf>, AppError> {
    let mut candidates = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(timestamp) = parse_backup_file_name(&file_name) else {
            continue;
        };
        let path = entry.path();
        ensure_direct_child(directory, &path)?;
        candidates.push((Reverse(timestamp), path));
    }
    candidates.sort_by_key(|(timestamp, _)| *timestamp);

    let mut removed = Vec::new();
    for (_, path) in candidates.into_iter().skip(retention_count.max(1)) {
        fs::remove_file(&path)?;
        removed.push(path);
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_settings() -> UpdateSettingsInput {
        UpdateSettingsInput {
            default_currency: " usd ".into(),
            default_invoice_term: InvoiceTerm::FourteenDays,
            date_format: "MMMM d, yyyy".into(),
            week_starts_on: 1,
            autostart_enabled: true,
            autostart_registered: false,
            close_to_tray: true,
            notifications_enabled: true,
            backup_directory: None,
            backup_enabled: true,
            backup_interval_hours: 24,
            backup_retention_count: 30,
            last_backup_at: None,
            last_backup_attempt_at: None,
            last_backup_error: None,
            backup_setup_required: true,
        }
    }

    fn test_directory(label: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("rudesync-{label}-{}", Uuid::new_v4().simple()));
        fs::create_dir(&directory).unwrap();
        directory
    }

    #[test]
    fn settings_validation_normalizes_currency_and_rejects_bad_retention() {
        let validated = validate_settings(sample_settings()).unwrap();
        assert_eq!(validated.default_currency, "USD");

        let mut invalid = sample_settings();
        invalid.backup_retention_count = 366;
        assert!(validate_settings(invalid).is_err());
    }

    #[test]
    fn settings_schema_extension_is_idempotent() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE app_settings (
                    id INTEGER PRIMARY KEY,
                    default_currency TEXT NOT NULL
                 );",
            )
            .unwrap();

        ensure_extended_settings_schema(&connection).unwrap();
        ensure_extended_settings_schema(&connection).unwrap();

        assert!(
            table_has_column(&connection, "app_settings", "default_invoice_term_days").unwrap()
        );
        assert!(table_has_column(&connection, "app_settings", "notifications_enabled").unwrap());
    }

    #[test]
    fn automatic_backup_runs_once_per_local_day_and_retries_with_backoff() {
        let now = DateTime::parse_from_rfc3339("2026-07-23T04:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(automatic_backup_is_due(
            true,
            true,
            Some("2026-07-22"),
            None,
            None,
            "2026-07-23",
            now,
        ));
        assert!(!automatic_backup_is_due(
            true,
            true,
            Some("2026-07-23"),
            None,
            None,
            "2026-07-23",
            now,
        ));
        assert!(!automatic_backup_is_due(
            true,
            true,
            None,
            Some("2026-07-23T03:30:01Z"),
            Some("2026-07-23"),
            "2026-07-23",
            now,
        ));
        assert!(automatic_backup_is_due(
            true,
            true,
            None,
            Some("2026-07-23T02:59:59Z"),
            Some("2026-07-23"),
            "2026-07-23",
            now,
        ));
    }

    #[test]
    fn online_backup_creates_a_queryable_sqlite_copy() {
        let source = Connection::open_in_memory().unwrap();
        source
            .execute_batch(
                "CREATE TABLE example(value TEXT NOT NULL);
                 INSERT INTO example(value) VALUES ('kept');",
            )
            .unwrap();
        let directory = test_directory("sqlite-backup-test");
        let destination = directory.join(backup_file_name(Utc::now(), Uuid::new_v4()));

        let size = create_sqlite_backup(&source, &destination).unwrap();
        let copy = Connection::open(&destination).unwrap();
        let value: String = copy
            .query_row("SELECT value FROM example", [], |row| row.get(0))
            .unwrap();

        assert!(size > 0);
        assert_eq!(value, "kept");
        drop(copy);
        fs::remove_file(destination).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn retention_removes_only_recognized_direct_backup_files() {
        let directory = test_directory("retention-test");
        let first = backup_file_name(
            DateTime::parse_from_rfc3339("2026-07-21T08:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            Uuid::new_v4(),
        );
        let second = backup_file_name(
            DateTime::parse_from_rfc3339("2026-07-22T08:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            Uuid::new_v4(),
        );
        let third = backup_file_name(
            DateTime::parse_from_rfc3339("2026-07-23T08:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            Uuid::new_v4(),
        );
        let unrelated = "RudeSync-backup-important.db";
        let manual = backup_file_name_with_prefix(
            "RudeSync-manual-backup-",
            DateTime::parse_from_rfc3339("2026-07-20T08:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            Uuid::new_v4(),
        );
        for name in [&first, &second, &third, unrelated, &manual] {
            fs::write(directory.join(name), b"test").unwrap();
        }

        let removed = prune_backups(&directory, 2).unwrap();

        assert_eq!(removed, vec![directory.join(&first)]);
        assert!(!directory.join(&first).exists());
        assert!(directory.join(&second).exists());
        assert!(directory.join(&third).exists());
        assert!(directory.join(unrelated).exists());
        assert!(directory.join(&manual).exists());

        for name in [&second, &third, unrelated, &manual] {
            fs::remove_file(directory.join(name)).unwrap();
        }
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn atomic_replace_keeps_a_rollback_copy() {
        let directory = test_directory("atomic-restore-test");
        let active = directory.join("active.sqlite3");
        let replacement = directory.join("replacement.sqlite3");
        let rollback = directory.join("rollback.sqlite3");
        fs::write(&active, b"before").unwrap();
        fs::write(&replacement, b"after").unwrap();

        atomic_replace_database(&replacement, &active, &rollback).unwrap();

        assert_eq!(fs::read(&active).unwrap(), b"after");
        assert_eq!(fs::read(&rollback).unwrap(), b"before");
        assert!(!replacement.exists());
        fs::remove_file(active).unwrap();
        fs::remove_file(rollback).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
