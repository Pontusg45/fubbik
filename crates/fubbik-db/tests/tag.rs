//! `chunk_tag` is a composite-key join with no `user_id` of its own —
//! ownership derives entirely from the two parent rows (`chunk`, `tag`).
//! That creates two independent holes; a fix for one does not fix the
//! other, so each direction gets its own test (`Step 1` of the task brief).

use fubbik_db::repo::{chunk, tag, user};

async fn seed(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "U", None).await.unwrap().id
}

async fn a_chunk(pool: &sqlx::PgPool, uid: &str, title: &str) -> String {
    chunk::create(
        pool,
        uid,
        chunk::NewChunk {
            title: title.into(),
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

#[sqlx::test]
async fn cannot_tag_another_users_chunk(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let alices_chunk = a_chunk(&pool, &alice, "Alice's").await;
    let bobs_tag = tag::create(&pool, &bob, "bobs-tag", None).await.unwrap();

    // Bob attaches his own tag to Alice's chunk — must be rejected.
    let n = tag::set_chunk_tags(
        &pool,
        &bob,
        &alices_chunk,
        std::slice::from_ref(&bobs_tag.id),
    )
    .await
    .unwrap();
    assert_eq!(n, 0, "must not tag another user's chunk");
    assert!(
        tag::tags_for_chunk(&pool, &alice, &alices_chunk)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test]
async fn cannot_attach_another_users_tag(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let alices_chunk = a_chunk(&pool, &alice, "Alice's").await;
    let bobs_tag = tag::create(&pool, &bob, "bobs-tag", None).await.unwrap();

    // Alice attaches Bob's tag to her own chunk — must be rejected.
    let n = tag::set_chunk_tags(
        &pool,
        &alice,
        &alices_chunk,
        std::slice::from_ref(&bobs_tag.id),
    )
    .await
    .unwrap();
    assert_eq!(n, 0, "must not attach another user's tag");
    assert!(
        tag::tags_for_chunk(&pool, &alice, &alices_chunk)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test]
async fn own_chunk_and_own_tag_succeeds(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let alices_chunk = a_chunk(&pool, &alice, "Alice's").await;
    let alices_tag = tag::create(&pool, &alice, "alices-tag", None)
        .await
        .unwrap();

    let n = tag::set_chunk_tags(
        &pool,
        &alice,
        &alices_chunk,
        std::slice::from_ref(&alices_tag.id),
    )
    .await
    .unwrap();
    assert_eq!(n, 1);
    let tags = tag::tags_for_chunk(&pool, &alice, &alices_chunk)
        .await
        .unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].id, alices_tag.id);
}

#[sqlx::test]
async fn set_chunk_tags_replaces_the_whole_set(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let alices_chunk = a_chunk(&pool, &alice, "Alice's").await;
    let tag_one = tag::create(&pool, &alice, "one", None).await.unwrap();
    let tag_two = tag::create(&pool, &alice, "two", None).await.unwrap();

    tag::set_chunk_tags(
        &pool,
        &alice,
        &alices_chunk,
        std::slice::from_ref(&tag_one.id),
    )
    .await
    .unwrap();
    let n = tag::set_chunk_tags(
        &pool,
        &alice,
        &alices_chunk,
        std::slice::from_ref(&tag_two.id),
    )
    .await
    .unwrap();
    assert_eq!(n, 1);

    let tags = tag::tags_for_chunk(&pool, &alice, &alices_chunk)
        .await
        .unwrap();
    assert_eq!(tags.len(), 1, "old tag must be replaced, not accumulated");
    assert_eq!(tags[0].id, tag_two.id);
}

#[sqlx::test]
async fn merge_moves_chunk_tags_from_source_to_target(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let alices_chunk = a_chunk(&pool, &alice, "Alice's").await;
    let source = tag::create(&pool, &alice, "source", None).await.unwrap();
    let target = tag::create(&pool, &alice, "target", None).await.unwrap();

    tag::set_chunk_tags(
        &pool,
        &alice,
        &alices_chunk,
        std::slice::from_ref(&source.id),
    )
    .await
    .unwrap();

    let result = tag::merge(&pool, &alice, &source.id, &target.id)
        .await
        .unwrap();
    assert_eq!(result.target_id, target.id);
    assert_eq!(result.chunk_count, 1);

    let tags = tag::tags_for_chunk(&pool, &alice, &alices_chunk)
        .await
        .unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].id, target.id);
}

/// The case `ON CONFLICT DO NOTHING` exists for: a chunk already carrying
/// BOTH tags before the merge must end with exactly one `chunk_tag` row
/// afterwards, not a primary-key violation from a naive `UPDATE`.
#[sqlx::test]
async fn merge_when_chunk_already_has_both_tags_ends_with_one_row(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let alices_chunk = a_chunk(&pool, &alice, "Alice's").await;
    let source = tag::create(&pool, &alice, "source", None).await.unwrap();
    let target = tag::create(&pool, &alice, "target", None).await.unwrap();

    tag::set_chunk_tags(
        &pool,
        &alice,
        &alices_chunk,
        &[source.id.clone(), target.id.clone()],
    )
    .await
    .unwrap();

    let result = tag::merge(&pool, &alice, &source.id, &target.id)
        .await
        .unwrap();
    assert_eq!(result.chunk_count, 1);

    let tags = tag::tags_for_chunk(&pool, &alice, &alices_chunk)
        .await
        .unwrap();
    assert_eq!(
        tags.len(),
        1,
        "the chunk must end with exactly one chunk_tag row, not two and not zero"
    );
    assert_eq!(tags[0].id, target.id);
}

#[sqlx::test]
async fn merge_deletes_the_source_tag(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let source = tag::create(&pool, &alice, "source", None).await.unwrap();
    let target = tag::create(&pool, &alice, "target", None).await.unwrap();

    tag::merge(&pool, &alice, &source.id, &target.id)
        .await
        .unwrap();

    let remaining = tag::list(&pool, &alice).await.unwrap();
    let names: Vec<&str> = remaining.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["target"]);
}

