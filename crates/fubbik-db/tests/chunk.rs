use fubbik_db::repo::{chunk, connection, space, tag, user};

async fn seed_user(pool: &sqlx::PgPool) -> String {
    user::create(pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id
}

async fn a_chunk(pool: &sqlx::PgPool, uid: &str, title: &str) -> chunk::Chunk {
    chunk::create(
        pool,
        uid,
        chunk::NewChunk {
            title: title.into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
        },
    )
    .await
    .unwrap()
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

// ── Task 7: `tags`/`after`/`enrichment`/`minConnections` (`ListParams`) ──

#[sqlx::test]
async fn tags_filter_is_or_semantics(pool: sqlx::PgPool) {
    let uid = seed_user(&pool).await;
    let tagged_a = a_chunk(&pool, &uid, "Tagged A").await;
    let tagged_b = a_chunk(&pool, &uid, "Tagged B").await;
    let untagged = a_chunk(&pool, &uid, "Untagged").await;

    let tag_a = tag::create(&pool, &uid, "alpha", None).await.unwrap();
    let tag_b = tag::create(&pool, &uid, "beta", None).await.unwrap();
    tag::set_chunk_tags(&pool, &uid, &tagged_a.id, std::slice::from_ref(&tag_a.id))
        .await
        .unwrap();
    tag::set_chunk_tags(&pool, &uid, &tagged_b.id, std::slice::from_ref(&tag_b.id))
        .await
        .unwrap();
    let _ = untagged;

    let found = chunk::list(
        &pool,
        &uid,
        &chunk::ListParams {
            tags: Some(vec!["alpha".into(), "beta".into()]),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let mut ids: Vec<&str> = found.iter().map(|c| c.id.as_str()).collect();
    ids.sort();
    let mut expected = vec![tagged_a.id.as_str(), tagged_b.id.as_str()];
    expected.sort();
    assert_eq!(
        ids, expected,
        "tags filter must be OR semantics: either tag name matches"
    );
}

/// The single most security-relevant case in this filter set: the tag
/// *name* "shared" collides across two independent users, each with their
/// own `tag` row and their own chunk tagged with it. Alice's `tags:
/// ["shared"]` filter must resolve only through the mandatory `user_id =
/// ..` predicate on `chunk`, never through the tag-name match alone —
/// otherwise a caller could enumerate another user's chunks just by
/// guessing tag names they also happen to use.
#[sqlx::test]
async fn tags_filter_cannot_leak_another_users_chunk_via_a_same_named_tag(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "alice@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let bob = user::create(&pool, "bob@b.test", "Bob", None)
        .await
        .unwrap()
        .id;

    let alices_chunk = a_chunk(&pool, &alice, "Alice's").await;
    let bobs_chunk = a_chunk(&pool, &bob, "Bob's").await;
    let alices_tag = tag::create(&pool, &alice, "shared", None).await.unwrap();
    let bobs_tag = tag::create(&pool, &bob, "shared", None).await.unwrap();
    tag::set_chunk_tags(&pool, &alice, &alices_chunk.id, &[alices_tag.id])
        .await
        .unwrap();
    tag::set_chunk_tags(&pool, &bob, &bobs_chunk.id, &[bobs_tag.id])
        .await
        .unwrap();

    let found = chunk::list(
        &pool,
        &alice,
        &chunk::ListParams {
            tags: Some(vec!["shared".into()]),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let ids: Vec<&str> = found.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![alices_chunk.id.as_str()],
        "a same-named tag owned by another user must not surface their chunk"
    );
}

#[sqlx::test]
async fn after_filters_by_updated_at_cutoff(pool: sqlx::PgPool) {
    let uid = seed_user(&pool).await;
    let recent = a_chunk(&pool, &uid, "Recent").await;
    let stale = a_chunk(&pool, &uid, "Stale").await;

    sqlx::query!(
        "UPDATE chunk SET updated_at = now() - interval '30 days' WHERE id = $1",
        stale.id
    )
    .execute(&pool)
    .await
    .unwrap();

    let found = chunk::list(
        &pool,
        &uid,
        &chunk::ListParams {
            after: Some(chrono::Utc::now().naive_utc() - chrono::Duration::days(7)),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        found.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        vec![recent.id.as_str()],
        "after must exclude chunks updated before the cutoff"
    );
}

#[sqlx::test]
async fn enrichment_filters_missing_and_complete(pool: sqlx::PgPool) {
    let uid = seed_user(&pool).await;
    let bare = a_chunk(&pool, &uid, "Bare").await;
    let enriched = a_chunk(&pool, &uid, "Enriched").await;

    let vector_literal = format!("[{}]", vec!["0"; 768].join(","));
    sqlx::query(
        "UPDATE chunk SET summary = 'a summary', aliases = '[\"x\"]'::jsonb, \
         embedding = $1::vector WHERE id = $2",
    )
    .bind(&vector_literal)
    .bind(&enriched.id)
    .execute(&pool)
    .await
    .unwrap();

    let missing = chunk::list(
        &pool,
        &uid,
        &chunk::ListParams {
            enrichment: Some(chunk::Enrichment::Missing),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        missing.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        vec![bare.id.as_str()]
    );

    let complete = chunk::list(
        &pool,
        &uid,
        &chunk::ListParams {
            enrichment: Some(chunk::Enrichment::Complete),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        complete.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        vec![enriched.id.as_str()]
    );
}

#[sqlx::test]
async fn min_connections_filters_by_total_connection_count_and_zero_means_unfiltered(
    pool: sqlx::PgPool,
) {
    let uid = seed_user(&pool).await;
    let hub = a_chunk(&pool, &uid, "Hub").await;
    let leaf1 = a_chunk(&pool, &uid, "Leaf1").await;
    let leaf2 = a_chunk(&pool, &uid, "Leaf2").await;
    let isolated = a_chunk(&pool, &uid, "Isolated").await;

    connection::create(
        &pool,
        &fubbik_db::new_id(),
        &uid,
        &hub.id,
        &leaf1.id,
        "related_to",
        "human",
        "approved",
    )
    .await
    .unwrap();
    connection::create(
        &pool,
        &fubbik_db::new_id(),
        &uid,
        &hub.id,
        &leaf2.id,
        "related_to",
        "human",
        "approved",
    )
    .await
    .unwrap();
    let _ = isolated;

    let at_least_two = chunk::list(
        &pool,
        &uid,
        &chunk::ListParams {
            min_connections: Some(2),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        at_least_two
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>(),
        vec![hub.id.as_str()],
        "only the chunk with >= 2 connections (as source or target) matches"
    );

    // Node's `if (params.minConnections && params.minConnections > 0)` treats
    // `0` as falsy — same as "no filter", not "at least zero connections".
    let zero = chunk::list(
        &pool,
        &uid,
        &chunk::ListParams {
            min_connections: Some(0),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        zero.len(),
        4,
        "min_connections: Some(0) must not filter anything"
    );
}

/// `space_id: Some(..)` matches chunks IN that space **or** chunks with no
/// space assignment at all — matching Node's `listChunks`
/// (`packages/db/src/repository/chunk.ts:108-111`) exactly. It is NOT
/// "chunks in this space only": a chunk in a *different* space must be
/// excluded, but a chunk in no space must still appear. `space_id: None`
/// applies no filter at all — every chunk regardless of space.
#[sqlx::test]
async fn space_id_filters_to_the_space_or_global_chunks_and_excludes_other_spaces(
    pool: sqlx::PgPool,
) {
    let uid = seed_user(&pool).await;
    let space_a = space::create(
        &pool,
        &uid,
        space::NewSpace {
            name: "space-a".into(),
            kind: "wiki".into(),
            description: None,
        },
        None,
    )
    .await
    .unwrap()
    .id;
    let space_b = space::create(
        &pool,
        &uid,
        space::NewSpace {
            name: "space-b".into(),
            kind: "wiki".into(),
            description: None,
        },
        None,
    )
    .await
    .unwrap()
    .id;

    let in_a = a_chunk(&pool, &uid, "In A").await;
    let in_b = a_chunk(&pool, &uid, "In B").await;
    let global = a_chunk(&pool, &uid, "Global").await;
    space::set_chunk_spaces(&pool, &uid, &in_a.id, std::slice::from_ref(&space_a))
        .await
        .unwrap();
    space::set_chunk_spaces(&pool, &uid, &in_b.id, std::slice::from_ref(&space_b))
        .await
        .unwrap();

    let scoped = chunk::list(
        &pool,
        &uid,
        &chunk::ListParams {
            space_id: Some(space_a.clone()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let mut ids: Vec<&str> = scoped.iter().map(|c| c.id.as_str()).collect();
    ids.sort();
    let mut expected = vec![in_a.id.as_str(), global.id.as_str()];
    expected.sort();
    assert_eq!(
        ids, expected,
        "must include the space's own chunk and global chunks, exclude space-b's"
    );

    let unscoped = chunk::list(&pool, &uid, &chunk::ListParams::default())
        .await
        .unwrap();
    assert_eq!(
        unscoped.len(),
        3,
        "space_id: None must apply no space filter at all"
    );
}
