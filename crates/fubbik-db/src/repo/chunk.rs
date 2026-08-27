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

/// Everything `POST /api/chunks` can set at insert time, matching Node's
/// `createChunkRepo` call in `chunk-mutations.ts:76-90`.
///
/// `tags` and `space_ids` are deliberately absent: both are join tables
/// written by separate calls *after* the insert, exactly as Node does it.
///
/// `origin` and `review_status` are supplied by the caller rather than left
/// to the column defaults, because Node derives them together — `origin =
/// "ai"` implies `review_status = "draft"`, anything else implies
/// `"approved"` (`chunk-mutations.ts:75,86`). Deferring to the DB default
/// would break that pairing silently; the vocabularies port's
/// `display_order` divergence is the precedent for spelling it out here.
pub struct NewChunk {
    pub title: String,
    pub content: String,
    pub chunk_type: String,
    pub rationale: Option<String>,
    pub alternatives: Option<Vec<String>>,
    pub consequences: Option<String>,
    pub origin: String,
    pub review_status: String,
    pub document_id: Option<String>,
    pub document_order: Option<i32>,
}

/// `origin`/`review_status` default to the **human** pair, not to `""` — a
/// blank origin is not a state any code path should be able to reach by
/// forgetting a field. This is the same pairing `review_status_for_origin`
/// computes for `origin = "human"`; the two must not drift, and a test that
/// wants the `ai`/`draft` pair has to name both explicitly.
impl Default for NewChunk {
    fn default() -> Self {
        Self {
            title: String::new(),
            content: String::new(),
            chunk_type: "note".to_string(),
            rationale: None,
            alternatives: None,
            consequences: None,
            origin: "human".to_string(),
            review_status: "approved".to_string(),
            document_id: None,
            document_order: None,
        }
    }
}

/// `alternatives`/`scope` are plain two-state (`None` = leave untouched,
/// `Some` = replace wholesale) — matching Node's `UpdateChunkParams`, which
/// has no explicit-null variant for either
/// (`packages/db/src/repository/chunk.ts:311-328`: both are typed
/// `alternatives?: string[]` / `scope?: Record<string, string>`, never
/// `| null`). Added so `chunk::update` can carry the two fields Node's
/// `updateChunk` applies that this struct was previously missing — see
/// `fubbik_api::proposals::service::approve_proposal`'s doc comment for why
/// that mattered (proposal approval was silently dropping them). Both
/// default to `None` via `#[derive(Default)]`, so every pre-existing caller
/// that builds a `ChunkPatch` without naming these two fields keeps its
/// exact prior behaviour (`COALESCE` leaves the column untouched).
///
/// `summary` is the one genuinely **tri-state** field: Node's PATCH schema
/// types it `t.Optional(t.Union([t.String(), t.Null()]))`, and its repo
/// spreads on `!== undefined`, so an explicit `null` clears the column
/// while omitting the key leaves it alone. `Option<Option<String>>` is the
/// faithful shape — flattening it to `Option<String>` would make "clear the
/// summary" unreachable. Every other field here has no null variant in
/// Node's schema and so stays two-state.
///
/// `reviewed_by`/`reviewed_at` are not client-settable. The service stamps
/// them whenever `review_status` is present, matching
/// `chunk-mutations.ts:175-178`.
#[derive(Default)]
pub struct ChunkPatch {
    pub title: Option<String>,
    pub content: Option<String>,
    pub chunk_type: Option<String>,
    pub rationale: Option<String>,
    pub consequences: Option<String>,
    pub alternatives: Option<Vec<String>>,
    pub scope: Option<serde_json::Value>,
    /// Tri-state — see the doc comment above.
    pub summary: Option<Option<String>>,
    pub aliases: Option<Vec<String>>,
    pub not_about: Option<Vec<String>>,
    pub origin: Option<String>,
    pub review_status: Option<String>,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<chrono::NaiveDateTime>,
    pub is_entry_point: Option<bool>,
    pub document_order: Option<i32>,
}

