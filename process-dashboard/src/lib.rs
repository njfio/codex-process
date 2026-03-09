use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, FromRow, SqlitePool};

#[derive(Clone)]
pub struct AppState {
    pool: SqlitePool,
}

#[derive(Debug)]
enum AppError {
    BadRequest(String),
    NotFound(String),
    Db(sqlx::Error),
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        Self::Db(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg).into_response(),
            Self::Db(err) => {
                let message = format!("database error: {err}");
                (StatusCode::INTERNAL_SERVER_ERROR, message).into_response()
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Repo {
    pub id: i64,
    pub identifier: String,
    pub mode: String,
    pub policy_profile: Option<String>,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    pub id: i64,
    pub repo_id: Option<i64>,
    pub kind: String,
    pub interval_minutes: Option<i64>,
    pub cron: Option<String>,
    pub target: String,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRepoRequest {
    pub identifier: String,
    pub mode: Option<String>,
    pub policy_profile: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchRepoRequest {
    pub identifier: Option<String>,
    pub mode: Option<String>,
    pub policy_profile: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateScheduleRequest {
    pub repo_id: Option<i64>,
    pub kind: String,
    pub interval_minutes: Option<i64>,
    pub cron: Option<String>,
    pub target: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchScheduleRequest {
    pub repo_id: Option<i64>,
    pub kind: Option<String>,
    pub interval_minutes: Option<i64>,
    pub cron: Option<String>,
    pub target: Option<String>,
    pub enabled: Option<bool>,
}

pub async fn build_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;

    run_migrations(&pool).await?;

    Ok(pool)
}

pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::migrate!("./migrations").run(pool).await
}

pub fn app_router(pool: SqlitePool) -> Router {
    let state = AppState { pool };

    Router::new()
        .route("/health", get(health))
        .route("/api/repos", get(list_repos).post(create_repo))
        .route("/api/repos/:id", patch(update_repo))
        .route("/api/schedules", get(list_schedules).post(create_schedule))
        .route("/api/schedules/:id", patch(update_schedule))
        .with_state(state)
}

async fn health() -> StatusCode {
    StatusCode::OK
}

async fn list_repos(State(state): State<AppState>) -> Result<Json<Vec<Repo>>, AppError> {
    let repos = sqlx::query_as::<_, Repo>(
        "SELECT id, identifier, mode, policy_profile, enabled, created_at, updated_at FROM repos ORDER BY id",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(repos))
}

async fn create_repo(
    State(state): State<AppState>,
    Json(payload): Json<CreateRepoRequest>,
) -> Result<(StatusCode, Json<Repo>), AppError> {
    validate_repo_identifier(&payload.identifier)?;

    let repo = sqlx::query_as::<_, Repo>(
        "INSERT INTO repos (identifier, mode, policy_profile, enabled)
         VALUES (?1, ?2, ?3, ?4)
         RETURNING id, identifier, mode, policy_profile, enabled, created_at, updated_at",
    )
    .bind(payload.identifier)
    .bind(payload.mode.unwrap_or_else(|| "observe-only".to_string()))
    .bind(payload.policy_profile)
    .bind(payload.enabled.unwrap_or(true))
    .fetch_one(&state.pool)
    .await
    .map_err(map_sqlite_error)?;

    Ok((StatusCode::CREATED, Json(repo)))
}

async fn update_repo(
    Path(id): Path<i64>,
    State(state): State<AppState>,
    Json(payload): Json<PatchRepoRequest>,
) -> Result<Json<Repo>, AppError> {
    let existing = find_repo_by_id(&state.pool, id).await?;

    if let Some(identifier) = payload.identifier.as_deref() {
        validate_repo_identifier(identifier)?;
    }

    let repo = sqlx::query_as::<_, Repo>(
        "UPDATE repos
         SET identifier = ?1,
             mode = ?2,
             policy_profile = ?3,
             enabled = ?4,
             updated_at = strftime('%s','now')
         WHERE id = ?5
         RETURNING id, identifier, mode, policy_profile, enabled, created_at, updated_at",
    )
    .bind(payload.identifier.unwrap_or(existing.identifier))
    .bind(payload.mode.unwrap_or(existing.mode))
    .bind(payload.policy_profile.or(existing.policy_profile))
    .bind(payload.enabled.unwrap_or(existing.enabled))
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .map_err(map_sqlite_error)?;

    Ok(Json(repo))
}

async fn list_schedules(State(state): State<AppState>) -> Result<Json<Vec<Schedule>>, AppError> {
    let schedules = sqlx::query_as::<_, Schedule>(
        "SELECT id, repo_id, kind, interval_minutes, cron, target, enabled, created_at, updated_at
         FROM schedules ORDER BY id",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(schedules))
}

async fn create_schedule(
    State(state): State<AppState>,
    Json(payload): Json<CreateScheduleRequest>,
) -> Result<(StatusCode, Json<Schedule>), AppError> {
    validate_schedule(
        payload.kind.as_str(),
        payload.interval_minutes,
        payload.cron.as_deref(),
    )?;

    if let Some(repo_id) = payload.repo_id {
        find_repo_by_id(&state.pool, repo_id).await?;
    }

    let schedule = sqlx::query_as::<_, Schedule>(
        "INSERT INTO schedules (repo_id, kind, interval_minutes, cron, target, enabled)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         RETURNING id, repo_id, kind, interval_minutes, cron, target, enabled, created_at, updated_at",
    )
    .bind(payload.repo_id)
    .bind(payload.kind)
    .bind(payload.interval_minutes)
    .bind(payload.cron)
    .bind(payload.target.unwrap_or_else(|| "pr_comment_watcher".to_string()))
    .bind(payload.enabled.unwrap_or(true))
    .fetch_one(&state.pool)
    .await
    .map_err(map_sqlite_error)?;

    Ok((StatusCode::CREATED, Json(schedule)))
}

async fn update_schedule(
    Path(id): Path<i64>,
    State(state): State<AppState>,
    Json(payload): Json<PatchScheduleRequest>,
) -> Result<Json<Schedule>, AppError> {
    let existing = find_schedule_by_id(&state.pool, id).await?;

    let repo_id = payload.repo_id.or(existing.repo_id);
    if let Some(repo_id) = repo_id {
        find_repo_by_id(&state.pool, repo_id).await?;
    }

    let kind = payload.kind.unwrap_or(existing.kind);
    let interval_minutes = payload.interval_minutes.or(existing.interval_minutes);
    let cron = payload.cron.or(existing.cron);

    validate_schedule(kind.as_str(), interval_minutes, cron.as_deref())?;

    let schedule = sqlx::query_as::<_, Schedule>(
        "UPDATE schedules
         SET repo_id = ?1,
             kind = ?2,
             interval_minutes = ?3,
             cron = ?4,
             target = ?5,
             enabled = ?6,
             updated_at = strftime('%s','now')
         WHERE id = ?7
         RETURNING id, repo_id, kind, interval_minutes, cron, target, enabled, created_at, updated_at",
    )
    .bind(repo_id)
    .bind(kind)
    .bind(interval_minutes)
    .bind(cron)
    .bind(payload.target.unwrap_or(existing.target))
    .bind(payload.enabled.unwrap_or(existing.enabled))
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .map_err(map_sqlite_error)?;

    Ok(Json(schedule))
}

async fn find_repo_by_id(pool: &SqlitePool, id: i64) -> Result<Repo, AppError> {
    let maybe_repo = sqlx::query_as::<_, Repo>(
        "SELECT id, identifier, mode, policy_profile, enabled, created_at, updated_at
         FROM repos WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    maybe_repo.ok_or_else(|| AppError::NotFound(format!("repo not found: {id}")))
}

async fn find_schedule_by_id(pool: &SqlitePool, id: i64) -> Result<Schedule, AppError> {
    let maybe_schedule = sqlx::query_as::<_, Schedule>(
        "SELECT id, repo_id, kind, interval_minutes, cron, target, enabled, created_at, updated_at
         FROM schedules WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    maybe_schedule.ok_or_else(|| AppError::NotFound(format!("schedule not found: {id}")))
}

fn map_sqlite_error(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        let message = db_err.message();
        if message.contains("UNIQUE constraint failed") {
            return AppError::BadRequest("resource already exists".to_string());
        }
        if message.contains("FOREIGN KEY constraint failed") {
            return AppError::BadRequest("referenced resource does not exist".to_string());
        }
    }

    AppError::Db(err)
}

fn validate_repo_identifier(identifier: &str) -> Result<(), AppError> {
    let Some((owner, name)) = identifier.split_once('/') else {
        return Err(AppError::BadRequest(
            "repo identifier must use owner/name format".to_string(),
        ));
    };

    if owner.is_empty() || name.is_empty() {
        return Err(AppError::BadRequest(
            "repo identifier must use owner/name format".to_string(),
        ));
    }

    let valid_part = |part: &str| {
        part.chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    };

    if !valid_part(owner) || !valid_part(name) {
        return Err(AppError::BadRequest(
            "repo identifier may only include letters, digits, _, -, .".to_string(),
        ));
    }

    Ok(())
}

fn validate_schedule(
    kind: &str,
    interval_minutes: Option<i64>,
    cron: Option<&str>,
) -> Result<(), AppError> {
    match kind {
        "interval" => {
            let Some(minutes) = interval_minutes else {
                return Err(AppError::BadRequest(
                    "intervalMinutes is required for interval schedules".to_string(),
                ));
            };
            if minutes <= 0 {
                return Err(AppError::BadRequest(
                    "intervalMinutes must be greater than 0".to_string(),
                ));
            }
        }
        "cron" => {
            if cron.is_some_and(|expression| expression.trim().is_empty()) {
                return Err(AppError::BadRequest(
                    "cron expression cannot be empty".to_string(),
                ));
            }
        }
        _ => {
            return Err(AppError::BadRequest(
                "schedule kind must be one of: interval, cron".to_string(),
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_repo_identifier, validate_schedule};

    #[test]
    fn repo_identifier_validation_works() {
        assert!(validate_repo_identifier("openai/codex").is_ok());
        assert!(validate_repo_identifier("owner/repo.name").is_ok());

        assert!(validate_repo_identifier("missing-slash").is_err());
        assert!(validate_repo_identifier("owner/").is_err());
        assert!(validate_repo_identifier("owner/repo*").is_err());
    }

    #[test]
    fn schedule_validation_works() {
        assert!(validate_schedule("interval", Some(30), None).is_ok());
        assert!(validate_schedule("cron", None, Some("0 */2 * * *")).is_ok());

        assert!(validate_schedule("interval", None, None).is_err());
        assert!(validate_schedule("interval", Some(0), None).is_err());
        assert!(validate_schedule("daily", None, None).is_err());
    }
}
