//! `use_case` groups related requirements one level deep — a use case can
//! have child use cases (`parent_id`), but Node's service layer enforces
//! "one level of nesting only" as an application rule, not a DB constraint
//! (`packages/api/src/use-cases/service.ts:29-42`): a use case whose own
//! `parentId` is already set cannot itself be used as someone else's
//! parent. See `crate::repo::use_case`'s callers in
//! `fubbik_api::use_cases::service` for where that rule is reproduced.
//!
//! **Deletion is a bare `DELETE ... WHERE id = $1 AND user_id = $2`, with no
//! app-level participation in what happens to dependent requirements.**
//! `requirement.use_case_id` carries `ON DELETE SET NULL`
//! (`crates/fubbik-db/migrations/0001_init.sql`:
//! `requirement_use_case_id_use_case_id_fk ... ON DELETE SET NULL`), so the
//! database itself nulls out every `requirement.use_case_id` that pointed at
//! a deleted row — matching Node's *effective* behaviour on a successful
//! delete, where `deleteUseCase`
//! (`packages/db/src/repository/use-case.ts:120-131`) runs its own
//! `UPDATE requirement SET use_case_id = NULL WHERE use_case_id = $1`
//! immediately before the `DELETE`.
//!
//! This is **not** a byte-for-byte behavioural match, though: Node's
//! unlink `UPDATE` has no `user_id` filter at all — it runs unconditionally
//! against every requirement row referencing this use case id, *before* the
//! ownership-scoped `DELETE` even executes. So a non-owner's blocked delete
//! attempt (wrong `user_id`, `DELETE` affects 0 rows, service returns 404)
//! still has the side effect in Node of nulling out the real owner's
//! requirement links, even though the use case row itself survives. Relying
//! on `ON DELETE SET NULL` here does not reproduce that: the cascade only
//! fires when the row is actually deleted, so a rejected cross-user delete
//! in this port leaves requirement links untouched. This mirrors the
//! "database-level `ON DELETE SET NULL`, no app code" shape from the
//! Phase 2e task brief precisely for the *successful* path, and incidentally
//! fixes what reads as a real bug in Node's unscoped side effect on the
//! *rejected* path — see `tests/use_case.rs::delete_by_non_owner_does_not_unlink_the_owners_requirements`.

use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::timestamp::UtcTimestamp;

/// Bare `use_case` row — the shape `POST /use-cases` and `PATCH
/// /use-cases/{id}` both return (`createUseCaseRepo`/`updateUseCaseRepo`
/// `.returning()`). **Not** the shape `GET /use-cases` (list) returns — see
/// [`UseCaseListItem`] for that one, which adds `childCount` and
/// `requirementCount`.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UseCase {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub space_id: Option<String>,
    pub user_id: String,
    pub order: i32,
    pub parent_id: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

/// Shape of each entry in `GET /use-cases` (`listUseCasesRepo`,
/// `packages/db/src/repository/use-case.ts:46-86`): the bare row plus two
/// computed counts — `childCount` (a correlated subquery over `use_case`
/// itself) and `requirementCount` (looked up from a separate grouped query
/// over `requirement`, defaulting to `0` when the map has no entry for this
/// id). Both counts are `i64` here (`count(*)` is `bigint` in Postgres) even
/// though Node's `Number(...)` coercion loses that distinction on the wire —
/// JSON has no int64 type either way.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UseCaseListItem {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub space_id: Option<String>,
    pub user_id: String,
    pub order: i32,
    pub parent_id: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
    pub child_count: i64,
    pub requirement_count: i64,
}

pub struct NewUseCase {
    pub name: String,
    pub description: Option<String>,
    pub space_id: Option<String>,
    pub parent_id: Option<String>,
}