pub async fn create(pool: &PgPool, user_id: &str, new: NewChunk) -> AppResult<Chunk> {
    let id = crate::new_id();
    let c = sqlx::query_as!(
        Chunk,
        r#"INSERT INTO chunk (id, title, content, type, user_id, rationale,
                              alternatives, consequences, origin, review_status,
                              document_id, document_order)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
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
        // Node trims the title inside `createChunk`'s repo call
        // (`packages/db/src/repository/chunk.ts:302`), not in the service.
        new.title.trim(),
        new.content,
        new.chunk_type,
        user_id,
        new.rationale,
        new.alternatives.map(Json) as _,
        new.consequences,
        new.origin,
        new.review_status,
        new.document_id,
        new.document_order
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
             alternatives = COALESCE($8, alternatives),
             scope = COALESCE($9, scope),
             -- `summary` is tri-state, so COALESCE alone cannot express it:
             -- COALESCE($n, summary) can never write NULL. `$10` says
             -- "touch this column at all", `$11` carries the value (which
             -- may legitimately be NULL). See `ChunkPatch::summary`.
             summary = CASE WHEN $10 THEN $11 ELSE summary END,
             aliases = COALESCE($12, aliases),
             not_about = COALESCE($13, not_about),
             origin = COALESCE($14, origin),
             review_status = COALESCE($15, review_status),
             reviewed_by = COALESCE($16, reviewed_by),
             reviewed_at = COALESCE($17, reviewed_at),
             is_entry_point = COALESCE($18, is_entry_point),
             document_order = COALESCE($19, document_order),
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
        patch.consequences,
        patch.alternatives.map(Json) as _,
        patch.scope.map(Json) as _,
        patch.summary.is_some(),
        patch.summary.flatten(),
        patch.aliases.map(Json) as _,
        patch.not_about.map(Json) as _,
        patch.origin,
        patch.review_status,
        patch.reviewed_by,
        patch.reviewed_at,
        patch.is_entry_point,
        patch.document_order
    )
    .fetch_optional(pool)
    .await?;
    Ok(c)
}

/// Sparse patch for the AI-written columns. `None` means "leave the column
/// alone" — mirroring Node's conditional spread at
/// `packages/db/src/repository/chunk.ts:379-383`, where an absent key is
/// simply not part of the `SET`.
#[derive(Debug, Default, Clone)]
pub struct EnrichmentPatch {
    pub summary: Option<String>,
    pub aliases: Option<Vec<String>>,
    pub not_about: Option<Vec<String>>,
    pub embedding: Option<Vec<f32>>,
}

