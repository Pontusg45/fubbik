//! Context snapshots — frozen, token-budgeted context persisted as JSONB.
//! `context_snapshot` is `(id, user_id, query jsonb, chunks jsonb,
//! token_count, created_at)`; `chunks` stores the already-budgeted
//! `ChunkWithMetadata[]` verbatim, and `query` echoes back whichever
//! subset of the create request produced it. Ports
//! `packages/db/src/repository/context-snapshot.ts`.
//!
//! **Every read and every delete is scoped to `user_id` at the SQL level —
//! a deliberate divergence from Node.** Node's `getSnapshotById(id)` and
//! `deleteSnapshot(id)` (`packages/db/src/repository/context-snapshot.ts:16-20,27-31`)
//! take no `user_id` at all; ownership is enforced only once, upstream, in
//! `snapshot-service.ts`'s `getSnapshot` (which checks
//! `snapshot.userId !== userId` after an unscoped fetch) — and
//! `snapshot-routes.ts`'s `DELETE` handler relies on that same upstream
//! check running first and then calls the unscoped repository `delete`
//! with nothing stopping a caller who reached the delete some other way.
//! This is the same class of gap Phase 4b closed in `enrich` and Task 5 of
//! this phase closed in `resolve_for_plan`: a downstream filter is not a
//! substitute for a `WHERE user_id = $N` on the query itself, so
//! [`find_by_id_for_user`] and [`delete_for_user`] both carry the
//! ownership check directly, and neither has an unscoped sibling for a
//! caller to reach around it by mistake.
use fubbik_core::error::AppResult;
use fubbik_core::format::ChunkWithMetadata;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::timestamp::UtcTimestamp;

/// `camelCase` on the wire, matching every other repo row type in this
/// crate. `query`/`chunks` are stored and returned as opaque JSON blobs —
/// `query` is a free-form subset of the create request
/// (`packages/api/src/context/snapshot-service.ts:46-52`'s
/// `Record<string, unknown>`), and `chunks` is the frozen
/// `ChunkWithMetadata[]` the create call budgeted.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextSnapshot {
    pub id: String,
    pub user_id: String,
    #[schema(value_type = serde_json::Value)]
    pub query: Json<serde_json::Value>,
    #[schema(value_type = Vec<ChunkWithMetadata>)]
    pub chunks: Json<Vec<ChunkWithMetadata>>,
    pub token_count: i32,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

/// Plain insert — creation carries no ownership check to make (the row is
/// created *for* `user_id`, there is nothing yet to own).
pub async fn create(
    pool: &PgPool,
    user_id: &str,
    query: &serde_json::Value,
    chunks: &[ChunkWithMetadata],
    token_count: i32,
) -> AppResult<ContextSnapshot> {
    let id = crate::new_id();
    let row = sqlx::query_as!(
        ContextSnapshot,
        r#"INSERT INTO context_snapshot (id, user_id, query, chunks, token_count)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, user_id, query AS "query: Json<serde_json::Value>",
                     chunks AS "chunks: Json<Vec<ChunkWithMetadata>>",
                     token_count, created_at AS "created_at: UtcTimestamp""#,
        id,
        user_id,
        Json(query) as _,
        Json(chunks) as _,
        token_count
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Scoped read — see this module's doc comment. A snapshot that exists but
/// belongs to a different user is indistinguishable from an unknown id:
/// both return `Ok(None)`.
pub async fn find_by_id_for_user(
    pool: &PgPool,
    user_id: &str,
    id: &str,
) -> AppResult<Option<ContextSnapshot>> {
    let row = sqlx::query_as!(
        ContextSnapshot,
        r#"SELECT id, user_id, query AS "query: Json<serde_json::Value>",
                  chunks AS "chunks: Json<Vec<ChunkWithMetadata>>",
                  token_count, created_at AS "created_at: UtcTimestamp"
           FROM context_snapshot
           WHERE id = $1 AND user_id = $2"#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Backs `GET /api/context/snapshots`. `ORDER BY created_at DESC, id ASC`
/// — Node's `listSnapshots` orders by `desc(createdAt)` alone
/// (`packages/db/src/repository/context-snapshot.ts:22-25`); `, id ASC` is
/// an added tiebreaker, the same pattern as `proposal::list` and
/// `notification::list`, needed because snapshots created in the same
/// request burst (or the same millisecond in a test) share a tied
/// `created_at` and an `ORDER BY` with no deterministic tiebreaker is a
/// query-plan artifact, not a stable order — see this phase's Task 6 doc
/// comments for why that distinction matters here.
pub async fn list_for_user(pool: &PgPool, user_id: &str) -> AppResult<Vec<ContextSnapshot>> {
    let rows = sqlx::query_as!(
        ContextSnapshot,
        r#"SELECT id, user_id, query AS "query: Json<serde_json::Value>",
                  chunks AS "chunks: Json<Vec<ChunkWithMetadata>>",
                  token_count, created_at AS "created_at: UtcTimestamp"
           FROM context_snapshot
           WHERE user_id = $1
           ORDER BY created_at DESC, id ASC
           LIMIT 50"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Scoped delete — see this module's doc comment. Returns whether a row
/// was actually removed, so the service layer can 404 a foreign or unknown
/// id without needing a separate ownership read first.
pub async fn delete_for_user(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let result = sqlx::query!(
        "DELETE FROM context_snapshot WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}