/// `space_id`, when given, must belong to `user_id` — an `EXISTS` guard on
/// the `INSERT`, the same "guard through the parent" shape
/// `collection::create` uses for its own `space_id` (Phase 2e wave 1: this
/// module used to explicitly *not* have this guard, contrasting itself with
/// `collection::create`'s — see the git history of this doc comment). Node's
/// `createUseCaseRepo` (`packages/db/src/repository/use-case.ts:17-22`) is a
/// bare `db.insert(useCase).values(params).returning()` with no such check
/// at all — this is a deliberate divergence, same shape as accepted
/// divergences #4/#9/#10/#13/#14/#15/#17/#19, not a faithful port. Returns
/// `Ok(None)` (not an error) when `space_id` is given but not owned by
/// `user_id`, matching `collection::create`'s shape exactly — see
/// `tests/use_case.rs` for the load-bearing proof.
pub async fn create(pool: &PgPool, user_id: &str, new: NewUseCase) -> AppResult<Option<UseCase>> {
    let id = crate::new_id();
    let row = match &new.space_id {
        Some(space_id) => {
            sqlx::query_as!(
                UseCase,
                r#"INSERT INTO use_case (id, name, description, space_id, user_id, parent_id)
                   SELECT $1, $2, $3, $4, $5, $6
                   WHERE EXISTS (SELECT 1 FROM space s WHERE s.id = $4 AND s.user_id = $5)
                   RETURNING id, name, description, space_id, user_id, "order",
                             parent_id, created_at AS "created_at: UtcTimestamp",
                             updated_at AS "updated_at: UtcTimestamp""#,
                id,
                new.name,
                new.description,
                space_id,
                user_id,
                new.parent_id
            )
            .fetch_optional(pool)
            .await?
        }
        None => Some(
            sqlx::query_as!(
                UseCase,
                r#"INSERT INTO use_case (id, name, description, space_id, user_id, parent_id)
                   VALUES ($1, $2, $3, $4, $5, $6)
                   RETURNING id, name, description, space_id, user_id, "order",
                             parent_id, created_at AS "created_at: UtcTimestamp",
                             updated_at AS "updated_at: UtcTimestamp""#,
                id,
                new.name,
                new.description,
                new.space_id,
                user_id,
                new.parent_id
            )
            .fetch_one(pool)
            .await?,
        ),
    };
    Ok(row)
}