/// Writes the enrichment columns and returns the updated row.
///
/// `COALESCE` gives the "absent means unchanged" semantics without building
/// the SQL dynamically, which would defeat `query_as!`'s compile-time
/// checking. `embedding_updated_at` is stamped only when an embedding is
/// actually supplied — Node ties the two together in the same conditional
/// spread, so a summary-only patch must not move the timestamp.
///
/// `None` means "leave this column unchanged" — never "set it to NULL".
/// `COALESCE($n, col)` cannot express clearing a column, so this diverges
/// from Node's `EnrichChunkParams.summary`, which is typed `string | null`
/// and can blank a summary via an explicit `null` (its conditional spread
/// keys off `!== undefined`, not truthiness). No current caller needs to
/// clear a column through this function — `packages/api/src/enrich/service.ts`
/// only ever writes non-null values — so the gap is intentional, not an
/// oversight, but it means this function cannot express what Node's type
/// allows.
///
/// `scope` is deliberately not part of `EnrichmentPatch`, even though
/// Node's `EnrichChunkParams` (`chunk.ts:365-372`) has an optional `scope`
/// spread the same way as the other fields. The only real caller
/// (`packages/api/src/enrich/service.ts:39`) never populates it, so adding
/// a fifth COALESCE branch (and the test to pin it) here would cover a
/// capability nothing exercises.
pub async fn update_chunk_enrichment(
    pool: &PgPool,
    chunk_id: &str,
    params: EnrichmentPatch,
) -> AppResult<Option<Chunk>> {
    // pgvector has no text input parser reachable through sqlx's inferred
    // parameter types, so the vector goes over the wire as text and is cast
    // in SQL — the exact mirror of how `embedding::text` reads it back.
    let embedding_text = params.embedding.as_ref().map(|v| {
        let joined = v
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(",");
        format!("[{joined}]")
    });

    let aliases = params.aliases.map(Json) as Option<Json<Vec<String>>>;
    let not_about = params.not_about.map(Json) as Option<Json<Vec<String>>>;

    let row = sqlx::query_as!(
        Chunk,
        r#"
        UPDATE chunk SET
            summary    = COALESCE($2, summary),
            aliases    = COALESCE($3, aliases),
            not_about  = COALESCE($4, not_about),
            embedding  = COALESCE($5::text::vector, embedding),
            embedding_updated_at = CASE
                WHEN $5::text IS NULL THEN embedding_updated_at
                ELSE now()
            END
        WHERE id = $1
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
                  document_id, document_order, is_entry_point
        "#,
        chunk_id,
        params.summary,
        aliases as _,
        not_about as _,
        embedding_text,
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
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

// ---------------------------------------------------------------------------
// Lifecycle: archive, restore, bulk operations, merge
// ---------------------------------------------------------------------------

/// Soft-deletes by stamping `archived_at`. Returns `Ok(None)` for a chunk the
/// caller does not own.
pub async fn archive(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Option<Chunk>> {
    let c = sqlx::query_as!(
        Chunk,
        r#"UPDATE chunk SET archived_at = now()
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
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(c)
}

/// Clears `archived_at`. Note neither this nor [`archive`] touches
/// `updated_at` — archiving is not an edit, and bumping it would make every
/// archived chunk look freshly modified in the health panel.
pub async fn restore(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Option<Chunk>> {
    let c = sqlx::query_as!(
        Chunk,
        r#"UPDATE chunk SET archived_at = NULL
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
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(c)
}

/// Archived chunks, most recently archived first.
///
/// `, id` is this port's tiebreaker: `archive_many` stamps every row in one
/// statement, so a bulk archive gives dozens of rows an identical
/// `archived_at` and Node's `ORDER BY archived_at DESC` alone leaves their
/// order to the query plan.
///
/// The `space_id` filter here is a plain "in this space", NOT the
/// "or in no space at all" form `chunk::list` and the health queries use —
/// Node's `listArchivedChunks` omits the global-chunk half. Reproduced
/// rather than harmonised; the inconsistency is Node's.
pub async fn list_archived(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<Chunk>> {
    let rows = sqlx::query_as!(
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
           FROM chunk
           WHERE user_id = $1 AND archived_at IS NOT NULL
             AND ($2::text IS NULL
                  OR id IN (SELECT chunk_id FROM chunk_space WHERE space_id = $2))
           ORDER BY archived_at DESC, id"#,
        user_id,
        space_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Archives every id the caller owns, returning how many were affected.
/// Ids belonging to someone else are silently skipped — the service checks
/// ownership up front and rejects the whole batch, so reaching this with a
/// foreign id means a caller bypassed that check.
pub async fn archive_many(pool: &PgPool, user_id: &str, ids: &[String]) -> AppResult<u64> {
    if ids.is_empty() {
        return Ok(0);
    }
    let n = sqlx::query!(
        "UPDATE chunk SET archived_at = now() WHERE id = ANY($1) AND user_id = $2",
        ids,
        user_id
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(n)
}

pub async fn delete_many(pool: &PgPool, user_id: &str, ids: &[String]) -> AppResult<u64> {
    if ids.is_empty() {
        return Ok(0);
    }
    let n = sqlx::query!(
        "DELETE FROM chunk WHERE id = ANY($1) AND user_id = $2",
        ids,
        user_id
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(n)
}

/// Sets `type` and/or `review_status` across a batch. Node's
/// `updateManyChunks` accepts exactly these two columns and no others.
///
/// Unlike the single-chunk `update`, this does NOT bump `updated_at` —
/// faithful to Node, whose `.set(data)` passes only the named columns. Worth
/// noting because it means a bulk retype leaves the chunk looking untouched
/// to the staleness scanner.
pub async fn update_many(
    pool: &PgPool,
    user_id: &str,
    ids: &[String],
    chunk_type: Option<&str>,
    review_status: Option<&str>,
) -> AppResult<u64> {
    if ids.is_empty() {
        return Ok(0);
    }
    let n = sqlx::query!(
        r#"UPDATE chunk SET
             type = COALESCE($3, type),
             review_status = COALESCE($4, review_status)
           WHERE id = ANY($1) AND user_id = $2"#,
        ids,
        user_id,
        chunk_type,
        review_status
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(n)
}

/// Folds `source` into `target` and deletes the source, in one transaction.
///
/// A direct port of Node's `mergeChunks`
/// (`packages/db/src/repository/chunk.ts:415-513`). Both chunks must belong
/// to `user_id`; `Ok(None)` if either does not.
///
/// The order of operations matters and is preserved:
///
/// 1. **Join tables with unique constraints** (`chunk_tag`, `chunk_space`,
///    `favorite`) are copied with `ON CONFLICT DO NOTHING`, then the source's
///    rows deleted — a plain re-parent would violate the constraint whenever
///    both chunks share a tag/space/favouriter.
/// 2. **Connections** are repointed source-side first, then target-side, each
///    guarded by a `NOT EXISTS` against `(source, target, relation)` so a
///    duplicate edge is dropped rather than colliding. `DELETE ... WHERE
///    source_id = target_id` afterwards removes the self-loop that appears
///    when source and target were already connected to each other.
/// 3. **Plain re-parents** (`chunk_file_ref`, `chunk_applies_to`,
///    `plan_task_chunk`, `plan_analyze_item`) have no unique constraint to
///    trip, so they move wholesale.
/// 4. **Content** is appended under a `## Merged from "<title>"` heading,
///    but only if the source body is non-empty and not already contained in
///    the target — merging twice does not duplicate the text.
/// 5. **The source row is deleted last**, and its cascades take
///    `chunk_version`, `chunk_staleness` and `chunk_proposal` with it.
///
/// Note step 5 means the source's **version history is destroyed**, not
/// moved. That is Node's behaviour and is reproduced, but it makes a merge
/// irreversible in a way the UI does not warn about.
pub async fn merge(
    pool: &PgPool,
    user_id: &str,
    source_id: &str,
    target_id: &str,
) -> AppResult<Option<Chunk>> {
    if source_id == target_id {
        return Ok(None);
    }
    let mut tx = pool.begin().await?;

    let rows = sqlx::query!(
        "SELECT id, title, content FROM chunk WHERE id = ANY($1) AND user_id = $2",
        &[source_id.to_string(), target_id.to_string()][..],
        user_id
    )
    .fetch_all(&mut *tx)
    .await?;

    let source = rows.iter().find(|r| r.id == source_id);
    let target = rows.iter().find(|r| r.id == target_id);
    let (Some(source), Some(target)) = (source, target) else {
        return Ok(None);
    };
    let source_title = source.title.clone();
    let source_body = source.content.trim().to_string();
    let target_body = target.content.clone();

    sqlx::query!(
        "INSERT INTO chunk_tag (chunk_id, tag_id)
         SELECT $2, tag_id FROM chunk_tag WHERE chunk_id = $1
         ON CONFLICT (chunk_id, tag_id) DO NOTHING",
        source_id,
        target_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("DELETE FROM chunk_tag WHERE chunk_id = $1", source_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query!(
        "INSERT INTO chunk_space (chunk_id, space_id)
         SELECT $2, space_id FROM chunk_space WHERE chunk_id = $1
         ON CONFLICT (chunk_id, space_id) DO NOTHING",
        source_id,
        target_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("DELETE FROM chunk_space WHERE chunk_id = $1", source_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query!(
        "UPDATE chunk_connection SET source_id = $2
         WHERE source_id = $1
           AND NOT EXISTS (SELECT 1 FROM chunk_connection c2
                           WHERE c2.source_id = $2
                             AND c2.target_id = chunk_connection.target_id
                             AND c2.relation = chunk_connection.relation)",
        source_id,
        target_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "DELETE FROM chunk_connection WHERE source_id = $1",
        source_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE chunk_connection SET target_id = $2
         WHERE target_id = $1
           AND NOT EXISTS (SELECT 1 FROM chunk_connection c2
                           WHERE c2.target_id = $2
                             AND c2.source_id = chunk_connection.source_id
                             AND c2.relation = chunk_connection.relation)",
        source_id,
        target_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "DELETE FROM chunk_connection WHERE target_id = $1",
        source_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("DELETE FROM chunk_connection WHERE source_id = target_id")
        .execute(&mut *tx)
        .await?;

    // Four plain re-parents, spelled out one statement each: sqlx's
    // compile-time macro needs a literal query string, so a loop over table
    // names is not available here.
    sqlx::query!(
        "UPDATE chunk_file_ref SET chunk_id = $2 WHERE chunk_id = $1",
        source_id,
        target_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE chunk_applies_to SET chunk_id = $2 WHERE chunk_id = $1",
        source_id,
        target_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE plan_task_chunk SET chunk_id = $2 WHERE chunk_id = $1",
        source_id,
        target_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE plan_analyze_item SET chunk_id = $2 WHERE chunk_id = $1",
        source_id,
        target_id
    )
    .execute(&mut *tx)
    .await?;

    // DIVERGENCE, and the reason Node's merge cannot work at all.
    //
    // Node's raw SQL here says `INSERT INTO favorite ... FROM favorite`
    // (`packages/db/src/repository/chunk.ts:483-488`). There is no relation
    // named `favorite` — the table is `user_favorite` — so the statement
    // raises `relation "favorite" does not exist` inside the transaction and
    // rolls the whole merge back. `POST /api/chunks/merge` has therefore
    // never succeeded on Node.
    //
    // A second bug hides behind the first: even with the name corrected, the
    // column list omits `id`, which is `text NOT NULL` with no default, so
    // the INSERT fails with a not-null violation the moment the source chunk
    // has any favourites. Both verified by executing the statements directly.
    //
    // Not reproduced. There is no observable Node behaviour to be faithful
    // to here — the endpoint is unreachable — so this port implements what
    // the code plainly intends: carry the source's favourites over, keeping
    // each favouriter's original `created_at`, and generate the required id.
    sqlx::query!(
        r#"INSERT INTO user_favorite (id, user_id, chunk_id, "order", created_at)
           SELECT md5(random()::text || clock_timestamp()::text), user_id, $2,
                  "order", created_at
           FROM user_favorite WHERE chunk_id = $1
           ON CONFLICT (user_id, chunk_id) DO NOTHING"#,
        source_id,
        target_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("DELETE FROM user_favorite WHERE chunk_id = $1", source_id)
        .execute(&mut *tx)
        .await?;

    let merged_content = if !source_body.is_empty() && !target_body.contains(&source_body) {
        format!("{target_body}\n\n## Merged from \"{source_title}\"\n\n{source_body}")
    } else {
        target_body
    };

    let updated = sqlx::query_as!(
        Chunk,
        r#"UPDATE chunk SET content = $3, updated_at = now()
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
        target_id,
        user_id,
        merged_content
    )
    .fetch_optional(&mut *tx)
    .await?;

    sqlx::query!(
        "DELETE FROM chunk WHERE id = $1 AND user_id = $2",
        source_id,
        user_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(updated)
}

/// Just the ids for a user's chunks, for `enrich-all`'s sweep
/// (`enrich/routes.ts:35`, `listChunks(userId, { limit: "1000", offset: "0" })`).
/// `chunk::list` takes a large non-`Default` `ListParams` struct built for
/// the full listing endpoint; constructing one purely to read ids back out
/// would be more code than this direct query.
pub async fn list_ids_for_user(pool: &PgPool, user_id: &str, limit: i64) -> AppResult<Vec<String>> {
    let ids = sqlx::query_scalar!(
        "SELECT id FROM chunk WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2",
        user_id,
        limit
    )
    .fetch_all(pool)
    .await?;
    Ok(ids)
}
