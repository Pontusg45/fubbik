//! `requirement` domain: BDD-style requirements with Given/When/Then steps,
//! grouped by `use_case`, linked to `chunk` rows via the `requirement_chunk`
//! join table.
//!
//! Direct port of `packages/db/src/repository/requirement.ts`. Dependency
//! management (`requirement_dependency`) lives in `repo::requirement_dependency`
//! — a separate module because it is a self-contained bucket (Phase 2e task
//! brief's "dependencies second" split), not because the underlying table
//! isn't part of this same domain.

use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::timestamp::UtcTimestamp;

/// The five BDD step keywords. Constrained to an enum because Node's route
/// schema constrains it too — `StepSchema.keyword` is
/// `t.Union([t.Literal("given"), t.Literal("when"), t.Literal("then"),
/// t.Literal("and"), t.Literal("but")])`
/// (`packages/api/src/requirements/routes.ts:10`), on every route that
/// accepts steps (create, update, batch). Contrast `status`/`priority`,
/// which are genuinely free `text` at the DB layer and only sometimes
/// constrained at specific routes — see `Requirement::status`'s doc
/// comment.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum StepKeyword {
    Given,
    When,
    Then,
    And,
    But,
}

impl StepKeyword {
    pub fn as_str(self) -> &'static str {
        match self {
            StepKeyword::Given => "given",
            StepKeyword::When => "when",
            StepKeyword::Then => "then",
            StepKeyword::And => "and",
            StepKeyword::But => "but",
        }
    }
}

impl std::fmt::Display for StepKeyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One BDD step, matching Node's `RequirementStep`
/// (`packages/db/src/schema/requirement.ts:9-13`). `params` is a free-form
/// `{key: value}` map used by `export::interpolate` to fill `{key}`
/// placeholders in `text` — same shape Node's `t.Optional(t.Record(t.String(),
/// t.String()))` accepts.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequirementStep {
    pub keyword: StepKeyword,
    pub text: String,
    pub params: Option<std::collections::HashMap<String, String>>,
}

/// `camelCase` serialisation matches every other wire type in this crate.
///
/// `status` is plain `text NOT NULL DEFAULT 'untested'` at the DB layer —
/// no CHECK constraint (`crates/fubbik-db/migrations/0001_init.sql:603`).
/// Node's Elysia schema constrains it to `passing | failing | untested`
/// **only** on the two routes that write it directly (`PATCH
/// /requirements/{id}/status`'s `StatusSchema`, and `PATCH
/// /requirements/bulk`'s `set_status` action) — the list filter
/// (`GET /requirements?status=`) is `t.Optional(t.String())`, unconstrained.
/// `priority` is the same shape: free `text`, nullable, constrained to
/// `must | should | could | wont` only on create/update bodies, free-text
/// on the list filter. Both stay `String`/`Option<String>` here; the write
/// paths' DTOs are what carry the enum constraint (see
/// `fubbik_api::requirements::dto::Status`/`Priority`).
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    #[schema(value_type = Vec<RequirementStep>)]
    pub steps: Json<Vec<RequirementStep>>,
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

pub struct NewRequirement {
    pub title: String,
    pub description: Option<String>,
    pub steps: Vec<RequirementStep>,
    pub priority: Option<String>,
    pub space_id: Option<String>,
    pub use_case_id: Option<String>,
    pub origin: String,
    pub review_status: String,
}

