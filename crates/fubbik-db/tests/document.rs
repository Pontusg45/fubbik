//! Repo-level tests for `document`, proving each `user_id` guard is
//! load-bearing at the SQL layer — an API test can't distinguish "the SQL
//! guard is missing" from "a service pre-check already 404'd", so every
//! cross-user assertion here calls the repository function directly. See
//! `fubbik_db::repo::document`'s module doc comment for why this port
//! scopes `find_by_id`/`update`/`delete` in SQL where Node's equivalents
//! are unscoped and rely on a service-layer check instead.

use fubbik_db::repo::document::{self, DocumentPatch, NewDocument};
use fubbik_db::repo::user;

async fn seed_user(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "U", None).await.unwrap().id
}

fn new_doc(id: &str, title: &str, source_path: &str) -> NewDocument {
    NewDocument {
        id: id.to_string(),
        title: title.to_string(),
        source_path: source_path.to_string(),
        content_hash: "hash-1".to_string(),
        description: None,
        space_id: None,
        split_level: Some(2),
    }
}

#[sqlx::test]
async fn create_and_find_by_id_round_trip(pool: sqlx::PgPool) {
    let uid = seed_user(&pool, "a@b.test").await;
    let id = fubbik_db::new_id();

    let created = document::create(&pool, &uid, new_doc(&id, "Doc Title", "docs/a.md"))
        .await
        .unwrap();
    assert_eq!(created.title, "Doc Title");
    assert_eq!(created.source_path, "docs/a.md");
    assert_eq!(created.content_hash, "hash-1");
    assert_eq!(created.split_level, Some(2));

    let found = document::find_by_id(&pool, &uid, &created.id)
        .await
        .unwrap()
        .expect("must round-trip");
    assert_eq!(found.id, created.id);
}

/// `find_by_id`'s `WHERE id = $1 AND user_id = $2` guard, proved directly.
/// Removing this guard would make this test fail here (repo layer) — there
/// is no redundant caller-side check behind it in `documents::service`.
#[sqlx::test]
async fn find_by_id_is_user_scoped(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    let id = fubbik_db::new_id();
    document::create(&pool, &alice, new_doc(&id, "Alice's", "docs/a.md"))
        .await
        .unwrap();

    assert!(
        document::find_by_id(&pool, &bob, &id)
            .await
            .unwrap()
            .is_none(),
        "Bob must not be able to look up Alice's document by id"
    );
}

#[sqlx::test]
async fn find_by_source_path_none_space_id_matches_only_global_documents(pool: sqlx::PgPool) {
    let uid = seed_user(&pool, "a@b.test").await;
    let space_id = fubbik_db::repo::space::create(
        &pool,
        &uid,
        fubbik_db::repo::space::NewSpace {
            name: "my-space".into(),
            kind: "wiki".into(),
            description: None,
        },
        None,
    )
    .await
    .unwrap()
    .id;

    let global_id = fubbik_db::new_id();
    let mut global_doc = new_doc(&global_id, "Global", "docs/a.md");
    global_doc.space_id = None;
    document::create(&pool, &uid, global_doc).await.unwrap();

    let scoped_id = fubbik_db::new_id();
    let mut scoped_doc = new_doc(&scoped_id, "Scoped", "docs/a.md");
    scoped_doc.space_id = Some(space_id.clone());
    document::create(&pool, &uid, scoped_doc).await.unwrap();

    let found_global = document::find_by_source_path(&pool, &uid, "docs/a.md", None)
        .await
        .unwrap()
        .expect("must find the global (no-space) document");
    assert_eq!(found_global.id, global_id);

    let found_scoped =
        document::find_by_source_path(&pool, &uid, "docs/a.md", Some(space_id.as_str()))
            .await
            .unwrap()
            .expect("must find the space-scoped document");
    assert_eq!(found_scoped.id, scoped_id);
}

#[sqlx::test]
async fn list_is_scoped_to_caller(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    document::create(
        &pool,
        &alice,
        new_doc(&fubbik_db::new_id(), "Alice's", "docs/a.md"),
    )
    .await
    .unwrap();
    document::create(
        &pool,
        &bob,
        new_doc(&fubbik_db::new_id(), "Bob's", "docs/b.md"),
    )
    .await
    .unwrap();

    let alices = document::list(&pool, &alice, None).await.unwrap();
    assert_eq!(alices.len(), 1);
    assert_eq!(alices[0].title, "Alice's");
    assert_eq!(alices[0].chunk_count, 0);
    assert!(alices[0].last_chunk_updated_at.is_none());

    let bobs = document::list(&pool, &bob, None).await.unwrap();
    assert_eq!(bobs.len(), 1);
    assert_eq!(bobs[0].title, "Bob's");
}

