//! `user_favorite` carries its own `id` and `user_id`, so listing/removing
//! don't need to go through the parent `chunk` row the way the composite
//! join tables (`chunk_tag`, `chunk_space`) do. But the chunk a favorite
//! points at still belongs to someone, so `add`'s ownership guard gets the
//! same "prove it in SQL" test treatment as `connection::create` and
//! `tag::attach`.

use fubbik_db::repo::{chunk, favorite, user};

async fn seed_user(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "U", None).await.unwrap().id
}

async fn a_chunk(pool: &sqlx::PgPool, uid: &str, title: &str) -> String {
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
    .id
}

#[sqlx::test]
async fn cannot_favorite_another_users_chunk(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let bobs_chunk = a_chunk(&pool, &bob, "Bob's chunk").await;

    let id = fubbik_db::new_id();
    let created = favorite::add(&pool, &id, &alice, &bobs_chunk, 0)
        .await
        .unwrap();
    assert!(
        created.is_none(),
        "must not create a favorite pointing at another user's chunk"
    );
    assert!(
        favorite::list(&pool, &alice).await.unwrap().is_empty(),
        "no row may be created"
    );
    // Bob's chunk (and any favorite of it) must be entirely unaffected.
    assert!(favorite::list(&pool, &bob).await.unwrap().is_empty());
}

#[sqlx::test]
async fn add_on_own_chunk_succeeds(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let c = a_chunk(&pool, &alice, "Alice's chunk").await;

    let id = fubbik_db::new_id();
    let created = favorite::add(&pool, &id, &alice, &c, 0)
        .await
        .unwrap()
        .expect("own chunk must be favoritable");
    assert_eq!(created.id, id);
    assert_eq!(created.user_id, alice);
    assert_eq!(created.chunk_id, c);
    assert_eq!(created.order, 0);
}

#[sqlx::test]
async fn add_is_a_silent_no_op_on_duplicate(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let c = a_chunk(&pool, &alice, "Alice's chunk").await;

    favorite::add(&pool, &fubbik_db::new_id(), &alice, &c, 0)
        .await
        .unwrap()
        .expect("first insert must succeed");

    // Matches Node's `.onConflictDoNothing()`: a duplicate (user_id,
    // chunk_id) pair returns `Ok(None)`, not an error.
    let second = favorite::add(&pool, &fubbik_db::new_id(), &alice, &c, 1)
        .await
        .unwrap();
    assert!(
        second.is_none(),
        "duplicate favorite must be a silent no-op, not an error"
    );
    assert_eq!(
        favorite::list(&pool, &alice).await.unwrap().len(),
        1,
        "only the first row may exist"
    );
}

#[sqlx::test]
async fn list_is_user_scoped(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let alices_chunk = a_chunk(&pool, &alice, "Alice's chunk").await;
    let bobs_chunk = a_chunk(&pool, &bob, "Bob's chunk").await;

    favorite::add(&pool, &fubbik_db::new_id(), &alice, &alices_chunk, 0)
        .await
        .unwrap();
    favorite::add(&pool, &fubbik_db::new_id(), &bob, &bobs_chunk, 0)
        .await
        .unwrap();

    let alice_list = favorite::list(&pool, &alice).await.unwrap();
    assert_eq!(alice_list.len(), 1);
    assert_eq!(alice_list[0].chunk_id, alices_chunk);

    let bob_list = favorite::list(&pool, &bob).await.unwrap();
    assert_eq!(bob_list.len(), 1);
    assert_eq!(bob_list[0].chunk_id, bobs_chunk);
}

#[sqlx::test]
async fn remove_on_another_users_favorite_affects_nothing(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let alices_chunk = a_chunk(&pool, &alice, "Alice's chunk").await;

    favorite::add(&pool, &fubbik_db::new_id(), &alice, &alices_chunk, 0)
        .await
        .unwrap();

    // Bob tries to remove Alice's favorite by naming her chunk_id — the
    // `WHERE user_id = $1 AND chunk_id = $2` filter means this can only
    // ever touch Bob's own (nonexistent) favorite row.
    favorite::remove(&pool, &bob, &alices_chunk).await.unwrap();

    let alice_list = favorite::list(&pool, &alice).await.unwrap();
    assert_eq!(
        alice_list.len(),
        1,
        "Alice's favorite must survive Bob's remove call"
    );
}