/// Guards `space_id`/`use_case_id` ownership at INSERT time via `WHERE
/// EXISTS`/`SELECT` — Node's `createRequirement`
/// (`packages/db/src/repository/requirement.ts:24-31`) is a bare
/// `db.insert(requirement).values(params).returning()` with no such check
/// at all. This is the same accepted divergence `use_case::create` and
/// `collection::create` already carry (Phase 2e task briefs #4/#9/#10/#13/
/// #14/#15/#17/#19-style): a foreign `space_id`/`use_case_id` silently
/// inserts nothing (`Ok(None)`) rather than creating a requirement pointing
/// at data the caller cannot see. Four branches (one per combination of the
/// two optional guarded FKs) because sqlx's compile-time `query_as!` macro
/// needs a fixed SQL string per branch — see `use_case::create`'s doc
/// comment for why this isn't collapsed into a single dynamic query.
pub async fn create(
    pool: &PgPool,
    user_id: &str,
    new: NewRequirement,
) -> AppResult<Option<Requirement>> {
    let id = crate::new_id();
    let steps = Json(&new.steps);
    let row = match (&new.space_id, &new.use_case_id) {
        (None, None) => Some(
            sqlx::query_as!(
                Requirement,
                r#"INSERT INTO requirement (id, title, description, steps, priority, space_id, use_case_id, user_id, origin, review_status)
                   VALUES ($1, $2, $3, $4, $5, NULL, NULL, $6, $7, $8)
                   RETURNING id, title, description, steps AS "steps: Json<Vec<RequirementStep>>",
                             "order", status, priority, space_id, user_id,
                             created_at AS "created_at: UtcTimestamp",
                             updated_at AS "updated_at: UtcTimestamp",
                             origin, review_status, use_case_id, reviewed_by,
                             reviewed_at AS "reviewed_at: UtcTimestamp""#,
                id,
                new.title,
                new.description,
                steps as _,
                new.priority,
                user_id,
                new.origin,
                new.review_status
            )
            .fetch_one(pool)
            .await?,
        ),
        (Some(space_id), None) => sqlx::query_as!(
            Requirement,
            r#"INSERT INTO requirement (id, title, description, steps, priority, space_id, use_case_id, user_id, origin, review_status)
               SELECT $1, $2, $3, $4, $5, $6, NULL, $7, $8, $9
               WHERE EXISTS (SELECT 1 FROM space s WHERE s.id = $6 AND s.user_id = $7)
               RETURNING id, title, description, steps AS "steps: Json<Vec<RequirementStep>>",
                         "order", status, priority, space_id, user_id,
                         created_at AS "created_at: UtcTimestamp",
                         updated_at AS "updated_at: UtcTimestamp",
                         origin, review_status, use_case_id, reviewed_by,
                         reviewed_at AS "reviewed_at: UtcTimestamp""#,
            id,
            new.title,
            new.description,
            steps as _,
            new.priority,
            space_id,
            user_id,
            new.origin,
            new.review_status
        )
        .fetch_optional(pool)
        .await?,
        (None, Some(use_case_id)) => sqlx::query_as!(
            Requirement,
            r#"INSERT INTO requirement (id, title, description, steps, priority, space_id, use_case_id, user_id, origin, review_status)
               SELECT $1, $2, $3, $4, $5, NULL, $6, $7, $8, $9
               WHERE EXISTS (SELECT 1 FROM use_case uc WHERE uc.id = $6 AND uc.user_id = $7)
               RETURNING id, title, description, steps AS "steps: Json<Vec<RequirementStep>>",
                         "order", status, priority, space_id, user_id,
                         created_at AS "created_at: UtcTimestamp",
                         updated_at AS "updated_at: UtcTimestamp",
                         origin, review_status, use_case_id, reviewed_by,
                         reviewed_at AS "reviewed_at: UtcTimestamp""#,
            id,
            new.title,
            new.description,
            steps as _,
            new.priority,
            use_case_id,
            user_id,
            new.origin,
            new.review_status
        )
        .fetch_optional(pool)
        .await?,
        (Some(space_id), Some(use_case_id)) => sqlx::query_as!(
            Requirement,
            r#"INSERT INTO requirement (id, title, description, steps, priority, space_id, use_case_id, user_id, origin, review_status)
               SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9, $10
               WHERE EXISTS (SELECT 1 FROM space s WHERE s.id = $6 AND s.user_id = $8)
                 AND EXISTS (SELECT 1 FROM use_case uc WHERE uc.id = $7 AND uc.user_id = $8)
               RETURNING id, title, description, steps AS "steps: Json<Vec<RequirementStep>>",
                         "order", status, priority, space_id, user_id,
                         created_at AS "created_at: UtcTimestamp",
                         updated_at AS "updated_at: UtcTimestamp",
                         origin, review_status, use_case_id, reviewed_by,
                         reviewed_at AS "reviewed_at: UtcTimestamp""#,
            id,
            new.title,
            new.description,
            steps as _,
            new.priority,
            space_id,
            use_case_id,
            user_id,
            new.origin,
            new.review_status
        )
        .fetch_optional(pool)
        .await?,
    };
    Ok(row)
}

