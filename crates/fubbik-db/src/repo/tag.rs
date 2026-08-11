use fubbik_core::error::{AppError, AppResult};
use sqlx::PgPool;

use crate::timestamp::UtcTimestamp;

/// `camelCase` serialisation matches every other wire type in this crate —
/// see the note on `chunk::Chunk` for why that's mandatory, not cosmetic.
///
/// This is the *bare* row shape — what `POST /api/tags` and
/// `PATCH /api/tags/{id}` return. `GET /api/tags` returns a different,
/// joined shape ([`TagListItem`]); Node's `service-new.ts` explicitly notes
/// create/update responses carry no `tagTypeName`/`tagTypeColor`/
/// `chunkCount` fields, unlike the list endpoint.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub tag_type_id: Option<String>,
    pub user_id: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    pub origin: String,
    pub review_status: String,
    pub reviewed_by: Option<String>,
    #[schema(value_type = Option<chrono::NaiveDateTime>)]
    pub reviewed_at: Option<UtcTimestamp>,
}

/// Shape of `GET /api/tags` list items — joined with `tag_type` and a
/// `chunk_tag` count, matching Node's `getTagsForUser`
/// (`packages/db/src/repository/tag-new.ts:23-38`).
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TagListItem {
    pub id: String,
    pub name: String,
    pub tag_type_id: Option<String>,
    pub tag_type_name: Option<String>,
    pub tag_type_color: Option<String>,
    pub tag_type_icon: Option<String>,
    pub chunk_count: i64,
}

/// Return value of [`merge`], matching Node's `mergeTags` return shape
/// (`packages/db/src/repository/tag-new.ts:105-107`).
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MergeResult {
    pub target_id: String,
    pub chunk_count: i64,
}

/// `tag_type_id` has no database default (nullable, no `DEFAULT`), unlike
/// `tag_type.color` — so unlike `tag_type::create`, `None` here is just
/// bound straight through as `NULL`, no branching needed.
pub async fn create(
    pool: &PgPool,
    user_id: &str,
    name: &str,
    tag_type_id: Option<&str>,
) -> AppResult<Tag> {
    let id = crate::new_id();
    let t = sqlx::query_as!(
        Tag,
        r#"INSERT INTO tag (id, name, tag_type_id, user_id)
           VALUES ($1, $2, $3, $4)
           RETURNING id, name, tag_type_id, user_id,
                     created_at AS "created_at: UtcTimestamp",
                     origin, review_status, reviewed_by,
                     reviewed_at AS "reviewed_at: UtcTimestamp""#,
        id,
        name,
        tag_type_id,
        user_id
    )
    .fetch_one(pool)
    .await?;
    Ok(t)
}

