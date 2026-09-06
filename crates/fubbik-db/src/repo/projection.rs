use fubbik_core::error::AppResult;
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, Transaction};

#[derive(Debug, sqlx::FromRow)]
struct ProjectionEvent {
    id: String,
    event_type: String,
    payload: Json<serde_json::Value>,
    attempts: i32,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ProjectionBatch {
    pub completed: u64,
    pub failed: u64,
}

pub async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    aggregate_type: &str,
    aggregate_id: &str,
    event_type: &str,
    payload: serde_json::Value,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO projection_outbox (id, aggregate_type, aggregate_id, event_type, payload) VALUES ($1,$2,$3,$4,$5)",
    )
    .bind(crate::new_id())
    .bind(aggregate_type)
    .bind(aggregate_id)
    .bind(event_type)
    .bind(payload)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn pending_count(pool: &PgPool) -> AppResult<i64> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM projection_outbox WHERE status = 'pending'",
    )
    .fetch_one(pool)
    .await?)
}

async fn claim_batch(pool: &PgPool, limit: i64) -> AppResult<Vec<ProjectionEvent>> {
    Ok(sqlx::query_as::<_, ProjectionEvent>(
        r#"WITH claimed AS (
               SELECT id FROM projection_outbox
               WHERE status = 'pending' AND available_at <= now()
                 AND (locked_at IS NULL OR locked_at < now() - interval '5 minutes')
               ORDER BY available_at, created_at
               FOR UPDATE SKIP LOCKED LIMIT $1
           )
           UPDATE projection_outbox p
           SET locked_at = now(), attempts = attempts + 1
           FROM claimed WHERE p.id = claimed.id
           RETURNING p.id, p.event_type, p.payload, p.attempts"#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?)
}

fn field<'a>(event: &'a ProjectionEvent, name: &str) -> AppResult<&'a str> {
    event
        .payload
        .0
        .get(name)
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            fubbik_core::error::AppError::Validation(format!(
                "projection event {} is missing {name}",
                event.id
            ))
        })
}

async fn apply(pool: &PgPool, event: &ProjectionEvent) -> AppResult<()> {
    let source = field(event, "sourceId")?;
    let target = field(event, "targetId")?;
    let relation = field(event, "relation")?;
    match event.event_type.as_str() {
        "connection.upserted" => {
            crate::age::ensure_vertex(pool, source).await?;
            crate::age::ensure_vertex(pool, target).await?;
            crate::age::create_edge(pool, relation, source, target).await?;
        }
        "connection.deleted" => crate::age::delete_edge(pool, relation, source, target).await?,
        other => {
            return Err(fubbik_core::error::AppError::Validation(format!(
                "unknown projection event type: {other}"
            )));
        }
    }
    Ok(())
}

pub async fn process_batch(pool: &PgPool, limit: i64) -> AppResult<ProjectionBatch> {
    if !crate::age::is_available(pool).await {
        return Ok(ProjectionBatch::default());
    }
    let mut result = ProjectionBatch::default();
    for event in claim_batch(pool, limit.clamp(1, 100)).await? {
        match apply(pool, &event).await {
            Ok(()) => {
                sqlx::query("UPDATE projection_outbox SET status='completed', completed_at=now(), locked_at=NULL, last_error=NULL WHERE id=$1")
                    .bind(&event.id)
                    .execute(pool)
                    .await?;
                result.completed += 1;
            }
            Err(error) => {
                sqlx::query(
                    r#"UPDATE projection_outbox
                       SET status = CASE WHEN $2 >= 10 THEN 'dead' ELSE 'pending' END,
                           available_at = now() + make_interval(secs => LEAST(300, (power(2, $2)::int))),
                           locked_at = NULL, last_error = $3
                       WHERE id = $1"#,
                )
                .bind(&event.id)
                .bind(event.attempts)
                .bind(error.to_string())
                .execute(pool)
                .await?;
                result.failed += 1;
            }
        }
    }
    Ok(result)
}