pub async fn find_by_id(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Option<Requirement>> {
    let row = sqlx::query_as!(
        Requirement,
        r#"SELECT id, title, description, steps AS "steps: Json<Vec<RequirementStep>>",
                  "order", status, priority, space_id, user_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp",
                  origin, review_status, use_case_id, reviewed_by,
                  reviewed_at AS "reviewed_at: UtcTimestamp"
           FROM requirement WHERE id = $1 AND user_id = $2"#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub struct ListParams<'a> {
    pub space_id: Option<&'a str>,
    pub use_case_id: Option<&'a str>,
    pub status: Option<&'a str>,
    pub priority: Option<&'a str>,
    pub origin: Option<&'a str>,
    pub review_status: Option<&'a str>,
    pub search: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
}

fn push_filters(
    qb: &mut sqlx::QueryBuilder<'_, sqlx::Postgres>,
    user_id: &str,
    params: &ListParams<'_>,
) {
    qb.push(" WHERE user_id = ").push_bind(user_id.to_string());

    if let Some(v) = params.space_id {
        qb.push(" AND space_id = ").push_bind(v.to_string());
    }
    if let Some(v) = params.use_case_id {
        qb.push(" AND use_case_id = ").push_bind(v.to_string());
    }
    if let Some(v) = params.status {
        qb.push(" AND status = ").push_bind(v.to_string());
    }
    if let Some(v) = params.priority {
        qb.push(" AND priority = ").push_bind(v.to_string());
    }
    if let Some(v) = params.origin {
        qb.push(" AND origin = ").push_bind(v.to_string());
    }
    if let Some(v) = params.review_status {
        qb.push(" AND review_status = ").push_bind(v.to_string());
    }
    if let Some(s) = params.search {
        // Wildcards escaped exactly like Node's `listRequirements`
        // (`packages/db/src/repository/requirement.ts:68-69`):
        // `search.replace(/[%_\\]/g, c => \\${c})`.
        let pattern = format!(
            "%{}%",
            s.replace('\\', r"\\")
                .replace('%', r"\%")
                .replace('_', r"\_")
        );
        qb.push(" AND (title ILIKE ").push_bind(pattern.clone());
        qb.push(" OR description ILIKE ").push_bind(pattern);
        qb.push(")");
    }
}

/// `ORDER BY "order" ASC, created_at ASC` matches Node's `listRequirements`
/// exactly (`packages/db/src/repository/requirement.ts:77`); `, id ASC` is
/// an added tiebreaker — both `order` (defaults to `0` for every newly
/// created row) and `created_at` (batch inserts, seed data) can tie, and an
/// untied `ORDER BY` over tied rows is a query-plan artifact, same
/// rationale as `chunk::list`.
///
/// Limit clamped to `[1, 100]` — same floor-of-1 divergence `chunk::list`
/// documents. Node's own clamp (`packages/api/src/requirements/service.ts:72`)
/// is `Math.min(Number(query.limit ?? 50), 100)`, which has no floor at all.
pub async fn list(
    pool: &PgPool,
    user_id: &str,
    params: &ListParams<'_>,
) -> AppResult<Vec<Requirement>> {
    let mut qb = sqlx::QueryBuilder::new(
        r#"SELECT id, title, description, steps,
                  "order", status, priority, space_id, user_id,
                  created_at, updated_at,
                  origin, review_status, use_case_id, reviewed_by, reviewed_at
           FROM requirement"#,
    );
    push_filters(&mut qb, user_id, params);
    qb.push(r#" ORDER BY "order" ASC, created_at ASC, id ASC"#);
    qb.push(" LIMIT ").push_bind(params.limit.clamp(1, 100));
    qb.push(" OFFSET ").push_bind(params.offset.max(0));

    let rows = qb.build_query_as::<Requirement>().fetch_all(pool).await?;
    Ok(rows)
}

/// Shares `push_filters` with [`list`] so `total` can never drift from what
/// `list` actually returns.
pub async fn count(pool: &PgPool, user_id: &str, params: &ListParams<'_>) -> AppResult<i64> {
    let mut qb = sqlx::QueryBuilder::new("SELECT COUNT(*) FROM requirement");
    push_filters(&mut qb, user_id, params);
    let total: i64 = qb.build_query_scalar().fetch_one(pool).await?;
    Ok(total)
}

/// Every field but `title`/`steps`/`origin`/`review_status` is tri-state
/// (`None` = untouched, `Some(None)` = clear, `Some(Some(v))` = set),
/// matching Node's `UpdateRequirementParams`
/// (`packages/db/src/repository/requirement.ts:90-102`): `description`,
/// `priority`, `space_id`, `use_case_id` are all `t.Optional(t.Union([T,
/// t.Null()]))` at the route layer. `title`/`steps`/`origin`/
/// `review_status` have no null variant in Node's schema, so plain
/// `Option<T>` (COALESCE) is enough. `reviewed_by`/`reviewed_at` are set by
/// the service layer only when `review_status` is provided — never
/// independently clearable via the API — so they stay plain `Option<T>`
/// too.
#[derive(Default)]
pub struct RequirementPatch {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub steps: Option<Vec<RequirementStep>>,
    pub priority: Option<Option<String>>,
    pub space_id: Option<Option<String>>,
    pub use_case_id: Option<Option<String>>,
    pub origin: Option<String>,
    pub review_status: Option<String>,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<UtcTimestamp>,
}

/// Node's `updateRequirement` short-circuits to a plain re-select when the
/// computed `setClause` is empty (`packages/db/src/repository/requirement.ts:
/// 119-125`) rather than running a no-op `UPDATE` — reproduced here the
/// same way `use_case::update` does, so `updated_at` (no `$onUpdate` hook
/// fires without a `.set()` call) stays untouched on a fully-omitted PATCH.
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    patch: RequirementPatch,
) -> AppResult<Option<Requirement>> {
    if patch.title.is_none()
        && patch.description.is_none()
        && patch.steps.is_none()
        && patch.priority.is_none()
        && patch.space_id.is_none()
        && patch.use_case_id.is_none()
        && patch.origin.is_none()
        && patch.review_status.is_none()
        && patch.reviewed_by.is_none()
        && patch.reviewed_at.is_none()
    {
        return find_by_id(pool, user_id, id).await;
    }

    let (description_set, description_val) = match patch.description {
        Some(v) => (true, v),
        None => (false, None),
    };
    let (priority_set, priority_val) = match patch.priority {
        Some(v) => (true, v),
        None => (false, None),
    };
    let (space_id_set, space_id_val) = match patch.space_id {
        Some(v) => (true, v),
        None => (false, None),
    };
    let (use_case_id_set, use_case_id_val) = match patch.use_case_id {
        Some(v) => (true, v),
        None => (false, None),
    };
    let row = sqlx::query_as!(
        Requirement,
        r#"UPDATE requirement SET
             title = COALESCE($3, title),
             description = CASE WHEN $4 THEN $5 ELSE description END,
             steps = COALESCE($6, steps),
             priority = CASE WHEN $7 THEN $8 ELSE priority END,
             space_id = CASE WHEN $9 THEN $10 ELSE space_id END,
             use_case_id = CASE WHEN $11 THEN $12 ELSE use_case_id END,
             origin = COALESCE($13, origin),
             review_status = COALESCE($14, review_status),
             reviewed_by = COALESCE($15, reviewed_by),
             reviewed_at = COALESCE($16, reviewed_at)
           WHERE id = $1 AND user_id = $2
           RETURNING id, title, description, steps AS "steps: Json<Vec<RequirementStep>>",
                     "order", status, priority, space_id, user_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp",
                     origin, review_status, use_case_id, reviewed_by,
                     reviewed_at AS "reviewed_at: UtcTimestamp""#,
        id,
        user_id,
        patch.title,
        description_set,
        description_val,
        patch.steps.map(Json) as _,
        priority_set,
        priority_val,
        space_id_set,
        space_id_val,
        use_case_id_set,
        use_case_id_val,
        patch.origin,
        patch.review_status,
        patch.reviewed_by,
        patch.reviewed_at.map(chrono::NaiveDateTime::from)
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let res = sqlx::query!(
        "DELETE FROM requirement WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn update_status(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    status: &str,
) -> AppResult<Option<Requirement>> {
    let row = sqlx::query_as!(
        Requirement,
        r#"UPDATE requirement SET status = $3
           WHERE id = $1 AND user_id = $2
           RETURNING id, title, description, steps AS "steps: Json<Vec<RequirementStep>>",
                     "order", status, priority, space_id, user_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp",
                     origin, review_status, use_case_id, reviewed_by,
                     reviewed_at AS "reviewed_at: UtcTimestamp""#,
        id,
        user_id,
        status
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// One `chunk` row's summary fields — the projection `getChunksForRequirement`
/// returns (`packages/db/src/repository/requirement.ts:182-195`).
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequirementChunk {
    pub id: String,
    pub title: String,
    pub content: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
}

/// One `requirement_chunk` join row, matching Node's `setRequirementChunks`
/// return shape exactly: `.returning()` on the raw Drizzle insert, no
/// column projection, so both columns come back
/// (`packages/db/src/repository/requirement.ts:158-180`). This is the
/// actual response body of `PUT /requirements/{id}/chunks` — **not** the
/// linked chunks' own title/content/type (that shape is
/// [`RequirementChunk`], returned only by `GET /requirements/{id}` and
/// nothing else).
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequirementChunkLink {
    pub requirement_id: String,
    pub chunk_id: String,
}

/// Deletes and reinserts a requirement's full chunk-link set in one
/// transaction. `requirement_chunk` has no `user_id` of its own — ownership
/// derives entirely from its two parent rows, and BOTH are independently
/// verified in SQL, the same three-guard shape `tag::set_chunk_tags` uses
/// for `chunk_tag`:
///
/// - the `DELETE` carries an ownership `EXISTS` guard against `requirement`
///   — calling this for a requirement the caller doesn't own is a safe
///   no-op, so a rejected call can never wipe another user's links out from
///   under them (the "silent-data-loss" case the Phase 2e task brief calls
///   out by name);
/// - the `INSERT ... SELECT` joins through both `requirement` and `chunk`,
///   requiring `r.user_id = $2 AND c.user_id = $2` — linking another user's
///   chunk, or linking chunks to another user's requirement, are both
///   rejected by the same query.
///
/// Both guards are a defensive backstop here, not the primary rejection
/// path: `fubbik_api::requirements::service::set_chunks` independently
/// verifies every `chunk_id` exists and belongs to `user_id` *before*
/// calling this at all (matching Node's `setChunks` service, which fails
/// the whole call with a 404 naming the missing chunk rather than silently
/// dropping it — `packages/api/src/requirements/service.ts:219-238`), so
/// this function's own guard should never actually reject anything through
/// that call site in practice.
pub async fn set_chunks(
    pool: &PgPool,
    user_id: &str,
    requirement_id: &str,
    chunk_ids: &[String],
) -> AppResult<Vec<RequirementChunkLink>> {
    let mut tx = pool.begin().await?;

    sqlx::query!(
        "DELETE FROM requirement_chunk WHERE requirement_id = $1 \
           AND EXISTS (SELECT 1 FROM requirement r WHERE r.id = $1 AND r.user_id = $2)",
        requirement_id,
        user_id
    )
    .execute(&mut *tx)
    .await?;

    let inserted = if chunk_ids.is_empty() {
        vec![]
    } else {
        sqlx::query_as!(
            RequirementChunkLink,
            r#"INSERT INTO requirement_chunk (requirement_id, chunk_id)
               SELECT r.id, c.id
               FROM requirement r
               JOIN chunk c ON c.id = ANY($3)
               WHERE r.id = $2
                 AND r.user_id = $1
                 AND c.user_id = $1
               ON CONFLICT (requirement_id, chunk_id) DO NOTHING
               RETURNING requirement_id, chunk_id"#,
            user_id,
            requirement_id,
            chunk_ids
        )
        .fetch_all(&mut *tx)
        .await?
    };

    tx.commit().await?;
    Ok(inserted)
}

/// Scoped through the requirement's owner, the same "through the parent"
/// pattern as `tag::tags_for_chunk`. **No `ORDER BY`**, matching Node's
/// `getChunksForRequirement` exactly (`packages/db/src/repository/
/// requirement.ts:182-195`, a bare `db.select().from(requirementChunk)
/// .innerJoin(chunk, ...)` with no `.orderBy(...)` at all) — this is one of
/// the rare list queries in this crate deliberately left without a
/// tiebreaker, because Node itself has none.
pub async fn get_chunks(
    pool: &PgPool,
    user_id: &str,
    requirement_id: &str,
) -> AppResult<Vec<RequirementChunk>> {
    let rows = sqlx::query_as!(
        RequirementChunk,
        r#"SELECT c.id, c.title, c.content, c.type AS "chunk_type"
           FROM requirement_chunk rc
           JOIN chunk c ON c.id = rc.chunk_id
           WHERE rc.requirement_id = $1
             AND EXISTS (SELECT 1 FROM requirement r WHERE r.id = $1 AND r.user_id = $2)"#,
        requirement_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Matches Node's `bulkUpdateRequirements`
/// (`packages/db/src/repository/requirement.ts:197-211`): only `status` or
/// `use_case_id` may be set in bulk. `use_case_id` is genuinely tri-state
/// here too — Node's bulk body accepts `useCaseId: string | null` to clear
/// it, distinct from omitting the field entirely (no bulk action at all,
/// since `action` is a required discriminator and `set_use_case` is the
/// only action that touches this column).
pub struct BulkPatch {
    pub status: Option<String>,
    pub use_case_id: Option<Option<String>>,
}

/// Returns the number of rows affected, matching Node's `result.rowCount ??
/// 0`. An empty patch (`status: None, use_case_id: None`) matches Node's
/// own short-circuit (`Object.keys(setClause).length === 0` returns `0`
/// without running a query) — reproduced here by never reaching the SQL at
/// all.
pub async fn bulk_update(
    pool: &PgPool,
    user_id: &str,
    ids: &[String],
    patch: BulkPatch,
) -> AppResult<u64> {
    if patch.status.is_none() && patch.use_case_id.is_none() {
        return Ok(0);
    }
    let (use_case_id_set, use_case_id_val) = match patch.use_case_id {
        Some(v) => (true, v),
        None => (false, None),
    };
    let res = sqlx::query!(
        r#"UPDATE requirement SET
             status = COALESCE($4, status),
             use_case_id = CASE WHEN $3 THEN $5 ELSE use_case_id END
           WHERE id = ANY($1) AND user_id = $2"#,
        ids,
        user_id,
        use_case_id_set,
        patch.status,
        use_case_id_val
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

pub async fn bulk_delete(pool: &PgPool, user_id: &str, ids: &[String]) -> AppResult<u64> {
    let res = sqlx::query!(
        "DELETE FROM requirement WHERE id = ANY($1) AND user_id = $2",
        ids,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

/// Minimal projection for `reorderRequirements`'s ownership check —
/// matches Node's `getRequirementsByIds` column set exactly
/// (`packages/db/src/repository/requirement.ts:220-227`).
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct RequirementSummary {
    pub id: String,
    pub use_case_id: Option<String>,
    pub title: String,
    pub status: String,
}

pub async fn find_by_ids(
    pool: &PgPool,
    user_id: &str,
    ids: &[String],
) -> AppResult<Vec<RequirementSummary>> {
    let rows = sqlx::query_as!(
        RequirementSummary,
        "SELECT id, use_case_id, title, status FROM requirement WHERE id = ANY($1) AND user_id = $2",
        ids,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Sets `"order"` to each id's position in `requirement_ids`, one `UPDATE`
/// per row — matches Node's `setRequirementOrder`
/// (`packages/db/src/repository/requirement.ts:229-238`) loop shape
/// exactly. Unlike Node's repo function, this one *does* scope every
/// `UPDATE` by `user_id` in SQL — Node's own `setRequirementOrder` has no
/// `user_id` filter at all and relies entirely on the service layer's
/// `getRequirementsByIds` ownership check running first
/// (`packages/api/src/requirements/service.ts:263-277`). This port adds
/// the guard anyway, matching this codebase's "scope by `user_id` in SQL,
/// never in the caller" convention — a defensive belt-and-suspenders
/// addition, not a required-for-parity divergence, since the service layer
/// here performs the identical pre-check before calling this at all.
pub async fn set_order(pool: &PgPool, user_id: &str, requirement_ids: &[String]) -> AppResult<u64> {
    let mut updated = 0u64;
    for (i, id) in requirement_ids.iter().enumerate() {
        let res = sqlx::query!(
            r#"UPDATE requirement SET "order" = $3 WHERE id = $1 AND user_id = $2"#,
            id,
            user_id,
            i as i32
        )
        .execute(pool)
        .await?;
        updated += res.rows_affected();
    }
    Ok(updated)
}

/// Matches Node's `getRequirementStats`
/// (`packages/db/src/repository/requirement.ts:266-290`): counts grouped
/// by `status`, folded into fixed `passing`/`failing`/`untested` buckets
/// (any other status value, though nothing in this port's write paths can
/// produce one, would count toward `total` but no named bucket — same as
/// Node).
#[derive(Debug, Clone, Default, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequirementStats {
    pub total: i64,
    pub passing: i64,
    pub failing: i64,
    pub untested: i64,
}

pub async fn stats(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<RequirementStats> {
    let rows = sqlx::query!(
        r#"SELECT status, COUNT(*) AS "count!"
           FROM requirement
           WHERE user_id = $1 AND ($2::text IS NULL OR space_id = $2)
           GROUP BY status"#,
        user_id,
        space_id
    )
    .fetch_all(pool)
    .await?;

    let mut out = RequirementStats::default();
    for row in rows {
        out.total += row.count;
        match row.status.as_str() {
            "passing" => out.passing = row.count,
            "failing" => out.failing = row.count,
            "untested" => out.untested = row.count,
            _ => {}
        }
    }
    Ok(out)
}

/// One `(id, title)` match from [`search_titles`].
#[derive(Debug, Clone)]
pub struct RequirementTitleMatch {
    pub id: String,
    pub title: String,
}

/// Direct port of Node's `searchRequirementTitles`
/// (`packages/db/src/repository/requirement.ts:292-299`) — same shape as
/// `chunk::search_titles`, and the same two things that look like bugs but
/// are the ported behaviour: `ILIKE '%prefix%'` with the pattern
/// unescaped, and no `ORDER BY`. See `chunk::search_titles`'s doc comment
/// for the full rationale; it applies here identically.
///
/// One intentional deviation from Node — divergence #17 (Phase 2c task
/// 8b): this now adds `AND user_id = $2`, scoping the leak Node's
/// unscoped original had. See `chunk::search_titles`'s doc comment.
pub async fn search_titles(
    pool: &PgPool,
    user_id: &str,
    prefix: &str,
    limit: i64,
) -> AppResult<Vec<RequirementTitleMatch>> {
    let pattern = format!("%{prefix}%");
    let rows = sqlx::query_as!(
        RequirementTitleMatch,
        r#"SELECT id, title FROM requirement WHERE title ILIKE $1 AND user_id = $2 LIMIT $3"#,
        pattern,
        user_id,
        limit
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// One row of `GET /api/chunks/{id}`'s `requirements` array, matching
/// Node's `getRequirementsForChunks` projection
/// (`packages/db/src/repository/requirement.ts:249-264`): the join's
/// `chunkId` plus a five-field slice of the requirement.
///
/// `chunk_id` is carried even though the detail path queries a single chunk
/// — Node's function is plural and its caller re-filters on `chunkId`
/// afterwards, so the column is part of the published shape.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunkRequirement {
    pub chunk_id: String,
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: Option<String>,
    #[schema(value_type = Vec<RequirementStep>)]
    pub steps: Json<Vec<RequirementStep>>,
}

/// Requirements linked to any of `chunk_ids`, one row per
/// `(chunk, requirement)` link.
///
/// Scoped through the *chunk's* owner, not the requirement's — the link
/// table has no `user_id`, and the caller's claim here is on the chunk it
/// asked about. A requirement another user linked to your chunk is
/// therefore still returned; that mirrors Node, which scopes neither side.
///
/// **No `ORDER BY`**, matching Node exactly. Returns `Ok(vec![])` for an
/// empty input without touching the database — Node's `inArray(col, [])`
/// would build `WHERE false` and return the same thing, but only after a
/// round trip.
pub async fn requirements_for_chunks(
    pool: &PgPool,
    chunk_ids: &[String],
    user_id: &str,
) -> AppResult<Vec<ChunkRequirement>> {
    if chunk_ids.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query_as!(
        ChunkRequirement,
        r#"SELECT rc.chunk_id, r.id, r.title, r.status, r.priority,
                  r.steps AS "steps: Json<Vec<RequirementStep>>"
           FROM requirement_chunk rc
           JOIN requirement r ON r.id = rc.requirement_id
           WHERE rc.chunk_id = ANY($1)
             AND EXISTS (
               SELECT 1 FROM chunk c
               WHERE c.id = rc.chunk_id AND c.user_id = $2
             )"#,
        chunk_ids,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
