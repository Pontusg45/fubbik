use fubbik_db::repo::{chunk, chunk_meta, user};

#[sqlx::test]
async fn replace_applies_to_shrinks_and_is_idempotent(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let c = chunk::create(
        &pool,
        &uid,
        chunk::NewChunk {
            title: "T".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    chunk_meta::replace_applies_to(
        &pool,
        &c.id,
        &uid,
        &["src/**/*.ts".into(), "docs/**".into()],
    )
    .await
    .unwrap();
    assert_eq!(
        chunk_meta::get_applies_to(&pool, &c.id, &uid)
            .await
            .unwrap()
            .len(),
        2
    );

    // Replacing with a smaller set must delete the old rows, not merge.
    chunk_meta::replace_applies_to(&pool, &c.id, &uid, &["src/**/*.ts".into()])
        .await
        .unwrap();
    let patterns = chunk_meta::get_applies_to(&pool, &c.id, &uid)
        .await
        .unwrap();
    assert_eq!(patterns.len(), 1);
    assert_eq!(patterns[0].pattern, "src/**/*.ts");

    // Replacing with the exact same set again must land on the same
    // observable state, not accumulate duplicate rows.
    chunk_meta::replace_applies_to(&pool, &c.id, &uid, &["src/**/*.ts".into()])
        .await
        .unwrap();
    let patterns = chunk_meta::get_applies_to(&pool, &c.id, &uid)
        .await
        .unwrap();
    assert_eq!(
        patterns.len(),
        1,
        "PUTting the same set twice must not duplicate rows"
    );
    assert_eq!(patterns[0].pattern, "src/**/*.ts");
}

#[sqlx::test]
async fn replace_applies_to_with_empty_set_clears_all_rows(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let c = chunk::create(
        &pool,
        &uid,
        chunk::NewChunk {
            title: "T".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    chunk_meta::replace_applies_to(
        &pool,
        &c.id,
        &uid,
        &["src/**/*.ts".into(), "docs/**".into()],
    )
    .await
    .unwrap();
    assert_eq!(
        chunk_meta::get_applies_to(&pool, &c.id, &uid)
            .await
            .unwrap()
            .len(),
        2
    );

    chunk_meta::replace_applies_to(&pool, &c.id, &uid, &[])
        .await
        .unwrap();
    assert_eq!(
        chunk_meta::get_applies_to(&pool, &c.id, &uid)
            .await
            .unwrap()
            .len(),
        0,
        "PUTting an empty set must clear all rows"
    );
}

#[sqlx::test]
async fn replace_file_refs_round_trips(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let c = chunk::create(
        &pool,
        &uid,
        chunk::NewChunk {
            title: "T".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    chunk_meta::replace_file_refs(&pool, &c.id, &uid, &["src/index.ts".into()])
        .await
        .unwrap();
    let refs = chunk_meta::get_file_refs(&pool, &c.id, &uid).await.unwrap();
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].path, "src/index.ts");
}

#[sqlx::test]
async fn replace_file_refs_with_empty_set_clears_all_rows(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let c = chunk::create(
        &pool,
        &uid,
        chunk::NewChunk {
            title: "T".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    chunk_meta::replace_file_refs(
        &pool,
        &c.id,
        &uid,
        &["src/index.ts".into(), "src/lib.ts".into()],
    )
    .await
    .unwrap();
    assert_eq!(
        chunk_meta::get_file_refs(&pool, &c.id, &uid)
            .await
            .unwrap()
            .len(),
        2
    );

    chunk_meta::replace_file_refs(&pool, &c.id, &uid, &[])
        .await
        .unwrap();
    assert_eq!(
        chunk_meta::get_file_refs(&pool, &c.id, &uid)
            .await
            .unwrap()
            .len(),
        0,
        "PUTting an empty set must clear all rows"
    );
}

/// Proves the scoping lives in the SQL itself, not just in callers that
/// remember to check ownership first: a repo call made directly with
/// another user's id must not see the owner's applies-to patterns.
#[sqlx::test]
async fn get_applies_to_scoped_to_owner_at_repo_layer(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "alice@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let bob = user::create(&pool, "bob@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let c = chunk::create(
        &pool,
        &alice,
        chunk::NewChunk {
            title: "Alice's chunk".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    chunk_meta::replace_applies_to(&pool, &c.id, &alice, &["src/**/*.ts".into()])
        .await
        .unwrap();

    let as_bob = chunk_meta::get_applies_to(&pool, &c.id, &bob)
        .await
        .unwrap();
    assert!(
        as_bob.is_empty(),
        "another user's id must not see Alice's applies-to patterns, even calling the repo directly"
    );
}

/// A `replace_applies_to` call made with another user's id must be a
/// no-op: it must neither delete the owner's existing rows nor insert new
/// rows under the wrong user's authority.
#[sqlx::test]
async fn replace_applies_to_with_wrong_user_is_noop_at_repo_layer(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "alice@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let bob = user::create(&pool, "bob@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let c = chunk::create(
        &pool,
        &alice,
        chunk::NewChunk {
            title: "Alice's chunk".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    chunk_meta::replace_applies_to(&pool, &c.id, &alice, &["src/**/*.ts".into()])
        .await
        .unwrap();

    // Bob attempts to overwrite Alice's chunk's patterns directly at the
    // repo layer, bypassing any service-level ownership check.
    chunk_meta::replace_applies_to(&pool, &c.id, &bob, &["evil/**".into()])
        .await
        .unwrap();

    let patterns = chunk_meta::get_applies_to(&pool, &c.id, &alice)
        .await
        .unwrap();
    assert_eq!(
        patterns.len(),
        1,
        "Bob's unauthorized replace must not touch Alice's rows"
    );
    assert_eq!(patterns[0].pattern, "src/**/*.ts");
}

/// Same guarantee as above, for file refs.
#[sqlx::test]
async fn get_file_refs_scoped_to_owner_at_repo_layer(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "alice@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let bob = user::create(&pool, "bob@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let c = chunk::create(
        &pool,
        &alice,
        chunk::NewChunk {
            title: "Alice's chunk".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    chunk_meta::replace_file_refs(&pool, &c.id, &alice, &["src/index.ts".into()])
        .await
        .unwrap();

    let as_bob = chunk_meta::get_file_refs(&pool, &c.id, &bob).await.unwrap();
    assert!(
        as_bob.is_empty(),
        "another user's id must not see Alice's file refs, even calling the repo directly"
    );
}

/// A `replace_file_refs` call made with another user's id must be a no-op.
#[sqlx::test]
async fn replace_file_refs_with_wrong_user_is_noop_at_repo_layer(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "alice@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let bob = user::create(&pool, "bob@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let c = chunk::create(
        &pool,
        &alice,
        chunk::NewChunk {
            title: "Alice's chunk".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    chunk_meta::replace_file_refs(&pool, &c.id, &alice, &["src/index.ts".into()])
        .await
        .unwrap();

    chunk_meta::replace_file_refs(&pool, &c.id, &bob, &["evil.ts".into()])
        .await
        .unwrap();

    let refs = chunk_meta::get_file_refs(&pool, &c.id, &alice)
        .await
        .unwrap();
    assert_eq!(
        refs.len(),
        1,
        "Bob's unauthorized replace must not touch Alice's rows"
    );
    assert_eq!(refs[0].path, "src/index.ts");
}

/// Same bug class as `chunk::list` (see its equivalent test in
/// `tests/chunk.rs`): `ORDER BY pattern, id` breaks a tie on `pattern` only
/// via the `id` tiebreaker. This is a reachable case, not a contrived one —
/// nothing stops `replace_applies_to` from being called with the same glob
/// twice (see `PUTting the same set twice must not duplicate rows` above),
/// so duplicate `pattern` values across rows for one chunk are normal. A
/// test using distinct patterns would pass even without the tiebreaker and
/// prove nothing.
#[sqlx::test]
async fn get_applies_to_breaks_pattern_ties_by_id(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let c = chunk::create(
        &pool,
        &uid,
        chunk::NewChunk {
            title: "T".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let patterns = vec![chunk_meta::AppliesToInput::from("src/**/*.ts"); 5];
    chunk_meta::replace_applies_to(&pool, &c.id, &uid, &patterns)
        .await
        .unwrap();

    // Ground truth from Postgres directly, so this test does not depend on
    // Rust's default string ordering happening to agree with the
    // database's collation.
    let expected_id_order: Vec<String> = sqlx::query_scalar!(
        "SELECT id FROM chunk_applies_to WHERE chunk_id = $1 ORDER BY id ASC",
        c.id
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(expected_id_order.len(), 5);

    let first = chunk_meta::get_applies_to(&pool, &c.id, &uid)
        .await
        .unwrap();
    let second = chunk_meta::get_applies_to(&pool, &c.id, &uid)
        .await
        .unwrap();

    let first_ids: Vec<String> = first.iter().map(|p| p.id.clone()).collect();
    let second_ids: Vec<String> = second.iter().map(|p| p.id.clone()).collect();

    assert_eq!(
        first_ids, second_ids,
        "repeated calls over tied rows must return byte-identical order"
    );
    assert_eq!(
        first_ids, expected_id_order,
        "ties must be broken by ascending id, not left to query-plan chance"
    );
}

/// Same bug class as above, for `get_file_refs` — `ORDER BY path, id` ties
/// on `path`, which `replace_file_refs` does not enforce as unique either.
#[sqlx::test]
async fn get_file_refs_breaks_path_ties_by_id(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let c = chunk::create(
        &pool,
        &uid,
        chunk::NewChunk {
            title: "T".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let paths = vec![chunk_meta::FileRefInput::from("src/index.ts"); 5];
    chunk_meta::replace_file_refs(&pool, &c.id, &uid, &paths)
        .await
        .unwrap();

    let expected_id_order: Vec<String> = sqlx::query_scalar!(
        "SELECT id FROM chunk_file_ref WHERE chunk_id = $1 ORDER BY id ASC",
        c.id
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(expected_id_order.len(), 5);

    let first = chunk_meta::get_file_refs(&pool, &c.id, &uid).await.unwrap();
    let second = chunk_meta::get_file_refs(&pool, &c.id, &uid).await.unwrap();

    let first_ids: Vec<String> = first.iter().map(|r| r.id.clone()).collect();
    let second_ids: Vec<String> = second.iter().map(|r| r.id.clone()).collect();

    assert_eq!(
        first_ids, second_ids,
        "repeated calls over tied rows must return byte-identical order"
    );
    assert_eq!(
        first_ids, expected_id_order,
        "ties must be broken by ascending id, not left to query-plan chance"
    );
}

// ---------------------------------------------------------------------
// file_ref_path_exists (backs requirements::cross_ref)
// ---------------------------------------------------------------------

#[sqlx::test]
async fn file_ref_path_exists_finds_a_matching_path(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "cross-ref@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let c = chunk::create(
        &pool,
        &uid,
        chunk::NewChunk {
            title: "T".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    chunk_meta::replace_file_refs(&pool, &c.id, &uid, &["src/lib.rs".into()])
        .await
        .unwrap();

    assert!(
        chunk_meta::file_ref_path_exists(&pool, &uid, "src/lib.rs")
            .await
            .unwrap()
    );
    assert!(
        !chunk_meta::file_ref_path_exists(&pool, &uid, "src/missing.rs")
            .await
            .unwrap()
    );
}

/// Proves the `c.user_id = $2` scope: Bob must not see Alice's file
/// reference as "existing" through this check.
#[sqlx::test]
async fn file_ref_path_exists_is_user_scoped(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "cross-ref-owner@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let bob = user::create(&pool, "cross-ref-other@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let c = chunk::create(
        &pool,
        &alice,
        chunk::NewChunk {
            title: "T".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    chunk_meta::replace_file_refs(&pool, &c.id, &alice, &["src/lib.rs".into()])
        .await
        .unwrap();

    assert!(
        !chunk_meta::file_ref_path_exists(&pool, &bob, "src/lib.rs")
            .await
            .unwrap(),
        "must not see another user's file reference as existing"
    );
}
