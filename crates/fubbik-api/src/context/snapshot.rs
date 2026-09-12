//! `POST /api/context/snapshot`, `GET /api/context/snapshot/{id}`,
//! `GET /api/context/snapshots`, `DELETE /api/context/snapshot/{id}`.
//!
//! Ports `packages/api/src/context/snapshot-service.ts` and
//! `snapshot-routes.ts`: a snapshot freezes the resolve->enrich->budget
//! pipeline's output — the same one `context::routes`' three GET endpoints
//! run live — as a JSONB blob, so an AI agent can retrieve *exactly* the
//! context it was given even if the underlying chunks change later.
//!
//! **Every read and delete filters on `user_id` at the SQL level — a
//! deliberate divergence from Node.** Node's repository-level
//! `getSnapshotById`/`deleteSnapshot`
//! (`packages/db/src/repository/context-snapshot.ts:16-20,27-31`) take no
//! `user_id` at all; ownership is enforced exactly once, upstream, in
//! `getSnapshot` (`snapshot-service.ts:70-78`), and the `DELETE` route
//! relies on that same check having already run before calling the
//! unscoped repository `delete`
//! (`snapshot-routes.ts:50-59`: `getSnapshot(...).pipe(Effect.flatMap(() =>
//! deleteSnapshot(...)))`) — a downstream filter standing in for a query-level
//! one, the same class of gap Phase 4b closed in `enrich` and Task 5 of
//! this phase closed in `resolve_for_plan`. This port's
//! [`fubbik_db::repo::context_snapshot::find_by_id_for_user`] and
//! [`fubbik_db::repo::context_snapshot::delete_for_user`] both carry
//! `WHERE ... AND user_id = $N` directly, so there is no unscoped sibling
//! for a caller to reach around by mistake, and [`delete_snapshot`] below
//! no longer needs Node's preceding `getSnapshot` call at all — the delete
//! itself is now the ownership check.

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use fubbik_core::error::{AppError, AppResult};
use fubbik_core::format::{ChunkWithMetadata, format_structured, format_structured_markdown};
use fubbik_core::tokens::estimate_tokens;
use fubbik_db::repo::context_snapshot::{self, ContextSnapshot};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;

use super::resolvers::{resolve_for_concept, resolve_for_files, resolve_for_plan};
use super::routes::budget_metadata;
use super::service::enrich_chunks;
use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;
use fubbik_db::timestamp::UtcTimestamp;

/// Matches Node's `DEFAULT_MAX_TOKENS` (`snapshot-service.ts:14`) — a
/// higher default than the three live `/api/context/*` GET routes'
/// `DEFAULT_MAX_TOKENS = 4000` (`context/dto.rs`), because a snapshot is
/// meant to stand alone for an agent with no further context calls to
/// make.
const DEFAULT_MAX_TOKENS: usize = 8000;

/// Body of `POST /api/context/snapshot`
/// (`packages/api/src/context/snapshot-routes.ts:27-34`). Resolver
/// selection order — `planId`, else non-empty `filePaths`, else `concept`
/// — is decided in [`create_snapshot`], mirroring
/// `snapshot-service.ts:32-38`'s `if`/`else if` chain exactly.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSnapshotBody {
    pub plan_id: Option<String>,
    pub task_id: Option<String>,
    pub file_paths: Option<Vec<String>>,
    pub concept: Option<String>,
    pub max_tokens: Option<usize>,
    pub space_id: Option<String>,
}

