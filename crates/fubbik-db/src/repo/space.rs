use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::timestamp::UtcTimestamp;

/// Bare `space` row shape — what `GET /api/spaces` (list), `POST /api/spaces`
/// (create), `PATCH /api/spaces/{id}` (update), and `GET /api/spaces/detect`
/// on a match all return. **Not** the shape of `GET /api/spaces/{id}`
/// (detail) — that one nests this struct inside [`SpaceDetail`] alongside
/// `space_code_metadata`. Three different endpoints, three different "space"
/// shapes: reusing this struct for all of them would silently flatten or
/// drop the `code` metadata on the detail route. See `_questions.md` in
/// `tests/fixtures/node-contract/` ("space detail vs list: nested `code`
/// metadata, not flattened").
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Space {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub description: Option<String>,
    pub user_id: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

/// `space_code_metadata` side-table row, holding the extra fields that only
/// apply to `kind = 'code'` spaces. Only ever surfaced nested inside
/// [`SpaceDetail`] — never flattened into `Space` itself.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpaceCodeMetadata {
    pub space_id: String,
    pub user_id: String,
    pub remote_url: Option<String>,
    #[schema(value_type = Vec<String>)]
    pub local_paths: Json<Vec<String>>,
}

/// Shape of `GET /api/spaces/{id}` — `{ space, code }`, matching Node's
/// `getSpaceWithCodeMetadata` (`packages/db/src/repository/space.ts:58-72`),
/// which `leftJoin`s `space_code_metadata` and returns the two halves as
/// sibling keys, un-flattened. `code` is `null` for a non-`code`-kind space
/// (no side-table row ever exists for it).
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct SpaceDetail {
    pub space: Space,
    pub code: Option<SpaceCodeMetadata>,
}

/// Return value of [`reset`], matching Node's `resetSpaceData` return shape
/// (`packages/db/src/repository/space.ts:144-184`).
#[derive(Debug, Clone, Default, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResetResult {
    pub chunks_deleted: i64,
    pub docs_deleted: i64,
    pub plans_deleted: i64,
    pub requirements_deleted: i64,
}

/// Input for [`create`]'s `space_code_metadata` side-table insert. `Some`
/// is passed whenever `kind == "code"`, mirroring Node's
/// `code: kind === "code" ? { remoteUrl, localPaths: body.localPaths } :
/// undefined` (`packages/api/src/spaces/service.ts:55`) — note this means a
/// `code`-kind space *always* gets a side-table row, even if the caller gave
/// neither `remoteUrl` nor `localPaths`, because the ternary keys off `kind`
/// alone, not off whether the object's fields are populated.
pub struct CodeInput {
    pub remote_url: Option<String>,
    pub local_paths: Vec<String>,
}

pub struct NewSpace {
    pub name: String,
    pub kind: String,
    pub description: Option<String>,
}