/// Lists a user's tags, joined with their tag type and a live count of
/// attached chunks. `LEFT JOIN`s throughout: a tag with no type, or no
/// chunks, must still appear.
///
/// `, t.id ASC` is a tiebreaker: tags created in the same batch (seed data,
/// a bulk import) can share `created_at` exactly, and `ORDER BY
/// created_at` alone over tied rows is a query-plan artifact — see
/// `chunk::list`'s equivalent comment. Note this makes Rust's ordering
/// stricter than Node's `getTagsForUser`, which has no `ORDER BY` at all;
/// see the module doc on `differential.rs` for that divergence.
pub async fn list(pool: &PgPool, user_id: &str) -> AppResult<Vec<TagListItem>> {
    let rows = sqlx::query_as!(
        TagListItem,
        r#"SELECT t.id, t.name, t.tag_type_id,
                  tt.name AS "tag_type_name?",
                  tt.color AS "tag_type_color?",
                  tt.icon AS "tag_type_icon?",
                  COUNT(ct.chunk_id) AS "chunk_count!"
           FROM tag t
           LEFT JOIN tag_type tt ON tt.id = t.tag_type_id
           LEFT JOIN chunk_tag ct ON ct.tag_id = t.id
           WHERE t.user_id = $1
           GROUP BY t.id, tt.id
           ORDER BY t.created_at ASC, t.id ASC"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// `true` if another tag (not `id`) owned by `user_id` already has `name`.
/// Mirrors Node's `tagNameConflict` (tag-new.ts:65-74) — called by the
/// service layer *before* a rename, so a collision surfaces as a 400
/// `ValidationError` instead of the DB's `tag_user_name_idx` unique-index
/// violation bubbling up as a raw 500.
pub async fn name_conflict(pool: &PgPool, user_id: &str, id: &str, name: &str) -> AppResult<bool> {
    let hit = sqlx::query_scalar!(
        "SELECT id FROM tag WHERE user_id = $1 AND name = $2 AND id != $3 LIMIT 1",
        user_id,
        name,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(hit.is_some())
}

/// Every field but `name` is tri-state on `tag_type_id`: `None` leaves the
/// column untouched, `Some(None)` clears it, `Some(Some(id))` sets it —
/// mirroring Node's `t.Optional(t.Union([t.String(), t.Null()]))` body
/// schema for `tagTypeId`, which a plain `COALESCE` cannot express (it
/// cannot distinguish "omitted" from "explicitly null").
///
/// `review_status`/`reviewed_by`/`reviewed_at` change together or not at
/// all — the service layer sets the latter two only when `review_status`
/// is `Some`, so a plain `COALESCE` is sufficient for them.
#[derive(Default)]
pub struct TagPatch {
    pub name: Option<String>,
    pub tag_type_id: Option<Option<String>>,
    pub review_status: Option<String>,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<UtcTimestamp>,
}

pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    patch: TagPatch,
) -> AppResult<Option<Tag>> {
    let (tag_type_id_set, tag_type_id_val) = match patch.tag_type_id {
        Some(v) => (true, v),
        None => (false, None),
    };
    let t = sqlx::query_as!(
        Tag,
        r#"UPDATE tag SET
             name = COALESCE($3, name),
             tag_type_id = CASE WHEN $4::bool THEN $5::text ELSE tag_type_id END,
             review_status = COALESCE($6, review_status),
             reviewed_by = COALESCE($7, reviewed_by),
             reviewed_at = COALESCE($8, reviewed_at)
           WHERE id = $1 AND user_id = $2
           RETURNING id, name, tag_type_id, user_id,
                     created_at AS "created_at: UtcTimestamp",
                     origin, review_status, reviewed_by,
                     reviewed_at AS "reviewed_at: UtcTimestamp""#,
        id,
        user_id,
        patch.name,
        tag_type_id_set,
        tag_type_id_val,
        patch.review_status,
        patch.reviewed_by,
        patch.reviewed_at.map(chrono::NaiveDateTime::from)
    )
    .fetch_optional(pool)
    .await?;
    Ok(t)
}

/// Deletes a tag. `chunk_tag` rows referencing it via `tag_id` are removed
/// by the database's own `ON DELETE CASCADE` foreign key
/// (`migrations/0001_init.sql`, `chunk_tag_tag_id_tag_id_fk`) — this
/// function deliberately does not touch `chunk_tag` itself, matching Node
/// (`deleteTag`, tag-new.ts:111-119).
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let res = sqlx::query!(
        "DELETE FROM tag WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// Merges `source_id` into `target_id`, matching Node's `mergeTags`
/// algorithm exactly (`packages/db/src/repository/tag-new.ts:76-109`), with
/// one approved divergence: Node throws an untagged `Error` (-> untyped 500)
/// when either id doesn't belong to `user_id`; this returns
/// [`AppError::NotFound`] (-> 404) instead.
///
/// All four steps run inside one transaction — a partial merge would leave
/// `chunk_tag` rows pointing at a tag that no longer exists.
pub async fn merge(
    pool: &PgPool,
    user_id: &str,
    source_id: &str,
    target_id: &str,
) -> AppResult<MergeResult> {
    let mut tx = pool.begin().await?;

    // Step 1: both ids must belong to the caller. Node selects both rows
    // and requires exactly 2 back; an `IN`-scoped `COUNT` is the same
    // check without the extra round-trip of materialising the rows.
    let owned = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM tag WHERE id IN ($1, $2) AND user_id = $3"#,
        source_id,
        target_id,
        user_id
    )
    .fetch_one(&mut *tx)
    .await?;
    if owned != 2 {
        return Err(AppError::NotFound("tag".to_string()));
    }

    // Step 2: re-point chunk_tag rows from source -> target. ON CONFLICT
    // DO NOTHING is load-bearing here: a chunk already carrying both tags
    // would otherwise violate the (chunk_id, tag_id) primary key.
    sqlx::query!(
        r#"INSERT INTO chunk_tag (chunk_id, tag_id)
           SELECT chunk_id, $1 FROM chunk_tag WHERE tag_id = $2
           ON CONFLICT (chunk_id, tag_id) DO NOTHING"#,
        target_id,
        source_id
    )
    .execute(&mut *tx)
    .await?;

    // Step 3: drop the leftover chunk_tag rows still pointing at source
    // (the ones ON CONFLICT skipped because target was already present).
    sqlx::query!("DELETE FROM chunk_tag WHERE tag_id = $1", source_id)
        .execute(&mut *tx)
        .await?;

    // Step 4: delete the source tag itself.
    sqlx::query!(
        "DELETE FROM tag WHERE id = $1 AND user_id = $2",
        source_id,
        user_id
    )
    .execute(&mut *tx)
    .await?;

    let chunk_count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM chunk_tag WHERE tag_id = $1"#,
        target_id
    )
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(MergeResult {
        target_id: target_id.to_string(),
        chunk_count,
    })
}

/// Deletes and reinserts a chunk's full tag set in one transaction, so a
/// failure part-way through cannot leave a truncated set.
///
/// `chunk_tag` has no `user_id` of its own — ownership derives entirely
/// from its two parent rows, and BOTH must be independently verified in
/// SQL:
///
/// - the `DELETE` carries the same ownership `EXISTS` guard as
///   `chunk_meta::replace_applies_to`, so calling this with a chunk the
///   caller doesn't own is a safe no-op (in particular, it cannot wipe a
///   victim chunk's existing tags out from under them);
/// - the `INSERT ... SELECT` joins through *both* `chunk` and `tag`,
///   requiring `c.user_id = $1 AND t.user_id = $1` — tagging another
///   user's chunk, and attaching another user's tag, are both rejected by
///   the same query.
///
/// Returns the number of `chunk_tag` rows actually inserted, so callers can
/// detect a rejected attach (0 inserted despite non-empty `tag_ids`).
pub async fn set_chunk_tags(
    pool: &PgPool,
    user_id: &str,
    chunk_id: &str,
    tag_ids: &[String],
) -> AppResult<u64> {
    let mut tx = pool.begin().await?;

    sqlx::query!(
        "DELETE FROM chunk_tag WHERE chunk_id = $1 \
           AND EXISTS (SELECT 1 FROM chunk c WHERE c.id = $1 AND c.user_id = $2)",
        chunk_id,
        user_id
    )
    .execute(&mut *tx)
    .await?;

    let inserted = if tag_ids.is_empty() {
        0
    } else {
        sqlx::query!(
            r#"INSERT INTO chunk_tag (chunk_id, tag_id)
               SELECT c.id, t.id
               FROM chunk c
               JOIN tag t ON t.id = ANY($3)
               WHERE c.id = $2
                 AND c.user_id = $1
                 AND t.user_id = $1
               ON CONFLICT (chunk_id, tag_id) DO NOTHING"#,
            user_id,
            chunk_id,
            tag_ids
        )
        .execute(&mut *tx)
        .await?
        .rows_affected()
    };

    tx.commit().await?;
    Ok(inserted)
}

/// Lists the tags attached to a chunk, scoped through the chunk's owner —
/// the same "through the parent" pattern as `set_chunk_tags`, so a caller
/// can never read another user's chunk's tags either.
pub async fn tags_for_chunk(pool: &PgPool, user_id: &str, chunk_id: &str) -> AppResult<Vec<Tag>> {
    let rows = sqlx::query_as!(
        Tag,
        r#"SELECT t.id, t.name, t.tag_type_id, t.user_id,
                  t.created_at AS "created_at: UtcTimestamp",
                  t.origin, t.review_status, t.reviewed_by,
                  t.reviewed_at AS "reviewed_at: UtcTimestamp"
           FROM chunk_tag ct
           JOIN tag t ON t.id = ct.tag_id
           WHERE ct.chunk_id = $1
             AND EXISTS (SELECT 1 FROM chunk c WHERE c.id = $1 AND c.user_id = $2)
           ORDER BY t.name, t.id ASC"#,
        chunk_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// One `(chunkId, tagName)` pair — the bulk-fetch shape `search::service`
/// uses to enrich a page of search results with tag names in one query
/// instead of one round trip per chunk. Mirrors Node's `getTagsForChunks`
/// (`packages/db/src/repository/tag-new.ts:143-157`).
#[derive(Debug, Clone)]
pub struct ChunkTagName {
    pub chunk_id: String,
    pub tag_name: String,
}

/// Bulk variant of [`tags_for_chunk`] for a whole page of chunk ids at
/// once. Scoped the same way: a chunk id not owned by `user_id` simply
/// contributes no rows, via the same `EXISTS` join against `chunk`. An
/// empty `chunk_ids` returns an empty result without querying, matching
/// Node's `getTagsForChunks`, which short-circuits to `Effect.succeed([])`
/// on an empty input array rather than issuing a query with an empty
/// `IN ()`.
pub async fn tags_for_chunks(
    pool: &PgPool,
    user_id: &str,
    chunk_ids: &[String],
) -> AppResult<Vec<ChunkTagName>> {
    if chunk_ids.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query_as!(
        ChunkTagName,
        r#"SELECT ct.chunk_id AS "chunk_id!", t.name AS "tag_name!"
           FROM chunk_tag ct
           JOIN tag t ON t.id = ct.tag_id
           WHERE ct.chunk_id = ANY($1)
             AND EXISTS (SELECT 1 FROM chunk c WHERE c.id = ct.chunk_id AND c.user_id = $2)"#,
        chunk_ids,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
