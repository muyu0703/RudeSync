use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use super::{
    optional_trimmed, required_trimmed, utc_now, validate_optional_date, AppError, Database,
};

// ---------------------------------------------------------------------------
// Clients

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Client {
    id: String,
    name: String,
    company_name: Option<String>,
    email: Option<String>,
    billing_address: Option<String>,
    currency: String,
    notes: Option<String>,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateClientInput {
    name: String,
    company_name: Option<String>,
    email: Option<String>,
    billing_address: Option<String>,
    currency: Option<String>,
    notes: Option<String>,
}

#[tauri::command]
pub(crate) fn list_clients(
    database: State<'_, Database>,
    include_deleted: Option<bool>,
) -> Result<Vec<Client>, AppError> {
    let connection = database.lock()?;
    let mut statement = connection.prepare(
        "SELECT id, name, company_name, email, billing_address, currency, notes,
                created_at, updated_at, deleted_at
         FROM clients
         WHERE (?1 = 1 OR deleted_at IS NULL)
         ORDER BY name COLLATE NOCASE ASC",
    )?;
    let rows = statement.query_map(params![include_deleted.unwrap_or(false)], client_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

#[tauri::command]
pub(crate) fn create_client(
    database: State<'_, Database>,
    input: CreateClientInput,
) -> Result<Client, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = utc_now();
    let values = validate_client_input(input)?;
    let connection = database.lock()?;

    connection.execute(
        "INSERT INTO clients (
            id, name, company_name, email, billing_address, currency, notes,
            created_at, updated_at, deleted_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, NULL)",
        params![
            id,
            values.name,
            values.company_name,
            values.email,
            values.billing_address,
            values.currency,
            values.notes,
            now
        ],
    )?;
    load_client(&connection, &id, false)
}

#[tauri::command]
pub(crate) fn update_client(
    database: State<'_, Database>,
    id: String,
    input: CreateClientInput,
) -> Result<Client, AppError> {
    let connection = database.lock()?;
    update_client_in_connection(&connection, &id, input)
}

fn update_client_in_connection(
    connection: &Connection,
    id: &str,
    input: CreateClientInput,
) -> Result<Client, AppError> {
    let id = required_trimmed(id.to_owned(), "Client ID", 64)?;
    let values = validate_client_input(input)?;
    let changed = connection.execute(
        "UPDATE clients
         SET name = ?2,
             company_name = ?3,
             email = ?4,
             billing_address = ?5,
             currency = ?6,
             notes = ?7,
             updated_at = ?8
         WHERE id = ?1 AND deleted_at IS NULL",
        params![
            id,
            values.name,
            values.company_name,
            values.email,
            values.billing_address,
            values.currency,
            values.notes,
            utc_now()
        ],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound("Client not found.".into()));
    }
    load_client(connection, &id, false)
}

struct ValidatedClient {
    name: String,
    company_name: Option<String>,
    email: Option<String>,
    billing_address: Option<String>,
    currency: String,
    notes: Option<String>,
}

fn validate_client_input(input: CreateClientInput) -> Result<ValidatedClient, AppError> {
    Ok(ValidatedClient {
        name: required_trimmed(input.name, "Client name", 240)?,
        company_name: optional_trimmed(input.company_name, 240, "Company name")?,
        email: optional_trimmed(input.email, 320, "Client email")?,
        billing_address: optional_trimmed(input.billing_address, 2_000, "Billing address")?,
        currency: validate_currency(input.currency.as_deref().unwrap_or("USD"))?,
        notes: optional_trimmed(input.notes, 20_000, "Client notes")?,
    })
}

fn load_client(
    connection: &Connection,
    id: &str,
    include_deleted: bool,
) -> Result<Client, AppError> {
    connection
        .query_row(
            "SELECT id, name, company_name, email, billing_address, currency, notes,
                    created_at, updated_at, deleted_at
             FROM clients
             WHERE id = ?1 AND (?2 = 1 OR deleted_at IS NULL)",
            params![id, include_deleted],
            client_from_row,
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("Client not found.".into()))
}

fn client_from_row(row: &Row<'_>) -> rusqlite::Result<Client> {
    Ok(Client {
        id: row.get(0)?,
        name: row.get(1)?,
        company_name: row.get(2)?,
        email: row.get(3)?,
        billing_address: row.get(4)?,
        currency: row.get(5)?,
        notes: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        deleted_at: row.get(9)?,
    })
}

