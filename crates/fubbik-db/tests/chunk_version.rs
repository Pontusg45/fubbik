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
