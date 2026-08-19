//! `requirement_dependency`: a self-referential many-to-many join
//! recording "requirement X depends on requirement Y". Direct port of
//! `packages/db/src/repository/requirement-dependency.ts`, minus the
//! Apache AGE fast path — every function here always takes the recursive-CTE
//! branch Node falls back to when AGE is unavailable
//! (`isAgeAvailable()` false). This crate's `age` module only knows how to
//! project `chunk` vertices/`connects` edges (see `age::ensure_vertex`'s
//! doc comment); generalising it to a second `requirement`/`depends_on`
//! vertex-and-edge kind is out of scope for this task and would touch
//! shared graph-sync code other domains depend on. The CTE path is not a
//! degraded approximation — it is Node's own correctness fallback, exercised
//! identically here.
//!
//! **Self-dependency is guarded at the database level**, not in
//! application code: `requirement_dependency` carries `CHECK
//! (requirement_id <> depends_on_id)` (`crates/fubbik-db/migrations/
//! 0001_init.sql`, constraint `no_self_dependency`), mirrored from Node's
//! own schema (`packages/db/src/schema/requirement-dependency.ts:18`). This
//! port adds no additional application-level self-dependency check — same
//! as Node, which relies on the same DB constraint and does not check for
//! it in `dependency-service.ts` either.
//!
//! **Cycles beyond direct self-reference ARE guarded at the application
//! level**, matching Node: `addDependency`
//! (`packages/api/src/requirements/dependency-service.ts:13-27`) calls
//! `checkCircularDependency` before inserting and rejects with a
//! `ValidationError` if the new edge would close a cycle. Ported as
//! [`check_circular`], called from
//! `fubbik_api::requirements::dependency_service::add_dependency`.

use fubbik_core::error::AppResult;
use sqlx::PgPool;

