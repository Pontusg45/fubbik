use fubbik_db::repo::{chunk, chunk_meta, user};

#[sqlx::test]
async fn replace_applies_to_is_idempotent(pool: sqlx::PgPool) {
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

    chunk_meta::replace_applies_to(&pool, &c.id, &["src/**/*.ts".into(), "docs/**".into()])
        .await
        .unwrap();
    assert_eq!(chunk_meta::get_applies_to(&pool, &c.id).await.unwrap().len(), 2);

    // Replacing with a smaller set must delete the old rows, not merge.
    chunk_meta::replace_applies_to(&pool, &c.id, &["src/**/*.ts".into()])
        .await
        .unwrap();
    let patterns = chunk_meta::get_applies_to(&pool, &c.id).await.unwrap();
    assert_eq!(patterns.len(), 1);
    assert_eq!(patterns[0].pattern, "src/**/*.ts");
}

#[sqlx::test]
async fn replace_file_refs_round_trips(pool: sqlx::PgPool) {
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

    chunk_meta::replace_file_refs(&pool, &c.id, &["src/index.ts".into()])
        .await
        .unwrap();
    let refs = chunk_meta::get_file_refs(&pool, &c.id).await.unwrap();
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].path, "src/index.ts");
}
