//! The four reads behind `GET /api/graph`.
//!
//! Ports `packages/db/src/repository/graph.ts` (107 LOC). Tag types are NOT
//! here — `tag_type::list` already returns exactly the columns Node's
//! `db.select().from(tagType)` does, and duplicating it would give the graph
//! its own drifting copy.
//!
//! Every query is ordered, unlike Node, which leaves order to the planner.
//! The web sorts nothing, so this costs nothing and makes the tests
//! deterministic.

use fubbik_core::error::AppResult;
use sqlx::PgPool;

use crate::timestamp::UtcTimestamp;

/// The five columns the graph needs off `chunk` — deliberately not the whole
/// row. The payload carries one of these per node and the full chunk is
/// fetched separately when a node is opened.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunkMeta {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub summary: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphConnection {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub relation: String,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunkTagWithType {
    pub chunk_id: String,
    pub tag_id: String,
    pub tag_name: String,
    /// `None` for an untyped tag — the `tag_type` join is a LEFT join.
    pub tag_type_id: Option<String>,
    pub tag_type_name: Option<String>,
    pub tag_type_color: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunkSpaceMapping {
    pub chunk_id: String,
    pub space_id: String,
    pub space_name: String,
}

/// Chunk metadata, optionally scoped to a workspace or a single space.
///
/// **`workspace_id` wins over `space_id`.** Node's `if (workspaceId) … else if
/// (codebaseId)` (`graph.ts:13-25`) never evaluates the space branch when a
/// workspace is present, and the web can send both.
///
/// **A chunk in no space is global and appears under every scope.** That is
/// the `NOT IN (SELECT chunk_id FROM chunk_space)` disjunct — the same idiom
/// `chunk::list` uses at `chunk.rs:523-525`. Dropping it would make the graph
/// hide every un-spaced chunk the moment a space is selected.
pub async fn list_chunk_meta(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
    workspace_id: Option<&str>,
) -> AppResult<Vec<ChunkMeta>> {
    let rows = sqlx::query_as!(
        ChunkMeta,
        r#"SELECT c.id, c.title, c.type AS chunk_type, c.summary,
                  c.created_at AS "created_at: UtcTimestamp"
           FROM chunk c
           WHERE c.user_id = $1
             AND (
               ($2::text IS NULL AND $3::text IS NULL)
               OR c.id NOT IN (SELECT chunk_id FROM chunk_space)
               OR ($3::text IS NOT NULL AND c.id IN (
                     SELECT cs.chunk_id FROM chunk_space cs
                     JOIN workspace_space ws ON ws.space_id = cs.space_id
                     WHERE ws.workspace_id = $3))
               OR ($3::text IS NULL AND $2::text IS NOT NULL AND c.id IN (
                     SELECT chunk_id FROM chunk_space WHERE space_id = $2))
             )
           ORDER BY c.created_at DESC, c.id ASC"#,
        user_id,
        space_id,
        workspace_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Connections where either endpoint is one of this user's chunks.
///
/// Deliberately **not** space-scoped, matching Node: the web filters against
/// the chunk-id set it already has (`search-graph.tsx:87`), and scoping here
/// would silently drop edges that cross a space boundary.
pub async fn list_connections(pool: &PgPool, user_id: &str) -> AppResult<Vec<GraphConnection>> {
    let rows = sqlx::query_as!(
        GraphConnection,
        r#"SELECT cc.id, cc.source_id, cc.target_id, cc.relation
           FROM chunk_connection cc
           WHERE cc.source_id IN (SELECT id FROM chunk WHERE user_id = $1)
              OR cc.target_id IN (SELECT id FROM chunk WHERE user_id = $1)
           ORDER BY cc.id ASC"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Every chunk-tag pairing with its tag type, scoped by **tag** ownership —
/// `tag.user_id`, not `chunk.user_id`. That is what Node filters on
/// (`graph.ts:52`).
pub async fn list_chunk_tags_with_types(
    pool: &PgPool,
    user_id: &str,
) -> AppResult<Vec<ChunkTagWithType>> {
    let rows = sqlx::query_as!(
        ChunkTagWithType,
        r#"SELECT ct.chunk_id, t.id AS tag_id, t.name AS tag_name,
                  t.tag_type_id, tt.name AS "tag_type_name?", tt.color AS "tag_type_color?"
           FROM chunk_tag ct
           JOIN tag t ON t.id = ct.tag_id
           LEFT JOIN tag_type tt ON tt.id = t.tag_type_id
           WHERE t.user_id = $1
           ORDER BY ct.chunk_id ASC, t.id ASC"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Chunk → space mappings, scoped by **space** ownership (`space.user_id`),
/// matching `graph.ts:78`. Only read when a workspace is selected; the service
/// keeps that conditional.
pub async fn list_chunk_space_mappings(
    pool: &PgPool,
    user_id: &str,
) -> AppResult<Vec<ChunkSpaceMapping>> {
    let rows = sqlx::query_as!(
        ChunkSpaceMapping,
        r#"SELECT cs.chunk_id, cs.space_id, s.name AS space_name
           FROM chunk_space cs
           JOIN space s ON s.id = cs.space_id
           WHERE s.user_id = $1
           ORDER BY cs.chunk_id ASC, cs.space_id ASC"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// The ids of every `behavior_rule` this user owns, via its matrix.
///
/// The AGE graph is swept for ALL users (`graph::sync::sync_once` — a
/// deliberate divergence from Node, which only ever wrote the implicit dev
/// user's rules and so never faced this seam), but `GET /api/graph` is
/// per-user. Without this filter, `age::list_behavior_rule_vertices` and
/// `age::list_governs_edges` — which have no user id to filter on, because
/// AGE vertices don't carry one — hand every user's behavior-rule titles
/// and matrix ids to whoever is logged in. This is the relational side of
/// that intersection: SQL ownership, not graph properties, decides what a
/// user is allowed to see.
pub async fn list_owned_behavior_rule_ids(pool: &PgPool, user_id: &str) -> AppResult<Vec<String>> {
    let rows = sqlx::query_scalar!(
        r#"SELECT r.id
           FROM behavior_rule r
           JOIN behavior_matrix m ON m.id = r.matrix_id
           WHERE m.user_id = $1"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