/// Every row here shares the same `title`, so only the `id ASC` tiebreaker
/// this port adds can determine a stable order.
#[sqlx::test]
async fn list_breaks_title_ties_by_id(pool: sqlx::PgPool) {
    let uid = seed_user(&pool, "a@b.test").await;
    for i in 0..15 {
        document::create(
            &pool,
            &uid,
            new_doc(&fubbik_db::new_id(), "Same Title", &format!("docs/{i}.md")),
        )
        .await
        .unwrap();
    }

    let expected_id_order: Vec<String> = sqlx::query_scalar!(
        "SELECT id FROM document WHERE user_id = $1 ORDER BY id ASC",
        uid
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(expected_id_order.len(), 15);

    let first = document::list(&pool, &uid, None).await.unwrap();
    let second = document::list(&pool, &uid, None).await.unwrap();

    let first_ids: Vec<String> = first.iter().map(|d| d.id.clone()).collect();
    let second_ids: Vec<String> = second.iter().map(|d| d.id.clone()).collect();

    assert_eq!(
        first_ids, second_ids,
        "repeated calls must be byte-identical"
    );
    assert_eq!(
        first_ids, expected_id_order,
        "ties must break by ascending id"
    );
}

#[sqlx::test]
async fn list_with_tags_defaults_type_and_splits_tags(pool: sqlx::PgPool) {
    let uid = seed_user(&pool, "a@b.test").await;
    let doc_id = fubbik_db::new_id();
    document::create(&pool, &uid, new_doc(&doc_id, "With Chunks", "docs/a.md"))
        .await
        .unwrap();

    let chunk_id = fubbik_db::new_id();
    document::insert_section_chunk(&pool, &uid, &chunk_id, "Section", "content", &doc_id, 0)
        .await
        .unwrap();
    let tag_ids =
        document::resolve_tag_ids(&pool, &uid, &["alpha".to_string(), "beta".to_string()])
            .await
            .unwrap();
    fubbik_db::repo::tag::set_chunk_tags(&pool, &uid, &chunk_id, &tag_ids)
        .await
        .unwrap();

    let list = document::list_with_tags(&pool, &uid, None).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].chunk_count, 1);
    assert_eq!(list[0].doc_type, "document");
    let mut tags = list[0].tags.clone();
    tags.sort();
    assert_eq!(tags, vec!["alpha".to_string(), "beta".to_string()]);

    // No chunks at all -> type falls back to "document", tags is empty.
    let empty_doc_id = fubbik_db::new_id();
    document::create(
        &pool,
        &uid,
        new_doc(&empty_doc_id, "Empty", "docs/empty.md"),
    )
    .await
    .unwrap();
    let list = document::list_with_tags(&pool, &uid, None).await.unwrap();
    let empty_entry = list.iter().find(|d| d.id == empty_doc_id).unwrap();
    assert_eq!(empty_entry.doc_type, "document");
    assert!(empty_entry.tags.is_empty());
}

