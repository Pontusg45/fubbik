//! `collection` is a saved query, not a container — see
//! `fubbik_db::repo::collection`'s module doc. These tests cover the CRUD
//! repo functions and the one thing that's genuinely new to this domain:
//! the `space_id` ownership guard on create, which Node itself does not
//! have (a deliberate divergence, see `collection::create`'s doc comment).
//! Filter-to-`ListParams` mapping and the actual chunk evaluation are
//! tested where they belong: the new `ListParams` keys in
//! `tests/chunk.rs`, and the end-to-end `GET /collections/{id}/chunks`
//! envelope/scoping in `fubbik-api/tests/collections.rs`.

use fubbik_db::repo::collection::{self, CollectionFilter, CollectionPatch, NewCollection};
use fubbik_db::repo::{space, user};

async fn seed_user(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "U", None).await.unwrap().id
}

async fn seed_space(pool: &sqlx::PgPool, user_id: &str, name: &str) -> String {
    space::create(
        pool,
        user_id,
        space::NewSpace {
            name: name.into(),
            kind: "wiki".into(),
            description: None,
        },
        None,
    )
    .await
    .unwrap()
    .id
}

fn type_filter(t: &str) -> CollectionFilter {
    CollectionFilter {
        filter_type: Some(t.into()),
        ..Default::default()
    }
}

#[sqlx::test]
async fn create_and_find_by_id_round_trip(pool: sqlx::PgPool) {
    let uid = seed_user(&pool, "a@b.test").await;

    let created = collection::create(
        &pool,
        &uid,
        NewCollection {
            name: "Conventions".into(),
            description: Some("Pinned".into()),
            filter: type_filter("convention"),
            space_id: None,
        },
    )
    .await
    .unwrap()
    .expect("no space_id means no ownership guard to fail");
    assert_eq!(created.name, "Conventions");
    assert_eq!(created.filter.0.filter_type.as_deref(), Some("convention"));
    assert_eq!(created.space_id, None);

    let found = collection::find_by_id(&pool, &uid, &created.id)
        .await
        .unwrap()
        .expect("must round-trip");
    assert_eq!(found.id, created.id);
    assert_eq!(found.description.as_deref(), Some("Pinned"));
}

/// A partial filter (only one of the nine keys set) must not grow the other
/// eight as explicit `null`s on read — matching
/// `tests/fixtures/node-contract-2b/collections-list.json`, where seed
/// data's stored `filter` is exactly `{"type": "convention"}`.
#[sqlx::test]
async fn partial_filter_round_trips_without_growing_null_keys(pool: sqlx::PgPool) {
    let uid = seed_user(&pool, "a@b.test").await;
    let created = collection::create(
        &pool,
        &uid,
        NewCollection {
            name: "Conventions".into(),
            description: None,
            filter: type_filter("convention"),
            space_id: None,
        },
    )
    .await
    .unwrap()
    .unwrap();

    let raw: serde_json::Value = sqlx::query_scalar!(
        r#"SELECT filter AS "filter!: serde_json::Value" FROM collection WHERE id = $1"#,
        created.id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        raw,
        serde_json::json!({ "type": "convention" }),
        "unset filter keys must not be stored/serialised as explicit nulls"
    );
}

#[sqlx::test]
async fn create_with_owned_space_succeeds(pool: sqlx::PgPool) {
    let uid = seed_user(&pool, "a@b.test").await;
    let space_id = seed_space(&pool, &uid, "my-space").await;

    let created = collection::create(
        &pool,
        &uid,
        NewCollection {
            name: "Scoped".into(),
            description: None,
            filter: CollectionFilter::default(),
            space_id: Some(space_id.clone()),
        },
    )
    .await
    .unwrap()
    .expect("creating in one's own space must succeed");
    assert_eq!(created.space_id.as_deref(), Some(space_id.as_str()));
}

