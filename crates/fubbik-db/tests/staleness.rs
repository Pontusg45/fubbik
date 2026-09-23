//! There is no `staleness::create` — every test seeds `chunk_staleness`
//! rows with a raw `INSERT`, same approach `notification.rs`/`activity.rs`
//! use for tables with no create route.

use fubbik_db::age;
use fubbik_db::repo::staleness::{self, ListParams};
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

async fn seed_flag(pool: &sqlx::PgPool, chunk_id: &str, reason: &str) -> String {
    let id = fubbik_db::new_id();
    sqlx::query!(
        "INSERT INTO chunk_staleness (id, chunk_id, reason) VALUES ($1, $2, $3)",
        id,
        chunk_id,
        reason
    )
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn seed_duplicate_flag(
    pool: &sqlx::PgPool,
    chunk_id: &str,
    related_chunk_id: &str,
) -> String {
    let id = fubbik_db::new_id();
    sqlx::query!(
        "INSERT INTO chunk_staleness (id, chunk_id, reason, related_chunk_id) \
         VALUES ($1, $2, 'diverged_duplicate', $3)",
        id,
        chunk_id,
        related_chunk_id
    )
    .execute(pool)
    .await
    .unwrap();
    id
}

#[sqlx::test]
async fn list_is_user_scoped_and_excludes_dismissed_and_suppressed(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    let alice_chunk = seed_chunk(&pool, &alice).await;
    let bob_chunk = seed_chunk(&pool, &bob).await;

    let visible = seed_flag(&pool, &alice_chunk, "age").await;
    let dismissed = seed_flag(&pool, &alice_chunk, "age").await;
    sqlx::query!(
        "UPDATE chunk_staleness SET dismissed_at = now() WHERE id = $1",
        dismissed
    )
    .execute(&pool)
    .await
    .unwrap();
    let suppressed = seed_flag(&pool, &alice_chunk, "diverged_duplicate").await;
    sqlx::query!(
        "UPDATE chunk_staleness SET suppress_pair = 'x:y' WHERE id = $1",
        suppressed
    )
    .execute(&pool)
    .await
    .unwrap();
    seed_flag(&pool, &bob_chunk, "age").await;

    // When
    let flags = staleness::list(&pool, &alice, ListParams::default())
        .await
        .unwrap();
    // Then
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].id, visible);
}

