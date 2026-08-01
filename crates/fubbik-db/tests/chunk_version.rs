use fubbik_db::repo::{chunk, chunk_version, user};

#[sqlx::test]
async fn snapshot_records_pre_edit_state(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "Alice", None).await.unwrap().id;
    let c = chunk::create(
        &pool,
        &uid,
        chunk::NewChunk {
            title: "Original".into(),
            content: "v1".into(),
            chunk_type: "note".into(),
            rationale: None,
        },
    )
    .await
    .unwrap();

    chunk_version::snapshot(&pool, &c).await.unwrap();

    let history = chunk_version::list_for_chunk(&pool, &c.id).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].title, "Original");
    assert_eq!(history[0].content, "v1");
    assert_eq!(history[0].version, 1, "first snapshot is version 1");
}

#[sqlx::test]
async fn version_numbers_increment_per_chunk(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "Alice", None).await.unwrap().id;
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

    chunk_version::snapshot(&pool, &c).await.unwrap();
    chunk_version::snapshot(&pool, &c).await.unwrap();

    let history = chunk_version::list_for_chunk(&pool, &c.id).await.unwrap();
    assert_eq!(
        history.iter().map(|v| v.version).collect::<Vec<_>>(),
        [2, 1],
        "history is newest-first with incrementing versions"
    );
}

/// Proves the `UNIQUE (chunk_id, version)` constraint (migration 0003) is
/// what actually prevents duplicate version numbers: two rows inserted
/// directly with the same `(chunk_id, version)` must fail with a unique
/// violation, not silently coexist.
#[sqlx::test]
async fn duplicate_chunk_id_version_is_rejected(pool: sqlx::PgPool) {
    let uid = user::create(&pool, "a@b.test", "Alice", None).await.unwrap().id;
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

    sqlx::query!(
        r#"INSERT INTO chunk_version (id, chunk_id, version, title, content, type, tags, created_at)
           VALUES ($1, $2, 1, 'T', '', 'note', '[]'::jsonb, now())"#,
        "dup-1",
        c.id
    )
    .execute(&pool)
    .await
    .unwrap();

    let err = sqlx::query!(
        r#"INSERT INTO chunk_version (id, chunk_id, version, title, content, type, tags, created_at)
           VALUES ($1, $2, 1, 'T', '', 'note', '[]'::jsonb, now())"#,
        "dup-2",
        c.id
    )
    .execute(&pool)
    .await
    .unwrap_err();

    let db_err = err.as_database_error().expect("expected a database error");
    assert_eq!(
        db_err.code().as_deref(),
        Some("23505"),
        "expected a unique_violation (23505), got: {db_err}"
    );
}