#[sqlx::test]
async fn update_changes_only_given_fields_and_is_user_scoped(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    let id = fubbik_db::new_id();
    let created = document::create(&pool, &bob, new_doc(&id, "Original", "docs/a.md"))
        .await
        .unwrap();

    // Cross-user update must be rejected and must not touch Bob's row.
    let hijack = document::update(
        &pool,
        &alice,
        &id,
        DocumentPatch {
            title: Some("Hijacked".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(hijack.is_none(), "another user's update must return None");

    let unchanged = document::find_by_id(&pool, &bob, &id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        unchanged.title, "Original",
        "Bob's document must be unchanged"
    );

    // Owner's update only touches the given field.
    let updated = document::update(
        &pool,
        &bob,
        &id,
        DocumentPatch {
            title: Some("Renamed".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(updated.title, "Renamed");
    assert_eq!(
        updated.content_hash, created.content_hash,
        "omitted field untouched"
    );
}

#[sqlx::test]
async fn delete_orphans_chunks_and_is_user_scoped(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    let doc_id = fubbik_db::new_id();
    document::create(&pool, &bob, new_doc(&doc_id, "Bob's", "docs/a.md"))
        .await
        .unwrap();
    let chunk_id = fubbik_db::new_id();
    document::insert_section_chunk(&pool, &bob, &chunk_id, "Section", "content", &doc_id, 0)
        .await
        .unwrap();

    // Alice cannot delete Bob's document.
    let rejected = document::delete(&pool, &alice, &doc_id).await.unwrap();
    assert!(rejected.is_none(), "another user's delete must return None");
    assert!(
        document::find_by_id(&pool, &bob, &doc_id)
            .await
            .unwrap()
            .is_some(),
        "Bob's document must survive Alice's rejected delete"
    );

    // Bob's own delete succeeds and orphans (not deletes) the chunk.
    let deleted = document::delete(&pool, &bob, &doc_id).await.unwrap();
    assert!(deleted.is_some());
    assert!(
        document::find_by_id(&pool, &bob, &doc_id)
            .await
            .unwrap()
            .is_none()
    );

    let (survived_document_id, survived_order): (Option<String>, Option<i32>) =
        sqlx::query_as("SELECT document_id, document_order FROM chunk WHERE id = $1")
            .bind(&chunk_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        survived_document_id, None,
        "chunk must survive with document_id cleared"
    );
    assert_eq!(
        survived_order, None,
        "chunk must survive with document_order cleared"
    );
}

#[sqlx::test]
async fn document_chunks_orders_by_document_order_then_id_and_is_user_scoped(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    let doc_id = fubbik_db::new_id();
    document::create(&pool, &alice, new_doc(&doc_id, "Doc", "docs/a.md"))
        .await
        .unwrap();

    let c2 = fubbik_db::new_id();
    document::insert_section_chunk(&pool, &alice, &c2, "Second", "b", &doc_id, 1)
        .await
        .unwrap();
    let c1 = fubbik_db::new_id();
    document::insert_section_chunk(&pool, &alice, &c1, "First", "a", &doc_id, 0)
        .await
        .unwrap();

    let chunks = document::document_chunks(&pool, &alice, &doc_id)
        .await
        .unwrap();
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].id, c1);
    assert_eq!(chunks[1].id, c2);

    // Bob (not the owner) sees none of Alice's chunks.
    let bobs_view = document::document_chunks(&pool, &bob, &doc_id)
        .await
        .unwrap();
    assert!(bobs_view.is_empty());
}

#[sqlx::test]
async fn update_section_chunk_sets_content_and_order(pool: sqlx::PgPool) {
    let uid = seed_user(&pool, "a@b.test").await;
    let doc_id = fubbik_db::new_id();
    document::create(&pool, &uid, new_doc(&doc_id, "Doc", "docs/a.md"))
        .await
        .unwrap();
    let chunk_id = fubbik_db::new_id();
    document::insert_section_chunk(&pool, &uid, &chunk_id, "Section", "old", &doc_id, 0)
        .await
        .unwrap();

    document::update_section_chunk(&pool, &uid, &chunk_id, "new content", 3)
        .await
        .unwrap();

    let chunks = document::document_chunks(&pool, &uid, &doc_id)
        .await
        .unwrap();
    assert_eq!(chunks[0].content, "new content");
    assert_eq!(chunks[0].document_order, Some(3));
}

/// `touch_chunk` intentionally does NOT clear `document_order` — see the
/// module doc comment on why this reproduces a live Node bug rather than
/// fixing it.
#[sqlx::test]
async fn touch_chunk_bumps_updated_at_but_leaves_document_order_alone(pool: sqlx::PgPool) {
    let uid = seed_user(&pool, "a@b.test").await;
    let doc_id = fubbik_db::new_id();
    document::create(&pool, &uid, new_doc(&doc_id, "Doc", "docs/a.md"))
        .await
        .unwrap();
    let chunk_id = fubbik_db::new_id();
    document::insert_section_chunk(&pool, &uid, &chunk_id, "Section", "content", &doc_id, 5)
        .await
        .unwrap();
    let before = document::document_chunks(&pool, &uid, &doc_id)
        .await
        .unwrap()[0]
        .updated_at
        .0;

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    document::touch_chunk(&pool, &uid, &chunk_id).await.unwrap();

    let after = &document::document_chunks(&pool, &uid, &doc_id)
        .await
        .unwrap()[0];
    assert_eq!(
        after.document_order,
        Some(5),
        "document_order must be left untouched"
    );
    assert!(after.updated_at.0 > before, "updated_at must still bump");
}

#[sqlx::test]
async fn resolve_tag_ids_reuses_existing_tags_and_creates_missing_ones(pool: sqlx::PgPool) {
    let uid = seed_user(&pool, "a@b.test").await;
    let existing = fubbik_db::repo::tag::create(&pool, &uid, "existing", None)
        .await
        .unwrap();

    let ids = document::resolve_tag_ids(
        &pool,
        &uid,
        &["existing".to_string(), "brand-new".to_string()],
    )
    .await
    .unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0], existing.id, "must reuse the existing tag's id");

    let count: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) AS \"count!\" FROM tag WHERE user_id = $1",
        uid
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 2, "only one new tag should have been created");
}

#[sqlx::test]
async fn search_chunks_matches_title_or_content_and_is_scoped(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    let doc_id = fubbik_db::new_id();
    document::create(&pool, &alice, new_doc(&doc_id, "Doc", "docs/a.md"))
        .await
        .unwrap();
    document::insert_section_chunk(
        &pool,
        &alice,
        &fubbik_db::new_id(),
        "Auth Guide",
        "irrelevant",
        &doc_id,
        0,
    )
    .await
    .unwrap();
    document::insert_section_chunk(
        &pool,
        &alice,
        &fubbik_db::new_id(),
        "Other",
        "mentions auth in body",
        &doc_id,
        1,
    )
    .await
    .unwrap();
    document::insert_section_chunk(
        &pool,
        &alice,
        &fubbik_db::new_id(),
        "Unrelated",
        "nothing matching",
        &doc_id,
        2,
    )
    .await
    .unwrap();

    let results = document::search_chunks(&pool, &alice, "auth", 20, None)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);

    let bobs_results = document::search_chunks(&pool, &bob, "auth", 20, None)
        .await
        .unwrap();
    assert!(bobs_results.is_empty(), "Bob must not see Alice's chunks");
}