/// Response of `POST /api/context/snapshot`
/// (`snapshot-service.ts:61-66`).
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSnapshotResponse {
    pub snapshot_id: String,
    pub token_count: i32,
    pub chunk_count: usize,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

/// Resolves candidate ids exactly like `snapshot-service.ts:32-38`: `planId`
/// takes priority, then a *non-empty* `filePaths`, then `concept`; an input
/// with none of the three resolves to no candidates at all (Node's `let
/// chunkIds: string[] = []` default, never touched by any branch).
async fn resolve_ids(
    state: &AppState,
    user_id: &str,
    body: &CreateSnapshotBody,
) -> AppResult<Vec<String>> {
    if let Some(plan_id) = &body.plan_id {
        return resolve_for_plan(&state.pool, user_id, plan_id).await;
    }
    if let Some(paths) = &body.file_paths
        && !paths.is_empty()
    {
        return resolve_for_files(
            &state.pool,
            &state.ai,
            &state.background,
            user_id,
            paths,
            body.space_id.as_deref(),
        )
        .await;
    }
    if let Some(concept) = &body.concept {
        return resolve_for_concept(
            &state.pool,
            &state.ai,
            user_id,
            concept,
            body.space_id.as_deref(),
        )
        .await;
    }
    Ok(vec![])
}

/// Rebuilds the `query` JSONB column exactly as `snapshot-service.ts:46-52`
/// does: each field is included only when present on the input, and
/// `maxTokens` additionally only when non-zero — Node's `if
/// (input.maxTokens)` falsy-checks the number, so an explicit `0` is
/// omitted the same as absent (`filePaths`, by contrast, is included
/// whenever present at all, even `[]`, because JS arrays are always
/// truthy).
fn build_query_json(body: &CreateSnapshotBody) -> serde_json::Value {
    let mut query = serde_json::Map::new();
    if let Some(v) = &body.plan_id {
        query.insert("planId".into(), serde_json::json!(v));
    }
    if let Some(v) = &body.task_id {
        query.insert("taskId".into(), serde_json::json!(v));
    }
    if let Some(v) = &body.file_paths {
        query.insert("filePaths".into(), serde_json::json!(v));
    }
    if let Some(v) = &body.concept {
        query.insert("concept".into(), serde_json::json!(v));
    }
    if let Some(v) = body.max_tokens
        && v != 0
    {
        query.insert("maxTokens".into(), serde_json::json!(v));
    }
    if let Some(v) = &body.space_id {
        query.insert("spaceId".into(), serde_json::json!(v));
    }
    serde_json::Value::Object(query)
}

/// Ports `createSnapshot` (`snapshot-service.ts:25-68`): resolve candidate
/// ids, enrich and score them, budget into `max_tokens`, freeze the
/// survivors into `context_snapshot.chunks`. `token_count` is computed the
/// same way Node's is — from the *formatted markdown* the budgeted chunks
/// would render as, not from the stored JSON's byte size — even though the
/// markdown itself is never persisted (`snapshot-service.ts:43-44`
/// computes `content` purely to feed `estimateTokens`, then discards it;
/// only `budgeted`, `tokenCount` and the query blob are written).
pub async fn create_snapshot(
    state: &AppState,
    user_id: &str,
    body: CreateSnapshotBody,
) -> AppResult<CreateSnapshotResponse> {
    let max_tokens = body.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);

    let ids = resolve_ids(state, user_id, &body).await?;
    let enriched = enrich_chunks(&state.pool, user_id, &ids).await?;
    let budgeted: Vec<ChunkWithMetadata> = budget_metadata(enriched, max_tokens);

    let structured = format_structured(budgeted.clone());
    let content = format_structured_markdown(&structured);
    let token_count = estimate_tokens(&content) as i32;

    let query = build_query_json(&body);
    let snapshot =
        context_snapshot::create(&state.pool, user_id, &query, &budgeted, token_count).await?;

    Ok(CreateSnapshotResponse {
        snapshot_id: snapshot.id,
        token_count: snapshot.token_count,
        chunk_count: budgeted.len(),
        created_at: snapshot.created_at,
    })
}

/// Ports `getSnapshot` (`snapshot-service.ts:70-78`). Node fetches
/// unscoped and then checks `snapshot.userId !== userId` in the service
/// layer; this port pushes the same check into the query itself (see this
/// module's doc comment), so a foreign or unknown id both resolve to the
/// identical `NotFound` here.
pub async fn get_snapshot(pool: &PgPool, user_id: &str, id: &str) -> AppResult<ContextSnapshot> {
    context_snapshot::find_by_id_for_user(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("ContextSnapshot".into()))
}

/// Ports `listSnapshots` (`snapshot-service.ts:80-82`).
pub async fn list_snapshots(pool: &PgPool, user_id: &str) -> AppResult<Vec<ContextSnapshot>> {
    context_snapshot::list_for_user(pool, user_id).await
}

/// Ports `deleteSnapshot` (`snapshot-service.ts:84-86`) plus the
/// ownership check `snapshot-routes.ts`'s `DELETE` handler used to perform
/// via a preceding `getSnapshot` call — folded into one scoped query here
/// (see this module's doc comment), so a foreign or unknown id both 404
/// without a separate lookup.
pub async fn delete_snapshot(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    let deleted = context_snapshot::delete_for_user(pool, user_id, id).await?;
    if !deleted {
        return Err(AppError::NotFound("ContextSnapshot".into()));
    }
    Ok(())
}

#[utoipa::path(post, path = "/api/context/snapshot", request_body = CreateSnapshotBody,
    responses((status = 200, body = CreateSnapshotResponse)))]
pub async fn create_snapshot_route(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateSnapshotBody>,
) -> ApiResult<Json<CreateSnapshotResponse>> {
    Ok(Json(create_snapshot(&state, &user.id, body).await?))
}

#[utoipa::path(get, path = "/api/context/snapshot/{id}",
    params(("id" = String, Path,)),
    responses((status = 200, body = ContextSnapshot), (status = 404)))]
pub async fn get_snapshot_route(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<ContextSnapshot>> {
    Ok(Json(get_snapshot(&state.pool, &user.id, &id).await?))
}

#[utoipa::path(get, path = "/api/context/snapshots",
    responses((status = 200, body = Vec<ContextSnapshot>)))]
pub async fn list_snapshots_route(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<Vec<ContextSnapshot>>> {
    Ok(Json(list_snapshots(&state.pool, &user.id).await?))
}

/// Always 200 with a `null` body on success — matching Node, whose
/// `DELETE` handler never sets an explicit response (`snapshot-routes.ts:49-63`):
/// the Effect chain resolves to `deleteSnapshot`'s return value, `void`,
/// which Elysia serialises as an empty/`null` body at the default 200
/// status. A foreign or unknown id 404s via [`delete_snapshot`] instead.
#[utoipa::path(delete, path = "/api/context/snapshot/{id}",
    params(("id" = String, Path,)),
    responses((status = 200), (status = 404)))]
pub async fn delete_snapshot_route(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<()>> {
    delete_snapshot(&state.pool, &user.id, &id).await?;
    Ok(Json(()))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/context/snapshot", post(create_snapshot_route))
        .route(
            "/api/context/snapshot/{id}",
            get(get_snapshot_route).delete(delete_snapshot_route),
        )
        .route("/api/context/snapshots", get(list_snapshots_route))
}
