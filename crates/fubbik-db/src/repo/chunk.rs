use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::embedding::EmbeddingVec;
use crate::timestamp::UtcTimestamp;

/// `camelCase` serialisation is mandatory, not cosmetic: the 106 web files
/// that consume this API were written against Drizzle's camelCase output.
/// Emitting snake_case would silently break every one of them.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub id: String,
    pub title: String,
    pub content: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub user_id: String,
    pub summary: Option<String>,
    #[schema(value_type = Vec<String>)]
    pub aliases: Json<Vec<String>>,
    #[schema(value_type = Vec<String>)]
    pub not_about: Json<Vec<String>>,
    #[schema(value_type = std::collections::HashMap<String, String>)]
    pub scope: Json<serde_json::Value>,
    pub rationale: Option<String>,
    // `alternatives` is nullable (unlike `aliases`/`not_about`/`scope`,
    // which are `NOT NULL DEFAULT`) — Node emits `null`, not `[]`, when
    // unset, so this stays `Option<Json<..>>` rather than defaulting to an
    // empty vec.
    #[schema(value_type = Option<Vec<String>>)]
    pub alternatives: Option<Json<Vec<String>>>,
    pub consequences: Option<String>,
    // See `fubbik_db::embedding` for why this is parsed text rather than
    // the `pgvector` crate.
    #[schema(value_type = Option<Vec<f32>>)]
    pub embedding: Option<EmbeddingVec>,
    #[schema(value_type = Option<chrono::NaiveDateTime>)]
    pub embedding_updated_at: Option<UtcTimestamp>,
    pub origin: String,
    pub review_status: String,
    pub reviewed_by: Option<String>,
    #[schema(value_type = Option<chrono::NaiveDateTime>)]
    pub reviewed_at: Option<UtcTimestamp>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
    #[schema(value_type = Option<chrono::NaiveDateTime>)]
    pub archived_at: Option<UtcTimestamp>,
    pub document_id: Option<String>,
    pub document_order: Option<i32>,
    pub is_entry_point: bool,
}

pub struct NewChunk {
    pub title: String,
    pub content: String,
    pub chunk_type: String,
    pub rationale: Option<String>,
}

#[derive(Default)]
pub struct ChunkPatch {
    pub title: Option<String>,
    pub content: Option<String>,
    pub chunk_type: Option<String>,
    pub rationale: Option<String>,
    pub consequences: Option<String>,
}

pub async fn create(pool: &PgPool, user_id: &str, new: NewChunk) -> AppResult<Chunk> {
    let id = crate::new_id();
    let c = sqlx::query_as!(
        Chunk,
        r#"INSERT INTO chunk (id, title, content, type, user_id, rationale)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING id, title, content, type AS chunk_type, user_id, summary,
                     aliases AS "aliases: Json<Vec<String>>",
                     not_about AS "not_about: Json<Vec<String>>",
                     scope AS "scope: Json<serde_json::Value>",
                     rationale,
                     alternatives AS "alternatives: Json<Vec<String>>",
                     consequences,
                     embedding::text AS "embedding: EmbeddingVec",
                     embedding_updated_at AS "embedding_updated_at: UtcTimestamp",
                     origin, review_status, reviewed_by,
                     reviewed_at AS "reviewed_at: UtcTimestamp",
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp",
                     archived_at AS "archived_at: UtcTimestamp",
                     document_id, document_order, is_entry_point"#,
        id,
        new.title,
        new.content,
        new.chunk_type,
        user_id,
        new.rationale
    )
    .fetch_one(pool)
    .await?;
    Ok(c)
}

