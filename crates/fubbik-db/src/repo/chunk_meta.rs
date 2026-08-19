use fubbik_core::error::AppResult;
use sqlx::PgPool;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppliesTo {
    pub id: String,
    pub chunk_id: String,
    pub pattern: String,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileRef {
    pub id: String,
    pub chunk_id: String,
    pub path: String,
}

/// Scoped by `user_id` in the SQL itself (via the parent `chunk` row), not
/// just by the caller having already checked ownership. This is the repo's
/// own guarantee: a Phase 2 caller that forgets to call `service::get`
/// first still cannot read another user's applies-to patterns.
///
/// `, id` breaks ties on `pattern`: nothing stops a caller from submitting
/// the same glob twice via `replace_applies_to`, and an untied `ORDER BY`
/// over duplicate patterns is a query-plan artifact — see `chunk::list`'s
/// equivalent comment.
pub async fn get_applies_to(
    pool: &PgPool,
    chunk_id: &str,
    user_id: &str,
) -> AppResult<Vec<AppliesTo>> {
    let rows = sqlx::query_as!(
        AppliesTo,
        "SELECT id, chunk_id, pattern FROM chunk_applies_to \
         WHERE chunk_id = $1 \
           AND EXISTS (SELECT 1 FROM chunk c WHERE c.id = $1 AND c.user_id = $2) \
         ORDER BY pattern, id",
        chunk_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Replaces the whole pattern set in one transaction, so a failure part-way
/// through cannot leave the chunk with a truncated set.
///
/// Both the DELETE and every INSERT carry the same `EXISTS` ownership
/// guard as `get_applies_to`, so calling this with a `user_id` that does
/// not own `chunk_id` is a safe no-op: nothing is deleted and nothing is
/// inserted, regardless of what the caller checked upstream.
pub async fn replace_applies_to(
    pool: &PgPool,
    chunk_id: &str,
    user_id: &str,
    patterns: &[String],
) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    sqlx::query!(
        "DELETE FROM chunk_applies_to WHERE chunk_id = $1 \
           AND EXISTS (SELECT 1 FROM chunk c WHERE c.id = $1 AND c.user_id = $2)",
        chunk_id,
        user_id
    )
    .execute(&mut *tx)
    .await?;

    for pattern in patterns {
        let id = crate::new_id();
        sqlx::query!(
            "INSERT INTO chunk_applies_to (id, chunk_id, pattern) \
             SELECT $1, $2, $3 \
             WHERE EXISTS (SELECT 1 FROM chunk c WHERE c.id = $2 AND c.user_id = $4)",
            id,
            chunk_id,
            pattern,
            user_id
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Scoped by `user_id` in the SQL itself; see `get_applies_to`. Same
/// duplicate-tiebreaker rationale as `get_applies_to`: `, id` breaks ties
/// on `path`.
pub async fn get_file_refs(
    pool: &PgPool,
    chunk_id: &str,
    user_id: &str,
) -> AppResult<Vec<FileRef>> {
    let rows = sqlx::query_as!(
        FileRef,
        "SELECT id, chunk_id, path FROM chunk_file_ref \
         WHERE chunk_id = $1 \
           AND EXISTS (SELECT 1 FROM chunk c WHERE c.id = $1 AND c.user_id = $2) \
         ORDER BY path, id",
        chunk_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Scoped by `user_id` in the SQL itself; see `replace_applies_to`.
pub async fn replace_file_refs(
    pool: &PgPool,
    chunk_id: &str,
    user_id: &str,
    paths: &[String],
) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    sqlx::query!(
        "DELETE FROM chunk_file_ref WHERE chunk_id = $1 \
           AND EXISTS (SELECT 1 FROM chunk c WHERE c.id = $1 AND c.user_id = $2)",
        chunk_id,
        user_id
    )
    .execute(&mut *tx)
    .await?;

    for path in paths {
        let id = crate::new_id();
        sqlx::query!(
            "INSERT INTO chunk_file_ref (id, chunk_id, path) \
             SELECT $1, $2, $3 \
             WHERE EXISTS (SELECT 1 FROM chunk c WHERE c.id = $2 AND c.user_id = $4)",
            id,
            chunk_id,
            path,
            user_id
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Reports whether any of `user_id`'s chunks carries a `chunk_file_ref`
/// with exactly this `path`. Backs
/// `fubbik_api::requirements::cross_ref`'s file-reference warnings (Node's
/// `crossReferenceSteps`, `packages/api/src/requirements/cross-ref.ts:
/// 27-52`, calls the full `lookupChunksByFilePath(path, userId)` —
/// `packages/db/src/repository/file-ref.ts:45-69` — and only ever checks
/// `results.length === 0`, so a plain existence check reproduces the
/// observable behaviour without materialising rows nothing reads).
/// `lookupChunksByFilePath`'s optional `spaceId` narrowing is not
/// reproduced here: no caller in this port's `requirements` domain ever
/// passes one (`crossReferenceSteps(body.steps, userId)` — no `spaceId`
/// argument, `packages/api/src/requirements/service.ts:131`).
pub async fn file_ref_path_exists(pool: &PgPool, user_id: &str, path: &str) -> AppResult<bool> {
    let hit = sqlx::query_scalar!(
        r#"SELECT 1 AS "exists!" FROM chunk_file_ref cfr
           JOIN chunk c ON c.id = cfr.chunk_id
           WHERE cfr.path = $1 AND c.user_id = $2
           LIMIT 1"#,
        path,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(hit.is_some())
}