#[sqlx::test]
async fn merge_of_unknown_tag_id_is_not_found(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let target = tag::create(&pool, &alice, "target", None).await.unwrap();

    let err = tag::merge(&pool, &alice, "does-not-exist", &target.id)
        .await
        .unwrap_err();
    assert!(matches!(err, fubbik_core::error::AppError::NotFound(_)));
}

#[sqlx::test]
async fn merge_of_another_users_tag_is_not_found(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let alices_tag = tag::create(&pool, &alice, "alices-tag", None)
        .await
        .unwrap();
    let bobs_tag = tag::create(&pool, &bob, "bobs-tag", None).await.unwrap();

    // Alice tries to merge Bob's tag into her own — must be rejected, not
    // silently merged across users.
    let err = tag::merge(&pool, &alice, &bobs_tag.id, &alices_tag.id)
        .await
        .unwrap_err();
    assert!(matches!(err, fubbik_core::error::AppError::NotFound(_)));

    // Bob's tag must survive untouched.
    let bobs_tags = tag::list(&pool, &bob).await.unwrap();
    assert_eq!(bobs_tags.len(), 1);
    assert_eq!(bobs_tags[0].id, bobs_tag.id);
}

/// Same bug class as `chunk::list` (see the equivalent test in
/// `tests/chunk.rs`), forced by the same live-run finding on `/api/tags`.
/// Every tag here shares the exact same `created_at`, so `ORDER BY
/// created_at ASC` alone cannot determine order — only the `id ASC`
/// tiebreaker can. A test with distinct `created_at` values would pass
/// without the fix and prove nothing.
///
/// Uses 20 tied tags plus an explicit `ANALYZE`, not 5: `tag::list`'s query
/// GROUPs BY `t.id, tt.id` (for the live chunk-count aggregate). At small
/// row counts, or before statistics exist, Postgres' planner satisfies
/// that GROUP BY with a Sort-based GroupAggregate whose input happens to
/// already be sorted by `t.id` — so the *final* sort on the (tied)
/// `created_at` column can incidentally come out in id order even WITHOUT
/// the explicit tiebreaker, which would make this test pass for the wrong
/// reason. Confirmed empirically with `EXPLAIN`: 20 rows plus `ANALYZE`
/// reliably makes the planner switch to a HashAggregate instead, whose
/// output order has no relationship to `id` at all — that's what actually
/// exercises whether the tiebreaker is present.
#[sqlx::test]
async fn list_breaks_created_at_ties_by_id(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;

    for i in 0..20 {
        tag::create(&pool, &alice, &format!("tag-{i}"), None)
            .await
            .unwrap();
    }

    // A single UPDATE statement's `now()` is fixed for the whole statement,
    // so this produces a genuine tie across all twenty rows, not twenty
    // close-but-distinct timestamps.
    sqlx::query!(
        "UPDATE tag SET created_at = now() WHERE user_id = $1",
        alice
    )
    .execute(&pool)
    .await
    .unwrap();
    // See the doc comment above: without this, the planner's row-count
    // estimate for a freshly-populated table can be stale enough to pick
    // the Sort-based plan that masks the missing tiebreaker.
    sqlx::query!("ANALYZE tag").execute(&pool).await.unwrap();

    // Ground truth from Postgres directly, so this test does not depend on
    // Rust's default string ordering happening to agree with the
    // database's collation.
    let expected_id_order: Vec<String> = sqlx::query_scalar!(
        "SELECT id FROM tag WHERE user_id = $1 ORDER BY id ASC",
        alice
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(expected_id_order.len(), 20);

    let first = tag::list(&pool, &alice).await.unwrap();
    let second = tag::list(&pool, &alice).await.unwrap();

    let first_ids: Vec<String> = first.iter().map(|t| t.id.clone()).collect();
    let second_ids: Vec<String> = second.iter().map(|t| t.id.clone()).collect();

    assert_eq!(
        first_ids, second_ids,
        "repeated calls over tied rows must return byte-identical order"
    );
    assert_eq!(
        first_ids, expected_id_order,
        "ties must be broken by ascending id, not left to query-plan chance"
    );
}

/// Same bug class as `list_breaks_created_at_ties_by_id` above, for
/// `tags_for_chunk`'s `ORDER BY t.name, t.id ASC`.
///
/// `tag.name` is unique per user (`tag_user_name_idx`), so two tags owned
/// by the *same* user can never tie on `name` — a name tie can only be
/// constructed across several distinct users' tags that happen to share
/// text. `tags_for_chunk`'s query (see its doc comment) does not filter on
/// `t.user_id` at all; it trusts that `chunk_tag` never links a chunk to a
/// tag outside the chunk owner's tags, a guarantee enforced by
/// `set_chunk_tags` at write time (see `cannot_attach_another_users_tag`
/// above), not by this SELECT. This test deliberately bypasses that guard
/// with a direct `INSERT INTO chunk_tag`, the same way
/// `get_applies_to_scoped_to_owner_at_repo_layer`-style tests poke at the
/// repo layer directly elsewhere in this suite, purely to construct a
/// genuine tie for the ordering guarantee under test — it is not claiming
/// this cross-user state is reachable through the guarded API.
#[sqlx::test]
async fn tags_for_chunk_breaks_name_ties_by_id(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let alices_chunk = a_chunk(&pool, &alice, "Alice's").await;

    for i in 0..5 {
        let owner = seed(&pool, &format!("owner{i}@b.test")).await;
        let t = tag::create(&pool, &owner, "shared-name", None)
            .await
            .unwrap();
        sqlx::query!(
            "INSERT INTO chunk_tag (chunk_id, tag_id) VALUES ($1, $2)",
            alices_chunk,
            t.id
        )
        .execute(&pool)
        .await
        .unwrap();
    }

    // Ground truth from Postgres directly, so this test does not depend on
    // Rust's default string ordering happening to agree with the
    // database's collation.
    let expected_id_order: Vec<String> = sqlx::query_scalar!(
        "SELECT tag_id FROM chunk_tag WHERE chunk_id = $1 ORDER BY tag_id ASC",
        alices_chunk
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(expected_id_order.len(), 5);

    let first = tag::tags_for_chunk(&pool, &alice, &alices_chunk)
        .await
        .unwrap();
    let second = tag::tags_for_chunk(&pool, &alice, &alices_chunk)
        .await
        .unwrap();
    assert!(
        first.iter().all(|t| t.name == "shared-name"),
        "sanity check: the tie must be genuine, not five distinct names"
    );

    let first_ids: Vec<String> = first.iter().map(|t| t.id.clone()).collect();
    let second_ids: Vec<String> = second.iter().map(|t| t.id.clone()).collect();

    assert_eq!(
        first_ids, second_ids,
        "repeated calls over tied rows must return byte-identical order"
    );
    assert_eq!(
        first_ids, expected_id_order,
        "ties must be broken by ascending id, not left to query-plan chance"
    );
}

// ── tags_for_chunks: the bulk variant `search::service` uses ──────────

#[sqlx::test]
async fn tags_for_chunks_returns_names_grouped_by_chunk(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let c1 = a_chunk(&pool, &alice, "One").await;
    let c2 = a_chunk(&pool, &alice, "Two").await;
    let t1 = tag::create(&pool, &alice, "alpha", None).await.unwrap();
    let t2 = tag::create(&pool, &alice, "beta", None).await.unwrap();
    tag::set_chunk_tags(&pool, &alice, &c1, &[t1.id.clone(), t2.id.clone()])
        .await
        .unwrap();
    tag::set_chunk_tags(&pool, &alice, &c2, std::slice::from_ref(&t1.id))
        .await
        .unwrap();

    let rows = tag::tags_for_chunks(&pool, &alice, &[c1.clone(), c2.clone()])
        .await
        .unwrap();
    let mut c1_names: Vec<&str> = rows
        .iter()
        .filter(|r| r.chunk_id == c1)
        .map(|r| r.tag_name.as_str())
        .collect();
    c1_names.sort_unstable();
    assert_eq!(c1_names, vec!["alpha", "beta"]);

    let c2_names: Vec<&str> = rows
        .iter()
        .filter(|r| r.chunk_id == c2)
        .map(|r| r.tag_name.as_str())
        .collect();
    assert_eq!(c2_names, vec!["alpha"]);
}

#[sqlx::test]
async fn tags_for_chunks_of_empty_input_returns_empty_without_querying(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let rows = tag::tags_for_chunks(&pool, &alice, &[]).await.unwrap();
    assert!(rows.is_empty());
}

/// Scoped the same way as `tags_for_chunk`: a chunk id owned by another
/// user contributes no rows, even when it's tagged.
#[sqlx::test]
async fn tags_for_chunks_excludes_another_users_chunk(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let alices_chunk = a_chunk(&pool, &alice, "Alice's").await;
    let bobs_chunk = a_chunk(&pool, &bob, "Bob's").await;
    let alices_tag = tag::create(&pool, &alice, "mine", None).await.unwrap();
    let bobs_tag = tag::create(&pool, &bob, "his", None).await.unwrap();
    tag::set_chunk_tags(
        &pool,
        &alice,
        &alices_chunk,
        std::slice::from_ref(&alices_tag.id),
    )
    .await
    .unwrap();
    tag::set_chunk_tags(&pool, &bob, &bobs_chunk, std::slice::from_ref(&bobs_tag.id))
        .await
        .unwrap();

    let rows = tag::tags_for_chunks(&pool, &alice, &[alices_chunk.clone(), bobs_chunk.clone()])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].chunk_id, alices_chunk);
    assert_eq!(rows[0].tag_name, "mine");
}