pub async fn find_by_id(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Option<Chunk>> {
    let c = sqlx::query_as!(
        Chunk,
        r#"SELECT id, title, content, type AS chunk_type, user_id, summary,
                  aliases AS "aliases: Json<Vec<String>>",
                  not_about AS "not_about: Json<Vec<String>>",
                  scope AS "scope: Json<serde_json::Value>",
                  rationale,
                  alternatives AS "alternatives: Json<Vec<String>>",
                  consequences,
                  embedding::text AS "embedding: EmbeddingVec",
                  embedding_updated_at AS "embedding_updated_at: UtcTimestamp",
                  origin, review_status, reviewed_by,
                  reviewed_at AS "reviewed_at: UtcTimestamp",
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp",
                  archived_at AS "archived_at: UtcTimestamp",
                  document_id, document_order, is_entry_point
           FROM chunk WHERE id = $1 AND user_id = $2"#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(c)
}

/// Applies only the fields present in the patch. COALESCE keeps unset
/// columns untouched, so a partial PATCH cannot silently clear data.
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    patch: ChunkPatch,
) -> AppResult<Option<Chunk>> {
    let c = sqlx::query_as!(
        Chunk,
        r#"UPDATE chunk SET
             title = COALESCE($3, title),
             content = COALESCE($4, content),
             type = COALESCE($5, type),
             rationale = COALESCE($6, rationale),
             consequences = COALESCE($7, consequences),
             updated_at = now()
           WHERE id = $1 AND user_id = $2
           RETURNING id, title, content, type AS chunk_type, user_id, summary,
                     aliases AS "aliases: Json<Vec<String>>",
                     not_about AS "not_about: Json<Vec<String>>",
                     scope AS "scope: Json<serde_json::Value>",
                     rationale,
                     alternatives AS "alternatives: Json<Vec<String>>",
                     consequences,
                     embedding::text AS "embedding: EmbeddingVec",
                     embedding_updated_at AS "embedding_updated_at: UtcTimestamp",
                     origin, review_status, reviewed_by,
                     reviewed_at AS "reviewed_at: UtcTimestamp",
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp",
                     archived_at AS "archived_at: UtcTimestamp",
                     document_id, document_order, is_entry_point"#,
        id,
        user_id,
        patch.title,
        patch.content,
        patch.chunk_type,
        patch.rationale,
        patch.consequences
    )
    .fetch_optional(pool)
    .await?;
    Ok(c)
}

pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let res = sqlx::query!(
        "DELETE FROM chunk WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Sort {
    #[default]
    Newest,
    Oldest,
    Alpha,
    Updated,
}

impl Sort {
    /// Lenient string -> `Sort` mapping mirroring Node's `switch (params.sort)`
    /// (`packages/db/src/repository/chunk.ts:128-141`): any unrecognised
    /// value (including `None`) silently falls through to `Newest`, the
    /// `default:` arm — it is never a parse error. Used when interpreting a
    /// collection's stored `filter.sort` string, which was never validated
    /// beyond "is a string" at write time (`CollectionFilterSchema`). This is
    /// deliberately looser than `Sort`'s own `Deserialize` impl, which the
    /// `GET /api/chunks` query-string path uses and which *does* reject an
    /// unrecognised value with 400 — see `ListChunksQuery` for that
    /// already-accepted divergence from Node.
    pub fn from_loose_str(s: Option<&str>) -> Self {
        match s {
            Some("oldest") => Sort::Oldest,
            Some("alpha") => Sort::Alpha,
            Some("updated") => Sort::Updated,
            _ => Sort::Newest,
        }
    }
}

/// Mirrors Node's `enrichment: "missing" | "complete"` filter
/// (`packages/db/src/repository/chunk.ts:123-127`). Used both as a strict
/// `GET /api/chunks?enrichment=` query-string enum (an unrecognised value is
/// a 400, same divergence as `Sort`) and, via `from_loose_str`, as a lenient
/// interpreter of a collection's stored `filter.enrichment` string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Enrichment {
    Missing,
    Complete,
}

impl Enrichment {
    /// Any value other than exactly `"missing"`/`"complete"` (including
    /// `None`) means "no enrichment filter" — matching Node's `if
    /// (params.enrichment === "missing") {...} else if (... === "complete")
    /// {...}` (no `else` branch, so any other string is silently a no-op).
    pub fn from_loose_str(s: Option<&str>) -> Option<Self> {
        match s {
            Some("missing") => Some(Enrichment::Missing),
            Some("complete") => Some(Enrichment::Complete),
            _ => None,
        }
    }
}