/// Node's `createCollection` has NO ownership check on `spaceId` at all
/// (`packages/db/src/repository/collection.ts:22-34`) — this is a
/// deliberate Rust-side addition, same shape as accepted divergences #4/#9/
/// #10. The write must be rejected outright (`None`, not a row that then
/// gets cleaned up) and must leave the victim's own collections/space
/// completely unaffected — a status-only assertion would pass even if the
/// rejected write partially mutated something.
#[sqlx::test]
async fn create_rejects_another_users_space_and_creates_nothing(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    let alices_space = seed_space(&pool, &alice, "alices-space").await;

    let result = collection::create(
        &pool,
        &bob,
        NewCollection {
            name: "hijack".into(),
            description: None,
            filter: CollectionFilter::default(),
            space_id: Some(alices_space.clone()),
        },
    )
    .await
    .unwrap();
    assert!(
        result.is_none(),
        "write against another user's space must be rejected"
    );

    let bobs_collections = collection::list(&pool, &bob).await.unwrap();
    assert!(
        bobs_collections.is_empty(),
        "the rejected write must not have created a row at all"
    );
    let alices_collections = collection::list(&pool, &alice).await.unwrap();
    assert!(alices_collections.is_empty(), "Alice gained nothing either");
}

/// `find_by_id`'s own `WHERE id = $1 AND user_id = $2` guard, tested
/// directly. Every other test that exercises cross-user access here goes
/// through `update`/`delete`, each of which has its own independent
/// ownership guard — none of them prove `find_by_id`'s guard specifically.
#[sqlx::test]
async fn find_by_id_is_user_scoped(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    let created = collection::create(
        &pool,
        &alice,
        NewCollection {
            name: "Alice's".into(),
            description: None,
            filter: CollectionFilter::default(),
            space_id: None,
        },
    )
    .await
    .unwrap()
    .unwrap();

    assert!(
        collection::find_by_id(&pool, &bob, &created.id)
            .await
            .unwrap()
            .is_none(),
        "Bob must not be able to look up Alice's collection by id"
    );
}

#[sqlx::test]
async fn list_is_scoped_to_caller(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    collection::create(
        &pool,
        &alice,
        NewCollection {
            name: "Alice's".into(),
            description: None,
            filter: CollectionFilter::default(),
            space_id: None,
        },
    )
    .await
    .unwrap();
    collection::create(
        &pool,
        &bob,
        NewCollection {
            name: "Bob's".into(),
            description: None,
            filter: CollectionFilter::default(),
            space_id: None,
        },
    )
    .await
    .unwrap();

    let alices = collection::list(&pool, &alice).await.unwrap();
    assert_eq!(alices.len(), 1);
    assert_eq!(alices[0].name, "Alice's");
}

