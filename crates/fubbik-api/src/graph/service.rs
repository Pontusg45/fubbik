//! Assembles the graph payload.
//!
//! The AGE half degrades to empty on any failure, matching Node's
//! `Effect.catchAll` (`packages/api/src/graph/service.ts:38`): a database
//! without the extension still serves a working graph rather than a 500.

use fubbik_core::error::AppResult;
use fubbik_db::repo::{graph as repo, tag_type};
use sqlx::PgPool;

use super::dto::GraphResponse;

pub async fn build(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
    workspace_id: Option<&str>,
) -> AppResult<GraphResponse> {
    let chunks = repo::list_chunk_meta(pool, user_id, space_id, workspace_id).await?;
    let connections = repo::list_connections(pool, user_id).await?;
    let chunk_tags = repo::list_chunk_tags_with_types(pool, user_id).await?;
    let tag_types = tag_type::list(pool, user_id).await?;

    // Only populated for the workspace view. Node returns an empty array
    // otherwise (`service.ts:21-23`) rather than paying for the join, and the
    // only consumer — the "group by space" strategy — is workspace-only.
    let chunk_spaces = if workspace_id.is_some() {
        repo::list_chunk_space_mappings(pool, user_id).await?
    } else {
        Vec::new()
    };

    // AGE vertices carry no user id — `behavior_sync::sync_once` sweeps
    // every user's matrices (deliberately; see its module docs), so the raw
    // read below returns every user's behavior rules. Intersect against the
    // ids this user actually owns before handing anything back, or user A's
    // `GET /api/graph` leaks user B's rule titles and matrix ids.
    let owned_rule_ids: std::collections::HashSet<String> =
        repo::list_owned_behavior_rule_ids(pool, user_id)
            .await?
            .into_iter()
            .collect();

    let behavior_rules = fubbik_db::age::list_behavior_rule_vertices(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|r| owned_rule_ids.contains(&r.id))
        .collect();
    let governs_edges = fubbik_db::age::list_governs_edges(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|e| owned_rule_ids.contains(&e.source_id))
        .collect();

    Ok(GraphResponse {
        chunks,
        connections,
        chunk_tags,
        tag_types,
        chunk_spaces,
        behavior_rules,
        governs_edges,
    })
}