pub struct ListParams {
    pub chunk_type: Option<String>,
    pub search: Option<String>,
    pub origin: Option<String>,
    pub review_status: Option<String>,
    pub sort: Sort,
    /// OR semantics only (chunk has at least one of these tag names) —
    /// Node's `tagMode: "all"` AND-mode is not reproduced here: it is not
    /// one of `CollectionFilterSchema`'s nine keys, and isn't part of this
    /// port's scope (see `fubbik-api/src/chunks/dto.rs::ListChunksQuery`).
    pub tags: Option<Vec<String>>,
    /// Chunks whose `updated_at` is on or after this instant. Callers
    /// compute this from a "days ago" offset the same way Node's
    /// `listChunks` service does (`packages/api/src/chunks/service.ts:66`)
    /// — see `ListChunksQuery::into_params` and
    /// `collections::service::filter_to_list_params`.
    pub after: Option<chrono::NaiveDateTime>,
    pub enrichment: Option<Enrichment>,
    /// Only applied when `> 0` — matching Node's `if (params.minConnections
    /// && params.minConnections > 0)`, where `0` is falsy in JS and so is
    /// treated identically to "no filter", not "at least zero connections".
    pub min_connections: Option<i64>,
    /// `None` means no space filter at all — every chunk the caller owns,
    /// in any space or none. `Some(id)` matches chunks in *that* space
    /// **or** chunks with no space assignment at all (global chunks),
    /// matching Node's `listChunks`
    /// (`packages/db/src/repository/chunk.ts:108-111`):
    /// `or(chunk.id IN inSpace, chunk.id NOT IN inAnySpace)`. This is NOT
    /// "chunks in this space only" — a caller wanting that narrower
    /// behaviour would need a filter this port does not expose, same as
    /// Node.
    pub space_id: Option<String>,
    /// Restricts the result to exactly these ids (still ANDed with the
    /// mandatory `user_id = ..` predicate below, and with every other
    /// filter). Added for Task 9's graph-clause search wiring
    /// (`near`/`path`/`affected-by`): Apache AGE resolves matching chunk
    /// ids from outside this crate's user-scoped SQL entirely, so a graph
    /// edge can point at another user's chunk. Applying those ids as an
    /// ordinary filter *on top of* this function's own `user_id` predicate
    /// — rather than fetching by id first and trusting the result — is
    /// what keeps a resolved graph id from ever being able to bypass
    /// ownership scoping. See
    /// `crates/fubbik-api/tests/search.rs`'s cross-user graph-edge test.
    pub ids: Option<Vec<String>>,
    pub limit: i64,
    pub offset: i64,
}

impl Default for ListParams {
    fn default() -> Self {
        Self {
            chunk_type: None,
            search: None,
            origin: None,
            review_status: None,
            sort: Sort::Newest,
            tags: None,
            after: None,
            enrichment: None,
            min_connections: None,
            space_id: None,
            ids: None,
            limit: 50,
            offset: 0,
        }
    }
}

/// Appends the `WHERE` clause shared by [`list`] and [`count`]: the fixed
/// `archived_at IS NULL AND user_id = ..` constraint plus every optional
/// filter. Both functions call this *same* function rather than each
/// hand-rolling their own copy of the filter conditions, so `total` from
/// `count` can never silently drift from what `list` actually returns —
/// every value is still pushed as a bind parameter, never formatted into
/// the SQL string.
fn push_filters<'a>(
    qb: &mut sqlx::QueryBuilder<'a, sqlx::Postgres>,
    user_id: &'a str,
    params: &'a ListParams,
) {
    qb.push(" WHERE archived_at IS NULL AND user_id = ");
    qb.push_bind(user_id);

    if let Some(t) = &params.chunk_type {
        qb.push(" AND type = ").push_bind(t);
    }
    if let Some(o) = &params.origin {
        qb.push(" AND origin = ").push_bind(o);
    }
    if let Some(r) = &params.review_status {
        qb.push(" AND review_status = ").push_bind(r);
    }
    if let Some(s) = &params.search {
        // ILIKE with escaped wildcards: a user searching for "100%" must not
        // match everything.
        let pattern = format!(
            "%{}%",
            s.replace('\\', r"\\")
                .replace('%', r"\%")
                .replace('_', r"\_")
        );
        qb.push(" AND (title ILIKE ").push_bind(pattern.clone());
        qb.push(" OR content ILIKE ").push_bind(pattern);
        qb.push(")");
    }
    if let Some(tags) = &params.tags {
        // OR semantics: the chunk must carry at least one of the named
        // tags. Deliberately NOT scoped by `tag.user_id` here, matching
        // Node's `inArray(tag.name, params.tags)`
        // (`packages/db/src/repository/chunk.ts:77-81`), which matches tag
        // *names* globally with no owner check on the `tag` row itself.
        // This can never leak another user's chunk: the join only proves a
        // `chunk_tag` row exists for a *specific* `chunk.id`, and every row
        // this query can return already satisfies the mandatory `user_id =
        // ..` predicate above — the tag-name lookup can narrow the result
        // set, never widen it past that boundary. See
        // `tests/chunk.rs::tags_filter_cannot_leak_another_users_chunk_via_a_same_named_tag`.
        qb.push(
            " AND id IN (SELECT chunk_tag.chunk_id FROM chunk_tag \
              JOIN tag ON tag.id = chunk_tag.tag_id WHERE tag.name = ANY(",
        );
        qb.push_bind(tags);
        qb.push("))");
    }
    if let Some(after) = &params.after {
        qb.push(" AND updated_at >= ").push_bind(*after);
    }
    if let Some(ids) = &params.ids {
        qb.push(" AND id = ANY(").push_bind(ids).push(")");
    }
    if let Some(min_connections) = params.min_connections
        && min_connections > 0
    {
        qb.push(
            " AND (SELECT COUNT(*) FROM chunk_connection cc \
              WHERE cc.source_id = chunk.id OR cc.target_id = chunk.id) >= ",
        );
        qb.push_bind(min_connections);
    }
    match params.enrichment {
        Some(Enrichment::Missing) => {
            qb.push(
                " AND (summary IS NULL OR embedding IS NULL OR jsonb_array_length(aliases) = 0)",
            );
        }
        Some(Enrichment::Complete) => {
            qb.push(" AND summary IS NOT NULL AND embedding IS NOT NULL");
        }
        None => {}
    }
    if let Some(space_id) = &params.space_id {
        // Matches Node's `listChunks` spaceId branch
        // (`packages/db/src/repository/chunk.ts:108-111`) exactly: a chunk
        // in the named space, OR a chunk with no space assignment at all
        // (global chunks always pass through every space filter). This is
        // deliberately not "chunks in this space only" — that would be a
        // narrower filter Node itself does not implement here. Not scoped
        // by a `space.user_id` ownership check in this predicate itself;
        // callers are expected to have already verified the space belongs
        // to `user_id` (or accepted returning nothing for a foreign/bogus
        // id) the same way Node does no such check in `listChunks` either
        // — the mandatory `chunk.user_id = ..` predicate above is what
        // keeps this from ever returning another user's chunk regardless.
        qb.push(" AND (id IN (SELECT chunk_id FROM chunk_space WHERE space_id = ");
        qb.push_bind(space_id);
        qb.push(") OR id NOT IN (SELECT chunk_id FROM chunk_space))");
    }
}