/// Inserts a `space` row and, if `code` is `Some`, its `space_code_metadata`
/// side-table row, in one transaction — a caller must never observe a
/// `code`-kind space with a `space` row but no metadata row, or vice versa.
pub async fn create(
    pool: &PgPool,
    user_id: &str,
    new: NewSpace,
    code: Option<CodeInput>,
) -> AppResult<Space> {
    let id = crate::new_id();
    let mut tx = pool.begin().await?;

    let s = sqlx::query_as!(
        Space,
        r#"INSERT INTO space (id, name, kind, description, user_id)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, name, kind, description, user_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        new.name,
        new.kind,
        new.description,
        user_id
    )
    .fetch_one(&mut *tx)
    .await?;

    if let Some(code) = code {
        sqlx::query!(
            r#"INSERT INTO space_code_metadata (space_id, user_id, remote_url, local_paths)
               VALUES ($1, $2, $3, $4)"#,
            s.id,
            user_id,
            code.remote_url,
            Json(code.local_paths) as _
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(s)
}

async fn get_row(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Option<Space>> {
    let s = sqlx::query_as!(
        Space,
        r#"SELECT id, name, kind, description, user_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM space WHERE id = $1 AND user_id = $2"#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(s)
}

/// Detail shape: `{ space, code }`. See [`SpaceDetail`].
pub async fn find_by_id(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Option<SpaceDetail>> {
    let space = match get_row(pool, user_id, id).await? {
        Some(s) => s,
        None => return Ok(None),
    };

    let code = sqlx::query_as!(
        SpaceCodeMetadata,
        r#"SELECT space_id, user_id, remote_url,
                  local_paths AS "local_paths: Json<Vec<String>>"
           FROM space_code_metadata WHERE space_id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(Some(SpaceDetail { space, code }))
}

/// List shape: bare rows, no `space_code_metadata` join at all — matching
/// Node's `listSpaces` (`packages/db/src/repository/space.ts:74-76`), a
/// plain `select().from(space)`. A `code`-kind space's `remoteUrl`/
/// `localPaths` are simply absent here; only the detail endpoint reveals
/// them.
pub async fn list(pool: &PgPool, user_id: &str) -> AppResult<Vec<Space>> {
    let rows = sqlx::query_as!(
        Space,
        r#"SELECT id, name, kind, description, user_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM space WHERE user_id = $1 ORDER BY created_at ASC"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// `remote_url` must already be normalized by the caller (see
/// `spaces::normalize_url::normalize_git_url`) — this does an exact-match
/// join against `space_code_metadata.remote_url` as stored, matching Node's
/// `getCodeSpaceByRemoteUrl` (`packages/db/src/repository/space.ts:78-87`).
/// Bare [`Space`] — detect never reveals code metadata, even on a match.
pub async fn find_by_remote_url(
    pool: &PgPool,
    user_id: &str,
    remote_url: &str,
) -> AppResult<Option<Space>> {
    let s = sqlx::query_as!(
        Space,
        r#"SELECT s.id, s.name, s.kind, s.description, s.user_id,
                  s.created_at AS "created_at: UtcTimestamp",
                  s.updated_at AS "updated_at: UtcTimestamp"
           FROM space s
           JOIN space_code_metadata cm ON cm.space_id = s.id
           WHERE cm.remote_url = $1 AND s.user_id = $2"#,
        remote_url,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(s)
}

/// jsonb containment (`@>`) against `local_paths`, matching Node's
/// `getCodeSpaceByLocalPath` (`packages/db/src/repository/space.ts:89-98`),
/// which checks whether the single-element array `[localPath]` is contained
/// in the stored array (i.e. the stored array contains that exact path).
pub async fn find_by_local_path(
    pool: &PgPool,
    user_id: &str,
    local_path: &str,
) -> AppResult<Option<Space>> {
    let needle = Json(vec![local_path.to_string()]);
    let s = sqlx::query_as!(
        Space,
        r#"SELECT s.id, s.name, s.kind, s.description, s.user_id,
                  s.created_at AS "created_at: UtcTimestamp",
                  s.updated_at AS "updated_at: UtcTimestamp"
           FROM space s
           JOIN space_code_metadata cm ON cm.space_id = s.id
           WHERE cm.local_paths @> $1 AND s.user_id = $2"#,
        needle as _,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(s)
}

/// `description` is tri-state, matching Node's `t.Optional(t.Union([t.String(),
/// t.Null()]))` body schema: omitted (`None`) leaves it untouched, explicit
/// `null` (`Some(None)`) clears it, a string (`Some(Some(..))`) sets it. See
/// `tag::TagPatch::tag_type_id` for the same pattern.
#[derive(Default)]
pub struct SpacePatch {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
}

/// Input for [`update`]'s `space_code_metadata` upsert.
///
/// **Deliberate divergence from Node (#3 in this slice — see
/// `spaces::service::update`'s doc comment for the full justification):**
/// Node's `code` param is `{ remoteUrl: remoteUrl ?? null, localPaths:
/// body.localPaths ?? [] }` — constructed, and applied, *unconditionally*
/// whenever the existing space is `kind == "code"`, regardless of whether
/// the request body mentioned either field. That means an ordinary `PATCH
/// {"name": "..."}` against a code-kind space silently clears its
/// `remoteUrl` to `null` and `localPaths` to `[]` in Node. This port does
/// NOT replicate that: both fields here are independently tri/two-state —
/// `None` means "leave this column exactly as it is", matching how every
/// other PATCH in this codebase treats an omitted field (`chunk::update`'s
/// `COALESCE($n, column)`, `tag::update`'s tri-state `CASE`). Do not
/// "fix" this back to Node's unconditional-overwrite behavior — that
/// behavior is the bug, not this.
pub struct CodeUpdate {
    /// Tri-state: `None` = don't touch, `Some(None)` = clear to `NULL`,
    /// `Some(Some(url))` = set.
    pub remote_url: Option<Option<String>>,
    /// Two-state: `None` = don't touch, `Some(paths)` = set (Node's schema
    /// has no `t.Null()` union for `localPaths`, so there is no "clear"
    /// state to represent here).
    pub local_paths: Option<Vec<String>>,
}

/// Updates a space's `name`/`description` and, independently, its
/// `space_code_metadata` row.
///
/// Mirrors Node's two-branch structure exactly
/// (`packages/db/src/repository/space.ts:106-142`): if neither `name` nor
/// `description` is present in the patch, no `UPDATE` runs at all (a plain
/// `SELECT` is issued instead) — in particular `updated_at` is *not* bumped
/// in that case, matching Drizzle's `$onUpdate` hook, which only fires on an
/// actual `.update().set(...)` call. The `code` upsert, when present, is
/// independent of whether `name`/`description` changed.
///
/// The `space_code_metadata` upsert carries its own ownership `EXISTS`
/// guard (`WHERE EXISTS (SELECT 1 FROM space s WHERE s.id = $1 AND
/// s.user_id = $2)`) as defense-in-depth beyond the caller having already
/// checked ownership via the `space` half of this same call — the same
/// belt-and-suspenders pattern as `chunk_meta::replace_applies_to`.
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    patch: SpacePatch,
    code: Option<CodeUpdate>,
) -> AppResult<Option<Space>> {
    let (desc_set, desc_val) = match patch.description {
        Some(v) => (true, v),
        None => (false, None),
    };
    let has_changes = patch.name.is_some() || desc_set;

    let mut tx = pool.begin().await?;

    let space = if has_changes {
        sqlx::query_as!(
            Space,
            r#"UPDATE space SET
                 name = COALESCE($3, name),
                 description = CASE WHEN $4::bool THEN $5::text ELSE description END,
                 updated_at = now()
               WHERE id = $1 AND user_id = $2
               RETURNING id, name, kind, description, user_id,
                         created_at AS "created_at: UtcTimestamp",
                         updated_at AS "updated_at: UtcTimestamp""#,
            id,
            user_id,
            patch.name,
            desc_set,
            desc_val
        )
        .fetch_optional(&mut *tx)
        .await?
    } else {
        sqlx::query_as!(
            Space,
            r#"SELECT id, name, kind, description, user_id,
                      created_at AS "created_at: UtcTimestamp",
                      updated_at AS "updated_at: UtcTimestamp"
               FROM space WHERE id = $1 AND user_id = $2"#,
            id,
            user_id
        )
        .fetch_optional(&mut *tx)
        .await?
    };

    let space = match space {
        Some(s) => s,
        None => {
            tx.rollback().await?;
            return Ok(None);
        }
    };

    if let Some(code) = code {
        // On a fresh INSERT (no existing `space_code_metadata` row — should
        // not happen for a `code`-kind space post-`create`, but handled
        // defensively) an untouched field has no prior value to preserve,
        // so it falls back to `NULL`/`[]`. On the `ON CONFLICT` branch, the
        // `*_set` flags gate whether `EXCLUDED.*` (the proposed value above)
        // or the table's own current column value wins — see the doc
        // comment on `CodeUpdate` for why "not set" must mean "untouched",
        // not "cleared".
        let (remote_url_set, remote_url_val) = match code.remote_url {
            Some(v) => (true, v),
            None => (false, None),
        };
        let (local_paths_set, local_paths_val) = match code.local_paths {
            Some(v) => (true, v),
            None => (false, Vec::new()),
        };

        sqlx::query!(
            r#"INSERT INTO space_code_metadata (space_id, user_id, remote_url, local_paths)
               SELECT $1, $2, $3, $4
               WHERE EXISTS (SELECT 1 FROM space s WHERE s.id = $1 AND s.user_id = $2)
               ON CONFLICT (space_id) DO UPDATE
                 SET remote_url = CASE WHEN $5::bool THEN EXCLUDED.remote_url
                                        ELSE space_code_metadata.remote_url END,
                     local_paths = CASE WHEN $6::bool THEN EXCLUDED.local_paths
                                         ELSE space_code_metadata.local_paths END"#,
            id,
            user_id,
            remote_url_val,
            Json(local_paths_val) as _,
            remote_url_set,
            local_paths_set,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(Some(space))
}

pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let res = sqlx::query!(
        "DELETE FROM space WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// Wipes a space's content without touching the `space` row or its
/// `space_code_metadata` — matching Node's `resetSpaceData`
/// (`packages/db/src/repository/space.ts:144-184`, confirmed against source
/// per the task brief, since the captured HTTP contract left the semantics
/// ambiguous). In order:
///
/// 1. Finds every `chunk_space` row for `space_id`.
/// 2. Of those chunk ids, computes which are *exclusive* to this space (not
///    also linked to any other space via `chunk_space`).
/// 3. Hard-deletes the exclusive chunks (scoped to `chunk.user_id =
///    user_id`) — a chunk shared with another space survives, losing only
///    its association with this one.
/// 4. Deletes *all* `chunk_space` rows for this space, including the ones
///    for surviving shared chunks.
/// 5. Deletes space-scoped `document`/`plan`/`requirement` rows.
///
/// `chunks_deleted` reports `toDelete.length` (the count of chunk ids
/// identified as exclusive-and-therefore-slated-for-deletion), not the
/// database's `rows_affected()` from the actual `DELETE` — this matches
/// Node's own `chunksDeleted = toDelete.length` (assigned from the input
/// list, not the delete's return), a deliberate parity choice even though
/// it could in principle over-report if a `chunk_space` row somehow pointed
/// at a chunk not owned by `user_id` (should not happen in practice; no
/// code path inserts such a row).
///
/// Ownership of `space_id` itself is checked once, up front, inside this
/// same transaction (`EXISTS (SELECT 1 FROM space ...)`) — a caller that
/// somehow reaches this function for a `space_id` it doesn't own gets an
/// all-zero, no-op [`ResetResult`] rather than silently wiping another
/// user's chunks via the unconditional `DELETE FROM chunk_space WHERE
/// space_id = $1` at step 4. This guard is not present in Node (whose
/// service layer is the only thing standing between an unauthenticated
/// caller and this wipe) — added here as defense-in-depth for the same
/// reason `chunk_tag`'s delete-half needed one: an unscoped `DELETE ...
/// WHERE space_id = $1` on a join table is exactly the silent-data-loss
/// shape that bit `chunk_meta` in Phase 1.
pub async fn reset(pool: &PgPool, user_id: &str, space_id: &str) -> AppResult<ResetResult> {
    let mut tx = pool.begin().await?;

    let owns = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM space WHERE id = $1 AND user_id = $2) AS "owns!""#,
        space_id,
        user_id
    )
    .fetch_one(&mut *tx)
    .await?;
    if !owns {
        tx.rollback().await?;
        return Ok(ResetResult::default());
    }

    let exclusive_ids: Vec<String> = sqlx::query_scalar!(
        "SELECT chunk_id FROM chunk_space WHERE space_id = $1",
        space_id
    )
    .fetch_all(&mut *tx)
    .await?;

    let mut chunks_deleted: i64 = 0;
    if !exclusive_ids.is_empty() {
        let shared: std::collections::HashSet<String> = sqlx::query_scalar!(
            "SELECT chunk_id FROM chunk_space \
               WHERE chunk_id = ANY($1) AND space_id != $2",
            &exclusive_ids,
            space_id
        )
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .collect();

        let to_delete: Vec<String> = exclusive_ids
            .into_iter()
            .filter(|id| !shared.contains(id))
            .collect();

        if !to_delete.is_empty() {
            sqlx::query!(
                "DELETE FROM chunk WHERE id = ANY($1) AND user_id = $2",
                &to_delete,
                user_id
            )
            .execute(&mut *tx)
            .await?;
            chunks_deleted = to_delete.len() as i64;
        }
    }

    sqlx::query!("DELETE FROM chunk_space WHERE space_id = $1", space_id)
        .execute(&mut *tx)
        .await?;

    let docs_deleted = sqlx::query!(
        "DELETE FROM document WHERE space_id = $1 AND user_id = $2",
        space_id,
        user_id
    )
    .execute(&mut *tx)
    .await?
    .rows_affected() as i64;

    let plans_deleted = sqlx::query!(
        "DELETE FROM plan WHERE space_id = $1 AND user_id = $2",
        space_id,
        user_id
    )
    .execute(&mut *tx)
    .await?
    .rows_affected() as i64;

    let requirements_deleted = sqlx::query!(
        "DELETE FROM requirement WHERE space_id = $1 AND user_id = $2",
        space_id,
        user_id
    )
    .execute(&mut *tx)
    .await?
    .rows_affected() as i64;

    tx.commit().await?;

    Ok(ResetResult {
        chunks_deleted,
        docs_deleted,
        plans_deleted,
        requirements_deleted,
    })
}

/// Deletes and reinserts a chunk's full space set in one transaction — the
/// same "replace-set" pattern as `tag::set_chunk_tags`, and `chunk_space` is
/// the same shape of join: composite key `(chunk_id, space_id)`, no `id`, no
/// `user_id` of its own.
///
/// Ownership derives entirely from the two parent rows, and BOTH are
/// verified in SQL, not just trusted from the caller:
///
/// - the `DELETE` carries the same ownership `EXISTS` guard as
///   `chunk_meta::replace_applies_to` / `tag::set_chunk_tags`'s delete half
///   — proven load-bearing there by deleting it and watching a cross-user
///   test fail: without it, a correctly-rejected attach would still wipe
///   the victim chunk's *existing* space associations out from under them,
///   even though nothing new got inserted;
/// - the `INSERT ... SELECT` joins through *both* `chunk` and `space`,
///   requiring `c.user_id = $1 AND s.user_id = $1` — putting another user's
///   chunk into a space, and putting a chunk into another user's space, are
///   both rejected by the same query.
///
/// Returns the number of `chunk_space` rows actually inserted, so callers
/// can detect a rejected attach (0 inserted despite non-empty `space_ids`).
pub async fn set_chunk_spaces(
    pool: &PgPool,
    user_id: &str,
    chunk_id: &str,
    space_ids: &[String],
) -> AppResult<u64> {
    let mut tx = pool.begin().await?;

    sqlx::query!(
        "DELETE FROM chunk_space WHERE chunk_id = $1 \
           AND EXISTS (SELECT 1 FROM chunk c WHERE c.id = $1 AND c.user_id = $2)",
        chunk_id,
        user_id
    )
    .execute(&mut *tx)
    .await?;

    let inserted = if space_ids.is_empty() {
        0
    } else {
        sqlx::query!(
            r#"INSERT INTO chunk_space (chunk_id, space_id)
               SELECT c.id, s.id
               FROM chunk c
               JOIN space s ON s.id = ANY($3)
               WHERE c.id = $2
                 AND c.user_id = $1
                 AND s.user_id = $1
               ON CONFLICT (chunk_id, space_id) DO NOTHING"#,
            user_id,
            chunk_id,
            space_ids
        )
        .execute(&mut *tx)
        .await?
        .rows_affected()
    };

    tx.commit().await?;
    Ok(inserted)
}

/// Lists the spaces a chunk belongs to, scoped through the chunk's owner —
/// the same "through the parent" pattern as `tag::tags_for_chunk`.
pub async fn spaces_for_chunk(
    pool: &PgPool,
    user_id: &str,
    chunk_id: &str,
) -> AppResult<Vec<Space>> {
    let rows = sqlx::query_as!(
        Space,
        r#"SELECT s.id, s.name, s.kind, s.description, s.user_id,
                  s.created_at AS "created_at: UtcTimestamp",
                  s.updated_at AS "updated_at: UtcTimestamp"
           FROM chunk_space cs
           JOIN space s ON s.id = cs.space_id
           WHERE cs.chunk_id = $1
             AND EXISTS (SELECT 1 FROM chunk c WHERE c.id = $1 AND c.user_id = $2)
           ORDER BY s.name"#,
        chunk_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
