//! `document` rows are the "source file" side of the documents<->chunks
//! relationship: a document is split into ordered sections, and each
//! section is materialised as a `chunk` row carrying a plain `document_id`
//! FK (`ON DELETE SET NULL`) and a `document_order` integer — **not** a
//! join table. Ordering within a document is owned entirely by this
//! domain (via `document_order`), matching
//! `packages/db/src/schema/chunk.ts:53-54` and the unique
//! `(document_id, document_order)` index (`0001_init.sql`,
//! `chunk_document_order_idx`) that enforces "at most one chunk per slot".
//!
//! This module writes directly to `chunk`/`tag`/`chunk_tag` with its own
//! SQL (never through `chunk::create`/`chunk::update`) — the same pattern
//! `proposal::approve` uses for its own chunk/tag writes (see that
//! function's doc comment). `chunk.rs` itself is untouched by this port.
//!
//! **Node's `deleteChunk`-adjacent `syncDocument` step for orphaned
//! sections has a live bug this port intentionally reproduces**: when a
//! section's heading disappears from a re-imported file, Node calls
//! `updateChunkRepo(existing.id, { documentOrder: undefined })`
//! (`packages/api/src/documents/service.ts:220`) intending to clear the
//! chunk's `document_order`. But `updateChunk`'s repo implementation only
//! assigns a field when `params.x !== undefined`
//! (`packages/db/src/repository/chunk.ts:349`), and an object literal key
//! explicitly set to `undefined` still reads as `undefined` in that check
//! — so `document_order` is **never actually cleared**. Drizzle's
//! `$onUpdate` on `updated_at` still fires (any `.set()` call touches it),
//! so the only real effect is `updated_at` bumping, which is externally
//! visible via `listDocuments`/`listDocumentsWithTags`'s `lastChunkUpdatedAt`
//! aggregate. [`touch_chunk`] reproduces exactly that: `updated_at = now()`,
//! `document_order` untouched. Fixing the bug would change chunk/document
//! coupling semantics (whether an orphaned section still occupies a
//! `document_order` slot) and is out of scope for this port to decide
//! unilaterally — flagged for the human call.

use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::embedding::EmbeddingVec;
use crate::repo::chunk::Chunk;
use crate::timestamp::UtcTimestamp;

/// `camelCase` serialisation matches every other wire type in this crate.
/// This is the bare `document` row — what create/update/the `document` half
/// of `render` return. List endpoints use the wider [`DocumentListItem`]/
/// [`DocumentWithTagsItem`] shapes instead (aggregated chunk stats), and
/// `GET /api/documents/{id}` flattens this shape together with `chunks`
/// rather than nesting it (see `fubbik_api::documents::dto::DocumentDetail`,
/// mirroring Node's `{ ...doc, chunks }` spread in `getDocument`,
/// `packages/api/src/documents/service.ts:293`).
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: String,
    pub title: String,
    pub source_path: String,
    pub content_hash: String,
    pub description: Option<String>,
    pub split_level: Option<i32>,
    pub space_id: Option<String>,
    pub user_id: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

/// Shape of `GET /api/documents` list items (bare `listDocuments`, unused by
/// any wired route — Node's actual `/documents` route calls
/// `listDocumentsWithTags`, see [`DocumentWithTagsItem`] — kept for parity
/// since the underlying repo function exists and is exercised by
/// `service.test.ts`-adjacent coverage). Matches
/// `packages/db/src/repository/document.ts:49-74`: a `LEFT JOIN chunk`
/// aggregate, so `chunk_count` is `0` (never `NULL`) and the two timestamp
/// aggregates are `NULL` for a document with no chunks.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentListItem {
    pub id: String,
    pub title: String,
    pub source_path: String,
    pub content_hash: String,
    pub description: Option<String>,
    pub space_id: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
    pub chunk_count: i64,
    #[schema(value_type = Option<chrono::NaiveDateTime>)]
    pub last_chunk_updated_at: Option<UtcTimestamp>,
    #[schema(value_type = Option<chrono::NaiveDateTime>)]
    pub oldest_chunk_updated_at: Option<UtcTimestamp>,
}