// ---------------------------------------------------------------------------
// Fixed-price projects

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Project {
    id: String,
    client_id: Option<String>,
    name: String,
    description: Option<String>,
    urls: Vec<String>,
    status: String,
    currency: String,
    quoted_total_minor: i64,
    kickoff_percent_basis_points: i64,
    kickoff_label: String,
    completion_percent_basis_points: i64,
    completion_label: String,
    start_date: Option<String>,
    due_date: Option<String>,
    completed_at: Option<String>,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
    milestones: Vec<Milestone>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateProjectInput {
    client_id: Option<String>,
    name: String,
    description: Option<String>,
    urls: Option<Vec<String>>,
    status: Option<String>,
    currency: Option<String>,
    quoted_total_minor: i64,
    kickoff_percent_basis_points: Option<i64>,
    kickoff_label: Option<String>,
    completion_percent_basis_points: Option<i64>,
    completion_label: Option<String>,
    start_date: Option<String>,
    due_date: Option<String>,
    milestones: Option<Vec<MilestoneInput>>,
}

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

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectListFilter {
    client_id: Option<String>,
    status: Option<String>,
    include_deleted: Option<bool>,
}

#[tauri::command]
pub(crate) fn list_projects(
    database: State<'_, Database>,
    filter: Option<ProjectListFilter>,
) -> Result<Vec<Project>, AppError> {
    let connection = database.lock()?;
    list_projects_from_connection(&connection, filter.unwrap_or_default())
}

fn list_projects_from_connection(
    connection: &Connection,
    filter: ProjectListFilter,
) -> Result<Vec<Project>, AppError> {
    let status = filter
        .status
        .map(|status| validate_project_status(&status))
        .transpose()?;
    let mut statement = connection.prepare(
        "SELECT id, client_id, name, description, urls_json, status, currency,
                quoted_total_minor, kickoff_percent_basis_points,
                kickoff_label, completion_percent_basis_points,
                completion_label, start_date, due_date,
                completed_at, created_at, updated_at, deleted_at
         FROM projects
         WHERE (?1 IS NULL OR client_id = ?1)
           AND (?2 IS NULL OR status = ?2)
           AND (?3 = 1 OR deleted_at IS NULL)
         ORDER BY
            CASE status
                WHEN 'active' THEN 0
                WHEN 'draft' THEN 1
                WHEN 'completed' THEN 2
                ELSE 3
            END,
            name COLLATE NOCASE ASC",
    )?;
    let rows = statement.query_map(
        params![
            filter.client_id,
            status,
            filter.include_deleted.unwrap_or(false)
        ],
        project_from_row,
    )?;
    let mut projects = rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)?;
    for project in &mut projects {
        project.milestones = load_project_milestones(connection, &project.id)?;
    }
    Ok(projects)
}

#[tauri::command]
pub(crate) fn create_project(
    database: State<'_, Database>,
    input: CreateProjectInput,
) -> Result<Project, AppError> {
    let mut connection = database.lock()?;
    create_project_atomic(&mut connection, input)
}

fn create_project_atomic(
    connection: &mut Connection,
    input: CreateProjectInput,
) -> Result<Project, AppError> {
    let transaction = connection.transaction()?;
    let project = create_project_in_connection(&transaction, input)?;
    transaction.commit()?;
    load_project(connection, &project.id, false)
}

fn create_project_in_connection(
    connection: &Connection,
    input: CreateProjectInput,
) -> Result<Project, AppError> {
    let values = validate_project_input(connection, input)?;
    let id = Uuid::new_v4().to_string();
    let now = utc_now();
    let completed_at = (values.status == "completed").then(|| now.clone());
    connection.execute(
        "INSERT INTO projects (
            id, client_id, name, description, urls_json, status, currency,
            quoted_total_minor, kickoff_percent_basis_points,
            kickoff_label, completion_percent_basis_points,
            completion_label, start_date, due_date,
            completed_at, created_at, updated_at, deleted_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?16, NULL
         )",
        params![
            id,
            values.client_id,
            values.name,
            values.description,
            values.urls_json,
            values.status,
            values.currency,
            values.quoted_total_minor,
            values.kickoff_percent_basis_points,
            values.kickoff_label,
            values.completion_percent_basis_points,
            values.completion_label,
            values.start_date,
            values.due_date,
            completed_at,
            now
        ],
    )?;
    sync_project_milestones(connection, &id, &values.milestones, &now)?;
    load_project(connection, &id, false)
}

#[tauri::command]
pub(crate) fn update_project(
    database: State<'_, Database>,
    id: String,
    input: CreateProjectInput,
) -> Result<Project, AppError> {
    let mut connection = database.lock()?;
    update_project_atomic(&mut connection, &id, input)
}

fn update_project_atomic(
    connection: &mut Connection,
    id: &str,
    input: CreateProjectInput,
) -> Result<Project, AppError> {
    let transaction = connection.transaction()?;
    let project = update_project_in_connection(&transaction, id, input)?;
    transaction.commit()?;
    load_project(connection, &project.id, false)
}

fn update_project_in_connection(
    connection: &Connection,
    id: &str,
    input: CreateProjectInput,
) -> Result<Project, AppError> {
    let id = required_trimmed(id.to_owned(), "Project ID", 64)?;
    let values = validate_project_input(connection, input)?;
    let now = utc_now();
    let changed = connection.execute(
        "UPDATE projects
         SET client_id = ?2,
             name = ?3,
             description = ?4,
             urls_json = ?5,
             status = ?6,
             currency = ?7,
             quoted_total_minor = ?8,
             kickoff_percent_basis_points = ?9,
             kickoff_label = ?10,
             completion_percent_basis_points = ?11,
             completion_label = ?12,
             start_date = ?13,
             due_date = ?14,
             completed_at = CASE
                 WHEN ?6 = 'completed' THEN COALESCE(completed_at, ?15)
                 ELSE NULL
             END,
             updated_at = ?15
         WHERE id = ?1 AND deleted_at IS NULL",
        params![
            id,
            values.client_id,
            values.name,
            values.description,
            values.urls_json,
            values.status,
            values.currency,
            values.quoted_total_minor,
            values.kickoff_percent_basis_points,
            values.kickoff_label,
            values.completion_percent_basis_points,
            values.completion_label,
            values.start_date,
            values.due_date,
            now
        ],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound("Project not found.".into()));
    }
    sync_project_milestones(connection, &id, &values.milestones, &now)?;
    load_project(connection, &id, false)
}

