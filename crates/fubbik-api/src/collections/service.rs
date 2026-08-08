use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::chunk::{Enrichment, ListParams, Sort};
use fubbik_db::repo::collection::{
    self, Collection, CollectionFilter, CollectionPatch, NewCollection,
};
use fubbik_db::repo::space;
use sqlx::PgPool;

use super::dto::{CreateCollectionBody, UpdateCollectionBody};
use crate::chunks::dto::{ChunkListResponse, parse_after, parse_tags};
use crate::chunks::service as chunk_service;

pub async fn list(pool: &PgPool, user_id: &str) -> AppResult<Vec<Collection>> {
    collection::list(pool, user_id).await
}

/// Mirrors Node's `createCollection`
/// (`packages/api/src/collections/service.ts:18-35`): a thin pass-through
/// insert — `name`/`filter` are stored exactly as given, with no trim or
/// blank-name check (unlike `workspaces::service::create`, which does trim
/// and reject a blank name; collections has no such check in Node, and this
/// port does not add one it doesn't have). The one addition is `space_id`
/// ownership — see `fubbik_db::repo::collection::create`'s doc comment for
/// why, and `tests/collection.rs` / `tests/collections.rs` for proof both
/// this pre-check and the repo's own `EXISTS` guard are independently
/// load-bearing.
pub async fn create(
    pool: &PgPool,
    user_id: &str,
    body: CreateCollectionBody,
) -> AppResult<Collection> {
    if let Some(space_id) = &body.space_id {
        space::find_by_id(pool, user_id, space_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Space".into()))?;
    }

    collection::create(
        pool,
        user_id,
        NewCollection {
            name: body.name,
            description: body.description,
            filter: body.filter,
            space_id: body.space_id,
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Space".into()))
}

/// Mirrors Node's `updateCollection`
/// (`packages/api/src/collections/service.ts:37-51`): Node checks ownership
/// twice — once before the `UPDATE`, once again on its result — a
/// belt-and-suspenders pair that can't actually observe different outcomes
/// (nothing can change ownership of the row between the two calls within
/// one request). This collapses that into a single pre-check plus the
/// `UPDATE`'s own `WHERE id = .. AND user_id = ..`, which is equivalent.
/// `filter`, when provided, replaces the stored object wholesale — see
/// `collection::CollectionPatch`'s doc comment.
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    body: UpdateCollectionBody,
) -> AppResult<Collection> {
    collection::find_by_id(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Collection".into()))?;

    collection::update(
        pool,
        user_id,
        id,
        CollectionPatch {
            name: body.name,
            description: body.description,
            filter: body.filter,
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Collection".into()))
}

pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    if collection::delete(pool, user_id, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("Collection".into()))
    }
}

/// Maps a stored `CollectionFilter` onto `chunk::ListParams` — see
/// `fubbik_db::repo::collection`'s module doc for why this thin mapping,
/// not a bespoke filter evaluator, is the entire "evaluate the filter"
/// step. Every string value is interpreted leniently: an unrecognised
/// `sort`/`enrichment` is silently ignored (falls back to the default / no
/// filter), matching Node's `listChunks`
/// (`packages/db/src/repository/chunk.ts:120-141`) — this filter was never
/// validated beyond "is a string" at write time.
///
/// `spaceId` is NOT one of these nine keys — it's a separate column on the
/// `collection` row itself (`col.spaceId`), threaded through by `get_chunks`
/// after this function returns. See that function's doc comment.
fn filter_to_list_params(filter: &CollectionFilter) -> ListParams {
    ListParams {
        chunk_type: filter.filter_type.clone(),
        search: filter.search.clone(),
        origin: filter.origin.clone(),
        review_status: filter.review_status.clone(),
        sort: Sort::from_loose_str(filter.sort.as_deref()),
        tags: filter.tags.as_deref().and_then(parse_tags),
        after: filter.after.as_deref().and_then(parse_after),
        enrichment: Enrichment::from_loose_str(filter.enrichment.as_deref()),
        min_connections: filter
            .min_connections
            .as_deref()
            .and_then(|s| s.parse().ok()),
        space_id: None,
        // `GET /collections/{id}/chunks` takes no query params of its own in
        // Node (`packages/api/src/collections/routes.ts:69-73` reads only
        // `ctx.params.id`) — the inline `listChunks(...)` call omits
        // `limit`/`offset` entirely, so they fall back to
        // `chunks::service::list`'s own defaults
        // (`packages/api/src/chunks/service.ts:50-51`: 50 / 0).
        limit: 50,
        offset: 0,
    }
}

/// Mirrors Node's `getCollectionChunks`
/// (`packages/api/src/collections/service.ts:59-78`): loads the collection
/// (404 if not the caller's), maps its filter onto `ListParams`, and
/// delegates to the exact same `chunks::service::list` that backs `GET
/// /api/chunks` — so the response is the `{chunks, total, limit, offset}`
/// envelope, not a bare array, even though every other list endpoint in
/// this slice is bare. See
/// `tests/fixtures/node-contract-2b/collections-chunks-filter-{type,tags}.json`.
///
/// `col.spaceId` (a separate column, not one of `CollectionFilter`'s nine
/// keys) is threaded through as `ListParams::space_id` here, matching
/// Node's `spaceId: col.spaceId ?? undefined`
/// (`packages/api/src/collections/service.ts:74`) exactly — including its
/// "chunk in that space OR chunk in no space at all" semantics, see
/// `chunk::ListParams::space_id`'s doc comment. A collection with no
/// `spaceId` set applies no space filter, same as Node.
pub async fn get_chunks(pool: &PgPool, user_id: &str, id: &str) -> AppResult<ChunkListResponse> {
    let found = collection::find_by_id(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Collection".into()))?;

    let mut params = filter_to_list_params(&found.filter.0);
    params.space_id = found.space_id;
    chunk_service::list(pool, user_id, params).await
}
