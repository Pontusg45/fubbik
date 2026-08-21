//! Read-only queries behind three small analysis endpoints — `GET /density`,
//! `GET /timeline`, and the two top-level `file-refs` lookups — plus
//! `scope_key` CRUD.
//!
//! Grouped in one module because none is large enough to justify its own, and
//! all four are leaves: nothing else in the crate calls them.

use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::timestamp::UtcTimestamp;

// ---------------------------------------------------------------------------
// Density
// ---------------------------------------------------------------------------

/// One `(path, chunk)` pairing, from either an applies-to glob or an explicit
/// file reference. The service folds these into a directory tree.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DensityPath {
    pub path: String,
    pub chunk_id: String,
    pub chunk_title: String,
    pub chunk_type: String,
    /// `applies_to | file_ref` — which side the pairing came from. The
    /// service dedupes on `(chunk_id, source)`, so a chunk that both
    /// glob-matches and explicitly references the same path appears twice,
    /// deliberately.
    pub source: String,
}

/// Both halves in one query, unioned.
///
/// Node runs two selects concurrently and concatenates them in JS
/// (`packages/db/src/repository/density.ts:38-59`); one `UNION ALL` produces
/// the same rows from a single consistent snapshot.
///
/// **The glob prefix is computed in SQL.** Node's `globPrefix` truncates an
/// applies-to pattern at the first `*?[{` and strips trailing slashes, so
/// `src/**/*.ts` becomes `src`. Reproduced here with
/// `regexp_replace`/`split_part` rather than post-processing in Rust, so the
/// rows arriving are already the paths the tree is built from — and a pattern
/// that reduces to nothing (a bare `**`) is dropped by the `<> ''` filter,
/// matching Node's `if (!prefix) continue`.
///
/// Archived chunks are excluded, matching Node.
pub async fn density_paths(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<DensityPath>> {
    let rows = sqlx::query_as!(
        DensityPath,
        r#"WITH visible AS (
             SELECT c.id, c.title, c.type
             FROM chunk c
             WHERE c.user_id = $1
               AND c.archived_at IS NULL
               AND ($2::text IS NULL
                    OR c.id IN (SELECT chunk_id FROM chunk_space WHERE space_id = $2))
           )
           SELECT prefix AS "path!", chunk_id AS "chunk_id!",
                  chunk_title AS "chunk_title!", chunk_type AS "chunk_type!",
                  source AS "source!"
           FROM (
             SELECT rtrim(split_part(a.pattern, '*', 1), '/') AS prefix,
                    v.id AS chunk_id, v.title AS chunk_title, v.type AS chunk_type,
                    'applies_to' AS source
             FROM chunk_applies_to a
             JOIN visible v ON v.id = a.chunk_id
             UNION ALL
             SELECT f.path AS prefix,
                    v.id, v.title, v.type, 'file_ref'
             FROM chunk_file_ref f
             JOIN visible v ON v.id = f.chunk_id
           ) rows
           WHERE prefix <> ''
           ORDER BY prefix, chunk_id, source"#,
        user_id,
        space_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Timeline
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEvent {
    pub chunk_id: String,
    pub chunk_title: String,
    pub chunk_type: String,
    /// `created | updated`.
    pub kind: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub at: UtcTimestamp,
    /// Present only on `updated` events — the `chunk_version` number.
    pub version: Option<i32>,
}

/// Creation and edit events since `from`, newest first, capped at 500.
///
/// The union is Node's verbatim: `chunk.created_at` supplies the `created`
/// events and `chunk_version.created_at` the `updated` ones, so "updated"
/// means "a version snapshot was written", not "the row changed". A bulk
/// retype therefore does NOT appear here — `chunk::update_many` writes no
/// version — which is consistent with that column not being bumped either.
///
/// `ORDER BY at DESC` gains `, chunk_id, kind` as a tiebreaker: a chunk
/// created and immediately versioned shares a timestamp with itself, and
/// Node's ordering leaves that pair's order to the query plan.
pub async fn timeline(
    pool: &PgPool,
    user_id: &str,
    from: chrono::NaiveDateTime,
    space_id: Option<&str>,
    tag: Option<&str>,
) -> AppResult<Vec<TimelineEvent>> {
    let rows = sqlx::query_as!(
        TimelineEvent,
        r#"SELECT chunk_id AS "chunk_id!", chunk_title AS "chunk_title!",
                  chunk_type AS "chunk_type!", kind AS "kind!",
                  at AS "at!: UtcTimestamp", version
           FROM (
             SELECT c.id AS chunk_id, c.title AS chunk_title, c.type AS chunk_type,
                    'created'::text AS kind, c.created_at AS at,
                    NULL::integer AS version
             FROM chunk c
             WHERE c.user_id = $1 AND c.archived_at IS NULL AND c.created_at >= $2
               AND ($3::text IS NULL
                    OR c.id IN (SELECT chunk_id FROM chunk_space WHERE space_id = $3))
               AND ($4::text IS NULL
                    OR c.id IN (SELECT ct.chunk_id FROM chunk_tag ct
                                JOIN tag t ON t.id = ct.tag_id WHERE t.name = $4))
             UNION ALL
             SELECT cv.chunk_id, c.title, c.type,
                    'updated'::text, cv.created_at, cv.version
             FROM chunk_version cv
             JOIN chunk c ON c.id = cv.chunk_id
             WHERE c.user_id = $1 AND c.archived_at IS NULL AND cv.created_at >= $2
               AND ($3::text IS NULL
                    OR c.id IN (SELECT chunk_id FROM chunk_space WHERE space_id = $3))
               AND ($4::text IS NULL
                    OR c.id IN (SELECT ct.chunk_id FROM chunk_tag ct
                                JOIN tag t ON t.id = ct.tag_id WHERE t.name = $4))
           ) combined
           ORDER BY at DESC, chunk_id, kind
           LIMIT 500"#,
        user_id,
        from,
        space_id,
        tag
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// File references (top-level)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileRefLookup {
    pub chunk_id: String,
    pub chunk_title: String,
    pub chunk_type: String,
    pub ref_id: String,
    pub path: String,
    pub anchor: Option<String>,
    pub relation: String,
}

/// Which chunks reference a given path — the reverse lookup behind the VS Code
/// extension's file-aware surfacing.
///
/// The optional `space_id` uses the "in this space **or** in no space at all"
/// form, matching Node here (unlike `list_archived`, which omits the global
/// half — Node is inconsistent between the two).
pub async fn lookup_by_path(
    pool: &PgPool,
    user_id: &str,
    path: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<FileRefLookup>> {
    let rows = sqlx::query_as!(
        FileRefLookup,
        r#"SELECT c.id AS chunk_id, c.title AS chunk_title, c.type AS chunk_type,
                  f.id AS ref_id, f.path, f.anchor, f.relation
           FROM chunk_file_ref f
           JOIN chunk c ON c.id = f.chunk_id
           WHERE f.path = $2 AND c.user_id = $1
             AND ($3::text IS NULL
                  OR c.id IN (SELECT chunk_id FROM chunk_space WHERE space_id = $3)
                  OR c.id NOT IN (SELECT chunk_id FROM chunk_space))
           ORDER BY c.title, f.id"#,
        user_id,
        path,
        space_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Every file reference the user's chunks carry.
///
/// Node's projection is only `{chunkId, chunkTitle, path, anchor}` — no
/// `relation`, no `refId`. This returns the fuller [`FileRefLookup`] shape
/// instead: a superset is harmless to readers, and the two endpoints
/// returning different columns for the same rows is a wart worth not
/// carrying forward. **No `LIMIT`**, matching Node.
pub async fn list_all_file_refs(pool: &PgPool, user_id: &str) -> AppResult<Vec<FileRefLookup>> {
    let rows = sqlx::query_as!(
        FileRefLookup,
        r#"SELECT c.id AS chunk_id, c.title AS chunk_title, c.type AS chunk_type,
                  f.id AS ref_id, f.path, f.anchor, f.relation
           FROM chunk_file_ref f
           JOIN chunk c ON c.id = f.chunk_id
           WHERE c.user_id = $1
           ORDER BY f.path, f.id"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Scope keys
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScopeKey {
    pub id: String,
    pub user_id: String,
    pub key: String,
    pub description: Option<String>,
    /// `string | number | boolean | enum`, `NOT NULL DEFAULT 'string'`.
    pub value_type: String,
    /// Only meaningful for `value_type = 'enum'`; nullable, so a key with no
    /// constraint reads back as `null` rather than `[]`.
    #[schema(value_type = Option<Vec<String>>)]
    pub allowed_values: Option<Json<Vec<String>>>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

/// `ORDER BY key, id` — Node has no `ORDER BY`, and this is the registry the
/// scope autocomplete reads, so a stable order is what makes the dropdown
/// stop reshuffling between requests.
pub async fn list_scope_keys(pool: &PgPool, user_id: &str) -> AppResult<Vec<ScopeKey>> {
    let rows = sqlx::query_as!(
        ScopeKey,
        r#"SELECT id, user_id, key, description, value_type,
                  allowed_values AS "allowed_values: Json<Vec<String>>",
                  created_at AS "created_at: UtcTimestamp"
           FROM scope_key WHERE user_id = $1
           ORDER BY key, id"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub struct NewScopeKey {
    pub key: String,
    pub description: Option<String>,
    pub value_type: String,
    pub allowed_values: Option<Vec<String>>,
}

pub async fn create_scope_key(
    pool: &PgPool,
    user_id: &str,
    new: NewScopeKey,
) -> AppResult<ScopeKey> {
    let id = crate::new_id();
    let row = sqlx::query_as!(
        ScopeKey,
        r#"INSERT INTO scope_key (id, user_id, key, description, value_type, allowed_values)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING id, user_id, key, description, value_type,
                     allowed_values AS "allowed_values: Json<Vec<String>>",
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        user_id,
        new.key,
        new.description,
        new.value_type,
        new.allowed_values.map(Json) as _
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn delete_scope_key(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let n = sqlx::query!(
        "DELETE FROM scope_key WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(n > 0)
}