/// Shape of `GET /api/documents` (the route Node actually wires,
/// `packages/api/src/documents/routes.ts:8-21`) — [`DocumentListItem`] plus
/// `type` (the first section's chunk type, `document_order = 0`, defaulting
/// to `"document"` when absent) and `tags` (union of every chunk's tags in
/// the document, deduped). Matches
/// `packages/db/src/repository/document.ts:76-112`.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentWithTagsItem {
    pub id: String,
    pub title: String,
    pub source_path: String,
    pub content_hash: String,
    pub description: Option<String>,
    pub space_id: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
    pub chunk_count: i64,
    #[schema(value_type = Option<chrono::NaiveDateTime>)]
    pub last_chunk_updated_at: Option<UtcTimestamp>,
    #[schema(value_type = Option<chrono::NaiveDateTime>)]
    pub oldest_chunk_updated_at: Option<UtcTimestamp>,
    #[serde(rename = "type")]
    pub doc_type: String,
    pub tags: Vec<String>,
}

/// Raw row shape for [`list_with_tags`]'s query, before the `type`
/// default-fallback and `tags_raw` -> `tags` split happen in Rust — mirrors
/// Node's `docs.map(d => ({ ...d, type: d.type ?? "document", tags:
/// d.tagsRaw ? d.tagsRaw.split(",") ... }))` post-processing step
/// (`packages/db/src/repository/document.ts:105-110`).
struct RawListWithTagsRow {
    id: String,
    title: String,
    source_path: String,
    content_hash: String,
    description: Option<String>,
    space_id: Option<String>,
    created_at: UtcTimestamp,
    updated_at: UtcTimestamp,
    chunk_count: i64,
    last_chunk_updated_at: Option<UtcTimestamp>,
    oldest_chunk_updated_at: Option<UtcTimestamp>,
    doc_type: Option<String>,
    tags_raw: Option<String>,
}

/// Shape of `GET /api/documents/search` items, matching
/// `searchDocumentChunks` (`packages/db/src/repository/document.ts:138-159`).
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSearchResult {
    pub chunk_id: String,
    pub chunk_title: String,
    pub chunk_content: String,
    pub document_order: Option<i32>,
    pub document_id: String,
    pub document_title: String,
    pub source_path: String,
}

pub struct NewDocument {
    pub id: String,
    pub title: String,
    pub source_path: String,
    pub content_hash: String,
    pub description: Option<String>,
    pub space_id: Option<String>,
    pub split_level: Option<i32>,
}

pub async fn create(pool: &PgPool, user_id: &str, new: NewDocument) -> AppResult<Document> {
    let mut connection = pool.acquire().await?;
    create_in(&mut connection, user_id, new).await
}

pub async fn create_in(
    connection: &mut sqlx::PgConnection,
    user_id: &str,
    new: NewDocument,
) -> AppResult<Document> {
    let row = sqlx::query_as!(
        Document,
        r#"INSERT INTO document (id, title, source_path, content_hash, description, space_id, user_id, split_level)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING id, title, source_path, content_hash, description, split_level, space_id, user_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        new.id,
        new.title,
        new.source_path,
        new.content_hash,
        new.description,
        new.space_id,
        user_id,
        new.split_level
    )
    .fetch_one(&mut *connection)
    .await?;
    Ok(row)
}

