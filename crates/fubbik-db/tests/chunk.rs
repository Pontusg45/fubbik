use fubbik_db::repo::{chunk, user};

async fn seed_user(pool: &sqlx::PgPool) -> String {
    user::create(pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id
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

    let patch = chunk::ChunkPatch {
        title: Some("Renamed".into()),
        ..Default::default()
    };
    let updated = chunk::update(&pool, &uid, &created.id, patch)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.title, "Renamed");
    assert_eq!(
        updated.content, "Use kebab-case.",
        "unset patch fields must not clear columns"
    );

    assert!(chunk::delete(&pool, &uid, &created.id).await.unwrap());
    assert!(
        chunk::find_by_id(&pool, &uid, &created.id)
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test]
async fn other_users_chunks_are_invisible(pool: sqlx::PgPool) {
    let owner = seed_user(&pool).await;
    let intruder = user::create(&pool, "c@d.test", "Bob", None)
        .await
        .unwrap()
        .id;

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

    assert!(
        chunk::find_by_id(&pool, &intruder, &c.id)
            .await
            .unwrap()
            .is_none()
    );
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
        &chunk::ListParams {
            chunk_type: Some("note".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(notes.len(), 2);

    let searched = chunk::list(
        &pool,
        &uid,
        &chunk::ListParams {
            search: Some("Beta".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(searched.len(), 1);
    assert_eq!(searched[0].title, "Beta");

    let alpha = chunk::list(
        &pool,
        &uid,
        &chunk::ListParams {
            sort: chunk::Sort::Alpha,
            ..Default::default()
        },
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
        &chunk::ListParams {
            limit: 2,
            offset: 1,
            sort: chunk::Sort::Alpha,
            ..Default::default()
        },
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
        &chunk::ListParams {
            search: Some("uniquebody".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(found.len(), 1);
}

/// The live differential run against Node found a real, non-cosmetic
/// mismatch on `/api/chunks?search=convention`: same row set, different
/// order, because seed data ties `created_at` across several rows and
/// neither stack had a deterministic tiebreaker. `ORDER BY` over tied rows
/// with no tiebreaker is a query-plan artifact — it can differ between two
/// calls to the *same* stack, and (worse) between the query serving page 1
/// and the one serving page 2 of a `LIMIT`/`OFFSET` walk, silently
/// skipping or duplicating rows across pages.
///
/// This test forces `title`, `created_at`, AND `updated_at` to be
/// identical across every row, so for every `Sort` branch the primary sort
/// key cannot break the tie — only `id ASC` can. A test using distinct
/// timestamps would pass even without the tiebreaker fix and prove
/// nothing.
#[sqlx::test]
async fn list_breaks_ties_by_id_for_every_sort(pool: sqlx::PgPool) {
    let uid = seed_user(&pool).await;

    for _ in 0..5 {
        chunk::create(
            &pool,
            &uid,
            chunk::NewChunk {
                title: "Same title".into(),
                content: String::new(),
                chunk_type: "note".into(),
                rationale: None,
            },
        )
        .await
        .unwrap();
    }

    // A single UPDATE statement's `now()` is fixed for the whole statement,
    // so this produces a genuine tie across all five rows on both columns,
    // not five close-but-distinct timestamps.
    sqlx::query!(
        "UPDATE chunk SET created_at = now(), updated_at = now() WHERE user_id = $1",
        uid
    )
    .execute(&pool)
    .await
    .unwrap();

    // Ground truth: ask Postgres directly for ascending id order, rather
    // than sorting the ids in Rust — Rust's default string ordering could
    // in principle disagree with the database's collation, and this test
    // must not depend on the two happening to agree.
    let expected_id_order: Vec<String> = sqlx::query_scalar!(
        "SELECT id FROM chunk WHERE user_id = $1 ORDER BY id ASC",
        uid
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(expected_id_order.len(), 5);

    for sort in [
        chunk::Sort::Newest,
        chunk::Sort::Oldest,
        chunk::Sort::Alpha,
        chunk::Sort::Updated,
    ] {
        let params = chunk::ListParams {
            sort,
            ..Default::default()
        };
        let first = chunk::list(&pool, &uid, &params).await.unwrap();
        let second = chunk::list(&pool, &uid, &params).await.unwrap();

        let first_ids: Vec<String> = first.iter().map(|c| c.id.clone()).collect();
        let second_ids: Vec<String> = second.iter().map(|c| c.id.clone()).collect();

        assert_eq!(
            first_ids, second_ids,
            "{sort:?}: repeated calls over fully-tied rows must return byte-identical order"
        );
        assert_eq!(
            first_ids, expected_id_order,
            "{sort:?}: ties must be broken by ascending id, not left to query-plan chance"
        );
    }
}
