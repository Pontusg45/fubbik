//! Wire types for the `search` domain — 6 endpoints under `/api/search`.
//! Response shape is per-endpoint here, not uniform: `parse` wraps its
//! array in `{clauses: [...]}`, `query` returns the `SearchResult`
//! envelope, `autocomplete`/`saved` (GET) return bare arrays, `saved`
//! (POST) a bare object — see each route's doc comment in `routes.rs` and
//! `tests/fixtures/node-contract-2c/search-*.json` for the captured
//! contract.
//!
//! [`GraphMeta`], [`GraphContext`], and [`DuplicateHint`] exist for shape
//! parity with Node's `types.ts` and so `SearchResult`'s OpenAPI schema is
//! complete, but nothing in this task populates them: `graphMeta`/graph
//! context require the `near`/`path`/`affected-by`/`similar-to` clauses,
//! which land in Task 9 (Apache AGE), and `duplicateHints` requires
//! `findDuplicatePairs`, a pgvector-embedding-similarity query this port
//! has not landed anywhere yet — out of scope for both Task 8 and 9. Every
//! response from this task's implementation carries `graphMeta: null` and
//! `duplicateHints: null` (both fields are `skip_serializing_if = "Option::is_none"`,
//! so they're simply absent, matching Node's `undefined` fields never
//! appearing in a JSON body either).

use fubbik_db::timestamp::UtcTimestamp;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::search::parser::QueryClause;

/// `GET /api/search/parse` response: `{clauses: [...]}` — the raw clause
/// array, not a normalised query string or a validation verdict.
#[derive(Serialize, ToSchema)]
pub struct ParseResponse {
    pub clauses: Vec<QueryClause>,
}

/// Query params for `GET /api/search/parse`
/// (`packages/api/src/search/routes.ts:45-53`): `q` only, required.
#[derive(Deserialize, IntoParams)]
pub struct ParseQuery {
    pub q: String,
}

/// Body of `POST /api/search/query`
/// (`packages/api/src/search/routes.ts:34-43`). `join` is accepted and
/// **completely ignored** — see `search::service`'s module doc for why:
/// Node's schema declares it (`t.Optional(t.Union([t.Literal("and"), t.Literal("or")]))`)
/// but `service.ts`'s `executeSearch` never reads `searchQuery.join`
/// anywhere; every clause is AND-combined by `listChunks`'s SQL
/// regardless. Kept as a loose `Option<String>` rather than a `Literal`
/// enum precisely because nothing ever inspects its value.
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchQueryBody {
    pub clauses: Vec<QueryClause>,
    #[serde(default)]
    pub join: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub space_id: Option<String>,
}

/// `POST /api/search/query`'s response envelope, matching Node's
/// `SearchResult` (`packages/api/src/search/types.ts:48-59`).
#[derive(Serialize, ToSchema, Default, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub chunks: Vec<SearchResultChunk>,
    pub total: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_meta: Option<GraphMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicate_hints: Option<Vec<DuplicateHint>>,
}

#[derive(Serialize, ToSchema, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultChunk {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub connection_count: i64,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_context: Option<GraphContext>,
    pub health_score: i64,
}

/// See module doc — never populated by this task's implementation, present
/// for shape parity and Task 9 to fill in.
#[derive(Serialize, ToSchema, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphMeta {
    #[serde(rename = "type")]
    pub meta_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_chunk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_chunks: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_edges: Option<Vec<PathEdgeInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hops: Option<i64>,
}

#[derive(Serialize, ToSchema, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PathEdgeInfo {
    pub source: String,
    pub target: String,
    pub relation: String,
}

#[derive(Serialize, ToSchema, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hop_distance: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_position: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_requirement: Option<String>,
}

#[derive(Serialize, ToSchema, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateHint {
    pub chunk_id_a: String,
    pub chunk_id_b: String,
    pub similarity: f64,
}

/// Query params for `GET /api/search/autocomplete`
/// (`packages/api/src/search/routes.ts`'s query schema: `field`, `prefix`,
/// both required plain strings).
#[derive(Deserialize, IntoParams)]
pub struct AutocompleteQuery {
    pub field: String,
    pub prefix: String,
}

/// Query params for `GET /api/search/saved` — `spaceId` only, optional.
#[derive(Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListSavedQuery {
    pub space_id: Option<String>,
}

/// The `query` object nested in `POST /api/search/saved`'s body, matching
/// Node's schema exactly (`packages/api/src/search/routes.ts:93-101`):
/// `clauses`, `join`, `sort`, `spaceId`, all validated at write time. Once
/// stored, `saved_query.query` is opaque JSONB never re-validated on read
/// (see `fubbik_db::repo::saved_query`'s module doc) — this type exists
/// only to validate the shape going *in*.
#[derive(Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SavedQueryPayload {
    pub clauses: Vec<QueryClause>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,
}

/// Body of `POST /api/search/saved`
/// (`packages/api/src/search/routes.ts:93-103`).
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSavedQueryBody {
    #[schema(max_length = 200)]
    pub name: String,
    pub query: SavedQueryPayload,
    pub space_id: Option<String>,
}

/// Shape of every `{ message: "..." }` response in this domain — just the
/// one, `DELETE /api/search/saved/{id}`'s unconditional `"Deleted"`.
#[derive(Serialize, ToSchema)]
pub struct MessageResponse {
    pub message: String,
}
