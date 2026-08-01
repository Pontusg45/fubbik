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

#[sqlx::test]
async fn list_filters_sorts_and_paginates(pool: sqlx::PgPool) {
    let uid = seed_user(&pool).await;

    for (title, ty) in [("Alpha", "note"), ("Beta", "document"), ("Gamma", "note")] {
        chunk::create(
            &pool,
            &uid,
            chunk::NewChunk {
                title: title.into(),
                content: format!("{title} body"),
                chunk_type: ty.into(),
                rationale: None,
            },
        )
        .await
        .unwrap();
    }

    let notes = chunk::list(
        &pool,
        &uid,
        chunk::ListParams { chunk_type: Some("note".into()), ..Default::default() },
    )
    .await
    .unwrap();
    assert_eq!(notes.len(), 2);

    let searched = chunk::list(
        &pool,
        &uid,
        chunk::ListParams { search: Some("Beta".into()), ..Default::default() },
    )
    .await
    .unwrap();
    assert_eq!(searched.len(), 1);
    assert_eq!(searched[0].title, "Beta");

    let alpha = chunk::list(
        &pool,
        &uid,
        chunk::ListParams { sort: chunk::Sort::Alpha, ..Default::default() },
    )
    .await
    .unwrap();
    assert_eq!(
        alpha.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(),
        ["Alpha", "Beta", "Gamma"]
    );

    let page = chunk::list(
        &pool,
        &uid,
        chunk::ListParams { limit: 2, offset: 1, sort: chunk::Sort::Alpha, ..Default::default() },
    )
    .await
    .unwrap();
    assert_eq!(page.len(), 2);
    assert_eq!(page[0].title, "Beta");
}

#[sqlx::test]
async fn search_is_case_insensitive_and_covers_content(pool: sqlx::PgPool) {
    let uid = seed_user(&pool).await;
    chunk::create(
        &pool,
        &uid,
        chunk::NewChunk {
            title: "Title".into(),
            content: "UNIQUEBODY".into(),
            chunk_type: "note".into(),
            rationale: None,
        },
    )
    .await
    .unwrap();

    let found = chunk::list(
        &pool,
        &uid,
        chunk::ListParams { search: Some("uniquebody".into()), ..Default::default() },
    )
    .await
    .unwrap();
    assert_eq!(found.len(), 1);
}
