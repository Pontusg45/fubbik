//! Integration tests for the query helpers Task 9 makes live:
//! `get_neighborhood`/`get_neighborhood_in_graph`,
//! `find_shortest_path_with_details`, `get_chunks_affected_by_requirement`,
//! and `compute_impact_ripple`. `tests/age.rs` already covers
//! `cypher`/`parse_agtype` themselves; this file is scoped to the five new
//! functions listed in the task-9 brief.
//!
//! Every test skips (rather than failing) when AGE isn't available in the
//! target database — the same pattern `tests/age.rs` already uses — since
//! these tests exercise a real Cypher round trip, not just the degradation
//! path.

use fubbik_db::age;
use fubbik_db::repo::{chunk, user};

async fn seed_user(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "U", None).await.unwrap().id
}

async fn seed_chunk(pool: &sqlx::PgPool, user_id: &str) -> String {
    chunk::create(
        pool,
        user_id,
        chunk::NewChunk {
            title: "A chunk".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .id
}

/// `MERGE (:requirement {id: '...'})` — the one vertex label these tests
/// need besides `chunk`, seeded directly via `age::cypher` since
/// `age::ensure_vertex` is hardcoded to the `chunk` label.
async fn seed_requirement_vertex(pool: &sqlx::PgPool, id: &str) {
    age::cypher(
        pool,
        &format!("MERGE (:requirement {{id: '{}'}})", age::esc_cypher(id)),
    )
    .await
    .unwrap();
}

/// `(r)-[:covers]->(c)` — Node's `createEdge("covers", "requirement", ...,
/// "chunk", ...)` (`packages/db/src/repository/requirement.ts:171`), no
/// properties.
async fn seed_covers_edge(pool: &sqlx::PgPool, requirement_id: &str, chunk_id: &str) {
    age::cypher(
        pool,
        &format!(
            "MATCH (r:requirement {{id: '{}'}}), (c:chunk {{id: '{}'}}) CREATE (r)-[:covers]->(c)",
            age::esc_cypher(requirement_id),
            age::esc_cypher(chunk_id)
        ),
    )
    .await
    .unwrap();
}

// ── get_neighborhood / get_neighborhood_in_graph ───────────────────────

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn neighborhood_returns_connected_chunk_ids(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable — skipping");
        return;
    }
    let alice = seed_user(&pool, "alice-neighborhood@b.test").await;
    let a = seed_chunk(&pool, &alice).await;
    let b = seed_chunk(&pool, &alice).await;
    age::ensure_vertex(&pool, &a).await.unwrap();
    age::ensure_vertex(&pool, &b).await.unwrap();
    age::create_edge(&pool, "connects", &a, &b).await.unwrap();

    let ids = age::get_neighborhood(&pool, &a, 1).await.unwrap();
    assert!(
        ids.contains(&b),
        "a 1-hop neighbourhood must include the directly connected chunk"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn neighborhood_at_hops_1_excludes_a_transitive_chunk_reachable_only_at_hops_2(
    pool: sqlx::PgPool,
) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable — skipping");
        return;
    }
    let alice = seed_user(&pool, "alice-hops@b.test").await;
    let a = seed_chunk(&pool, &alice).await;
    let b = seed_chunk(&pool, &alice).await;
    let c = seed_chunk(&pool, &alice).await;
    for id in [&a, &b, &c] {
        age::ensure_vertex(&pool, id).await.unwrap();
    }
    age::create_edge(&pool, "related_to", &a, &b).await.unwrap();
    age::create_edge(&pool, "related_to", &b, &c).await.unwrap();

    let one_hop = age::get_neighborhood(&pool, &a, 1).await.unwrap();
    assert!(one_hop.contains(&b));
    assert!(
        !one_hop.contains(&c),
        "c is 2 hops away from a, must not appear in a 1-hop neighbourhood"
    );

    let two_hop = age::get_neighborhood(&pool, &a, 2).await.unwrap();
    assert!(two_hop.contains(&b));
    assert!(two_hop.contains(&c), "c must appear within 2 hops");
}

/// Points at a graph name that does not exist, simulating AGE being
/// unavailable. Node wraps every graph clause in
/// `Effect.orElse(() => Effect.succeed([]))` — an unavailable graph must
/// degrade to empty results, never a 500. Unlike the other tests in this
/// file, this one runs even when AGE itself is unavailable: a missing
/// extension and a missing graph both take the same `Err` path inside
/// `get_neighborhood_in_graph`, and both must degrade the same way.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn a_graph_clause_degrades_to_empty_when_age_is_unavailable(pool: sqlx::PgPool) {
    let ids = age::get_neighborhood_in_graph(&pool, "no_such_graph", "whatever", 1).await;
    assert_eq!(
        ids.unwrap_or_default(),
        Vec::<String>::new(),
        "AGE failures must degrade to empty results, never a 500 — Node wraps each clause in Effect.orElse"
    );
}

