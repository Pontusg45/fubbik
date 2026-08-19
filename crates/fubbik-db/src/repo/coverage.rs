//! `coverage` domain: the two read-only reports behind
//! `GET /api/requirements/coverage` and `GET /api/requirements/traceability`.
//!
//! Direct port of `packages/db/src/repository/coverage.ts`. Three queries,
//! no writes.
//!
//! **The space filter's parameter is called `codebase_id` all the way down**,
//! not `space_id`. `codebase` is the deprecated name for `space` in this
//! codebase, but the *wire* contract for these two endpoints still says
//! `codebaseId` (`packages/api/src/coverage/routes.ts:22,35`, and both web
//! call sites — `apps/web/src/routes/coverage.tsx:102` and
//! `apps/web/src/features/coverage/traceability-content.tsx:43`). The name is
//! kept end-to-end so the Rust identifier and the query parameter it carries
//! never drift apart. It is matched against `space.id`/`chunk_space.space_id`
//! — the deprecated alias is the *name*, not a different column.
//!
//! **None of the three queries has an `ORDER BY`, matching Node exactly.**
//! `getChunkCoverage` is a bare `GROUP BY` with no `.orderBy(...)`,
//! `getChunkCoverageMatrix` and `getTraceabilityMatrix` are bare selects
//! (`packages/db/src/repository/coverage.ts:9-99`). Row order is therefore a
//! query-plan artifact in Node and stays one here — no tiebreaker is added,
//! following the same rule as `use_case::list_requirements` and
//! `requirement::get_chunks`, the other two ports of Node queries that have
//! no ordering at all. Contrast every `list` in this crate that *does* have a
//! Node `ORDER BY`: those get `, id ASC` appended.

use fubbik_core::error::AppResult;
use sqlx::PgPool;

/// One row of the raw coverage scan: a chunk and how many requirements point
/// at it. Not a wire type — `coverage::service::get_coverage` partitions
/// these into the `covered`/`uncovered` arrays the endpoint actually
/// returns, so this carries no `Serialize`/`ToSchema`.
///
/// `requirement_count` is `count(...)`, i.e. `bigint`, so it is `i64` here.
/// Node reads the same value as a string off the pg driver and coerces it
/// with `Number(row.requirementCount)`
/// (`packages/api/src/coverage/service.ts:27`).
#[derive(Debug, Clone)]
pub struct ChunkCoverageRow {
    pub id: String,
    pub title: String,
    pub requirement_count: i64,
}

/// One (chunk, requirement) pair, published as-is inside the `matrix` field
/// of `GET /api/requirements/coverage?detail=true`.
///
/// `requirement_status` is plain `text NOT NULL DEFAULT 'untested'` at the DB
/// layer with no CHECK constraint
/// (`crates/fubbik-db/migrations/0001_init.sql:601`) — it stays a `String`
/// here rather than becoming an enum, matching
/// `fubbik_db::repo::requirement::Requirement::status`. This is a read-only
/// projection; nothing on this path writes the column, so there is nothing
/// to validate.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CoverageMatrixRow {
    pub chunk_id: String,
    pub chunk_title: String,
    pub requirement_id: String,
    pub requirement_title: String,
    pub requirement_status: String,
}

/// The requirement columns Node's `getTraceabilityMatrix` selects. The two
/// always-empty `planSteps`/`sessions` arrays Node bolts on live in the API
/// layer's `TraceabilityRow` — see its doc comment.
#[derive(Debug, Clone)]
pub struct TraceabilityRequirement {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: Option<String>,
}