#[sqlx::test]
async fn list_filters_by_reason(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let c = seed_chunk(&pool, &alice).await;
    seed_flag(&pool, &c, "age").await;
    seed_flag(&pool, &c, "upstream_impact").await;

    // When
    let flags = staleness::list(
        &pool,
        &alice,
        ListParams {
            reason: Some("age".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    // Then
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].reason, "age");
}

/// Same bug class as `chunk::list`/`notification::list`/`activity::list`:
/// `ORDER BY detected_at DESC` alone over tied rows is a query-plan
/// artifact. Every row here is forced to share the exact same
/// `detected_at`, so only `id ASC` can determine order.
#[sqlx::test]
async fn list_breaks_detected_at_ties_by_id(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let c = seed_chunk(&pool, &alice).await;

    for _ in 0..20 {
        seed_flag(&pool, &c, "age").await;
    }

    sqlx::query!(
        "UPDATE chunk_staleness SET detected_at = now() WHERE chunk_id = $1",
        c
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query!("ANALYZE chunk_staleness")
        .execute(&pool)
        .await
        .unwrap();

    // When
    let expected_id_order: Vec<String> = sqlx::query_scalar!(
        "SELECT id FROM chunk_staleness WHERE chunk_id = $1 ORDER BY id ASC",
        c
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    // Then
    assert_eq!(expected_id_order.len(), 20);

    let params = ListParams {
        limit: Some(50),
        ..Default::default()
    };
    let first = staleness::list(&pool, &alice, params).await.unwrap();
    let second = staleness::list(
        &pool,
        &alice,
        ListParams {
            limit: Some(50),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let first_ids: Vec<String> = first.iter().map(|f| f.id.clone()).collect();
    let second_ids: Vec<String> = second.iter().map(|f| f.id.clone()).collect();
    assert_eq!(
        first_ids, second_ids,
        "repeated calls over tied rows must return byte-identical order"
    );
    assert_eq!(
        first_ids, expected_id_order,
        "ties must be broken by ascending id, not left to query-plan chance"
    );
}

#[sqlx::test]
async fn count_matches_list_len_and_is_user_scoped(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    let alice_chunk = seed_chunk(&pool, &alice).await;
    let bob_chunk = seed_chunk(&pool, &bob).await;
    seed_flag(&pool, &alice_chunk, "age").await;
    seed_flag(&pool, &alice_chunk, "age").await;
    seed_flag(&pool, &bob_chunk, "age").await;

    // When the operation is evaluated by the assertion.
    // Then
    assert_eq!(staleness::count(&pool, &alice, None).await.unwrap(), 2);
    assert_eq!(staleness::count(&pool, &bob, None).await.unwrap(), 1);
}

#[sqlx::test]
async fn cannot_dismiss_a_flag_on_another_users_chunk(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    let chunk_id = seed_chunk(&pool, &alice).await;
    let flag = seed_flag(&pool, &chunk_id, "age").await;

    // When
    let res = staleness::dismiss(&pool, &bob, &flag).await.unwrap();
    // Then
    assert!(
        res.is_none(),
        "divergence #14: Node lets any user dismiss any flag by id"
    );

    let flags = staleness::list(&pool, &alice, ListParams::default())
        .await
        .unwrap();
    assert_eq!(
        flags.len(),
        1,
        "alice's flag must still be undismissed after bob's attempt"
    );
}

#[sqlx::test]
async fn dismiss_by_owner_succeeds_and_returns_the_raw_update_shape(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let chunk_id = seed_chunk(&pool, &alice).await;
    let flag = seed_flag(&pool, &chunk_id, "age").await;

    // When
    let res = staleness::dismiss(&pool, &alice, &flag)
        .await
        .unwrap()
        .expect("owner must be able to dismiss their own flag");
    // Then
    assert_eq!(res.command, "UPDATE");
    assert_eq!(res.row_count, 1);
    assert_eq!(res.oid, None);
    assert!(res.rows.is_empty());
    assert!(res.fields.is_empty());

    let flags = staleness::list(&pool, &alice, ListParams::default())
        .await
        .unwrap();
    assert!(flags.is_empty(), "dismissed flag must no longer be listed");
}

#[sqlx::test]
async fn dismiss_nonexistent_flag_is_none(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    // When
    let res = staleness::dismiss(&pool, &alice, "does-not-exist")
        .await
        .unwrap();
    // Then
    assert!(res.is_none());
}

#[sqlx::test]
async fn suppress_hides_the_pair_while_dismiss_hides_one_flag(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let a = seed_chunk(&pool, &alice).await;
    let b = seed_chunk(&pool, &alice).await;
    seed_duplicate_flag(&pool, &a, &b).await;
    seed_duplicate_flag(&pool, &b, &a).await;

    staleness::suppress_duplicate(&pool, &alice, &a, &b)
        .await
        .unwrap()
        .expect("owner of both chunks must be able to suppress");
    // When
    let flags = staleness::list(&pool, &alice, ListParams::default())
        .await
        .unwrap();
    // Then
    assert!(
        flags.is_empty(),
        "suppress must hide BOTH directions of the pair, not just one row"
    );
}

/// The pair key is sorted, so suppressing (a, b) must also hide a row
/// created the other way round (b, a) — proven independently of the
/// "both directions" test above, which seeds both directions at once and
/// so can't distinguish "sorted key" from "matched by accident".
#[sqlx::test]
async fn suppress_pair_key_is_order_independent(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let a = seed_chunk(&pool, &alice).await;
    let b = seed_chunk(&pool, &alice).await;
    seed_duplicate_flag(&pool, &b, &a).await;

    staleness::suppress_duplicate(&pool, &alice, &a, &b)
        .await
        .unwrap()
        .unwrap();
    // When
    let flags = staleness::list(&pool, &alice, ListParams::default())
        .await
        .unwrap();
    // Then
    assert!(flags.is_empty());
}

/// This port's own addition (Node's `suppressDuplicatePair` takes no
/// `userId` at all — see the doc comment on
/// `staleness::suppress_duplicate`): a caller must own *both* chunks in
/// the pair, proven by removal-style test — Bob doesn't own either chunk.
#[sqlx::test]
async fn suppress_duplicate_requires_ownership_of_both_chunks(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    let a = seed_chunk(&pool, &alice).await;
    let b = seed_chunk(&pool, &alice).await;
    seed_duplicate_flag(&pool, &a, &b).await;
    seed_duplicate_flag(&pool, &b, &a).await;

    // When
    let res = staleness::suppress_duplicate(&pool, &bob, &a, &b)
        .await
        .unwrap();
    // Then
    assert!(
        res.is_none(),
        "bob owns neither chunk in the pair and must not be able to suppress it"
    );

    let flags = staleness::list(&pool, &alice, ListParams::default())
        .await
        .unwrap();
    assert_eq!(
        flags.len(),
        2,
        "alice's duplicate flags must be untouched by bob's attempt"
    );
}

/// One chunk owned by the caller, the other owned by someone else: still
/// must be rejected — proves the guard checks *both* ids, not just one.
#[sqlx::test]
async fn suppress_duplicate_requires_ownership_of_the_related_chunk_too(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    let alice_chunk = seed_chunk(&pool, &alice).await;
    let bob_chunk = seed_chunk(&pool, &bob).await;
    seed_duplicate_flag(&pool, &alice_chunk, &bob_chunk).await;

    // When
    let res = staleness::suppress_duplicate(&pool, &alice, &alice_chunk, &bob_chunk)
        .await
        .unwrap();
    // Then
    assert!(
        res.is_none(),
        "alice does not own bob's chunk, the pair must not be suppressible by her"
    );
}

#[sqlx::test]
async fn scan_age_is_idempotent(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let c = seed_chunk(&pool, &alice).await;
    sqlx::query("UPDATE chunk SET updated_at = now() - interval '200 days' WHERE id = $1")
        .bind(&c)
        .execute(&pool)
        .await
        .unwrap();

    let first = staleness::detect_age_stale_chunks(&pool, &alice, None, 90)
        .await
        .unwrap();
    // When
    let second = staleness::detect_age_stale_chunks(&pool, &alice, None, 90)
        .await
        .unwrap();
    // Then
    assert_eq!(first, 1);
    assert_eq!(
        second, 0,
        "re-running must not create a duplicate flag — the pre-filter is the guarantee"
    );

    let flags = staleness::list(&pool, &alice, ListParams::default())
        .await
        .unwrap();
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].reason, "age");
    assert!(
        flags[0]
            .detail
            .as_deref()
            .unwrap()
            .starts_with("Last updated ")
    );
}

#[sqlx::test]
async fn detect_age_stale_chunks_ignores_chunks_within_threshold(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    seed_chunk(&pool, &alice).await; // freshly created, updated_at ~ now()

    // When
    let flagged = staleness::detect_age_stale_chunks(&pool, &alice, None, 90)
        .await
        .unwrap();
    // Then
    assert_eq!(flagged, 0);
}

/// `requirement_chunk` is always empty in this workspace (no write API
/// yet), so `NOT IN (SELECT ... FROM requirement_chunk)` is vacuously true
/// — every eligible chunk gets flagged uncovered. Documented as expected,
/// not a defect, matching Node's identical behaviour over the same table.
#[sqlx::test]
async fn detect_uncovered_chunks_flags_every_eligible_chunk(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let c = seed_chunk(&pool, &alice).await;
    sqlx::query("UPDATE chunk SET updated_at = now() - interval '60 days' WHERE id = $1")
        .bind(&c)
        .execute(&pool)
        .await
        .unwrap();

    // When
    let flagged = staleness::detect_uncovered_chunks(&pool, &alice, None, 30)
        .await
        .unwrap();
    // Then
    assert_eq!(flagged, 1);

    let flags = staleness::list(
        &pool,
        &alice,
        ListParams {
            reason: Some("requirement_uncovered".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(flags.len(), 1);
}

/// `detect_uncovered_chunks` has its own idempotency pre-filter, separate
/// from `detect_age_stale_chunks`'s — proven independently rather than
/// assumed from the age-detector test above, since they filter on
/// different `reason` values.
#[sqlx::test]
async fn detect_uncovered_chunks_is_idempotent(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let c = seed_chunk(&pool, &alice).await;
    sqlx::query("UPDATE chunk SET updated_at = now() - interval '60 days' WHERE id = $1")
        .bind(&c)
        .execute(&pool)
        .await
        .unwrap();

    let first = staleness::detect_uncovered_chunks(&pool, &alice, None, 30)
        .await
        .unwrap();
    // When
    let second = staleness::detect_uncovered_chunks(&pool, &alice, None, 30)
        .await
        .unwrap();
    // Then
    assert_eq!(first, 1);
    assert_eq!(second, 0);
}

// ── flag_impact_ripple ───────────────────────────────────────────────────
//
// `crates/fubbik-api/tests/staleness.rs` already covers `flag_impact_ripple`
// at the HTTP level, but an API-level test cannot detect a removed SQL
// guard when the service's pre-check 404s first — the repo function itself
// must be exercised directly. These three tests mirror the HTTP-level
// scenarios (`scan_impact_ripple_targets_are_scoped_to_the_caller`,
// `scan_impact_rerun_does_not_accumulate_flags_for_cross_user_targets`,
// `scan_impact_on_another_users_chunk_is_404`) but call
// `staleness::flag_impact_ripple` directly, one layer below the route.

/// Divergence #19's own repo-level proof: Alice's chunk is edge-connected
/// to Bob's chunk. Alice's ripple must flag her own downstream chunk but
/// must NOT write a flag onto Bob's, even though the graph traversal
/// (`age::compute_impact_ripple`) walks through it to get there.
#[sqlx::test]
async fn flag_impact_ripple_does_not_flag_a_cross_user_ripple_target(pool: sqlx::PgPool) {
    // Given the inline inputs and test fixtures.
    // When the operation is evaluated by the assertion.
    // Then
    // Deliberately NOT guarded by `age::is_available`. This test IS the proof of
    // divergence #19, so a silent early return would let the guarantee lapse
    // wherever AGE happened to be missing — a security property must not rest on
    // a test that can quietly no-op. AGE ships in this project's test container;
    // if it is absent the test should fail loudly and be fixed.
    assert!(
        age::is_available(&pool).await,
        "AGE must be installed for the divergence #19 tests to mean anything"
    );
    let alice = seed_user(&pool, "alice-ripple@b.test").await;
    let bob = seed_user(&pool, "bob-ripple@b.test").await;
    let source = seed_chunk(&pool, &alice).await;
    let alices_downstream = seed_chunk(&pool, &alice).await;
    let bobs_chunk = seed_chunk(&pool, &bob).await;

    age::ensure_vertex(&pool, &source).await.unwrap();
    age::ensure_vertex(&pool, &alices_downstream).await.unwrap();
    age::ensure_vertex(&pool, &bobs_chunk).await.unwrap();
    age::create_edge(&pool, "depends_on", &source, &alices_downstream)
        .await
        .unwrap();
    age::create_edge(&pool, "depends_on", &source, &bobs_chunk)
        .await
        .unwrap();

    let flagged = staleness::flag_impact_ripple(&pool, &alice, &source, "Source")
        .await
        .unwrap();
    assert_eq!(
        flagged,
        Some(1),
        "only Alice's own ripple target may be flagged, not Bob's"
    );

    let flags = staleness::list(&pool, &alice, ListParams::default())
        .await
        .unwrap();
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].chunk_id, alices_downstream);

    let bob_flag_count: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) AS \"count!\" FROM chunk_staleness WHERE chunk_id = $1",
        bobs_chunk
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        bob_flag_count, 0,
        "Bob's chunk must receive no flag, even though the graph traversal reaches it"
    );
}

/// The duplicate-accumulation half of divergence #19: re-running must not
/// grow the flag count for any target, including a cross-user one that's
/// never written (and so is invisible to the `already_flagged` pre-filter).
#[sqlx::test]
async fn flag_impact_ripple_rerun_does_not_accumulate_duplicate_flags(pool: sqlx::PgPool) {
    // Given the inline inputs and test fixtures.
    // When the operation is evaluated by the assertion.
    // Then
    // Deliberately NOT guarded by `age::is_available` — see the sibling test above.
    assert!(
        age::is_available(&pool).await,
        "AGE must be installed for the divergence #19 tests to mean anything"
    );
    let alice = seed_user(&pool, "alice-ripple-rerun@b.test").await;
    let bob = seed_user(&pool, "bob-ripple-rerun@b.test").await;
    let source = seed_chunk(&pool, &alice).await;
    let alices_downstream = seed_chunk(&pool, &alice).await;
    let bobs_chunk = seed_chunk(&pool, &bob).await;

    age::ensure_vertex(&pool, &source).await.unwrap();
    age::ensure_vertex(&pool, &alices_downstream).await.unwrap();
    age::ensure_vertex(&pool, &bobs_chunk).await.unwrap();
    age::create_edge(&pool, "depends_on", &source, &alices_downstream)
        .await
        .unwrap();
    age::create_edge(&pool, "depends_on", &source, &bobs_chunk)
        .await
        .unwrap();

    for i in 0..3 {
        let flagged = staleness::flag_impact_ripple(&pool, &alice, &source, "Source")
            .await
            .unwrap();
        assert_eq!(
            flagged,
            Some(if i == 0 { 1 } else { 0 }),
            "only the first run may create a new flag; re-runs must not accumulate duplicates"
        );
    }

    let flags = staleness::list(&pool, &alice, ListParams::default())
        .await
        .unwrap();
    assert_eq!(
        flags.len(),
        1,
        "repeated runs must not accumulate duplicate flags for any target"
    );

    let bob_flag_count: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) AS \"count!\" FROM chunk_staleness WHERE chunk_id = $1",
        bobs_chunk
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        bob_flag_count, 0,
        "repeated runs must not accumulate flags on a cross-user target either"
    );
}

