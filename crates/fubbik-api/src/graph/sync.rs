//! Projects behavior rules into the AGE graph on a schedule.
//!
//! Ports `packages/api/src/matrices/graph-sync.ts`, with two deliberate
//! divergences recorded in
//! `docs/superpowers/specs/2026-08-26-rust-phase-4a-design.md`:
//!
//! 1. Its own interval variable, `BEHAVIOR_GRAPH_SYNC_INTERVAL_HOURS`. Node
//!    gates this job behind `STALENESS_SCAN_INTERVAL_HOURS`
//!    (`packages/api/src/startup.ts:60`), which names a different job.
//! 2. Every user, not only the implicit dev user (`startup.ts:52`). On a
//!    single-user install these are identical; on any other, Node's version
//!    silently syncs nothing for everyone else.
//!
//! One SQL read replaces Node's per-matrix, per-rule, per-cell loop: the join
//! it walks by hand is a join.

use fubbik_core::error::AppResult;
use fubbik_db::age::{self, BehaviorRuleVertex};
use sqlx::PgPool;

/// `None` means "disabled". Mirrors `staleness::service::resolve_scan_interval`
/// exactly, including its tolerance of unparseable input (fall back to the
/// default rather than crash a server at boot over a typo in an env var).
pub fn resolve_sync_interval(hours: Option<&str>) -> Option<std::time::Duration> {
    let hours: f64 = hours.and_then(|s| s.parse().ok()).unwrap_or(24.0);
    if hours <= 0.0 {
        None
    } else {
        Some(std::time::Duration::from_secs_f64(hours * 3600.0))
    }
}

#[derive(sqlx::FromRow)]
struct RuleRow {
    id: String,
    title: String,
    layer: String,
    matrix_id: String,
    category: Option<String>,
}

#[derive(sqlx::FromRow)]
struct CodeLinkRow {
    rule_id: String,
    kind: String,
    code_ref: String,
}

/// Runs one full sweep. Returns the number of rules projected.
///
/// Degrades to `Ok(0)` when AGE is unavailable rather than erroring — a
/// server without the extension must still boot and serve.
pub async fn sync_once(pool: &PgPool) -> AppResult<u64> {
    if !age::is_available(pool).await {
        return Ok(0);
    }

    let rules = sqlx::query_as!(
        RuleRow,
        r#"SELECT r.id, r.title, m.layer, r.matrix_id, r.category
           FROM behavior_rule r
           JOIN behavior_matrix m ON m.id = r.matrix_id
           ORDER BY r.id ASC"#
    )
    .fetch_all(pool)
    .await?;

    let links = sqlx::query_as!(
        CodeLinkRow,
        r#"SELECT c.rule_id, bcc.kind, bcc.ref AS code_ref
           FROM behavior_cell_code bcc
           JOIN behavior_cell c ON c.id = bcc.cell_id
           WHERE bcc.kind IN ('file', 'symbol')
           ORDER BY c.rule_id ASC, bcc.id ASC"#
    )
    .fetch_all(pool)
    .await?;

    let mut synced = 0u64;
    for rule in &rules {
        match sync_rule(pool, rule, &links).await {
            Ok(()) => synced += 1,
            Err(e) => {
                // Tolerant by design, matching Node's `.pipe(Effect.catchAll(...))`
                // on every one of these calls (`graph-sync.ts:48,51,62,68`): a
                // failure projecting one rule (e.g. a title containing `$$`,
                // which breaks the dollar-quoted Cypher block — see
                // `age::cypher_in_graph`'s safety note) must not abort the
                // sweep for every rule ordered after it.
                tracing::warn!(
                    rule_id = %rule.id,
                    error = %e,
                    "Behavior graph sync: failed to project rule, skipping"
                );
            }
        }
    }

    tracing::info!(rules = synced, "Behavior rules synced to graph");
    Ok(synced)
}

/// Projects a single rule's vertex and its `governs` edges. Split out of
/// [`sync_once`] so the whole per-rule unit of work can be treated as one
/// fallible step that the sweep can skip and continue past.
async fn sync_rule(pool: &PgPool, rule: &RuleRow, links: &[CodeLinkRow]) -> AppResult<()> {
    age::upsert_behavior_rule(
        pool,
        &BehaviorRuleVertex {
            id: rule.id.clone(),
            title: rule.title.clone(),
            layer: rule.layer.clone(),
            matrix_id: rule.matrix_id.clone(),
            // Node writes '' for a missing category (`graph-sync.ts:48`).
            category: rule.category.clone().unwrap_or_default(),
        },
    )
    .await?;

    // Delete before relinking: MERGE alone would leave edges behind for
    // cell-code links that have since been deleted.
    age::delete_governs_edges(pool, &rule.id).await?;
    for link in links.iter().filter(|l| l.rule_id == rule.id) {
        age::link_governs(pool, &rule.id, &link.kind, &link.code_ref).await?;
    }

    Ok(())
}

/// Spawns the recurring sweep. Called once, from `Commands::Serve`.
pub fn spawn_behavior_sync(pool: PgPool) {
    let Some(interval) = resolve_sync_interval(
        std::env::var("BEHAVIOR_GRAPH_SYNC_INTERVAL_HOURS")
            .ok()
            .as_deref(),
    ) else {
        tracing::info!("Behavior graph sync disabled (BEHAVIOR_GRAPH_SYNC_INTERVAL_HOURS<=0)");
        return;
    };
    tracing::info!(
        interval_secs = interval.as_secs(),
        "Behavior graph sync enabled"
    );

    tokio::spawn(async move {
        // 40s, matching Node's offset from the staleness scan
        // (`packages/api/src/startup.ts:78`) — the two jobs both touch AGE and
        // staggering them keeps a cold start from contending.
        tokio::time::sleep(std::time::Duration::from_secs(40)).await;
        run_once_logged(&pool).await;
        loop {
            tokio::time::sleep(interval).await;
            run_once_logged(&pool).await;
        }
    });
}

async fn run_once_logged(pool: &PgPool) {
    match sync_once(pool).await {
        Ok(n) => tracing::info!(rules = n, "Behavior graph sync completed"),
        Err(e) => tracing::error!(error = %e, "Behavior graph sync failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_sync_interval;

    #[test]
    fn defaults_to_24_hours_when_unset_or_garbage() {
        assert_eq!(resolve_sync_interval(None).unwrap().as_secs(), 86_400);
        assert_eq!(
            resolve_sync_interval(Some("banana")).unwrap().as_secs(),
            86_400
        );
    }

    #[test]
    fn zero_and_negative_disable_the_job() {
        assert!(resolve_sync_interval(Some("0")).is_none());
        assert!(resolve_sync_interval(Some("-1")).is_none());
    }

    #[test]
    fn fractional_hours_are_honoured() {
        assert_eq!(resolve_sync_interval(Some("0.5")).unwrap().as_secs(), 1_800);
    }
}
