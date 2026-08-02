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