struct ValidatedProject {
    client_id: Option<String>,
    name: String,
    description: Option<String>,
    urls_json: String,
    status: String,
    currency: String,
    quoted_total_minor: i64,
    kickoff_percent_basis_points: i64,
    kickoff_label: String,
    completion_percent_basis_points: i64,
    completion_label: String,
    start_date: Option<String>,
    due_date: Option<String>,
    milestones: Vec<ValidatedMilestone>,
}

struct ValidatedMilestone {
    id: Option<String>,
    label: String,
    amount_minor: i64,
    kind: String,
    sort_order: i64,
}

fn validate_project_input(
    connection: &Connection,
    input: CreateProjectInput,
) -> Result<ValidatedProject, AppError> {
    let client_id = optional_trimmed(input.client_id, 64, "Client ID")?;
    ensure_client(connection, client_id.as_deref())?;
    let currency = match input.currency {
        Some(currency) => validate_currency(&currency)?,
        None => currency_for_client_or_default(connection, client_id.as_deref())?,
    };
    let status = validate_project_status(input.status.as_deref().unwrap_or("active"))?;
    let start_date = validate_optional_date(input.start_date, "startDate")?;
    let due_date = validate_optional_date(input.due_date, "dueDate")?;
    validate_date_order(start_date.as_deref(), due_date.as_deref(), "Project")?;
    let urls = validate_urls(input.urls.unwrap_or_default())?;
    let urls_json = serde_json::to_string(&urls)
        .map_err(|error| AppError::State(format!("Could not serialize project URLs: {error}")))?;

    let allowed_kinds = [
        "kickoff",
        "completion",
        "phase",
        "weekly",
        "additional",
        "custom",
    ];
    let mut milestones = Vec::new();
    for (index, m) in input.milestones.unwrap_or_default().into_iter().enumerate() {
        let label = required_trimmed(m.label, "Milestone label", 80)?;
        if m.amount_minor < 0 {
            return Err(AppError::InvalidInput(
                "Milestone amount cannot be negative.".into(),
            ));
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

    Ok(ValidatedProject {
        client_id,
        name: required_trimmed(input.name, "Project name", 240)?,
        description: optional_trimmed(input.description, 20_000, "Project description")?,
        urls_json,
        status,
        currency,
        quoted_total_minor: nonnegative_minor(input.quoted_total_minor, "quotedTotalMinor")?,
        kickoff_percent_basis_points: 5_000,
        kickoff_label: required_trimmed(
            input.kickoff_label.unwrap_or_else(|| "Kickoff".into()),
            "Kickoff milestone label",
            80,
        )?,
        completion_percent_basis_points: 5_000,
        completion_label: required_trimmed(
            input
                .completion_label
                .unwrap_or_else(|| "Completion".into()),
            "Completion milestone label",
            80,
        )?,
        start_date,
        due_date,
        milestones,
    })
}

fn load_project(
    connection: &Connection,
    id: &str,
    include_deleted: bool,
) -> Result<Project, AppError> {
    let mut project = connection
        .query_row(
            "SELECT id, client_id, name, description, urls_json, status, currency,
                    quoted_total_minor, kickoff_percent_basis_points,
                    kickoff_label, completion_percent_basis_points,
                    completion_label, start_date, due_date,
                    completed_at, created_at, updated_at, deleted_at
             FROM projects
             WHERE id = ?1 AND (?2 = 1 OR deleted_at IS NULL)",
            params![id, include_deleted],
            project_from_row,
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("Project not found.".into()))?;
    project.milestones = load_project_milestones(connection, id)?;
    Ok(project)
}

fn project_from_row(row: &Row<'_>) -> rusqlite::Result<Project> {
    let urls_json: String = row.get(4)?;
    let urls = serde_json::from_str(&urls_json).unwrap_or_default();
    Ok(Project {
        id: row.get(0)?,
        client_id: row.get(1)?,
        name: row.get(2)?,
        description: row.get(3)?,
        urls,
        status: row.get(5)?,
        currency: row.get(6)?,
        quoted_total_minor: row.get(7)?,
        kickoff_percent_basis_points: row.get(8)?,
        kickoff_label: row.get(9)?,
        completion_percent_basis_points: row.get(10)?,
        completion_label: row.get(11)?,
        start_date: row.get(12)?,
        due_date: row.get(13)?,
        completed_at: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
        deleted_at: row.get(17)?,
        milestones: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// Completed work entries

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkEntry {
    id: String,
    project_id: Option<String>,
    title: String,
    details: Option<String>,
    work_date: String,
    urls: Vec<String>,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkEntryInput {
    project_id: Option<String>,
    title: String,
    details: Option<String>,
    work_date: String,
    urls: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkEntryListFilter {
    project_id: Option<String>,
    from_date: Option<String>,
    to_date: Option<String>,
    include_deleted: Option<bool>,
}

#[tauri::command]
pub(crate) fn list_work_entries(
    database: State<'_, Database>,
    filter: Option<WorkEntryListFilter>,
) -> Result<Vec<WorkEntry>, AppError> {
    let filter = filter.unwrap_or_default();
    let from_date = validate_optional_date(filter.from_date, "fromDate")?;
    let to_date = validate_optional_date(filter.to_date, "toDate")?;
    validate_date_order(
        from_date.as_deref(),
        to_date.as_deref(),
        "Work entry filter",
    )?;
    let connection = database.lock()?;
    let mut statement = connection.prepare(
        "SELECT id, project_id, title, details, work_date, urls_json,
                created_at, updated_at, deleted_at
         FROM work_entries
         WHERE (?1 IS NULL OR project_id = ?1)
           AND (?2 IS NULL OR work_date >= ?2)
           AND (?3 IS NULL OR work_date <= ?3)
           AND (?4 = 1 OR deleted_at IS NULL)
         ORDER BY work_date DESC, created_at DESC",
    )?;
    let rows = statement.query_map(
        params![
            filter.project_id,
            from_date,
            to_date,
            filter.include_deleted.unwrap_or(false)
        ],
        work_entry_from_row,
    )?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

#[tauri::command]
pub(crate) fn create_work_entry(
    database: State<'_, Database>,
    input: WorkEntryInput,
) -> Result<WorkEntry, AppError> {
    let connection = database.lock()?;
    let values = validate_work_entry(&connection, input)?;
    let id = Uuid::new_v4().to_string();
    let now = utc_now();
    connection.execute(
        "INSERT INTO work_entries (
            id, project_id, title, details, work_date, urls_json,
            created_at, updated_at, deleted_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, NULL)",
        params![
            id,
            values.project_id,
            values.title,
            values.details,
            values.work_date,
            values.urls_json,
            now
        ],
    )?;
    load_work_entry(&connection, &id, false)
}

#[tauri::command]
pub(crate) fn update_work_entry(
    database: State<'_, Database>,
    id: String,
    input: WorkEntryInput,
) -> Result<WorkEntry, AppError> {
    let connection = database.lock()?;
    let id = required_trimmed(id, "Work entry ID", 64)?;
    let values = validate_work_entry(&connection, input)?;
    let changed = connection.execute(
        "UPDATE work_entries
         SET project_id = ?2,
             title = ?3,
             details = ?4,
             work_date = ?5,
             urls_json = ?6,
             updated_at = ?7
         WHERE id = ?1 AND deleted_at IS NULL",
        params![
            id,
            values.project_id,
            values.title,
            values.details,
            values.work_date,
            values.urls_json,
            utc_now()
        ],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound("Work entry not found.".into()));
    }
    load_work_entry(&connection, &id, false)
}

struct ValidatedWorkEntry {
    project_id: Option<String>,
    title: String,
    details: Option<String>,
    work_date: String,
    urls_json: String,
}

fn validate_work_entry(
    connection: &Connection,
    input: WorkEntryInput,
) -> Result<ValidatedWorkEntry, AppError> {
    let project_id = optional_trimmed(input.project_id, 64, "Project ID")?;
    ensure_project(connection, project_id.as_deref())?;
    let title = required_trimmed(input.title, "Work title", 240)?;
    let details = optional_trimmed(input.details, 20_000, "Work details")?;
    let work_date = validate_optional_date(Some(input.work_date), "workDate")?
        .ok_or_else(|| AppError::InvalidInput("workDate is required.".into()))?;
    let urls = validate_urls(input.urls.unwrap_or_default())?;
    let urls_json = serde_json::to_string(&urls)
        .map_err(|error| AppError::State(format!("Could not serialize work URLs: {error}")))?;
    Ok(ValidatedWorkEntry {
        project_id,
        title,
        details,
        work_date,
        urls_json,
    })
}

fn load_work_entry(
    connection: &Connection,
    id: &str,
    include_deleted: bool,
) -> Result<WorkEntry, AppError> {
    connection
        .query_row(
            "SELECT id, project_id, title, details, work_date, urls_json,
                    created_at, updated_at, deleted_at
             FROM work_entries
             WHERE id = ?1 AND (?2 = 1 OR deleted_at IS NULL)",
            params![id, include_deleted],
            work_entry_from_row,
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("Work entry not found.".into()))
}

fn work_entry_from_row(row: &Row<'_>) -> rusqlite::Result<WorkEntry> {
    let urls_json: String = row.get(5)?;
    let urls = serde_json::from_str(&urls_json).unwrap_or_default();
    Ok(WorkEntry {
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        details: row.get(3)?,
        work_date: row.get(4)?,
        urls,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        deleted_at: row.get(8)?,
    })
}

// ---------------------------------------------------------------------------
// Shared validation

fn validate_currency(value: &str) -> Result<String, AppError> {
    let currency = value.trim().to_ascii_uppercase();
    if currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(AppError::InvalidInput(
            "currency must be a three-letter ISO-style code such as USD.".into(),
        ));
    }
    Ok(currency)
}

fn validate_project_status(value: &str) -> Result<String, AppError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "draft" => Ok("draft".into()),
        "active" => Ok("active".into()),
        "completed" => Ok("completed".into()),
        "archived" => Ok("archived".into()),
        _ => Err(AppError::InvalidInput(
            "status must be draft, active, completed, or archived.".into(),
        )),
    }
}

fn nonnegative_minor(value: i64, field: &str) -> Result<i64, AppError> {
    if value < 0 {
        return Err(AppError::InvalidInput(format!(
            "{field} cannot be negative."
        )));
    }
    Ok(value)
}

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
         AND ({})",
        if keep_ids.is_empty() {
            "1=1".to_string()
        } else {
            format!("id NOT IN ({placeholders})")
        }
    );
    let mut args: Vec<&dyn rusqlite::ToSql> = vec![&now, &now, &project_id];
    for id in &keep_ids {
        args.push(id);
    }
    connection.execute(&sql, args.as_slice())?;

    for m in milestones {
        match &m.id {
            Some(id) => {
                let changed = connection.execute(
                    "UPDATE project_milestones SET label=?2, amount_minor=?3, kind=?4, sort_order=?5, updated_at=?6 \
                     WHERE id=?1 AND project_id=?7 AND deleted_at IS NULL",
                    params![id, m.label, m.amount_minor, m.kind, m.sort_order, now, project_id],
                )?;
                if changed == 0 {
                    return Err(AppError::NotFound(
                        "Milestone not found for this project.".into(),
                    ));
                }
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

fn load_project_milestones(
    connection: &Connection,
    project_id: &str,
) -> Result<Vec<Milestone>, AppError> {
    // Single query per project: the correlated subquery resolves each
    // milestone's latest active (non-void, non-deleted) invoice inline via
    // the LEFT JOIN, rather than issuing one query per milestone from Rust.
    // That keeps list_projects (which calls this once per project) at O(1)
    // queries per project instead of O(milestones).
    // Defense in depth: even though invoice creation/update already keeps
    // milestone_id pointed at a milestone in the invoice's own project, the
    // join here also requires the invoice's project_id to match the
    // milestone's project_id. That way a row that somehow ends up with a
    // milestone_id belonging to a different project (e.g. a bug elsewhere)
    // can never colour this milestone's status.
    let mut stmt = connection.prepare(
        "SELECT m.id, m.label, m.amount_minor, m.kind, m.sort_order, i.status
         FROM project_milestones m
         LEFT JOIN invoices i ON i.id = (
             SELECT id FROM invoices
             WHERE milestone_id = m.id
               AND project_id = m.project_id
               AND deleted_at IS NULL AND status <> 'void'
             ORDER BY created_at DESC LIMIT 1
         )
         WHERE m.project_id = ?1 AND m.deleted_at IS NULL
         ORDER BY m.sort_order ASC, m.created_at ASC",
    )?;
    let rows = stmt.query_map(params![project_id], |row| {
        let invoice_status: Option<String> = row.get(5)?;
        let status = match invoice_status.as_deref() {
            None => "not-invoiced",
            Some("paid") => "paid",
            Some(_) => "invoiced",
        };
        Ok(Milestone {
            id: row.get(0)?,
            label: row.get(1)?,
            amount_minor: row.get(2)?,
            kind: row.get(3)?,
            sort_order: row.get(4)?,
            status: status.to_string(),
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

fn validate_date_order(
    start: Option<&str>,
    end: Option<&str>,
    context: &str,
) -> Result<(), AppError> {
    if let (Some(start), Some(end)) = (start, end) {
        if end < start {
            return Err(AppError::InvalidInput(format!(
                "{context} end date cannot be earlier than its start date."
            )));
        }
    }
    Ok(())
}

fn ensure_client(connection: &Connection, client_id: Option<&str>) -> Result<(), AppError> {
    let Some(client_id) = client_id else {
        return Ok(());
    };
    let exists: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM clients WHERE id = ?1 AND deleted_at IS NULL
        )",
        params![client_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(AppError::InvalidInput(
            "clientId does not identify an active client.".into(),
        ));
    }
    Ok(())
}

fn ensure_project(connection: &Connection, project_id: Option<&str>) -> Result<(), AppError> {
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

fn currency_for_client_or_default(
    connection: &Connection,
    client_id: Option<&str>,
) -> Result<String, AppError> {
    if let Some(client_id) = client_id {
        return connection
            .query_row(
                "SELECT currency FROM clients
                 WHERE id = ?1 AND deleted_at IS NULL",
                params![client_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| {
                AppError::InvalidInput("clientId does not identify an active client.".into())
            });
    }
    connection
        .query_row(
            "SELECT default_currency FROM app_settings WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .map_err(AppError::from)
}

fn validate_urls(values: Vec<String>) -> Result<Vec<String>, AppError> {
    if values.len() > 50 {
        return Err(AppError::InvalidInput(
            "A record can contain at most 50 URLs.".into(),
        ));
    }

    let mut urls = Vec::with_capacity(values.len());
    for value in values {
        let url = value.trim();
        if url.is_empty() {
            continue;
        }
        if url.chars().count() > 2_048 {
            return Err(AppError::InvalidInput(
                "Each URL must be no longer than 2048 characters.".into(),
            ));
        }
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(AppError::InvalidInput(
                "URLs must begin with https:// or http://.".into(),
            ));
        }
        urls.push(url.to_owned());
    }
    Ok(urls)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn currencies_and_work_urls_are_normalized() {
        assert_eq!(validate_currency(" usd ").unwrap(), "USD");
        assert!(validate_currency("US").is_err());
        assert_eq!(
            validate_urls(vec![" https://example.com/work ".into()]).unwrap(),
            vec!["https://example.com/work"]
        );
        assert!(validate_urls(vec!["file:///private/path".into()]).is_err());
    }

    #[test]
    fn project_dates_must_be_ordered() {
        assert!(validate_date_order(Some("2026-07-23"), Some("2026-08-23"), "Project").is_ok());
        assert!(validate_date_order(Some("2026-08-23"), Some("2026-07-23"), "Project").is_err());
    }

    #[test]
    fn editing_client_and_project_preserves_issued_invoice_snapshots() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE clients (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    company_name TEXT,
                    email TEXT,
                    billing_address TEXT,
                    currency TEXT NOT NULL,
                    notes TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    deleted_at TEXT
                );
                CREATE TABLE projects (
                    id TEXT PRIMARY KEY,
                    client_id TEXT,
                    name TEXT NOT NULL,
                    description TEXT,
                    urls_json TEXT NOT NULL DEFAULT '[]',
                    status TEXT NOT NULL,
                    currency TEXT NOT NULL,
                    quoted_total_minor INTEGER NOT NULL,
                    kickoff_percent_basis_points INTEGER NOT NULL,
                    kickoff_label TEXT NOT NULL,
                    completion_percent_basis_points INTEGER NOT NULL,
                    completion_label TEXT NOT NULL,
                    start_date TEXT,
                    due_date TEXT,
                    completed_at TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    deleted_at TEXT
                );
                CREATE TABLE invoices (
                    id TEXT PRIMARY KEY,
                    project_id TEXT NOT NULL,
                    project_name TEXT NOT NULL,
                    bill_to_name TEXT NOT NULL,
                    milestone_label TEXT,
                    milestone_id TEXT,
                    status TEXT NOT NULL DEFAULT 'issued',
                    created_at TEXT NOT NULL DEFAULT '2026-07-23T00:00:00Z',
                    deleted_at TEXT
                );
                CREATE TABLE project_milestones (
                    id TEXT PRIMARY KEY,
                    project_id TEXT NOT NULL,
                    label TEXT NOT NULL,
                    amount_minor INTEGER NOT NULL DEFAULT 0,
                    kind TEXT NOT NULL DEFAULT 'custom',
                    sort_order INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    deleted_at TEXT
                );
                INSERT INTO clients (
                    id, name, company_name, email, billing_address, currency,
                    notes, created_at, updated_at, deleted_at
                ) VALUES (
                    'client-1', 'Original Client', NULL, NULL, NULL, 'USD',
                    NULL, '2026-07-23T00:00:00Z', '2026-07-23T00:00:00Z', NULL
                );
                INSERT INTO projects (
                    id, client_id, name, description, urls_json, status, currency,
                    quoted_total_minor, kickoff_percent_basis_points, kickoff_label,
                    completion_percent_basis_points, completion_label, start_date,
                    due_date, completed_at, created_at, updated_at, deleted_at
                ) VALUES (
                    'project-1', 'client-1', 'Original Project', NULL, '[]',
                    'active', 'USD', 100000, 5000, 'Kickoff', 5000,
                    'Completion', '2026-07-23', '2026-08-23', NULL,
                    '2026-07-23T00:00:00Z', '2026-07-23T00:00:00Z', NULL
                );
                INSERT INTO invoices (
                    id, project_id, project_name, bill_to_name, milestone_label
                ) VALUES (
                    'invoice-1', 'project-1', 'Original Project',
                    'Original Client', 'Kickoff'
                );",
            )
            .unwrap();

        update_client_in_connection(
            &connection,
            "client-1",
            CreateClientInput {
                name: "Renamed Client".into(),
                company_name: Some("New Company".into()),
                email: Some("new@example.com".into()),
                billing_address: None,
                currency: Some("USD".into()),
                notes: Some("Updated notes".into()),
            },
        )
        .unwrap();
        let updated_project = update_project_in_connection(
            &connection,
            "project-1",
            CreateProjectInput {
                client_id: Some("client-1".into()),
                name: "Renamed Project".into(),
                description: Some("Updated notes".into()),
                urls: Some(vec!["https://example.com/project".into()]),
                status: Some("completed".into()),
                currency: Some("USD".into()),
                quoted_total_minor: 125000,
                kickoff_percent_basis_points: Some(4000),
                kickoff_label: Some("Deposit".into()),
                completion_percent_basis_points: Some(6000),
                completion_label: Some("Final delivery".into()),
                start_date: Some("2026-07-24".into()),
                due_date: Some("2026-08-24".into()),
                milestones: None,
            },
        )
        .unwrap();

        assert_eq!(updated_project.name, "Renamed Project");
        assert_eq!(updated_project.status, "completed");
        assert_eq!(updated_project.urls, vec!["https://example.com/project"]);
        let invoice_snapshot: (String, String, String) = connection
            .query_row(
                "SELECT project_name, bill_to_name, milestone_label
                 FROM invoices WHERE id = 'invoice-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            invoice_snapshot,
            (
                "Original Project".into(),
                "Original Client".into(),
                "Kickoff".into()
            )
        );
    }

    fn milestones_test_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE projects (
                    id TEXT PRIMARY KEY,
                    client_id TEXT,
                    name TEXT NOT NULL,
                    description TEXT,
                    urls_json TEXT NOT NULL DEFAULT '[]',
                    status TEXT NOT NULL,
                    currency TEXT NOT NULL,
                    quoted_total_minor INTEGER NOT NULL,
                    kickoff_percent_basis_points INTEGER NOT NULL,
                    kickoff_label TEXT NOT NULL,
                    completion_percent_basis_points INTEGER NOT NULL,
                    completion_label TEXT NOT NULL,
                    start_date TEXT,
                    due_date TEXT,
                    completed_at TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    deleted_at TEXT
                );
                CREATE TABLE project_milestones (
                    id TEXT PRIMARY KEY,
                    project_id TEXT NOT NULL,
                    label TEXT NOT NULL,
                    amount_minor INTEGER NOT NULL DEFAULT 0,
                    kind TEXT NOT NULL DEFAULT 'custom',
                    sort_order INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    deleted_at TEXT
                );
                CREATE TABLE invoices (
                    id TEXT PRIMARY KEY,
                    project_id TEXT NOT NULL,
                    milestone_id TEXT,
                    status TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    deleted_at TEXT
                );",
            )
            .unwrap();
        connection
    }

    fn milestone_project_input(milestones: Option<Vec<MilestoneInput>>) -> CreateProjectInput {
        CreateProjectInput {
            client_id: None,
            name: "Milestone Project".into(),
            description: None,
            urls: None,
            status: Some("active".into()),
            currency: Some("USD".into()),
            quoted_total_minor: 30_000,
            kickoff_percent_basis_points: None,
            kickoff_label: None,
            completion_percent_basis_points: None,
            completion_label: None,
            start_date: None,
            due_date: None,
            milestones,
        }
    }

    #[test]
    fn creating_project_with_milestones_persists_three_rows() {
        let connection = milestones_test_connection();
        let project = create_project_in_connection(
            &connection,
            milestone_project_input(Some(vec![
                MilestoneInput {
                    id: None,
                    label: "Kickoff".into(),
                    amount_minor: 10_000,
                    kind: Some("kickoff".into()),
                    sort_order: Some(0),
                },
                MilestoneInput {
                    id: None,
                    label: "Midpoint".into(),
                    amount_minor: 10_000,
                    kind: Some("phase".into()),
                    sort_order: Some(1),
                },
                MilestoneInput {
                    id: None,
                    label: "Completion".into(),
                    amount_minor: 10_000,
                    kind: Some("completion".into()),
                    sort_order: Some(2),
                },
            ])),
        )
        .unwrap();

        assert_eq!(project.milestones.len(), 3);
        let row_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM project_milestones
                 WHERE project_id = ?1 AND deleted_at IS NULL",
                params![project.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(row_count, 3);
    }

    #[test]
    fn updating_project_to_remove_a_milestone_soft_deletes_it() {
        let connection = milestones_test_connection();
        let project = create_project_in_connection(
            &connection,
            milestone_project_input(Some(vec![
                MilestoneInput {
                    id: None,
                    label: "Kickoff".into(),
                    amount_minor: 15_000,
                    kind: Some("kickoff".into()),
                    sort_order: Some(0),
                },
                MilestoneInput {
                    id: None,
                    label: "Completion".into(),
                    amount_minor: 15_000,
                    kind: Some("completion".into()),
                    sort_order: Some(1),
                },
            ])),
        )
        .unwrap();
        assert_eq!(project.milestones.len(), 2);
        let kept_id = project.milestones[0].id.clone();

        let updated = update_project_in_connection(
            &connection,
            &project.id,
            milestone_project_input(Some(vec![MilestoneInput {
                id: Some(kept_id.clone()),
                label: "Kickoff".into(),
                amount_minor: 15_000,
                kind: Some("kickoff".into()),
                sort_order: Some(0),
            }])),
        )
        .unwrap();

        assert_eq!(updated.milestones.len(), 1);
        assert_eq!(updated.milestones[0].id, kept_id);
        let deleted_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM project_milestones
                 WHERE project_id = ?1 AND deleted_at IS NOT NULL",
                params![project.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(deleted_count, 1);
    }

    #[test]
    fn load_project_returns_milestones_ordered_by_sort_order() {
        let connection = milestones_test_connection();
        let project = create_project_in_connection(
            &connection,
            milestone_project_input(Some(vec![
                MilestoneInput {
                    id: None,
                    label: "Second".into(),
                    amount_minor: 5_000,
                    kind: None,
                    sort_order: Some(1),
                },
                MilestoneInput {
                    id: None,
                    label: "First".into(),
                    amount_minor: 5_000,
                    kind: None,
                    sort_order: Some(0),
                },
            ])),
        )
        .unwrap();

        let loaded = load_project(&connection, &project.id, false).unwrap();
        let labels: Vec<&str> = loaded
            .milestones
            .iter()
            .map(|m| m.label.as_str())
            .collect();
        assert_eq!(labels, vec!["First", "Second"]);
        assert!(loaded.milestones.iter().all(|m| m.status == "not-invoiced"));
    }

    #[test]
    fn milestone_amounts_sum_to_project_total() {
        let connection = milestones_test_connection();
        let project = create_project_in_connection(
            &connection,
            milestone_project_input(Some(vec![
                MilestoneInput {
                    id: None,
                    label: "Kickoff".into(),
                    amount_minor: 12_000,
                    kind: Some("kickoff".into()),
                    sort_order: Some(0),
                },
                MilestoneInput {
                    id: None,
                    label: "Phase".into(),
                    amount_minor: 8_000,
                    kind: Some("phase".into()),
                    sort_order: Some(1),
                },
                MilestoneInput {
                    id: None,
                    label: "Completion".into(),
                    amount_minor: 10_000,
                    kind: Some("completion".into()),
                    sort_order: Some(2),
                },
            ])),
        )
        .unwrap();

        let sum: i64 = project.milestones.iter().map(|m| m.amount_minor).sum();
        assert_eq!(sum, 30_000);
    }

    #[test]
    fn updating_project_with_unresolvable_milestone_id_returns_error() {
        let connection = milestones_test_connection();
        let project = create_project_in_connection(
            &connection,
            milestone_project_input(Some(vec![MilestoneInput {
                id: None,
                label: "Kickoff".into(),
                amount_minor: 30_000,
                kind: Some("kickoff".into()),
                sort_order: Some(0),
            }])),
        )
        .unwrap();

        let result = update_project_in_connection(
            &connection,
            &project.id,
            milestone_project_input(Some(vec![MilestoneInput {
                id: Some("does-not-exist".into()),
                label: "Kickoff".into(),
                amount_minor: 30_000,
                kind: Some("kickoff".into()),
                sort_order: Some(0),
            }])),
        );

        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[test]
    fn failed_milestone_sync_rolls_back_entire_project_update_in_a_transaction() {
        let mut connection = milestones_test_connection();
        let project = create_project_in_connection(
            &connection,
            milestone_project_input(Some(vec![MilestoneInput {
                id: None,
                label: "Kickoff".into(),
                amount_minor: 30_000,
                kind: Some("kickoff".into()),
                sort_order: Some(0),
            }])),
        )
        .unwrap();
        let existing_milestone_id = project.milestones[0].id.clone();

        // Exercise the exact helper that the `update_project` tauri command
        // delegates to. If the transaction were ever removed from
        // `update_project_atomic`, this call would still return an error
        // (correct), but the partial writes made before the error would
        // stick, and the assertions below would fail.
        let result = update_project_atomic(
            &mut connection,
            &project.id,
            milestone_project_input(Some(vec![MilestoneInput {
                id: Some("does-not-exist".into()),
                label: "Kickoff".into(),
                amount_minor: 30_000,
                kind: Some("kickoff".into()),
                sort_order: Some(0),
            }])),
        );
        assert!(matches!(result, Err(AppError::NotFound(_))));

        let reloaded = load_project(&connection, &project.id, false).unwrap();
        assert_eq!(reloaded.milestones.len(), 1);
        assert_eq!(reloaded.milestones[0].id, existing_milestone_id);

        let deleted_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM project_milestones
                 WHERE project_id = ?1 AND deleted_at IS NOT NULL",
                params![project.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            deleted_count, 0,
            "the pre-existing milestone must not have been soft-deleted by the rolled-back update"
        );
    }

    #[test]
    fn milestone_status_tracks_its_active_invoice_lifecycle() {
        let connection = milestones_test_connection();
        let project = create_project_in_connection(
            &connection,
            milestone_project_input(Some(vec![MilestoneInput {
                id: None,
                label: "Kickoff".into(),
                amount_minor: 30_000,
                kind: Some("kickoff".into()),
                sort_order: Some(0),
            }])),
        )
        .unwrap();
        let milestone_id = project.milestones[0].id.clone();

        // No invoice yet.
        let reloaded = load_project(&connection, &project.id, false).unwrap();
        assert_eq!(reloaded.milestones[0].status, "not-invoiced");

        // Issued invoice -> invoiced.
        connection
            .execute(
                "INSERT INTO invoices (id, project_id, milestone_id, status, created_at, deleted_at)
                 VALUES ('invoice-1', ?1, ?2, 'issued', '2026-07-23T00:00:00Z', NULL)",
                params![project.id, milestone_id],
            )
            .unwrap();
        let reloaded = load_project(&connection, &project.id, false).unwrap();
        assert_eq!(reloaded.milestones[0].status, "invoiced");

        // Paid invoice -> paid.
        connection
            .execute(
                "UPDATE invoices SET status = 'paid' WHERE id = 'invoice-1'",
                [],
            )
            .unwrap();
        let reloaded = load_project(&connection, &project.id, false).unwrap();
        assert_eq!(reloaded.milestones[0].status, "paid");

        // Voided invoice -> back to not-invoiced.
        connection
            .execute(
                "UPDATE invoices SET status = 'void' WHERE id = 'invoice-1'",
                [],
            )
            .unwrap();
        let reloaded = load_project(&connection, &project.id, false).unwrap();
        assert_eq!(reloaded.milestones[0].status, "not-invoiced");
    }

    #[test]
    fn milestone_status_ignores_an_invoice_whose_project_id_no_longer_matches_the_milestone() {
        // Simulates the state a stale link would leave behind: an invoice
        // row whose milestone_id still points at milestone A, but whose
        // project_id has since moved to project B (e.g. the invoice's draft
        // was reassigned to a different project). Even though the DB-level
        // milestone_id column literally matches, the invoice no longer
        // belongs to milestone A's project, so it must not colour milestone
        // A's status. This is the defense-in-depth join condition, exercised
        // directly against a mismatched row rather than through the
        // finance.rs update path that normally prevents this state from
        // occurring in the first place.
        let connection = milestones_test_connection();
        let project_a = create_project_in_connection(
            &connection,
            milestone_project_input(Some(vec![MilestoneInput {
                id: None,
                label: "Kickoff".into(),
                amount_minor: 30_000,
                kind: Some("kickoff".into()),
                sort_order: Some(0),
            }])),
        )
        .unwrap();
        let milestone_a_id = project_a.milestones[0].id.clone();

        let project_b = create_project_in_connection(&connection, milestone_project_input(None))
            .unwrap();

        connection
            .execute(
                "INSERT INTO invoices (id, project_id, milestone_id, status, created_at, deleted_at)
                 VALUES ('invoice-1', ?1, ?2, 'draft', '2026-07-23T00:00:00Z', NULL)",
                params![project_b.id, milestone_a_id],
            )
            .unwrap();

        let reloaded = load_project(&connection, &project_a.id, false).unwrap();
        assert_eq!(
            reloaded.milestones[0].status, "not-invoiced",
            "an invoice belonging to a different project must not mark this milestone as invoiced"
        );
    }
}