/// Looks up a use case by id, scoped to its owner in SQL. Node's
/// `getUseCaseById` (`packages/db/src/repository/use-case.ts:34-44`) takes
/// an *optional* `userId` and only applies the filter when one is given —
/// but every call site in the service layer (`getUseCase`, `createUseCase`'s
/// parent check, `updateUseCase`'s self/parent checks,
/// `getUseCaseRequirements`) always passes one, so the unscoped branch is
/// dead code in practice. This port collapses that to a single scoped
/// signature — `user_id` is required, not optional — since no caller ever
/// needs the unscoped form. Proven load-bearing in
/// `tests/use_case.rs::find_by_id_is_user_scoped`.
pub async fn find_by_id(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Option<UseCase>> {
    let row = sqlx::query_as!(
        UseCase,
        r#"SELECT id, name, description, space_id, user_id, "order",
                  parent_id, created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM use_case WHERE id = $1 AND user_id = $2"#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// `ORDER BY "order" ASC, name ASC` matches Node's `listUseCases`
/// (`packages/db/src/repository/use-case.ts:66`) exactly. Unlike every other
/// list query in this port, this one does **not** need an appended `id ASC`
/// tiebreaker to be a genuine total order: `use_case_user_name_idx` is a
/// `UNIQUE (user_id, name)` index, so within one caller's rows `name` alone
/// is already unique — `("order", name)` can never tie two distinct rows.
/// `id ASC` is still appended for defensive consistency with the rest of
/// this crate's list queries, but there is no way to construct a
/// forced-identical-sort-key stability test for it (the seed would violate
/// the unique index), unlike `notification::list`'s or `chunk::list`'s
/// equivalent tests — see this module's test file for why that test is
/// absent here.
pub async fn list(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<UseCaseListItem>> {
    let rows = sqlx::query!(
        r#"SELECT uc.id, uc.name, uc.description, uc.space_id, uc.user_id, uc."order",
                  uc.parent_id, uc.created_at, uc.updated_at,
                  (SELECT count(*) FROM use_case uc2 WHERE uc2.parent_id = uc.id) AS "child_count!",
                  (SELECT count(*) FROM requirement r WHERE r.use_case_id = uc.id AND r.user_id = uc.user_id) AS "requirement_count!"
           FROM use_case uc
           WHERE uc.user_id = $1 AND ($2::text IS NULL OR uc.space_id = $2)
           ORDER BY uc."order" ASC, uc.name ASC, uc.id ASC"#,
        user_id,
        space_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| UseCaseListItem {
            id: r.id,
            name: r.name,
            description: r.description,
            space_id: r.space_id,
            user_id: r.user_id,
            order: r.order,
            parent_id: r.parent_id,
            created_at: UtcTimestamp(r.created_at),
            updated_at: UtcTimestamp(r.updated_at),
            child_count: r.child_count,
            requirement_count: r.requirement_count,
        })
        .collect())
}

/// All four fields are plain two-state (`None` = leave untouched, `Some` =
/// set) except `description`, which is tri-state at the DTO layer (Node's
/// PATCH body accepts `t.Union([t.String(), t.Null()])` for it) — the
/// caller (`fubbik_api::use_cases::service::update`) flattens that down to
/// `Option<String>` here (`Some(None)` clears, `Some(Some(s))` sets, `None`
/// leaves it) the same way `collection::CollectionPatch` distinguishes
/// "provided" from "omitted" per-field with `Option<Option<T>>` at the DTO
/// boundary and a plain nullable column value here.
#[derive(Default)]
pub struct UseCasePatch {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub order: Option<i32>,
    pub parent_id: Option<Option<String>>,
}

/// Node's `updateUseCase` (`packages/db/src/repository/use-case.ts:95-118`)
/// builds a dynamic `setClause` and, when nothing was provided, **skips the
/// `UPDATE` entirely** and re-selects the row instead — unlike
/// `collection::update`, whose `UPDATE` always runs (bumping `updated_at`
/// unconditionally) because Drizzle's `$onUpdate` hook fires on any
/// `.set(...)` call, empty or not. Here, an empty `setClause` means no
/// `.set(...)` call happens at all, so `updated_at` is untouched on a
/// fully-omitted PATCH body — reproduced with the same short-circuit below.
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    patch: UseCasePatch,
) -> AppResult<Option<UseCase>> {
    if patch.name.is_none()
        && patch.description.is_none()
        && patch.order.is_none()
        && patch.parent_id.is_none()
    {
        return find_by_id(pool, user_id, id).await;
    }

    let description_set = patch.description.is_some();
    let description_value = patch.description.flatten();
    let parent_id_set = patch.parent_id.is_some();
    let parent_id_value = patch.parent_id.flatten();

    let row = sqlx::query_as!(
        UseCase,
        r#"UPDATE use_case SET
             name = COALESCE($3, name),
             description = CASE WHEN $4 THEN $5 ELSE description END,
             "order" = COALESCE($6, "order"),
             parent_id = CASE WHEN $7 THEN $8 ELSE parent_id END
           WHERE id = $1 AND user_id = $2
           RETURNING id, name, description, space_id, user_id, "order",
                     parent_id, created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        user_id,
        patch.name,
        description_set,
        description_value,
        patch.order,
        parent_id_set,
        parent_id_value
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Bare ownership-scoped delete. Dependent `requirement.use_case_id` rows
/// are nulled out by the database's own `ON DELETE SET NULL` — see this
/// module's doc comment for the full deletion-semantics writeup and how it
/// differs from Node's unscoped app-level unlink on the rejected-delete
/// path.
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let res = sqlx::query!(
        "DELETE FROM use_case WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// One `requirement` row, in full — the projection `GET
/// /use-cases/{id}/requirements` returns (`listRequirementsByUseCase`,
/// `packages/db/src/repository/use-case.ts:133-140`, a bare
/// `db.select().from(requirement).where(...)` with every column). Lives here
/// rather than in a `requirement` repo module because no such module exists
/// yet in this port (`crate::repo::requirement` is deliberately minimal —
/// see its module doc — reserved for the full `requirements` domain task
/// this one unblocks). Do not grow this into a general-purpose requirement
/// type; a real port belongs in its own module.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UseCaseRequirement {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    #[schema(value_type = Vec<serde_json::Value>)]
    pub steps: Json<serde_json::Value>,
    pub order: i32,
    pub status: String,
    pub priority: Option<String>,
    pub space_id: Option<String>,
    pub user_id: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
    pub origin: String,
    pub review_status: String,
    pub use_case_id: Option<String>,
    pub reviewed_by: Option<String>,
    #[schema(value_type = Option<chrono::NaiveDateTime>)]
    pub reviewed_at: Option<UtcTimestamp>,
}

/// Node's `listRequirementsByUseCase`
/// (`packages/db/src/repository/use-case.ts:133-140`) scopes by both
/// `useCaseId` and the caller's `userId` in the same `WHERE`, and has no
/// `ORDER BY` at all — reproduced exactly, including the missing order (no
/// tiebreaker added here, matching Node's own lack of one; contrast every
/// other `list` in this crate, which does add one).
pub async fn list_requirements(
    pool: &PgPool,
    user_id: &str,
    use_case_id: &str,
) -> AppResult<Vec<UseCaseRequirement>> {
    let rows = sqlx::query_as!(
        UseCaseRequirement,
        r#"SELECT id, title, description, steps AS "steps: Json<serde_json::Value>",
                  "order", status, priority, space_id, user_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp",
                  origin, review_status, use_case_id, reviewed_by,
                  reviewed_at AS "reviewed_at: UtcTimestamp"
           FROM requirement WHERE use_case_id = $1 AND user_id = $2"#,
        use_case_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
