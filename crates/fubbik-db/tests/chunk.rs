use fubbik_db::repo::{chunk, user};

async fn seed_user(pool: &sqlx::PgPool) -> String {
    user::create(pool, "a@b.test", "Alice", None).await.unwrap().id
}

#[sqlx::test]
async fn create_read_update_delete(pool: sqlx::PgPool) {
    let uid = seed_user(&pool).await;

    let created = chunk::create(
        &pool,
        &uid,
        chunk::NewChunk {
            title: "Naming conventions".into(),
            content: "Use kebab-case.".into(),
            chunk_type: "note".into(),
            rationale: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(created.title, "Naming conventions");

    let patch = chunk::ChunkPatch { title: Some("Renamed".into()), ..Default::default() };
    let updated = chunk::update(&pool, &uid, &created.id, patch).await.unwrap().unwrap();
    assert_eq!(updated.title, "Renamed");
    assert_eq!(updated.content, "Use kebab-case.", "unset patch fields must not clear columns");

    assert!(chunk::delete(&pool, &uid, &created.id).await.unwrap());
    assert!(chunk::find_by_id(&pool, &uid, &created.id).await.unwrap().is_none());
}

#[sqlx::test]
async fn other_users_chunks_are_invisible(pool: sqlx::PgPool) {
    let owner = seed_user(&pool).await;
    let intruder = user::create(&pool, "c@d.test", "Bob", None).await.unwrap().id;

    let c = chunk::create(
        &pool,
        &owner,
        chunk::NewChunk {
            title: "Secret".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
        },
    )
    .await
    .unwrap();

    assert!(chunk::find_by_id(&pool, &intruder, &c.id).await.unwrap().is_none());
    assert!(!chunk::delete(&pool, &intruder, &c.id).await.unwrap());
}