/// The primary `chunk_id` ownership guard: Bob calling `flag_impact_ripple`
/// against Alice's chunk must affect nothing — not just return `None`, but
/// leave no flags written anywhere.
#[sqlx::test]
async fn flag_impact_ripple_requires_ownership_of_the_source_chunk(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice-ripple-owner@b.test").await;
    let bob = seed_user(&pool, "bob-ripple-owner@b.test").await;
    let alices_chunk = seed_chunk(&pool, &alice).await;

    // When
    let result = staleness::flag_impact_ripple(&pool, &bob, &alices_chunk, "Alice's chunk")
        .await
        .unwrap();
    // Then
    assert!(
        result.is_none(),
        "bob does not own the source chunk, the ripple must not run for him"
    );

    let flags = staleness::list(&pool, &alice, ListParams::default())
        .await
        .unwrap();
    assert!(
        flags.is_empty(),
        "no flags may be written when the caller doesn't own the source chunk"
    );
}

// ---------------------------------------------------------------------
// flag_requirement_failing
// ---------------------------------------------------------------------

#[sqlx::test]
async fn flag_requirement_failing_flags_every_linked_chunk_once(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice-req-failing@b.test").await;
    let c1 = seed_chunk(&pool, &alice).await;
    let c2 = seed_chunk(&pool, &alice).await;

    // When
    let flagged = staleness::flag_requirement_failing(
        &pool,
        &alice,
        "req-1",
        "Login flow",
        &[c1.clone(), c2.clone()],
    )
    .await
    .unwrap();
    // Then
    assert_eq!(flagged, 2);

    let flags = staleness::list(&pool, &alice, ListParams::default())
        .await
        .unwrap();
    assert_eq!(
        flags
            .iter()
            .filter(|f| f.reason == "requirement_failing")
            .count(),
        2
    );

    // Idempotent: calling again with the same requirement/title/chunks must
    // not double-flag (the `alreadyFlagged` pre-filter, matching Node).
    let flagged_again =
        staleness::flag_requirement_failing(&pool, &alice, "req-1", "Login flow", &[c1, c2])
            .await
            .unwrap();
    assert_eq!(
        flagged_again, 0,
        "must not re-flag chunks that already carry an undismissed flag for this exact requirement"
    );
}