/// Lists a user's non-archived chunks.
///
/// Uses QueryBuilder rather than `query_as!` because the filter set is
/// dynamic.
pub async fn list(pool: &PgPool, user_id: &str, params: &ListParams) -> AppResult<Vec<Chunk>> {
    let mut qb = sqlx::QueryBuilder::new(
        "SELECT id, title, content, type AS chunk_type, user_id, summary, \
         aliases, not_about, scope, rationale, alternatives, consequences, \
         embedding::text AS embedding, embedding_updated_at, \
         origin, review_status, reviewed_by, reviewed_at, \
         created_at, updated_at, archived_at, \
         document_id, document_order, is_entry_point \
         FROM chunk",
    );
    push_filters(&mut qb, user_id, params);

    // Every branch appends `, id ASC` as a tiebreaker. `created_at`/
    // `updated_at`/`title` are NOT unique — seed data (or any batch
    // insert) routinely produces ties, and `ORDER BY` over tied rows with
    // no deterministic tiebreaker is a query-plan artifact: it can differ
    // between two calls, or between the query serving page 1 and the one
    // serving page 2 of the same `LIMIT`/`OFFSET` walk, silently skipping
    // or duplicating rows across pages. `id` is the primary key, so it is
    // always unique and always present, making the final order total.
    qb.push(match params.sort {
        Sort::Newest => " ORDER BY created_at DESC, id ASC",
        Sort::Oldest => " ORDER BY created_at ASC, id ASC",
        Sort::Alpha => " ORDER BY title ASC, id ASC",
        Sort::Updated => " ORDER BY updated_at DESC, id ASC",
    });

    // Clamped to 100, the same cap `chunks::dto::ListChunksQuery::into_params`
    // applies before `GET /api/chunks` ever reaches here (divergence #11).
    // This clamp is the *only* one `POST /api/search/query` hits — nothing
    // upstream in `search::service::build_list_params` pre-clamps — so this
    // single line is the shared cap for both chunk-listing endpoints.
    // Deliberately still a floor of 1, not 0: `limit: 0` clamps *up* to 1
    // row, matching this port's existing (documented) lower-bound
    // behaviour, not "no rows" — see `chunk_list_limit_is_clamped_identically_above_both_caps`
    // (`fubbik-api/tests/differential.rs`) and `query_limit_is_clamped_to_100`
    // (`fubbik-api/tests/search.rs`) for the pinned cases. Node has no cap on
    // the search path at all (search bypasses the chunks service where the
    // 100-cap lives), so `limit: 1000` is a documented divergence: Node
    // returns 1000 rows, this port 100.
    qb.push(" LIMIT ").push_bind(params.limit.clamp(1, 100));
    qb.push(" OFFSET ").push_bind(params.offset.max(0));

    let rows = qb.build_query_as::<Chunk>().fetch_all(pool).await?;
    Ok(rows)
}