/// Companion to the test above. That test's `.unwrap_or_default()` means it
/// cannot actually distinguish `Ok(vec![])` (the intended degrade path)
/// from `Err(..)` propagating — `Result::unwrap_or_default()` maps *both*
/// to an empty `Vec`, so it would keep passing even if
/// `get_neighborhood_in_graph` were changed to propagate the underlying
/// `sqlx` error instead of swallowing it. This test uses `.expect(..)`
/// instead, which panics on `Err` — it is the one that actually goes red
/// when the degrade-to-`Ok(vec![])` behaviour is removed (see the task
/// report for the before/after proof).
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn a_graph_clause_degrades_via_ok_empty_not_a_propagated_error(pool: sqlx::PgPool) {
    let ids = age::get_neighborhood_in_graph(&pool, "no_such_graph", "whatever", 1)
        .await
        .expect(
            "must be Ok(vec![]), not Err — Node degrades every graph clause to empty results via Effect.orElse, never a 500",
        );
    assert_eq!(ids, Vec::<String>::new());
}

// ── find_shortest_path_with_details ─────────────────────────────────────

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn shortest_path_reports_the_chunk_chain_and_edge_relations(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable — skipping");
        return;
    }
    let alice = seed_user(&pool, "alice-path@b.test").await;
    let a = seed_chunk(&pool, &alice).await;
    let b = seed_chunk(&pool, &alice).await;
    let c = seed_chunk(&pool, &alice).await;
    for id in [&a, &b, &c] {
        age::ensure_vertex(&pool, id).await.unwrap();
    }
    age::create_edge(&pool, "depends_on", &a, &b).await.unwrap();
    age::create_edge(&pool, "extends", &b, &c).await.unwrap();

    let path = age::find_shortest_path_with_details(&pool, &a, &c)
        .await
        .unwrap()
        .expect("a path must be found through b");

    assert_eq!(path.chunk_ids, vec![a.clone(), b.clone(), c.clone()]);
    assert_eq!(path.edges.len(), 2);
    assert_eq!(path.edges[0].relation, "depends_on");
    assert_eq!(path.edges[1].relation, "extends");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn shortest_path_is_none_when_no_path_exists(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable — skipping");
        return;
    }
    let alice = seed_user(&pool, "alice-nopath@b.test").await;
    let a = seed_chunk(&pool, &alice).await;
    let b = seed_chunk(&pool, &alice).await;
    age::ensure_vertex(&pool, &a).await.unwrap();
    age::ensure_vertex(&pool, &b).await.unwrap();

    let path = age::find_shortest_path_with_details(&pool, &a, &b)
        .await
        .unwrap();
    assert!(path.is_none(), "two disconnected chunks have no path");
}

/// Companion to `a_graph_clause_degrades_via_ok_empty_not_a_propagated_error`
/// above, for `find_shortest_path_with_details`'s own `Err(_) => Ok(None)`
/// degrade branch. Neither "path found" nor "genuinely unreachable" above
/// forces the AGE-error branch (both point at real chunks in the real
/// `"knowledge"` graph); this test points at a graph that doesn't exist,
/// which cannot be reached via `find_shortest_path_with_details`'s
/// unparameterized public signature — hence
/// `find_shortest_path_with_details_in_graph`. Uses `.expect()`, not
/// `.unwrap_or_default()`/`.unwrap_or(None)`: this is the one that actually
/// goes red if the degrade-to-`Ok(None)` behaviour is removed.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn shortest_path_degrades_to_none_not_a_propagated_error(pool: sqlx::PgPool) {
    let path =
        age::find_shortest_path_with_details_in_graph(&pool, "no_such_graph", "a", "b")
            .await
            .expect(
                "must be Ok(None), not Err — Node degrades every graph clause to empty results via Effect.orElse, never a 500",
            );
    assert!(path.is_none());
}

// ── get_chunks_affected_by_requirement ──────────────────────────────────

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn affected_by_requirement_reaches_connected_chunks_via_the_covered_chunk(
    pool: sqlx::PgPool,
) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable — skipping");
        return;
    }
    let alice = seed_user(&pool, "alice-affected@b.test").await;
    let covered = seed_chunk(&pool, &alice).await;
    let related = seed_chunk(&pool, &alice).await;
    let unrelated = seed_chunk(&pool, &alice).await;
    for id in [&covered, &related, &unrelated] {
        age::ensure_vertex(&pool, id).await.unwrap();
    }
    age::create_edge(&pool, "related_to", &covered, &related)
        .await
        .unwrap();

    let requirement_id = fubbik_db::new_id();
    seed_requirement_vertex(&pool, &requirement_id).await;
    seed_covers_edge(&pool, &requirement_id, &covered).await;

    let ids = age::get_chunks_affected_by_requirement(&pool, &requirement_id, 1)
        .await
        .unwrap();
    assert!(
        ids.contains(&covered),
        "the covered chunk itself (0 hops) must be included"
    );
    assert!(
        ids.contains(&related),
        "a chunk connected within 1 hop must be included"
    );
    assert!(
        !ids.contains(&unrelated),
        "an unconnected chunk must not be included"
    );
}