/// Empty `chunk_ids` short-circuits without writing anything — matches
/// Node's early return before the whole function body runs.
#[sqlx::test]
async fn flag_requirement_failing_with_no_chunks_is_a_no_op(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice-req-failing-empty@b.test").await;
    // When
    let flagged = staleness::flag_requirement_failing(&pool, &alice, "req-1", "Empty", &[])
        .await
        .unwrap();
    // Then
    assert_eq!(flagged, 0);
}

/// The added `user_id` ownership guard on the INSERT: a chunk id Alice
/// doesn't own must not be flaggable through her call.
#[sqlx::test]
async fn flag_requirement_failing_requires_ownership_of_the_target_chunk(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice-req-failing-guard@b.test").await;
    let bob = seed_user(&pool, "bob-req-failing-guard@b.test").await;
    let bobs_chunk = seed_chunk(&pool, &bob).await;

    // When
    let flagged = staleness::flag_requirement_failing(
        &pool,
        &alice,
        "req-1",
        "Cross-user",
        std::slice::from_ref(&bobs_chunk),
    )
    .await
    .unwrap();
    // Then
    assert_eq!(
        flagged, 0,
        "Alice must not be able to flag a chunk she doesn't own"
    );

    let bobs_flags = staleness::list(&pool, &bob, ListParams::default())
        .await
        .unwrap();
    assert!(
        bobs_flags.is_empty(),
        "the rejected flag must not have been written for Bob either"
    );
}
