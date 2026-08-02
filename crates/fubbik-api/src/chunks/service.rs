use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::chunk::{self, Chunk, ChunkPatch, ListParams, NewChunk};
use sqlx::PgPool;

use super::dto::{ChunkListResponse, CreateChunkBody, UpdateChunkBody};

pub async fn list(
    pool: &PgPool,
    user_id: &str,
    params: ListParams,
) -> AppResult<ChunkListResponse> {
    let chunks = chunk::list(pool, user_id, &params).await?;
    let total = chunk::count(pool, user_id, &params).await?;
    Ok(ChunkListResponse {
        chunks,
        total,
        limit: params.limit,
        offset: params.offset,
    })
}

pub async fn create(pool: &PgPool, user_id: &str, body: CreateChunkBody) -> AppResult<Chunk> {
    let title = body.title.trim();
    if title.is_empty() {
        return Err(AppError::Validation("title is required".into()));
    }
    if title.chars().count() > 200 {
        return Err(AppError::Validation(
            "title must be at most 200 characters".into(),
        ));
    }

    chunk::create(
        pool,
        user_id,
        NewChunk {
            title: title.to_string(),
            content: body.content,
            chunk_type: body.chunk_type.unwrap_or_else(|| "note".into()),
            rationale: body.rationale,
        },
    )
    .await
}

pub async fn get(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Chunk> {
    chunk::find_by_id(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("chunk".into()))
}

pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    body: UpdateChunkBody,
) -> AppResult<Chunk> {
    // Mirrors `create`'s title validation (trim, reject blank, cap at 200
    // chars) so a PATCH cannot put a chunk in a state POST would refuse to
    // create in the first place. `None` means "leave title unchanged" and
    // is left untouched.
    let title = body
        .title
        .map(|title| {
            let trimmed = title.trim();
            if trimmed.is_empty() {
                return Err(AppError::Validation("title is required".into()));
            }
            if trimmed.chars().count() > 200 {
                return Err(AppError::Validation(
                    "title must be at most 200 characters".into(),
                ));
            }
            Ok(trimmed.to_string())
        })
        .transpose()?;

    let current = get(pool, user_id, id).await?;
    fubbik_db::repo::chunk_version::snapshot(pool, &current).await?;

    chunk::update(
        pool,
        user_id,
        id,
        ChunkPatch {
            title,
            content: body.content,
            chunk_type: body.chunk_type,
            rationale: body.rationale,
            consequences: body.consequences,
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("chunk".into()))
}

pub async fn history(
    pool: &PgPool,
    user_id: &str,
    id: &str,
) -> AppResult<Vec<fubbik_db::repo::chunk_version::ChunkVersion>> {
    // Fetch the chunk first so another user's history cannot be read.
    get(pool, user_id, id).await?;
    fubbik_db::repo::chunk_version::list_for_chunk(pool, id, user_id).await
}

pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    if chunk::delete(pool, user_id, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("chunk".into()))
    }
}