/// Counts the rows [`list`] would return for the same filters, WITHOUT
/// `LIMIT`/`OFFSET` applied — that is what makes pagination on the client
/// work (`total` must reflect the whole matching set, not just the current
/// page). Shares `push_filters` with `list` so the two can never disagree
/// about which rows match.
pub async fn count(pool: &PgPool, user_id: &str, params: &ListParams) -> AppResult<i64> {
    let mut qb = sqlx::QueryBuilder::new("SELECT COUNT(*) FROM chunk");
    push_filters(&mut qb, user_id, params);

    let total: i64 = qb.build_query_scalar().fetch_one(pool).await?;
    Ok(total)
}

/// Narrows an arbitrary id list down to the ones `user_id` may see: not
/// archived, and owned by `user_id`. Exists for callers that receive chunk
/// ids from a source with no ownership notion at all — Apache AGE graph
/// traversal (`fubbik_db::age`), specifically — and need to redact hidden
/// ids from a response payload *before* it's built, not just filter the
/// final chunk list. `chunk::list`/`count`'s `ids` filter already keeps a
/// foreign id from ever hydrating into a returned `Chunk` row; this
/// function is for the narrower case of scrubbing a bare id (or an edge
/// list referencing one) that would otherwise be echoed back verbatim in
/// metadata never routed through `list`/`count` at all. See
/// `fubbik-api/src/search/service.rs`'s `path:` clause handling and
/// `tests/chunk.rs::filter_visible_ids_drops_another_users_chunk`.
///
/// Uses `QueryBuilder` rather than `query_as!`/`query_scalar!` to match
/// [`list`]/[`count`]'s own style in this module, and to avoid a new
/// `.sqlx` cache entry for what is otherwise a one-line query.
pub async fn filter_visible_ids(
    pool: &PgPool,
    user_id: &str,
    ids: &[String],
) -> AppResult<Vec<String>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut qb =
        sqlx::QueryBuilder::new("SELECT id FROM chunk WHERE archived_at IS NULL AND user_id = ");
    qb.push_bind(user_id);
    qb.push(" AND id = ANY(");
    qb.push_bind(ids);
    qb.push(")");

    let rows: Vec<String> = qb.build_query_scalar().fetch_all(pool).await?;
    Ok(rows)
}

/// One `(id, title)` match from [`search_titles`].
#[derive(Debug, Clone)]
pub struct ChunkTitleMatch {
    pub id: String,
    pub title: String,
}

/// Backs `GET /api/search/autocomplete?field=chunk`. Direct port of
/// Node's `searchChunkTitles` (`packages/db/src/repository/chunk.ts:586-594`),
/// with one intentional deviation — divergence #17 (Phase 2c task 8b):
/// Node's original has **no `user_id` filter at all**, leaking every
/// user's chunk titles into the nav search bar's autocomplete. The human
/// partner decided to scope it here; this port now adds `AND user_id =
/// $2`, matching every other chunk query in this crate.
///
/// Everything else remains a faithful port, including what still looks
/// like a bug:
///
/// - **Not filtered by `archived_at IS NULL`.** Archived chunks' titles
///   are still suggested.
/// - **`ILIKE '%prefix%'` — contains, not a prefix match** — and the
///   pattern is **not escaped** (`%`/`_` in `prefix` act as wildcards),
///   unlike `push_filters`'s `search` branch, which does escape them. Two
///   different call sites, two different (both faithfully ported)
///   behaviours.
/// - **No `ORDER BY`.** Result order is whatever Postgres's query plan
///   happens to produce.
pub async fn search_titles(
    pool: &PgPool,
    user_id: &str,
    prefix: &str,
    limit: i64,
) -> AppResult<Vec<ChunkTitleMatch>> {
    let pattern = format!("%{prefix}%");
    let rows = sqlx::query_as!(
        ChunkTitleMatch,
        r#"SELECT id, title FROM chunk WHERE title ILIKE $1 AND user_id = $2 LIMIT $3"#,
        pattern,
        user_id,
        limit
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