#[sqlx::test]
async fn remove_removes_the_callers_own_favorite(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let c = a_chunk(&pool, &alice, "Alice's chunk").await;

    favorite::add(&pool, &fubbik_db::new_id(), &alice, &c, 0)
        .await
        .unwrap();
    favorite::remove(&pool, &alice, &c).await.unwrap();

    assert!(favorite::list(&pool, &alice).await.unwrap().is_empty());
}

#[sqlx::test]
async fn remove_on_nonexistent_favorite_does_not_error(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    // No chunk, no favorite — must not error, matching Node's unconditional
    // "Deleted" response.
    favorite::remove(&pool, &alice, "no-such-chunk-id")
        .await
        .unwrap();

    // The unconditional "success" must not have side effects either — the
    // call above must not have created or removed anything.
    assert!(
        favorite::list(&pool, &alice).await.unwrap().is_empty(),
        "a no-op remove must leave the caller's favorites list empty"
    );
}

#[sqlx::test]
async fn reorder_updates_only_the_named_entries_and_only_for_the_caller(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let c1 = a_chunk(&pool, &alice, "One").await;
    let c2 = a_chunk(&pool, &alice, "Two").await;
    let c3 = a_chunk(&pool, &alice, "Three").await;
    let bobs_chunk = a_chunk(&pool, &bob, "Bob's").await;

    favorite::add(&pool, &fubbik_db::new_id(), &alice, &c1, 0)
        .await
        .unwrap();
    favorite::add(&pool, &fubbik_db::new_id(), &alice, &c2, 1)
        .await
        .unwrap();
    favorite::add(&pool, &fubbik_db::new_id(), &alice, &c3, 2)
        .await
        .unwrap();
    favorite::add(&pool, &fubbik_db::new_id(), &bob, &bobs_chunk, 0)
        .await
        .unwrap();

    // Partial reorder: only c1 and c3 are mentioned. c2 must keep its
    // existing order. The entry naming Bob's chunk_id must not touch
    // Bob's row (Alice doesn't have Bob's chunk favorited at all, so this
    // simply updates zero rows).
    favorite::reorder(
        &pool,
        &alice,
        &[(c1.clone(), 10), (c3.clone(), 5), (bobs_chunk.clone(), 99)],
    )
    .await
    .unwrap();

    let list = favorite::list(&pool, &alice).await.unwrap();
    let by_chunk: std::collections::HashMap<_, _> =
        list.iter().map(|f| (f.chunk_id.clone(), f.order)).collect();
    assert_eq!(by_chunk[&c1], 10);
    assert_eq!(by_chunk[&c2], 1, "unmentioned entry must keep its order");
    assert_eq!(by_chunk[&c3], 5);

    let bob_list = favorite::list(&pool, &bob).await.unwrap();
    assert_eq!(
        bob_list[0].order, 0,
        "Bob's favorite must be untouched by Alice's reorder call"
    );
}

/// The total ordering is `"order" ASC, id ASC`. Ties on `"order"` are
/// entirely plausible (Node's own `addFavorite` has a documented TOCTOU
/// race that can produce them), so `id ASC` is load-bearing, not
/// theoretical. This seeds several favorites sharing the exact same
/// `"order"` value and proves the tiebreaker holds.
#[sqlx::test]
async fn list_breaks_order_ties_by_id(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;

    let mut chunk_ids = Vec::new();
    for title in ["one", "two", "three", "four", "five", "six"] {
        let c = a_chunk(&pool, &alice, title).await;
        favorite::add(&pool, &fubbik_db::new_id(), &alice, &c, 0)
            .await
            .unwrap();
        chunk_ids.push(c);
    }

    let expected_id_order: Vec<String> = sqlx::query_scalar!(
        r#"SELECT id FROM user_favorite WHERE user_id = $1 ORDER BY id ASC"#,
        alice
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(expected_id_order.len(), 6);

    let first = favorite::list(&pool, &alice).await.unwrap();
    let second = favorite::list(&pool, &alice).await.unwrap();

    let first_ids: Vec<String> = first.iter().map(|f| f.id.clone()).collect();
    let second_ids: Vec<String> = second.iter().map(|f| f.id.clone()).collect();

    assert_eq!(
        first_ids, second_ids,
        "repeated calls over tied rows must return byte-identical order"
    );
    assert_eq!(
        first_ids, expected_id_order,
        "ties on \"order\" must be broken by ascending id, not left to query-plan chance"
    );
}
