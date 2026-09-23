//! Repo-level tests for `saved_graph`, proving each guard is load-bearing at
//! the SQL layer — an API test can't distinguish "the SQL guard is missing"
//! from "the service pre-check already 404'd", so every cross-user
//! assertion here calls the repository function directly.

use std::collections::HashMap;

use fubbik_db::repo::saved_graph::{self, NewSavedGraph, Position, SavedGraphPatch};
use fubbik_db::repo::user;

async fn seed_user(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "U", None).await.unwrap().id
}

fn new_graph(name: &str) -> NewSavedGraph {
    NewSavedGraph {
        name: name.to_string(),
        description: None,
        chunk_ids: vec!["chunk-1".to_string(), "chunk-2".to_string()],
        positions: HashMap::from([("chunk-1".to_string(), Position { x: 1.0, y: 2.0 })]),
        layout_algorithm: "force".to_string(),
        space_id: None,
    }
}

#[sqlx::test]
async fn create_and_find_by_id_round_trip(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;

    // When
    let created = saved_graph::create(&pool, &uid, new_graph("My Graph"))
        .await
        .unwrap();
    // Then
    assert_eq!(created.name, "My Graph");
    assert_eq!(created.layout_algorithm, "force");
    assert_eq!(
        created.chunk_ids.0,
        vec!["chunk-1".to_string(), "chunk-2".to_string()]
    );
    assert_eq!(
        created.positions.0.get("chunk-1"),
        Some(&Position { x: 1.0, y: 2.0 })
    );

    let found = saved_graph::find_by_id(&pool, &uid, &created.id)
        .await
        .unwrap()
        .expect("must round-trip");
    assert_eq!(found.id, created.id);
}

/// `find_by_id`'s own `WHERE id = $1 AND user_id = $2` guard, proved
/// directly rather than only through `update`/`delete`'s independent
/// guards.
#[sqlx::test]
async fn find_by_id_is_user_scoped(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    // When
    let created = saved_graph::create(&pool, &alice, new_graph("Alice's"))
        .await
        .unwrap();

    // Then
    assert!(
        saved_graph::find_by_id(&pool, &bob, &created.id)
            .await
            .unwrap()
            .is_none(),
        "Bob must not be able to look up Alice's saved graph by id"
    );
}

#[sqlx::test]
async fn list_is_scoped_to_caller(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    saved_graph::create(&pool, &alice, new_graph("Alice's"))
        .await
        .unwrap();
    saved_graph::create(&pool, &bob, new_graph("Bob's"))
        .await
        .unwrap();

    // When
    let alices = saved_graph::list(&pool, &alice, None).await.unwrap();
    // Then
    assert_eq!(alices.len(), 1);
    assert_eq!(alices[0].name, "Alice's");

    let bobs = saved_graph::list(&pool, &bob, None).await.unwrap();
    assert_eq!(bobs.len(), 1);
    assert_eq!(bobs[0].name, "Bob's");
}

#[sqlx::test]
async fn list_filters_by_space_id_when_given(pool: sqlx::PgPool) {
    // Given
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

    let mut in_space = new_graph("In space");
    in_space.space_id = Some(space_id.clone());
    saved_graph::create(&pool, &uid, in_space).await.unwrap();
    saved_graph::create(&pool, &uid, new_graph("No space"))
        .await
        .unwrap();

    // When
    let all = saved_graph::list(&pool, &uid, None).await.unwrap();
    // Then
    assert_eq!(all.len(), 2);

    let scoped = saved_graph::list(&pool, &uid, Some(space_id.as_str()))
        .await
        .unwrap();
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].name, "In space");

    // Node's `if (spaceId)` truthy check treats an empty string as "no
    // filter" — reproduced here.
    let empty_filter = saved_graph::list(&pool, &uid, Some("")).await.unwrap();
    assert_eq!(
        empty_filter.len(),
        2,
        "an empty-string spaceId must behave as no filter, matching Node's truthy check"
    );
}

