//! Durable coordination for agents working against an existing Plan.
//!
//! The module owns reconnect identity, journal visibility/idempotency, and
//! leased task claims. Callers never assemble ownership checks or lease SQL.

use fubbik_core::error::{AppError, AppResult};
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, Transaction};

use crate::repo::plan::PlanTask;
use crate::timestamp::UtcTimestamp;

pub const ENTRY_KINDS: [&str; 8] = [
    "note", "question", "answer", "progress", "decision", "handoff", "artifact", "system",
];
pub const RUN_STATUSES: [&str; 3] = ["active", "finished", "abandoned"];
pub const TASK_STATUSES: [&str; 5] = ["pending", "in_progress", "done", "skipped", "blocked"];

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentRun {
    pub id: String,
    pub plan_id: String,
    pub parent_run_id: Option<String>,
    pub handle: String,
    pub external_key: Option<String>,
    pub status: String,
    #[schema(value_type = Vec<String>)]
    pub capabilities: Json<Vec<String>>,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub metadata: Json<serde_json::Value>,
    pub last_ack_sequence: i64,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub last_heartbeat_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

#[derive(Debug, Clone)]
pub struct JoinRun {
    pub handle: String,
    pub parent_run_id: Option<String>,
    pub external_key: Option<String>,
    pub capabilities: Vec<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CoordinationEntry {
    pub id: String,
    pub sequence: i64,
    pub plan_id: String,
    pub task_id: Option<String>,
    pub author_run_id: String,
    pub recipient_run_id: Option<String>,
    pub reply_to_id: Option<String>,
    pub kind: String,
    pub body: String,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub metadata: Json<serde_json::Value>,
    pub client_mutation_id: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

#[derive(Debug, Clone)]
pub struct NewEntry {
    pub task_id: Option<String>,
    pub author_run_id: String,
    pub recipient_run_id: Option<String>,
    pub reply_to_id: Option<String>,
    pub kind: String,
    pub body: String,
    pub metadata: serde_json::Value,
    pub client_mutation_id: String,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskClaim {
    pub task_id: String,
    pub plan_id: String,
    pub agent_run_id: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub claimed_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub lease_expires_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
    pub expired: bool,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BoardPlan {
    pub id: String,
    pub title: String,
    pub status: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BoardTask {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub order: i32,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BoardSnapshot {
    pub plan: BoardPlan,
    pub tasks: Vec<BoardTask>,
    pub runs: Vec<AgentRun>,
    pub claims: Vec<TaskClaim>,
    pub entries: Vec<CoordinationEntry>,
    pub next_sequence: i64,
    pub acknowledged_sequence: Option<i64>,
    pub has_more: bool,
}

const RUN_COLUMNS: &str = "id, plan_id, parent_run_id, handle, external_key, status, capabilities, metadata, last_ack_sequence, last_heartbeat_at, created_at, updated_at";
const RUN_COLUMNS_QUALIFIED: &str = "r.id, r.plan_id, r.parent_run_id, r.handle, r.external_key, r.status, r.capabilities, r.metadata, r.last_ack_sequence, r.last_heartbeat_at, r.created_at, r.updated_at";
const ENTRY_COLUMNS: &str = "id, sequence, plan_id, task_id, author_run_id, recipient_run_id, reply_to_id, kind, body, metadata, client_mutation_id, created_at";

async fn own_plan(pool: &PgPool, user_id: &str, plan_id: &str) -> AppResult<()> {
    let found: Option<i32> =
        sqlx::query_scalar("SELECT 1 FROM plan WHERE id = $1 AND user_id = $2")
            .bind(plan_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
    found
        .map(|_| ())
        .ok_or_else(|| AppError::NotFound("Plan".into()))
}

/// Reads the complete visible board from one repeatable-read transaction.
/// Ownership, direct-message visibility, task enrichment, and cursor
/// semantics stay behind this interface so callers cannot assemble a board
/// from different database moments.
pub async fn read_board(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    run_id: Option<&str>,
    after_sequence: i64,
    limit: i64,
) -> AppResult<BoardSnapshot> {
    if after_sequence < 0 {
        return Err(AppError::Validation(
            "afterSequence cannot be negative".into(),
        ));
    }
    if limit <= 0 {
        return Err(AppError::Validation("limit must be positive".into()));
    }
    let limit = limit.min(500);

    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let plan = sqlx::query_as::<_, BoardPlan>(
        "SELECT id, title, status FROM plan WHERE id = $1 AND user_id = $2",
    )
    .bind(plan_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Plan".into()))?;

    let run = match run_id {
        Some(id) => {
            let sql = format!("SELECT {RUN_COLUMNS} FROM agent_run WHERE id = $1 AND plan_id = $2");
            Some(
                sqlx::query_as::<_, AgentRun>(&sql)
                    .bind(id)
                    .bind(plan_id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .ok_or_else(|| AppError::NotFound("Agent run".into()))?,
            )
        }
        None => None,
    };
    let tasks = sqlx::query_as::<_, BoardTask>(
        r#"SELECT t.id, t.title, t.description, t.status, t."order",
                  COALESCE(
                    array_agg(d.depends_on_task_id ORDER BY d.created_at, d.id)
                      FILTER (WHERE d.depends_on_task_id IS NOT NULL),
                    ARRAY[]::text[]
                  ) AS depends_on
           FROM plan_task t
           LEFT JOIN plan_task_dependency d ON d.task_id = t.id
           WHERE t.plan_id = $1
           GROUP BY t.id, t.title, t.description, t.status, t."order"
           ORDER BY t."order" ASC, t.id ASC"#,
    )
    .bind(plan_id)
    .fetch_all(&mut *tx)
    .await?;
    let runs_sql = format!(
        "SELECT {RUN_COLUMNS} FROM agent_run WHERE plan_id = $1 ORDER BY created_at ASC, id ASC"
    );
    let runs = sqlx::query_as::<_, AgentRun>(&runs_sql)
        .bind(plan_id)
        .fetch_all(&mut *tx)
        .await?;
    let claims = sqlx::query_as::<_, TaskClaim>(
        "SELECT task_id, plan_id, agent_run_id, claimed_at, lease_expires_at, updated_at, lease_expires_at <= now() AS expired FROM plan_task_claim WHERE plan_id = $1 ORDER BY task_id",
    )
    .bind(plan_id)
    .fetch_all(&mut *tx)
    .await?;
    let entries_sql = format!(
        "SELECT {ENTRY_COLUMNS} FROM coordination_entry WHERE plan_id = $1 AND sequence > $2 AND ($3::text IS NULL OR recipient_run_id IS NULL OR author_run_id = $3 OR recipient_run_id = $3) ORDER BY sequence ASC LIMIT $4"
    );
    let mut entries = sqlx::query_as::<_, CoordinationEntry>(&entries_sql)
        .bind(plan_id)
        .bind(after_sequence)
        .bind(run_id)
        .bind(limit + 1)
        .fetch_all(&mut *tx)
        .await?;
    let has_more = entries.len() > limit as usize;
    if has_more {
        entries.truncate(limit as usize);
    }
    let global_max = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(MAX(sequence), 0) FROM coordination_entry WHERE plan_id = $1",
    )
    .bind(plan_id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    let next_sequence = if has_more {
        entries
            .last()
            .map(|entry| entry.sequence)
            .unwrap_or(after_sequence)
    } else {
        global_max.max(after_sequence)
    };
    Ok(BoardSnapshot {
        plan,
        tasks,
        runs,
        claims,
        entries,
        next_sequence,
        acknowledged_sequence: run.map(|value| value.last_ack_sequence),
        has_more,
    })
}

pub async fn find_run(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    run_id: &str,
) -> AppResult<Option<AgentRun>> {
    let sql = format!(
        "SELECT {RUN_COLUMNS} FROM agent_run r WHERE r.id = $1 AND r.plan_id = $2 AND EXISTS (SELECT 1 FROM plan p WHERE p.id = r.plan_id AND p.user_id = $3)"
    );
    Ok(sqlx::query_as::<_, AgentRun>(&sql)
        .bind(run_id)
        .bind(plan_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?)
}

pub async fn join_run(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    input: JoinRun,
) -> AppResult<AgentRun> {
    own_plan(pool, user_id, plan_id).await?;
    let handle = input.handle.trim();
    if handle.is_empty() {
        return Err(AppError::Validation("Agent handle is required".into()));
    }
    if let Some(parent) = &input.parent_run_id
        && find_run(pool, user_id, plan_id, parent).await?.is_none()
    {
        return Err(AppError::NotFound("Parent agent run".into()));
    }

    if let Some(key) = &input.external_key {
        let sql =
            format!("SELECT {RUN_COLUMNS} FROM agent_run WHERE plan_id = $1 AND external_key = $2");
        if let Some(existing) = sqlx::query_as::<_, AgentRun>(&sql)
            .bind(plan_id)
            .bind(key)
            .fetch_optional(pool)
            .await?
        {
            if existing.handle != handle || existing.parent_run_id != input.parent_run_id {
                return Err(AppError::Conflict(
                    "externalKey already belongs to a different agent identity".into(),
                ));
            }
            return Ok(existing);
        }
    }

    let id = crate::new_id();
    let insert = format!(
        "INSERT INTO agent_run (id, plan_id, parent_run_id, handle, external_key, capabilities, metadata) VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING RETURNING {RUN_COLUMNS}"
    );
    if let Some(created) = sqlx::query_as::<_, AgentRun>(&insert)
        .bind(&id)
        .bind(plan_id)
        .bind(&input.parent_run_id)
        .bind(handle)
        .bind(&input.external_key)
        .bind(Json(input.capabilities.clone()))
        .bind(Json(input.metadata.clone()))
        .fetch_optional(pool)
        .await?
    {
        return Ok(created);
    }

    let key = input
        .external_key
        .as_ref()
        .ok_or_else(|| AppError::Conflict("agent run could not be created".into()))?;
    let sql =
        format!("SELECT {RUN_COLUMNS} FROM agent_run WHERE plan_id = $1 AND external_key = $2");
    let existing = sqlx::query_as::<_, AgentRun>(&sql)
        .bind(plan_id)
        .bind(key)
        .fetch_one(pool)
        .await?;
    if existing.handle != handle || existing.parent_run_id != input.parent_run_id {
        return Err(AppError::Conflict(
            "externalKey already belongs to a different agent identity".into(),
        ));
    }
    Ok(existing)
}

pub async fn list_runs(pool: &PgPool, user_id: &str, plan_id: &str) -> AppResult<Vec<AgentRun>> {
    own_plan(pool, user_id, plan_id).await?;
    let sql = format!(
        "SELECT {RUN_COLUMNS} FROM agent_run WHERE plan_id = $1 ORDER BY created_at ASC, id ASC"
    );
    Ok(sqlx::query_as::<_, AgentRun>(&sql)
        .bind(plan_id)
        .fetch_all(pool)
        .await?)
}

fn same_entry(existing: &CoordinationEntry, input: &NewEntry) -> bool {
    existing.task_id == input.task_id
        && existing.author_run_id == input.author_run_id
        && existing.recipient_run_id == input.recipient_run_id
        && existing.reply_to_id == input.reply_to_id
        && existing.kind == input.kind
        && existing.body == input.body.trim()
        && existing.metadata.0 == input.metadata
}

pub async fn append_entry(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    input: NewEntry,
) -> AppResult<CoordinationEntry> {
    if !ENTRY_KINDS.contains(&input.kind.as_str()) {
        return Err(AppError::Validation(format!(
            "Invalid coordination entry kind: {}",
            input.kind
        )));
    }
    if input.body.trim().is_empty() || input.client_mutation_id.trim().is_empty() {
        return Err(AppError::Validation(
            "Entry body and clientMutationId are required".into(),
        ));
    }
    if find_run(pool, user_id, plan_id, &input.author_run_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound("Agent run".into()));
    }

    let existing_sql = format!(
        "SELECT {ENTRY_COLUMNS} FROM coordination_entry WHERE author_run_id = $1 AND client_mutation_id = $2"
    );
    if let Some(existing) = sqlx::query_as::<_, CoordinationEntry>(&existing_sql)
        .bind(&input.author_run_id)
        .bind(&input.client_mutation_id)
        .fetch_optional(pool)
        .await?
    {
        return if same_entry(&existing, &input) {
            Ok(existing)
        } else {
            Err(AppError::Conflict(
                "clientMutationId was reused with different entry content".into(),
            ))
        };
    }

    let id = crate::new_id();
    let insert = format!(
        "INSERT INTO coordination_entry (id, plan_id, task_id, author_run_id, recipient_run_id, reply_to_id, kind, body, metadata, client_mutation_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) RETURNING {ENTRY_COLUMNS}"
    );
    match sqlx::query_as::<_, CoordinationEntry>(&insert)
        .bind(id)
        .bind(plan_id)
        .bind(&input.task_id)
        .bind(&input.author_run_id)
        .bind(&input.recipient_run_id)
        .bind(&input.reply_to_id)
        .bind(&input.kind)
        .bind(input.body.trim())
        .bind(Json(input.metadata.clone()))
        .bind(&input.client_mutation_id)
        .fetch_one(pool)
        .await
    {
        Ok(row) => Ok(row),
        Err(sqlx::Error::Database(e)) if e.is_foreign_key_violation() => {
            Err(AppError::NotFound("Coordination entry reference".into()))
        }
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            let existing = sqlx::query_as::<_, CoordinationEntry>(&existing_sql)
                .bind(&input.author_run_id)
                .bind(&input.client_mutation_id)
                .fetch_one(pool)
                .await?;
            if same_entry(&existing, &input) {
                Ok(existing)
            } else {
                Err(AppError::Conflict(
                    "clientMutationId was reused with different entry content".into(),
                ))
            }
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn list_entries_after(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    run_id: Option<&str>,
    after_sequence: i64,
    limit: i64,
) -> AppResult<Vec<CoordinationEntry>> {
    own_plan(pool, user_id, plan_id).await?;
    if after_sequence < 0 {
        return Err(AppError::Validation(
            "afterSequence cannot be negative".into(),
        ));
    }
    if let Some(run) = run_id
        && find_run(pool, user_id, plan_id, run).await?.is_none()
    {
        return Err(AppError::NotFound("Agent run".into()));
    }
    let sql = format!(
        "SELECT {ENTRY_COLUMNS} FROM coordination_entry WHERE plan_id = $1 AND sequence > $2 AND ($3::text IS NULL OR recipient_run_id IS NULL OR author_run_id = $3 OR recipient_run_id = $3) ORDER BY sequence ASC LIMIT $4"
    );
    Ok(sqlx::query_as::<_, CoordinationEntry>(&sql)
        .bind(plan_id)
        .bind(after_sequence)
        .bind(run_id)
        .bind(limit)
        .fetch_all(pool)
        .await?)
}

pub async fn max_sequence(pool: &PgPool, user_id: &str, plan_id: &str) -> AppResult<i64> {
    own_plan(pool, user_id, plan_id).await?;
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(MAX(sequence), 0) FROM coordination_entry WHERE plan_id = $1",
    )
    .bind(plan_id)
    .fetch_one(pool)
    .await?)
}

pub async fn ack_run(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    run_id: &str,
    through_sequence: i64,
    status: Option<&str>,
) -> AppResult<AgentRun> {
    if through_sequence < 0 || through_sequence > max_sequence(pool, user_id, plan_id).await? {
        return Err(AppError::Validation(
            "throughSequence is outside this board's journal".into(),
        ));
    }
    if let Some(value) = status
        && !RUN_STATUSES.contains(&value)
    {
        return Err(AppError::Validation(format!(
            "Invalid agent run status: {value}"
        )));
    }
    let sql = format!(
        "UPDATE agent_run r SET last_ack_sequence = GREATEST(r.last_ack_sequence, $4), status = COALESCE($5, r.status), last_heartbeat_at = now(), updated_at = now() FROM plan p WHERE r.id = $1 AND r.plan_id = $2 AND p.id = r.plan_id AND p.user_id = $3 RETURNING {RUN_COLUMNS_QUALIFIED}"
    );
    sqlx::query_as::<_, AgentRun>(&sql)
        .bind(run_id)
        .bind(plan_id)
        .bind(user_id)
        .bind(through_sequence)
        .bind(status)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Agent run".into()))
}

pub async fn list_claims(pool: &PgPool, user_id: &str, plan_id: &str) -> AppResult<Vec<TaskClaim>> {
    own_plan(pool, user_id, plan_id).await?;
    Ok(sqlx::query_as::<_, TaskClaim>(
        "SELECT task_id, plan_id, agent_run_id, claimed_at, lease_expires_at, updated_at, lease_expires_at <= now() AS expired FROM plan_task_claim WHERE plan_id = $1 ORDER BY task_id",
    )
    .bind(plan_id)
    .fetch_all(pool)
    .await?)
}

pub async fn claim_task(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    task_id: &str,
    run_id: &str,
    lease_seconds: i64,
) -> AppResult<TaskClaim> {
    if !(60..=3600).contains(&lease_seconds) {
        return Err(AppError::Validation(
            "leaseSeconds must be between 60 and 3600".into(),
        ));
    }
    let row = sqlx::query_as::<_, TaskClaim>(
        r#"INSERT INTO plan_task_claim (task_id, plan_id, agent_run_id, lease_expires_at)
           SELECT t.id, t.plan_id, r.id, now() + make_interval(secs => $5)
           FROM plan_task t
           JOIN agent_run r ON r.id = $4 AND r.plan_id = t.plan_id
           JOIN plan p ON p.id = t.plan_id AND p.user_id = $1
           WHERE t.id = $3 AND t.plan_id = $2
           ON CONFLICT (task_id) DO UPDATE SET
             agent_run_id = EXCLUDED.agent_run_id,
             claimed_at = CASE WHEN plan_task_claim.agent_run_id = EXCLUDED.agent_run_id THEN plan_task_claim.claimed_at ELSE now() END,
             lease_expires_at = EXCLUDED.lease_expires_at,
             updated_at = now()
           WHERE plan_task_claim.agent_run_id = EXCLUDED.agent_run_id
              OR plan_task_claim.lease_expires_at <= now()
           RETURNING task_id, plan_id, agent_run_id, claimed_at, lease_expires_at, updated_at, false AS expired"#,
    )
    .bind(user_id)
    .bind(plan_id)
    .bind(task_id)
    .bind(run_id)
    .bind(lease_seconds as f64)
    .fetch_optional(pool)
    .await?;
    if let Some(claim) = row {
        return Ok(claim);
    }
    if find_run(pool, user_id, plan_id, run_id).await?.is_none() {
        return Err(AppError::NotFound("Agent run".into()));
    }
    let task_exists: Option<i32> = sqlx::query_scalar(
        "SELECT 1 FROM plan_task t JOIN plan p ON p.id=t.plan_id AND p.user_id=$1 WHERE t.id=$2 AND t.plan_id=$3",
    )
    .bind(user_id)
    .bind(task_id)
    .bind(plan_id)
    .fetch_optional(pool)
    .await?;
    if task_exists.is_none() {
        return Err(AppError::NotFound("Plan task".into()));
    }
    let held = sqlx::query_as::<_, TaskClaim>(
        "SELECT task_id, plan_id, agent_run_id, claimed_at, lease_expires_at, updated_at, lease_expires_at <= now() AS expired FROM plan_task_claim WHERE task_id=$1 AND plan_id=$2",
    )
    .bind(task_id)
    .bind(plan_id)
    .fetch_optional(pool)
    .await?;
    match held {
        Some(claim) => Err(AppError::Conflict(format!(
            "task is claimed by agent run {} until {}",
            claim.agent_run_id,
            claim.lease_expires_at.to_utc().to_rfc3339()
        ))),
        None => Err(AppError::Conflict(
            "task claim changed; retry the request".into(),
        )),
    }
}

pub async fn release_task(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    task_id: &str,
    run_id: &str,
) -> AppResult<()> {
    own_plan(pool, user_id, plan_id).await?;
    if find_run(pool, user_id, plan_id, run_id).await?.is_none() {
        return Err(AppError::NotFound("Agent run".into()));
    }
    if crate::repo::plan::find_task_by_id(pool, user_id, plan_id, task_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound("Plan task".into()));
    }
    let result = sqlx::query(
        "DELETE FROM plan_task_claim c USING agent_run r WHERE c.task_id=$1 AND c.plan_id=$2 AND c.agent_run_id=$3 AND r.id=$3 AND r.plan_id=$2 RETURNING c.task_id",
    )
    .bind(task_id)
    .bind(plan_id)
    .bind(run_id)
    .fetch_optional(pool)
    .await?;
    if result.is_some() {
        return Ok(());
    }
    let held: Option<String> = sqlx::query_scalar(
        "SELECT agent_run_id FROM plan_task_claim WHERE task_id=$1 AND plan_id=$2",
    )
    .bind(task_id)
    .bind(plan_id)
    .fetch_optional(pool)
    .await?;
    match held {
        Some(_) => Err(AppError::Conflict(
            "task claim is held by another agent run".into(),
        )),
        None => Ok(()),
    }
}

pub async fn renew_task(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    task_id: &str,
    run_id: &str,
    lease_seconds: i64,
) -> AppResult<TaskClaim> {
    if !(60..=3600).contains(&lease_seconds) {
        return Err(AppError::Validation(
            "leaseSeconds must be between 60 and 3600".into(),
        ));
    }
    own_plan(pool, user_id, plan_id).await?;
    if find_run(pool, user_id, plan_id, run_id).await?.is_none() {
        return Err(AppError::NotFound("Agent run".into()));
    }
    if crate::repo::plan::find_task_by_id(pool, user_id, plan_id, task_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound("Plan task".into()));
    }
    sqlx::query_as::<_, TaskClaim>(
        r#"UPDATE plan_task_claim c SET lease_expires_at=now()+make_interval(secs => $5), updated_at=now()
           FROM agent_run r, plan p
           WHERE c.task_id=$3 AND c.plan_id=$2 AND c.agent_run_id=$4
             AND c.lease_expires_at > now()
             AND r.id=$4 AND r.plan_id=$2 AND p.id=$2 AND p.user_id=$1
           RETURNING c.task_id, c.plan_id, c.agent_run_id, c.claimed_at, c.lease_expires_at, c.updated_at, false AS expired"#,
    )
    .bind(user_id)
    .bind(plan_id)
    .bind(task_id)
    .bind(run_id)
    .bind(lease_seconds as f64)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        AppError::Conflict("agent run does not hold an active claim for this task".into())
    })
}

async fn load_entry_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    run_id: &str,
    mutation_id: &str,
) -> AppResult<Option<CoordinationEntry>> {
    let sql = format!(
        "SELECT {ENTRY_COLUMNS} FROM coordination_entry WHERE author_run_id=$1 AND client_mutation_id=$2"
    );
    Ok(sqlx::query_as::<_, CoordinationEntry>(&sql)
        .bind(run_id)
        .bind(mutation_id)
        .fetch_optional(&mut **tx)
        .await?)
}

fn same_transition_entry(
    entry: &CoordinationEntry,
    plan_id: &str,
    task_id: &str,
    kind: &str,
    body: &str,
    metadata: &serde_json::Value,
) -> bool {
    entry.plan_id == plan_id
        && entry.task_id.as_deref() == Some(task_id)
        && entry.kind == kind
        && entry.body == body
        && entry.metadata.0 == *metadata
}

#[allow(clippy::too_many_arguments)]
pub async fn transition_claimed_task(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    task_id: &str,
    run_id: &str,
    status: &str,
    note: Option<&str>,
    client_mutation_id: &str,
) -> AppResult<(PlanTask, CoordinationEntry)> {
    if !TASK_STATUSES.contains(&status) || client_mutation_id.trim().is_empty() {
        return Err(AppError::Validation(
            "Valid status and clientMutationId are required".into(),
        ));
    }
    let meaningful_note = note.filter(|n| !n.trim().is_empty());
    let body = meaningful_note
        .map(str::trim)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Task status changed to {status}"));
    let kind = if meaningful_note.is_some() {
        "progress"
    } else {
        "system"
    };
    let metadata =
        serde_json::json!({ "operation": "task_transition", "status": status, "taskId": task_id });

    if find_run(pool, user_id, plan_id, run_id).await?.is_none() {
        return Err(AppError::NotFound("Agent run".into()));
    }
    if crate::repo::plan::find_task_by_id(pool, user_id, plan_id, task_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound("Plan task".into()));
    }

    let mut tx = pool.begin().await?;
    if let Some(existing) = load_entry_in_tx(&mut tx, run_id, client_mutation_id).await? {
        if !same_transition_entry(&existing, plan_id, task_id, kind, &body, &metadata) {
            return Err(AppError::Conflict(
                "clientMutationId was reused with a different task transition".into(),
            ));
        }
        let task = find_task_in_tx(&mut tx, user_id, plan_id, task_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Plan task".into()))?;
        tx.commit().await?;
        return Ok((task, existing));
    }

    let claim: Option<i32> = sqlx::query_scalar(
        "SELECT 1 FROM plan_task_claim c JOIN agent_run r ON r.id=c.agent_run_id AND r.plan_id=c.plan_id JOIN plan p ON p.id=c.plan_id AND p.user_id=$1 WHERE c.task_id=$2 AND c.plan_id=$3 AND c.agent_run_id=$4 AND c.lease_expires_at > now() FOR UPDATE OF c",
    )
    .bind(user_id)
    .bind(task_id)
    .bind(plan_id)
    .bind(run_id)
    .fetch_optional(&mut *tx)
    .await?;

    // A concurrent retry can pass the optimistic check above and then wait on
    // the claim row. Re-check after taking that lock so it observes the first
    // request's committed journal entry instead of reporting a false conflict
    // (or reaching the unique constraint for non-terminal transitions).
    if let Some(existing) = load_entry_in_tx(&mut tx, run_id, client_mutation_id).await? {
        if !same_transition_entry(&existing, plan_id, task_id, kind, &body, &metadata) {
            return Err(AppError::Conflict(
                "clientMutationId was reused with a different task transition".into(),
            ));
        }
        let task = find_task_in_tx(&mut tx, user_id, plan_id, task_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Plan task".into()))?;
        tx.commit().await?;
        return Ok((task, existing));
    }
    if claim.is_none() {
        return Err(AppError::Conflict(
            "agent run does not hold an active claim for this task".into(),
        ));
    }

    crate::repo::plan::transition_task_in_tx(&mut tx, user_id, plan_id, task_id, status)
        .await?
        .ok_or_else(|| AppError::NotFound("Plan task".into()))?;
    let entry_id = crate::new_id();
    let insert = format!(
        "INSERT INTO coordination_entry (id,plan_id,task_id,author_run_id,kind,body,metadata,client_mutation_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING {ENTRY_COLUMNS}"
    );
    let entry = sqlx::query_as::<_, CoordinationEntry>(&insert)
        .bind(entry_id)
        .bind(plan_id)
        .bind(task_id)
        .bind(run_id)
        .bind(kind)
        .bind(&body)
        .bind(Json(metadata))
        .bind(client_mutation_id)
        .fetch_one(&mut *tx)
        .await?;
    if matches!(status, "done" | "skipped") {
        sqlx::query("DELETE FROM plan_task_claim WHERE task_id=$1 AND plan_id=$2")
            .bind(task_id)
            .bind(plan_id)
            .execute(&mut *tx)
            .await?;
    }
    let task = find_task_in_tx(&mut tx, user_id, plan_id, task_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Plan task".into()))?;
    tx.commit().await?;
    Ok((task, entry))
}

async fn find_task_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    user_id: &str,
    plan_id: &str,
    task_id: &str,
) -> AppResult<Option<PlanTask>> {
    Ok(sqlx::query_as::<_, PlanTask>(
        r#"SELECT id, plan_id, title, description, acceptance_criteria, status, "order", created_at, updated_at, metadata
           FROM plan_task WHERE id=$1 AND plan_id=$2 AND EXISTS (SELECT 1 FROM plan p WHERE p.id=$2 AND p.user_id=$3)"#,
    )
    .bind(task_id)
    .bind(plan_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?)
}
