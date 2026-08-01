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

pub async fn get_applies_to(pool: &PgPool, chunk_id: &str) -> AppResult<Vec<AppliesTo>> {
    let rows = sqlx::query_as!(
        AppliesTo,
        "SELECT id, chunk_id, pattern FROM chunk_applies_to WHERE chunk_id = $1 ORDER BY pattern",
        chunk_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Replaces the whole pattern set in one transaction, so a failure part-way
/// through cannot leave the chunk with a truncated set.
pub async fn replace_applies_to(
    pool: &PgPool,
    chunk_id: &str,
    patterns: &[String],
) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    sqlx::query!("DELETE FROM chunk_applies_to WHERE chunk_id = $1", chunk_id)
        .execute(&mut *tx)
        .await?;

    for pattern in patterns {
        let id = crate::new_id();
        sqlx::query!(
            "INSERT INTO chunk_applies_to (id, chunk_id, pattern) VALUES ($1, $2, $3)",
            id,
            chunk_id,
            pattern
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn get_file_refs(pool: &PgPool, chunk_id: &str) -> AppResult<Vec<FileRef>> {
    let rows = sqlx::query_as!(
        FileRef,
        "SELECT id, chunk_id, path FROM chunk_file_ref WHERE chunk_id = $1 ORDER BY path",
        chunk_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn replace_file_refs(pool: &PgPool, chunk_id: &str, paths: &[String]) -> AppResult<()> {
    let mut tx = pool.begin().await?;

    sqlx::query!("DELETE FROM chunk_file_ref WHERE chunk_id = $1", chunk_id)
        .execute(&mut *tx)
        .await?;

    for path in paths {
        let id = crate::new_id();
        sqlx::query!(
            "INSERT INTO chunk_file_ref (id, chunk_id, path) VALUES ($1, $2, $3)",
            id,
            chunk_id,
            path
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}