/// Same bug class as `collection::list`/`notification::list`: `ORDER BY
/// created_at DESC` alone over tied rows is a query-plan artifact, not a
/// stable order. Every row here shares the exact same `created_at`, so only
/// the `id ASC` tiebreaker can determine order.
#[sqlx::test]
async fn list_breaks_created_at_ties_by_id(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    for i in 0..20 {
        saved_graph::create(&pool, &uid, new_graph(&format!("Graph {i}")))
            .await
            .unwrap();
    }

    sqlx::query!(
        "UPDATE saved_graph SET created_at = now() WHERE user_id = $1",
        uid
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query!("ANALYZE saved_graph")
        .execute(&pool)
        .await
        .unwrap();

    // When
    let expected_id_order: Vec<String> = sqlx::query_scalar!(
        "SELECT id FROM saved_graph WHERE user_id = $1 ORDER BY id ASC",
        uid
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    // Then
    assert_eq!(expected_id_order.len(), 20);

    let first = saved_graph::list(&pool, &uid, None).await.unwrap();
    let second = saved_graph::list(&pool, &uid, None).await.unwrap();

    let first_ids: Vec<String> = first.iter().map(|g| g.id.clone()).collect();
    let second_ids: Vec<String> = second.iter().map(|g| g.id.clone()).collect();

    assert_eq!(
        first_ids, second_ids,
        "repeated calls over tied rows must return byte-identical order"
    );
    assert_eq!(
        first_ids, expected_id_order,
        "ties must be broken by ascending id, not left to query-plan chance"
    );
}

#[sqlx::test]
async fn update_changes_only_the_given_fields(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let created = saved_graph::create(&pool, &uid, new_graph("Original"))
        .await
        .unwrap();

    // When
    let updated = saved_graph::update(
        &pool,
        &uid,
        &created.id,
        SavedGraphPatch {
            name: Some("Renamed".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .unwrap();

    // Then
    assert_eq!(updated.name, "Renamed");
    assert_eq!(
        updated.chunk_ids.0, created.chunk_ids.0,
        "omitted chunk_ids must be untouched"
    );
    assert_eq!(
        updated.layout_algorithm, created.layout_algorithm,
        "omitted layout_algorithm must be untouched"
    );
}

/// `description` is tri-state: `None` leaves it untouched, `Some(None)`
/// explicitly clears it.
#[sqlx::test]
async fn update_description_tri_state(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let mut with_desc = new_graph("Original");
    with_desc.description = Some("has a description".into());
    // When
    let created = saved_graph::create(&pool, &uid, with_desc).await.unwrap();
    // Then
    assert_eq!(created.description.as_deref(), Some("has a description"));

    // Omitted: untouched.
    let untouched = saved_graph::update(
        &pool,
        &uid,
        &created.id,
        SavedGraphPatch {
            name: Some("still original".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(untouched.description.as_deref(), Some("has a description"));

    // Explicit null: cleared.
    let cleared = saved_graph::update(
        &pool,
        &uid,
        &created.id,
        SavedGraphPatch {
            description: Some(None),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(cleared.description, None);
}

/// Mirrors Node's `updateSavedGraph`: when the patch has no fields set at
/// all, the row is re-selected unchanged and `updated_at` does NOT bump —
/// unlike `collection::update`, whose Drizzle `$onUpdate` hook fires even on
/// an empty `.set(...)`.
#[sqlx::test]
async fn update_with_no_fields_does_not_touch_updated_at(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let created = saved_graph::create(&pool, &uid, new_graph("Original"))
        .await
        .unwrap();

    // Give the clock a moment to move, so a bug that DID bump updated_at
    // would show up as a real timestamp difference, not a rounding
    // coincidence.
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    // When
    let result = saved_graph::update(&pool, &uid, &created.id, SavedGraphPatch::default())
        .await
        .unwrap()
        .unwrap();

    // Then
    assert_eq!(
        result.updated_at.0, created.updated_at.0,
        "a no-op patch must not bump updated_at"
    );
}

#[sqlx::test]
async fn update_on_another_users_saved_graph_returns_none_and_leaves_it_unchanged(
    pool: sqlx::PgPool,
) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    let created = saved_graph::create(&pool, &bob, new_graph("Bob's"))
        .await
        .unwrap();

    // When
    let result = saved_graph::update(
        &pool,
        &alice,
        &created.id,
        SavedGraphPatch {
            name: Some("hijacked".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    // Then
    assert!(
        result.is_none(),
        "another user's update must not affect the row"
    );

    let bobs = saved_graph::find_by_id(&pool, &bob, &created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(bobs.name, "Bob's", "Bob's saved graph must be unchanged");
}

#[sqlx::test]
async fn delete_removes_row_and_is_user_scoped(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    // When
    let created = saved_graph::create(&pool, &bob, new_graph("Bob's"))
        .await
        .unwrap();

    // Then
    assert!(
        !saved_graph::delete(&pool, &alice, &created.id)
            .await
            .unwrap(),
        "another user's delete must not remove the row"
    );
    assert!(
        saved_graph::find_by_id(&pool, &bob, &created.id)
            .await
            .unwrap()
            .is_some(),
        "Bob's saved graph must survive Alice's rejected delete"
    );

    assert!(saved_graph::delete(&pool, &bob, &created.id).await.unwrap());
    assert!(
        saved_graph::find_by_id(&pool, &bob, &created.id)
            .await
            .unwrap()
            .is_none()
    );
}