// ---------------------------------------------------------------------------
// find_or_create — backs the `tags: ["name"]` field on chunk create/update
// ---------------------------------------------------------------------------

/// A second call with the same name returns the same row rather than
/// inserting a duplicate.
#[sqlx::test]
async fn find_or_create_reuses_an_existing_tag(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;

    let first = tag::find_or_create(&pool, &uid, "runbook").await.unwrap();
    let second = tag::find_or_create(&pool, &uid, "runbook").await.unwrap();
    assert_eq!(first.id, second.id, "the same name must resolve to one row");
    assert_eq!(tag::list(&pool, &uid).await.unwrap().len(), 1);
}

/// The lookup is scoped to the caller: two users may each own a tag named
/// `runbook`, and neither may be handed the other's row.
///
/// The `user_id = $2` in the SELECT is load-bearing — remove it and Alice's
/// row is returned to Bob, silently attaching her tag to his chunk. Proven
/// at the fubbik-db layer, where the guard is observed directly.
#[sqlx::test]
async fn find_or_create_is_user_scoped(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let bob = user::create(&pool, "b@b.test", "Bob", None)
        .await
        .unwrap()
        .id;

    let hers = tag::find_or_create(&pool, &alice, "runbook").await.unwrap();
    let his = tag::find_or_create(&pool, &bob, "runbook").await.unwrap();

    assert_ne!(
        hers.id, his.id,
        "each user must get their own tag row for the same name"
    );
    assert_eq!(hers.user_id, alice);
    assert_eq!(his.user_id, bob);
}

/// Matching is exact, including case — `Runbook` and `runbook` are two
/// tags, same as Node's `eq(tag.name, name)`.
#[sqlx::test]
async fn find_or_create_matches_case_sensitively(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;

    let lower = tag::find_or_create(&pool, &uid, "runbook").await.unwrap();
    let upper = tag::find_or_create(&pool, &uid, "Runbook").await.unwrap();
    assert_ne!(lower.id, upper.id);
    assert_eq!(tag::list(&pool, &uid).await.unwrap().len(), 2);
}