/// `WHERE id = $1 AND user_id = $2` — a deliberate divergence from Node's
/// `getDocumentById`, which is unscoped SQL
/// (`packages/db/src/repository/document.ts:26-31`) and relies entirely on
/// a service-layer `doc.userId !== userId` check for cross-user 404s
/// (`packages/api/src/documents/service.ts:164,244,291,304`). Every one of
/// this port's callers scopes in SQL instead, per this slice's guidance —
/// see `tests/document.rs` for the cross-user 404 proof, and note removing
/// this `AND user_id = $2` is what should make that specific test fail
/// (there is no redundant service-layer check behind it to catch the
/// regression instead).
pub async fn find_by_id(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Option<Document>> {
    let row = sqlx::query_as!(
        Document,
        r#"SELECT id, title, source_path, content_hash, description, split_level, space_id, user_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM document WHERE id = $1 AND user_id = $2"#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Already scoped by `user_id` in Node
/// (`packages/db/src/repository/document.ts:33-47`); `space_id: None` means
/// "match a document with `space_id IS NULL`", not "no filter" — Node
/// branches into `isNull(document.spaceId)` for that case. `IS NOT DISTINCT
/// FROM` is Postgres's null-safe equality: it requires `space_id IS NULL`
/// when `$3` is `NULL`, and `space_id = $3` otherwise — exactly Node's two
/// branches in one expression.
pub async fn find_by_source_path(
    pool: &PgPool,
    user_id: &str,
    source_path: &str,
    space_id: Option<&str>,
) -> AppResult<Option<Document>> {
    let space_id = space_id.filter(|s| !s.is_empty());
    let row = sqlx::query_as!(
        Document,
        r#"SELECT id, title, source_path, content_hash, description, split_level, space_id, user_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM document
           WHERE source_path = $1 AND user_id = $2 AND space_id IS NOT DISTINCT FROM $3"#,
        source_path,
        user_id,
        space_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// `d.title ASC, d.id ASC` — Node's `listDocuments` orders by `title` alone
/// (`packages/db/src/repository/document.ts:71`), which is not a total
/// order; the `id` tiebreaker is this port's addition, same convention as
/// every other list query in this crate.
pub async fn list(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<DocumentListItem>> {
    let space_id = space_id.filter(|s| !s.is_empty());
    let rows = sqlx::query_as!(
        DocumentListItem,
        r#"SELECT d.id, d.title, d.source_path, d.content_hash, d.description, d.space_id,
                  d.created_at AS "created_at: UtcTimestamp",
                  d.updated_at AS "updated_at: UtcTimestamp",
                  COUNT(c.id) AS "chunk_count!",
                  MAX(c.updated_at) AS "last_chunk_updated_at: UtcTimestamp",
                  MIN(c.updated_at) AS "oldest_chunk_updated_at: UtcTimestamp"
           FROM document d
           LEFT JOIN chunk c ON c.document_id = d.id
           WHERE d.user_id = $1 AND ($2::text IS NULL OR d.space_id = $2)
           GROUP BY d.id
           ORDER BY d.title ASC, d.id ASC"#,
        user_id,
        space_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// See [`DocumentWithTagsItem`] for the shape. `d.title ASC, d.id ASC` adds
/// the same tiebreaker as [`list`]; Node has no `ORDER BY` guarantee beyond
/// `title` here either.
pub async fn list_with_tags(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<DocumentWithTagsItem>> {
    let space_id = space_id.filter(|s| !s.is_empty());
    let rows = sqlx::query_as!(
        RawListWithTagsRow,
        r#"SELECT d.id, d.title, d.source_path, d.content_hash, d.description, d.space_id,
                  d.created_at AS "created_at: UtcTimestamp",
                  d.updated_at AS "updated_at: UtcTimestamp",
                  COUNT(DISTINCT c.id) AS "chunk_count!",
                  MAX(c.updated_at) AS "last_chunk_updated_at: UtcTimestamp",
                  MIN(c.updated_at) AS "oldest_chunk_updated_at: UtcTimestamp",
                  MIN(CASE WHEN c.document_order = 0 THEN c.type END) AS "doc_type?",
                  string_agg(DISTINCT t.name, ',') AS "tags_raw?"
           FROM document d
           LEFT JOIN chunk c ON c.document_id = d.id
           LEFT JOIN chunk_tag ct ON ct.chunk_id = c.id
           LEFT JOIN tag t ON t.id = ct.tag_id
           WHERE d.user_id = $1 AND ($2::text IS NULL OR d.space_id = $2)
           GROUP BY d.id
           ORDER BY d.title ASC, d.id ASC"#,
        user_id,
        space_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| DocumentWithTagsItem {
            id: r.id,
            title: r.title,
            source_path: r.source_path,
            content_hash: r.content_hash,
            description: r.description,
            space_id: r.space_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
            chunk_count: r.chunk_count,
            last_chunk_updated_at: r.last_chunk_updated_at,
            oldest_chunk_updated_at: r.oldest_chunk_updated_at,
            doc_type: r.doc_type.unwrap_or_else(|| "document".to_string()),
            tags: r
                .tags_raw
                .map(|raw| {
                    raw.split(',')
                        .filter(|t| !t.is_empty())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect())
}

/// Two-state `Option<T>` fields (`None` = leave untouched), matching Node's
/// conditional-spread `updateDocument`
/// (`packages/db/src/repository/document.ts:114-128`) — no field has an
/// explicit-null-clear variant in Node's own type (`title?: string`, not
/// `string | null`, for every field).
#[derive(Default)]
pub struct DocumentPatch {
    pub title: Option<String>,
    pub content_hash: Option<String>,
    pub description: Option<String>,
    pub split_level: Option<i32>,
}

/// `WHERE id = $1 AND user_id = $2` — Node's `updateDocument` is unscoped
/// SQL (`packages/db/src/repository/document.ts:114-128`), relying on its
/// one real caller (`syncDocument`) having already verified ownership via
/// `getDocumentById` earlier in the same request. This port scopes in SQL
/// directly instead, per this slice's guidance.
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    patch: DocumentPatch,
) -> AppResult<Option<Document>> {
    let row = sqlx::query_as!(
        Document,
        r#"UPDATE document SET
             title = COALESCE($3, title),
             content_hash = COALESCE($4, content_hash),
             description = COALESCE($5, description),
             split_level = COALESCE($6, split_level),
             updated_at = now()
           WHERE id = $1 AND user_id = $2
           RETURNING id, title, source_path, content_hash, description, split_level, space_id, user_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        user_id,
        patch.title,
        patch.content_hash,
        patch.description,
        patch.split_level
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Unsets `document_id`/`document_order` on every chunk under this document
/// (so they survive as orphaned, un-ordered chunks rather than being
/// deleted — matching Node's `deleteDocument`,
/// `packages/db/src/repository/document.ts:130-136`), then deletes the
/// document row, both scoped by `user_id` and run in one transaction. Node
/// scopes neither statement by `user_id` at the SQL level; this port adds
/// it to both, consistent with [`update`]/[`find_by_id`].
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Option<Document>> {
    let mut tx = pool.begin().await?;

    sqlx::query!(
        "UPDATE chunk SET document_id = NULL, document_order = NULL \
           WHERE document_id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(&mut *tx)
    .await?;

    let row = sqlx::query_as!(
        Document,
        r#"DELETE FROM document WHERE id = $1 AND user_id = $2
           RETURNING id, title, source_path, content_hash, description, split_level, space_id, user_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        user_id
    )
    .fetch_optional(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(row)
}

/// `d.title ASC, c.document_order ASC, c.id ASC` — Node orders by
/// `document.title, chunk.documentOrder`
/// (`packages/db/src/repository/document.ts:155`), not a total order; the
/// `chunk.id` tiebreaker is this port's addition. `limit` is always called
/// with `20` by the service layer (`packages/api/src/documents/service.ts:298`;
/// there is no route-level override), reproduced as a plain parameter here
/// rather than a hardcoded literal so the one real call site stays explicit
/// about it.
pub async fn search_chunks(
    pool: &PgPool,
    user_id: &str,
    query: &str,
    limit: i64,
    space_id: Option<&str>,
) -> AppResult<Vec<DocumentSearchResult>> {
    let space_id = space_id.filter(|s| !s.is_empty());
    let pattern = format!("%{query}%");
    let rows = sqlx::query_as!(
        DocumentSearchResult,
        r#"SELECT c.id AS chunk_id, c.title AS chunk_title, c.content AS chunk_content,
                  c.document_order, d.id AS document_id, d.title AS document_title, d.source_path
           FROM chunk c
           JOIN document d ON c.document_id = d.id
           WHERE d.user_id = $1
             AND (c.title ILIKE $2 OR c.content ILIKE $2)
             AND ($3::text IS NULL OR d.space_id = $3)
           ORDER BY d.title ASC, c.document_order ASC, c.id ASC
           LIMIT $4"#,
        user_id,
        pattern,
        space_id,
        limit
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// `document_order ASC, id ASC` — Node orders by `chunk.documentOrder` alone
/// (`packages/db/src/repository/document.ts:161-166`), which is not a total
/// order (and Postgres already places `NULL`s last for `ASC`, matching
/// Node's behaviour for any chunk that lost its slot); the `id` tiebreaker
/// is this port's addition.
///
/// `user_id` scoping is this port's addition — Node's `getDocumentChunks`
/// is unscoped SQL (`WHERE chunk.documentId = documentId` only). Every real
/// caller already owns the parent document by the time this runs, so this
/// is defense in depth rather than a behaviour change.
pub async fn document_chunks(
    pool: &PgPool,
    user_id: &str,
    document_id: &str,
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
           WHERE document_id = $1 AND user_id = $2
           ORDER BY document_order ASC, id ASC"#,
        document_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Creates a section chunk for a document import/sync, always `type =
/// 'document'` — matching the default (non-template) import path's
/// `createChunkRepo({..., type: "document", documentId, documentOrder})`
/// (`packages/api/src/documents/service.ts:139-147,197-205`). Written as its
/// own `INSERT` (not via `chunk::create`) because it needs `document_id`/
/// `document_order`, which `chunk::create`'s `NewChunk` does not carry —
/// see this module's top doc comment.
pub async fn insert_section_chunk(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    title: &str,
    content: &str,
    document_id: &str,
    document_order: i32,
) -> AppResult<()> {
    sqlx::query!(
        "INSERT INTO chunk (id, title, content, type, user_id, document_id, document_order) \
           VALUES ($1, $2, $3, 'document', $4, $5, $6)",
        id,
        title,
        content,
        user_id,
        document_id,
        document_order
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Updates an existing section chunk's content/order on re-sync, matching
/// `updateChunkRepo(match.id, { content: section.content, documentOrder:
/// section.order })` (`packages/api/src/documents/service.ts:189-192`) —
/// both fields are always supplied together here, so a plain `UPDATE`
/// (not `COALESCE`) is correct.
pub async fn update_section_chunk(
    pool: &PgPool,
    user_id: &str,
    chunk_id: &str,
    content: &str,
    document_order: i32,
) -> AppResult<()> {
    sqlx::query!(
        "UPDATE chunk SET content = $3, document_order = $4, updated_at = now() \
           WHERE id = $1 AND user_id = $2",
        chunk_id,
        user_id,
        content,
        document_order
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Bumps `updated_at` only — see this module's top doc comment for why this
/// intentionally does **not** clear `document_order`, reproducing a live
/// Node bug rather than silently fixing chunk/document coupling behaviour.
pub async fn touch_chunk(pool: &PgPool, user_id: &str, chunk_id: &str) -> AppResult<()> {
    sqlx::query!(
        "UPDATE chunk SET updated_at = now() WHERE id = $1 AND user_id = $2",
        chunk_id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Looks up a tag by exact `(name, user_id)`, creating it if absent —
/// mirrors Node's `findOrCreateTag`
/// (`packages/db/src/repository/tag-new.ts:159-171`) exactly, including its
/// benign select-then-insert race (no `ON CONFLICT`): a concurrent import
/// of the same new tag name for the same user could in theory violate
/// `tag_user_name_idx` and surface as a raw `AppError::Database`, same as
/// Node would surface an unhandled Postgres error in that case. Implemented
/// here (not added to `tag::` module) for the same reason
/// `proposal::approve` inlines its own copy — see this module's top doc
/// comment.
async fn find_or_create_tag(pool: &PgPool, user_id: &str, name: &str) -> AppResult<String> {
    if let Some(id) = sqlx::query_scalar!(
        "SELECT id FROM tag WHERE name = $1 AND user_id = $2",
        name,
        user_id
    )
    .fetch_optional(pool)
    .await?
    {
        return Ok(id);
    }
    let id = crate::new_id();
    sqlx::query!(
        "INSERT INTO tag (id, name, user_id) VALUES ($1, $2, $3)",
        id,
        name,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(id)
}

/// Resolves a list of tag names to ids, creating any that don't already
/// exist for `user_id` — the batch form of [`find_or_create_tag`], matching
/// the `for (const name of tagNames) { yield* findOrCreateTag(...) }` loop
/// in `service.ts`'s `resolveTagIds` helper.
pub async fn resolve_tag_ids(
    pool: &PgPool,
    user_id: &str,
    names: &[String],
) -> AppResult<Vec<String>> {
    let mut ids = Vec::with_capacity(names.len());
    for name in names {
        ids.push(find_or_create_tag(pool, user_id, name).await?);
    }
    Ok(ids)
}
