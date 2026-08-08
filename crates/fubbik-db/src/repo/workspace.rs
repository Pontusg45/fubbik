use fubbik_core::error::AppResult;
use sqlx::PgPool;

use crate::timestamp::UtcTimestamp;

/// `camelCase` serialisation matches every other wire type in this crate —
/// see the note on `chunk::Chunk` for why that's mandatory, not cosmetic.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub user_id: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

/// Bare summary of a space attached to a workspace — `{id, name, kind}`
/// only, matching Node's `getSpacesForWorkspace`
/// (`packages/db/src/repository/workspace.ts:80-92`), which selects just
/// these three columns, not a full `Space` row (no `description`,
/// `userId`, timestamps). See
/// `tests/fixtures/node-contract-2b/workspaces-detail.json`.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSpaceSummary {
    pub id: String,
    pub name: String,
    pub kind: String,
}

/// `workspace_space` has only these two columns (no `id`, no timestamps) —
/// this is both the row shape and the response shape of
/// `POST /api/workspaces/{id}/spaces`, matching Node's
/// `addSpaceToWorkspace` return value: either the row just inserted, or,
/// on an `onConflictDoNothing` no-op, a literal `{ workspaceId, spaceId }`
/// fallback (`packages/db/src/repository/workspace.ts:94-99`) —
/// structurally identical either way.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSpaceLink {
    pub workspace_id: String,
    pub space_id: String,
}

pub struct NewWorkspace {
    pub name: String,
    pub description: Option<String>,
}

pub async fn create(pool: &PgPool, user_id: &str, new: NewWorkspace) -> AppResult<Workspace> {
    let id = crate::new_id();
    let w = sqlx::query_as!(
        Workspace,
        r#"INSERT INTO workspace (id, name, description, user_id)
           VALUES ($1, $2, $3, $4)
           RETURNING id, name, description, user_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        new.name,
        new.description,
        user_id
    )
    .fetch_one(pool)
    .await?;
    Ok(w)
}