/// Pins the bug the split-query implementation exists to work around: in a
/// fresh graph where no `:connects` edge has *ever* been created (exactly
/// what every `#[sqlx::test]` pool starts as), AGE 1.7.0's `*0..hops`
/// variable-length pattern fails to match even the zero-hop case, so a
/// covered chunk with no connections of its own would silently vanish from
/// the result — verified directly against this workspace's `fubbik-rs-db`
/// before this port's `get_chunks_affected_by_requirement` was changed to
/// query the covered chunks unconditionally instead of folding them into
/// the `*0..hops` pattern.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn affected_by_requirement_includes_the_covered_chunk_even_with_zero_connects_edges_ever_created(
    pool: sqlx::PgPool,
) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable — skipping");
        return;
    }
    let alice = seed_user(&pool, "alice-affected-lonely@b.test").await;
    let covered = seed_chunk(&pool, &alice).await;
    age::ensure_vertex(&pool, &covered).await.unwrap();
    // Deliberately no `create_edge` call anywhere in this test — the
    // `:connects` relationship label has never been used in this graph.

    let requirement_id = fubbik_db::new_id();
    seed_requirement_vertex(&pool, &requirement_id).await;
    seed_covers_edge(&pool, &requirement_id, &covered).await;

    let ids = age::get_chunks_affected_by_requirement(&pool, &requirement_id, 2)
        .await
        .unwrap();
    assert_eq!(
        ids,
        vec![covered],
        "the covered chunk must be included even though :connects has never been used in this graph"
    );
}

/// `get_chunks_affected_by_requirement`'s own `Err(_) => Ok(vec![])`
/// degrade branch, previously untested — the one existing test above only
/// covers the real `"knowledge"` graph. Points at a nonexistent graph via
/// `get_chunks_affected_by_requirement_in_graph`, and uses `.expect()`, not
/// `.unwrap_or_default()`, so it goes red if the degrade is removed.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn affected_by_requirement_degrades_to_empty_not_a_propagated_error(pool: sqlx::PgPool) {
    let ids = age::get_chunks_affected_by_requirement_in_graph(
        &pool,
        "no_such_graph",
        "whatever",
        1,
    )
    .await
    .expect(
        "must be Ok(vec![]), not Err — Node degrades every graph clause to empty results via Effect.orElse, never a 500",
    );
    assert_eq!(ids, Vec::<String>::new());
}

// ── compute_impact_ripple ───────────────────────────────────────────────

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn impact_ripple_includes_a_strongly_connected_downstream_chunk(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable — skipping");
        return;
    }
    let alice = seed_user(&pool, "alice-impact@b.test").await;
    let source = seed_chunk(&pool, &alice).await;
    let downstream = seed_chunk(&pool, &alice).await;
    age::ensure_vertex(&pool, &source).await.unwrap();
    age::ensure_vertex(&pool, &downstream).await.unwrap();
    // 1 hop, "depends_on" (weight 1.0) => degree 0.9 * 1.0 = 0.9, well above
    // the 0.1 cutoff.
    age::create_edge(&pool, "depends_on", &source, &downstream)
        .await
        .unwrap();

    let ids = age::compute_impact_ripple(&pool, &source).await.unwrap();
    assert!(
        ids.contains(&downstream),
        "a 1-hop depends_on target must clear the degree cutoff"
    );
    assert!(
        !ids.contains(&source),
        "the source chunk must not flag itself as impacted"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn impact_ripple_is_empty_for_a_chunk_with_no_downstream_connections(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable — skipping");
        return;
    }
    let alice = seed_user(&pool, "alice-impact-lonely@b.test").await;
    let lonely = seed_chunk(&pool, &alice).await;
    age::ensure_vertex(&pool, &lonely).await.unwrap();

    let ids = age::compute_impact_ripple(&pool, &lonely).await.unwrap();
    assert_eq!(ids, Vec::<String>::new());
}

/// `compute_impact_ripple`'s own `Err(_) => Ok(vec![])` degrade branch,
/// previously untested — lower risk than the other two functions here
/// because its only caller (`staleness::flag_impact_ripple`) propagates
/// *other* errors via `?`, but the AGE-failure case itself was never
/// exercised. Points at a nonexistent graph via
/// `compute_impact_ripple_in_graph`, and uses `.expect()`, not
/// `.unwrap_or_default()`, so it goes red if the degrade is removed.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn impact_ripple_degrades_to_empty_not_a_propagated_error(pool: sqlx::PgPool) {
    let ids = age::compute_impact_ripple_in_graph(&pool, "no_such_graph", "whatever")
        .await
        .expect(
            "must be Ok(vec![]), not Err — Node degrades every graph clause to empty results via Effect.orElse, never a 500",
        );
    assert_eq!(ids, Vec::<String>::new());
}