/// Every non-archived chunk the caller owns, with the number of requirements
/// linked to it. A `LEFT JOIN` so zero-requirement chunks still produce a
/// row with `requirement_count = 0` — those become the `uncovered` array,
/// and dropping the `LEFT` would silently make the report claim 100%
/// coverage.
///
/// **`c.user_id = $1` is the only thing separating callers.** This report is
/// a full table scan of `chunk` by construction; without that predicate it
/// returns every user's chunk titles. It is asserted load-bearing by
/// `crates/fubbik-db/tests/coverage.rs::chunk_coverage_is_user_scoped`.
///
/// Node's space-filtered branch uses `.innerJoin(chunkSpace, ...)` plus
/// `eq(chunkSpace.spaceId, codebaseId)`
/// (`packages/db/src/repository/coverage.ts:22-24`). That is rewritten as an
/// `EXISTS` subquery so the whole function is one statement instead of two.
/// The two are exactly equivalent here: `chunk_space` is keyed
/// `PRIMARY KEY (chunk_id, space_id)`
/// (`crates/fubbik-db/migrations/0001_init.sql:1022`), so the inner join can
/// match at most one row per chunk and cannot inflate the `count(...)`.
///
/// `$2 IS NULL` covers the unfiltered branch. The service maps an empty
/// `codebaseId` to `None` before calling — see
/// `fubbik_api::coverage::service::blank_to_none`.
pub async fn get_chunk_coverage(
    pool: &PgPool,
    user_id: &str,
    codebase_id: Option<&str>,
) -> AppResult<Vec<ChunkCoverageRow>> {
    let rows = sqlx::query_as!(
        ChunkCoverageRow,
        r#"SELECT c.id, c.title, count(rc.requirement_id) AS "requirement_count!"
           FROM chunk c
           LEFT JOIN requirement_chunk rc ON rc.chunk_id = c.id
           WHERE c.user_id = $1
             AND c.archived_at IS NULL
             AND ($2::text IS NULL
                  OR EXISTS (SELECT 1 FROM chunk_space cs
                              WHERE cs.chunk_id = c.id AND cs.space_id = $2))
           GROUP BY c.id, c.title"#,
        user_id,
        codebase_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Every (chunk, requirement) link for the caller's non-archived chunks.
/// Only computed for `?detail=true` — the default path must not pay for it,
/// which is why this is a separate function rather than something
/// `get_chunk_coverage` always returns.
///
/// **Scoped by `c.user_id = $1` only** — deliberately *not* also by
/// `r.user_id`, matching Node's `where(and(eq(chunk.userId, userId),
/// isNull(chunk.archivedAt), ...))`
/// (`packages/db/src/repository/coverage.ts:58,71`), which never mentions
/// `requirement.userId`. Scoping through the chunk alone is sufficient in
/// practice because every write path that creates a `requirement_chunk` row
/// checks both sides' ownership first, but the asymmetry is Node's and is
/// reproduced rather than "fixed". Proven load-bearing by
/// `crates/fubbik-db/tests/coverage.rs::coverage_matrix_is_user_scoped`.
///
/// Same `EXISTS`-instead-of-`innerJoin` rewrite and same equivalence
/// argument as `get_chunk_coverage`.
pub async fn get_chunk_coverage_matrix(
    pool: &PgPool,
    user_id: &str,
    codebase_id: Option<&str>,
) -> AppResult<Vec<CoverageMatrixRow>> {
    let rows = sqlx::query_as!(
        CoverageMatrixRow,
        r#"SELECT rc.chunk_id, c.title AS chunk_title,
                  rc.requirement_id, r.title AS requirement_title,
                  r.status AS requirement_status
           FROM requirement_chunk rc
           JOIN chunk c ON c.id = rc.chunk_id
           JOIN requirement r ON r.id = rc.requirement_id
           WHERE c.user_id = $1
             AND c.archived_at IS NULL
             AND ($2::text IS NULL
                  OR EXISTS (SELECT 1 FROM chunk_space cs
                              WHERE cs.chunk_id = c.id AND cs.space_id = $2))"#,
        user_id,
        codebase_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// The caller's requirements, optionally narrowed to one space.
///
/// Two things differ from the coverage queries above and both are Node's:
/// the space filter is `requirement.space_id` **directly** (not a join
/// through `chunk_space`), and there is **no archived filter** — `requirement`
/// has no `archived_at` column
/// (`crates/fubbik-db/migrations/0001_init.sql:597-613`).
///
/// `user_id = $1` is the only cross-user guard; asserted load-bearing by
/// `crates/fubbik-db/tests/coverage.rs::traceability_is_user_scoped`.
pub async fn get_traceability_matrix(
    pool: &PgPool,
    user_id: &str,
    codebase_id: Option<&str>,
) -> AppResult<Vec<TraceabilityRequirement>> {
    let rows = sqlx::query_as!(
        TraceabilityRequirement,
        r#"SELECT id, title, status, priority
           FROM requirement
           WHERE user_id = $1
             AND ($2::text IS NULL OR space_id = $2)"#,
        user_id,
        codebase_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