pub async fn find_by_id(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Option<Workspace>> {
    let w = sqlx::query_as!(
        Workspace,
        r#"SELECT id, name, description, user_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM workspace WHERE id = $1 AND user_id = $2"#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(w)
}

/// `, id ASC` is a tiebreaker over `created_at`, which is not unique — see
/// `chunk::list`'s equivalent comment for why an untied `ORDER BY` over
/// tied rows is a query-plan artifact rather than a stable order.
pub async fn list(pool: &PgPool, user_id: &str) -> AppResult<Vec<Workspace>> {
    let rows = sqlx::query_as!(
        Workspace,
        r#"SELECT id, name, description, user_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM workspace WHERE user_id = $1 ORDER BY created_at ASC, id ASC"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// `description` is tri-state, matching Node's `t.Optional(t.Union([t.String(),
/// t.Null()]))` body schema: omitted (`None`) leaves it untouched, explicit
/// `null` (`Some(None)`) clears it, a string (`Some(Some(..))`) sets it. See
/// `tag::TagPatch::tag_type_id` for the same pattern.
#[derive(Default)]
pub struct WorkspacePatch {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
}

/// If neither `name` nor `description` is present in the patch, no `UPDATE`
/// runs at all (a plain `SELECT` is issued instead) — matching Node's
/// `updateWorkspace` (`packages/db/src/repository/workspace.ts:45-66`),
/// whose `setClause` stays empty in that case. In particular `updated_at`
/// is *not* bumped on a no-op patch, matching Drizzle's `$onUpdate` hook,
/// which only fires on an actual `.update().set(...)` call — same
/// structure as `space::update`.
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    patch: WorkspacePatch,
) -> AppResult<Option<Workspace>> {
    let (desc_set, desc_val) = match patch.description {
        Some(v) => (true, v),
        None => (false, None),
    };
    let has_changes = patch.name.is_some() || desc_set;

    let w = if has_changes {
        sqlx::query_as!(
            Workspace,
            r#"UPDATE workspace SET
                 name = COALESCE($3, name),
                 description = CASE WHEN $4::bool THEN $5::text ELSE description END,
                 updated_at = now()
               WHERE id = $1 AND user_id = $2
               RETURNING id, name, description, user_id,
                         created_at AS "created_at: UtcTimestamp",
                         updated_at AS "updated_at: UtcTimestamp""#,
            id,
            user_id,
            patch.name,
            desc_set,
            desc_val
        )
        .fetch_optional(pool)
        .await?
    } else {
        sqlx::query_as!(
            Workspace,
            r#"SELECT id, name, description, user_id,
                      created_at AS "created_at: UtcTimestamp",
                      updated_at AS "updated_at: UtcTimestamp"
               FROM workspace WHERE id = $1 AND user_id = $2"#,
            id,
            user_id
        )
        .fetch_optional(pool)
        .await?
    };
    Ok(w)
}

/// `workspace_space` rows for this workspace are removed by the database's
/// own `ON DELETE CASCADE` foreign key
/// (`workspace_space_workspace_id_fkey`, `migrations/0001_init.sql`) — this
/// function deliberately does not touch `workspace_space` itself, matching
/// Node (`deleteWorkspace`, `packages/db/src/repository/workspace.ts:68-76`,
/// which relies on the same FK cascade per `_mutating.md`).
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let res = sqlx::query!(
        "DELETE FROM workspace WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// Attaches a space to a workspace. `workspace_space` is a composite-key
/// join with no `user_id` of its own — ownership derives entirely from its
/// two parent rows, and BOTH are verified in this one SQL statement, the
/// same "proven pattern" as `tag::set_chunk_tags` / `space::set_chunk_spaces`:
///
/// - the `WHERE` clause requires `w.user_id = $1` (the workspace must be
///   the caller's) AND `s.user_id = $1` (the space must be the caller's)
///   in the same query — cross-user attach is impossible in *either*
///   direction, not just one;
/// - `ON CONFLICT (workspace_id, space_id) DO NOTHING` reproduces Node's
///   `.onConflictDoNothing()` on the composite primary key — attaching an
///   already-linked space is a no-op, not an error.
///
/// Returns `None` both when the guard rejected the attach and when it was
/// already-linked (`ON CONFLICT` skipped the insert) — the two cases are
/// deliberately not distinguished here, the same ambiguity documented on
/// `favorite::add`. The caller (`workspaces::service::add_space_to_workspace`)
/// disambiguates by checking workspace/space ownership itself *before*
/// calling this, matching Node's service layer, which does the same
/// `getWorkspaceById`/`getSpaceById` checks ahead of the insert
/// (`packages/api/src/workspaces/service.ts:84-94`) — so by the time this
/// function is reached through the API, `None` can only mean "already
/// linked", and the service falls back to the same
/// `{ workspaceId, spaceId }` literal Node does.
pub async fn add_space(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
    space_id: &str,
) -> AppResult<Option<WorkspaceSpaceLink>> {
    let row = sqlx::query_as!(
        WorkspaceSpaceLink,
        r#"INSERT INTO workspace_space (workspace_id, space_id)
           SELECT w.id, s.id
           FROM workspace w
           JOIN space s ON s.id = $3
           WHERE w.id = $2 AND w.user_id = $1 AND s.user_id = $1
           ON CONFLICT (workspace_id, space_id) DO NOTHING
           RETURNING workspace_id, space_id"#,
        user_id,
        workspace_id,
        space_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Removes a space from a workspace. This is the guard that matters most
/// and is easiest to omit: the `DELETE` itself carries a workspace-ownership
/// check (`EXISTS (SELECT 1 FROM workspace w WHERE w.id = $2 AND w.user_id
/// = $1)`) as defense-in-depth *in addition to* the service layer's own
/// `workspace::find_by_id` pre-check
/// (`packages/api/src/workspaces/service.ts:96-105`'s `getWorkspaceById`
/// call, mirrored in `workspaces::service::remove_space_from_workspace`).
/// Without this guard, a caller that reaches this function without going
/// through that pre-check (a future refactor, a direct repo call) would
/// silently delete another user's `workspace_space` row: an unscoped
/// `DELETE FROM workspace_space WHERE workspace_id = $1 AND space_id = $2`
/// has no `user_id` column of its own to filter on at all, so ownership
/// has to come from the parent `workspace` row, in SQL, not just be
/// assumed. See `tests/workspace.rs::rejected_remove_does_not_wipe_the_victims_existing_association`.
pub async fn remove_space(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
    space_id: &str,
) -> AppResult<bool> {
    let res = sqlx::query!(
        r#"DELETE FROM workspace_space
           WHERE workspace_id = $2 AND space_id = $3
             AND EXISTS (SELECT 1 FROM workspace w WHERE w.id = $2 AND w.user_id = $1)"#,
        user_id,
        workspace_id,
        space_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// Lists the spaces attached to a workspace, scoped through the workspace's
/// owner — the same "through the parent" pattern as `tag::tags_for_chunk` /
/// `space::spaces_for_chunk`.
///
/// `ORDER BY s.name, s.id ASC` is a Rust-side addition: Node's
/// `getSpacesForWorkspace` has no `.orderBy(...)` at all
/// (`packages/db/src/repository/workspace.ts:80-92`), so its row order is
/// undefined, not merely unspecified-but-stable — there is no defined Node
/// behaviour to diverge from here, the same non-divergence documented on
/// `tag::list` and `differential.rs`'s divergence #6.
pub async fn spaces_for_workspace(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
) -> AppResult<Vec<WorkspaceSpaceSummary>> {
    let rows = sqlx::query_as!(
        WorkspaceSpaceSummary,
        r#"SELECT s.id, s.name, s.kind
           FROM workspace_space ws
           JOIN space s ON s.id = ws.space_id
           WHERE ws.workspace_id = $1
             AND EXISTS (SELECT 1 FROM workspace w WHERE w.id = $1 AND w.user_id = $2)
           ORDER BY s.name ASC, s.id ASC"#,
        workspace_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
