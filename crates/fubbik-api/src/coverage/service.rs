use fubbik_core::error::AppResult;
use fubbik_db::repo::coverage;
use sqlx::PgPool;

use super::dto::{CoverageResponse, CoverageStats, CoveredChunk, TraceabilityRow, UncoveredChunk};

/// Node reaches the space filter through `if (codebaseId)`, and `""` is
/// falsy in JS — so `?codebaseId=` means *unfiltered* there
/// (`packages/db/src/repository/coverage.ts:14,45,80`). Rust's
/// `Option<String>` deserialises the same query string to `Some("")`, which
/// would filter on `space_id = ''` and return nothing. Collapsing blank to
/// `None` reproduces JS truthiness; without it the two backends disagree on
/// a request the wire format permits.
fn blank_to_none(value: Option<&str>) -> Option<&str> {
    value.filter(|v| !v.is_empty())
}

/// `ctx.query.detail === "true"` (`packages/api/src/coverage/routes.ts:14`)
/// — a literal string comparison, deliberately not a boolean parse. See
/// `dto::CoverageQuery::detail`.
pub fn wants_detail(detail: Option<&str>) -> bool {
    detail == Some("true")
}

/// Port of Node's `getCoverage` (`packages/api/src/coverage/service.ts:20-47`):
/// fetch every chunk with its requirement count, then partition on
/// `count > 0`.
///
/// The partition preserves the query's row order within each bucket, exactly
/// as Node's `for (const row of rows)` loop does. That order is not
/// specified — see `fubbik_db::repo::coverage`'s module doc on the missing
/// `ORDER BY`.
///
/// `percentage` reproduces `total > 0 ? Math.round((covered / total) * 100) : 0`
/// including the zero-total guard. `f64::round` and JS `Math.round` differ
/// only on negative halves (`-2.5` rounds to `-3` vs `-2`), and both inputs
/// here are non-negative counts, so they agree on every reachable value.
///
/// The `total > 0` guard is **not** load-bearing in Rust and is kept for
/// intent, not for correctness — verified by deleting it and watching
/// `percentage_is_zero_when_there_are_no_chunks` stay green. `0.0 / 0.0` is
/// `NaN` here as it is in JS, but Rust's float-to-int cast *saturates*, and
/// `NaN as i64` is defined to be `0` — the same answer the guard produces.
/// It matters in Node, where the guard is the only thing stopping
/// `Math.round(NaN)` from reaching `JSON.stringify` and serialising as
/// `null`. It would start mattering here the moment `percentage` became an
/// `f64` on the wire, since `serde_json` cannot serialise `NaN`. Left in
/// place so that change is safe and so the code still reads as a mirror of
/// Node's.
pub async fn get_coverage(
    pool: &PgPool,
    user_id: &str,
    codebase_id: Option<&str>,
) -> AppResult<CoverageResponse> {
    let rows = coverage::get_chunk_coverage(pool, user_id, blank_to_none(codebase_id)).await?;

    let mut covered: Vec<CoveredChunk> = Vec::new();
    let mut uncovered: Vec<UncoveredChunk> = Vec::new();
    for row in rows {
        if row.requirement_count > 0 {
            covered.push(CoveredChunk {
                id: row.id,
                title: row.title,
                requirement_count: row.requirement_count,
            });
        } else {
            uncovered.push(UncoveredChunk {
                id: row.id,
                title: row.title,
            });
        }
    }

    let covered_count = covered.len() as i64;
    let uncovered_count = uncovered.len() as i64;
    let total = covered_count + uncovered_count;
    let percentage = if total > 0 {
        ((covered_count as f64 / total as f64) * 100.0).round() as i64
    } else {
        0
    };

    Ok(CoverageResponse {
        covered,
        uncovered,
        stats: CoverageStats {
            total,
            covered: covered_count,
            uncovered: uncovered_count,
            percentage,
        },
        matrix: None,
    })
}

/// Port of Node's `getCoverageMatrix` (`packages/api/src/coverage/service.ts:4-14`):
/// the plain coverage object plus a `matrix` field. Reached only when
/// `?detail=true`, so the extra `requirement_chunk` query never runs on the
/// default path.
pub async fn get_coverage_matrix(
    pool: &PgPool,
    user_id: &str,
    codebase_id: Option<&str>,
) -> AppResult<CoverageResponse> {
    let mut response = get_coverage(pool, user_id, codebase_id).await?;
    response.matrix =
        Some(coverage::get_chunk_coverage_matrix(pool, user_id, blank_to_none(codebase_id)).await?);
    Ok(response)
}

/// Port of Node's `getTraceability` (`packages/api/src/coverage/service.ts:16-18`),
/// which is a bare pass-through to the repository. The always-empty
/// `planSteps`/`sessions` arrays are attached here rather than in the SQL
/// layer — see `dto::TraceabilityRow` for why they still exist at all.
pub async fn get_traceability(
    pool: &PgPool,
    user_id: &str,
    codebase_id: Option<&str>,
) -> AppResult<Vec<TraceabilityRow>> {
    let rows = coverage::get_traceability_matrix(pool, user_id, blank_to_none(codebase_id)).await?;
    Ok(rows
        .into_iter()
        .map(|r| TraceabilityRow {
            id: r.id,
            title: r.title,
            status: r.status,
            priority: r.priority,
            plan_steps: Vec::new(),
            sessions: Vec::new(),
        })
        .collect())
}