/// Same "force a genuine tie, then verify Postgres's own ascending-id order"
/// shape as `chunk.rs::list_breaks_ties_by_id_for_every_sort`: 20 rows with
/// byte-identical `name`, so `ORDER BY name ASC` alone cannot break the tie
/// — only `, id ASC` can. Repeated calls must return the same order, and
/// that order must match Postgres's own `ORDER BY id ASC`.
///
/// `collection_user_name_idx` (`UNIQUE (user_id, name)`) makes a name tie
/// within one user's own collections unreachable through `collection::create`
/// itself — every `INSERT` past the first would fail the constraint before
/// ever reaching `ORDER BY`. That's a genuine, DB-enforced guarantee, not an
/// oversight, so this test drops the index for the duration of THIS
/// `#[sqlx::test]`'s isolated database only (each test gets its own ephemeral
/// DB from the migration template — this can't leak into any other test) to
/// construct the tie the tiebreaker exists to handle, the same way
/// `tag.rs::tags_for_chunk_breaks_name_ties_by_id` bypasses `set_chunk_tags`'s
/// write-time guard with a direct `INSERT` purely to construct its tie.
#[sqlx::test]
async fn list_breaks_name_ties_by_id_and_is_stable(pool: sqlx::PgPool) {
    sqlx::query!("DROP INDEX collection_user_name_idx")
        .execute(&pool)
        .await
        .unwrap();

    let uid = seed_user(&pool, "a@b.test").await;
    for _ in 0..20 {
        collection::create(
            &pool,
            &uid,
            NewCollection {
                name: "Same name".into(),
                description: None,
                filter: CollectionFilter::default(),
                space_id: None,
            },
        )
        .await
        .unwrap();
    }

    // Forces the planner to consider the tie seriously rather than
    // incidentally returning heap/insertion order for a tiny table — same
    // rationale as `tag.rs::list_breaks_created_at_ties_by_id`.
    sqlx::query!("ANALYZE collection")
        .execute(&pool)
        .await
        .unwrap();

    let expected_id_order: Vec<String> = sqlx::query_scalar!(
        "SELECT id FROM collection WHERE user_id = $1 ORDER BY id ASC",
        uid
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(expected_id_order.len(), 20);

    let first = collection::list(&pool, &uid).await.unwrap();
    let second = collection::list(&pool, &uid).await.unwrap();
    assert!(
        first.iter().all(|c| c.name == "Same name"),
        "sanity check: the tie must be genuine, not twenty distinct names"
    );
    let first_ids: Vec<String> = first.iter().map(|c| c.id.clone()).collect();
    let second_ids: Vec<String> = second.iter().map(|c| c.id.clone()).collect();

    assert_eq!(
        first_ids, second_ids,
        "repeated calls over fully-tied names must return byte-identical order"
    );
    assert_eq!(
        first_ids, expected_id_order,
        "ties must be broken by ascending id, not left to query-plan chance"
    );
}

#[sqlx::test]
async fn update_replaces_filter_wholesale_not_merge(pool: sqlx::PgPool) {
    let uid = seed_user(&pool, "a@b.test").await;
    let created = collection::create(
        &pool,
        &uid,
        NewCollection {
            name: "C".into(),
            description: None,
            filter: CollectionFilter {
                filter_type: Some("convention".into()),
                tags: Some("x".into()),
                ..Default::default()
            },
            space_id: None,
        },
    )
    .await
    .unwrap()
    .unwrap();

    let updated = collection::update(
        &pool,
        &uid,
        &created.id,
        CollectionPatch {
            filter: Some(type_filter("note")),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(updated.filter.0.filter_type.as_deref(), Some("note"));
    assert_eq!(
        updated.filter.0.tags, None,
        "PATCH filter replaces wholesale — the old `tags` key must be gone, not merged"
    );
}

#[sqlx::test]
async fn update_leaves_name_and_description_untouched_when_omitted(pool: sqlx::PgPool) {
    let uid = seed_user(&pool, "a@b.test").await;
    let created = collection::create(
        &pool,
        &uid,
        NewCollection {
            name: "Original".into(),
            description: Some("desc".into()),
            filter: CollectionFilter::default(),
            space_id: None,
        },
    )
    .await
    .unwrap()
    .unwrap();

    let updated = collection::update(
        &pool,
        &uid,
        &created.id,
        CollectionPatch {
            filter: Some(type_filter("note")),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(updated.name, "Original", "omitted name must be untouched");
    assert_eq!(
        updated.description.as_deref(),
        Some("desc"),
        "omitted description must be untouched"
    );
}

#[sqlx::test]
async fn update_on_another_users_collection_returns_none_and_leaves_it_unchanged(
    pool: sqlx::PgPool,
) {
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    let created = collection::create(
        &pool,
        &bob,
        NewCollection {
            name: "Bob's".into(),
            description: None,
            filter: CollectionFilter::default(),
            space_id: None,
        },
    )
    .await
    .unwrap()
    .unwrap();

    let result = collection::update(
        &pool,
        &alice,
        &created.id,
        CollectionPatch {
            name: Some("hijacked".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(result.is_none());

    let bobs = collection::find_by_id(&pool, &bob, &created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(bobs.name, "Bob's", "Bob's collection must be unchanged");
}

#[sqlx::test]
async fn delete_removes_row_and_is_user_scoped(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    let created = collection::create(
        &pool,
        &bob,
        NewCollection {
            name: "Bob's".into(),
            description: None,
            filter: CollectionFilter::default(),
            space_id: None,
        },
    )
    .await
    .unwrap()
    .unwrap();

    assert!(
        !collection::delete(&pool, &alice, &created.id)
            .await
            .unwrap(),
        "another user's delete must not remove the row"
    );
    assert!(
        collection::find_by_id(&pool, &bob, &created.id)
            .await
            .unwrap()
            .is_some(),
        "Bob's collection must survive Alice's rejected delete"
    );

    assert!(collection::delete(&pool, &bob, &created.id).await.unwrap());
    assert!(
        collection::find_by_id(&pool, &bob, &created.id)
            .await
            .unwrap()
            .is_none()
    );
}