/// Inserts one dependency edge. `ON CONFLICT DO NOTHING` matches Node's
/// `.onConflictDoNothing()` — re-adding an existing edge is a silent no-op,
/// not an error.
///
/// No `user_id` parameter: `requirement_dependency` carries no `user_id`
/// column of its own (ownership derives entirely from its two parent
/// `requirement` rows), and both `requirement_id` and `depends_on_id` are
/// verified to belong to the caller by
/// `fubbik_api::requirements::dependency_service::add_dependency` *before*
/// this is called — the same two-parent-guarded-on-INSERT shape the Phase
/// 2e task brief calls for, just enforced at the service layer (via two
/// `requirement::find_by_id` ownership lookups) rather than in this
/// function's own SQL, because unlike `requirement_chunk`'s `set_chunks`
/// (a wholesale replace where an unguarded DELETE could destroy a victim's
/// data), a single `INSERT` with no matching guard simply does nothing
/// useful — there is no silent-data-loss failure mode here to guard
/// against in SQL specifically.
pub async fn add(pool: &PgPool, requirement_id: &str, depends_on_id: &str) -> AppResult<bool> {
    let res = sqlx::query!(
        "INSERT INTO requirement_dependency (requirement_id, depends_on_id) \
         VALUES ($1, $2) ON CONFLICT DO NOTHING",
        requirement_id,
        depends_on_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn remove(pool: &PgPool, requirement_id: &str, depends_on_id: &str) -> AppResult<bool> {
    let res = sqlx::query!(
        "DELETE FROM requirement_dependency WHERE requirement_id = $1 AND depends_on_id = $2",
        requirement_id,
        depends_on_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// One requirement's summary, as returned by [`get`] and [`transitive`].
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DependencySummary {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: Option<String>,
}

/// Both directions: requirements this one depends on, and requirements
/// that depend on this one. Matches Node's `getDependencies`
/// (`packages/db/src/repository/requirement-dependency.ts:39-65`) exactly,
/// including having no `ORDER BY` on either side.
pub struct Dependencies {
    pub depends_on: Vec<DependencySummary>,
    pub depended_on_by: Vec<DependencySummary>,
}

pub async fn get(pool: &PgPool, requirement_id: &str) -> AppResult<Dependencies> {
    let depends_on = sqlx::query_as!(
        DependencySummary,
        r#"SELECT r.id, r.title, r.status, r.priority
           FROM requirement_dependency rd
           JOIN requirement r ON r.id = rd.depends_on_id
           WHERE rd.requirement_id = $1"#,
        requirement_id
    )
    .fetch_all(pool)
    .await?;

    let depended_on_by = sqlx::query_as!(
        DependencySummary,
        r#"SELECT r.id, r.title, r.status, r.priority
           FROM requirement_dependency rd
           JOIN requirement r ON r.id = rd.requirement_id
           WHERE rd.depends_on_id = $1"#,
        requirement_id
    )
    .fetch_all(pool)
    .await?;

    Ok(Dependencies {
        depends_on,
        depended_on_by,
    })
}

/// One `(source, target)` dependency edge, as returned by [`transitive`].
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct DependencyEdge {
    pub source: String,
    pub target: String,
}

pub struct Transitive {
    pub ancestors: Vec<DependencySummary>,
    pub descendants: Vec<DependencySummary>,
    pub edges: Vec<DependencyEdge>,
}

async fn fetch_summaries_by_ids(
    pool: &PgPool,
    ids: &[String],
) -> AppResult<Vec<DependencySummary>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query_as!(
        DependencySummary,
        "SELECT id, title, status, priority FROM requirement WHERE id = ANY($1)",
        ids
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Recursive-CTE walk of the dependency graph, matching Node's
/// `getTransitiveDependencies` AGE-unavailable fallback exactly
/// (`packages/db/src/repository/requirement-dependency.ts:92-133`):
/// "ancestors" are everything `requirement_id` (transitively) depends on,
/// "descendants" are everything that (transitively) depends on it. `edges`
/// covers every `requirement_dependency` row whose `requirement_id` is
/// `requirement_id` itself or any ancestor/descendant — same
/// `[requirementId, ...ancestorIds, ...descendantIds]` scan Node's
/// fallback uses.
pub async fn transitive(pool: &PgPool, requirement_id: &str) -> AppResult<Transitive> {
    let ancestor_ids: Vec<String> = sqlx::query_scalar!(
        r#"WITH RECURSIVE chain AS (
               SELECT depends_on_id AS id FROM requirement_dependency WHERE requirement_id = $1
               UNION
               SELECT rd.depends_on_id FROM requirement_dependency rd
               INNER JOIN chain c ON rd.requirement_id = c.id
           )
           SELECT id AS "id!" FROM chain"#,
        requirement_id
    )
    .fetch_all(pool)
    .await?;

    let descendant_ids: Vec<String> = sqlx::query_scalar!(
        r#"WITH RECURSIVE chain AS (
               SELECT requirement_id AS id FROM requirement_dependency WHERE depends_on_id = $1
               UNION
               SELECT rd.requirement_id FROM requirement_dependency rd
               INNER JOIN chain c ON rd.depends_on_id = c.id
           )
           SELECT id AS "id!" FROM chain"#,
        requirement_id
    )
    .fetch_all(pool)
    .await?;

    let ancestors = fetch_summaries_by_ids(pool, &ancestor_ids).await?;
    let descendants = fetch_summaries_by_ids(pool, &descendant_ids).await?;

    let mut scan_ids: Vec<String> = vec![requirement_id.to_string()];
    scan_ids.extend(ancestor_ids);
    scan_ids.extend(descendant_ids);

    let edges = sqlx::query_as!(
        DependencyEdge,
        r#"SELECT requirement_id AS "source!", depends_on_id AS "target!"
           FROM requirement_dependency WHERE requirement_id = ANY($1)"#,
        &scan_ids
    )
    .fetch_all(pool)
    .await?;

    Ok(Transitive {
        ancestors,
        descendants,
        edges,
    })
}

/// `true` if adding `requirement_id -> depends_on_id` would close a cycle,
/// i.e. `depends_on_id` (transitively) already depends on `requirement_id`.
/// Matches Node's `checkCircularDependency` AGE-unavailable fallback
/// exactly (`packages/db/src/repository/requirement-dependency.ts:
/// 136-153`): walk everything `depends_on_id` depends on, and see if
/// `requirement_id` shows up.
pub async fn check_circular(
    pool: &PgPool,
    requirement_id: &str,
    depends_on_id: &str,
) -> AppResult<bool> {
    let hit = sqlx::query_scalar!(
        r#"WITH RECURSIVE chain AS (
               SELECT depends_on_id AS id FROM requirement_dependency WHERE requirement_id = $2
               UNION
               SELECT rd.depends_on_id FROM requirement_dependency rd
               INNER JOIN chain c ON rd.requirement_id = c.id
           )
           SELECT 1 AS "hit!" FROM chain WHERE id = $1 LIMIT 1"#,
        requirement_id,
        depends_on_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(hit.is_some())
}
