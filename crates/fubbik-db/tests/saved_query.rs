//! `saved_query` repo layer — ownership scoping, the missing unique
//! constraint, and the `created_at DESC, id ASC` total order.

use fubbik_db::repo::{saved_query, user};

async fn seed_user(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "U", None).await.unwrap().id
}

#[sqlx::test]
async fn create_and_list_round_trip(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let created = saved_query::create(
        &pool,
        &alice,
        "my query",
        serde_json::json!({"clauses": []}),
        None,
    )
    .await
    .unwrap();
    assert_eq!(created.name, "my query");
    assert_eq!(created.user_id, alice);
    assert_eq!(created.space_id, None);

    let list = saved_query::list(&pool, &alice, None).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, created.id);
}

/// No unique constraint on `(user_id, name)` — Node's schema doesn't have
/// one, so duplicate names must be allowed.
#[sqlx::test]
async fn duplicate_names_are_allowed(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    saved_query::create(&pool, &alice, "same", serde_json::json!({}), None)
        .await
        .unwrap();
    let second = saved_query::create(&pool, &alice, "same", serde_json::json!({}), None).await;
    assert!(
        second.is_ok(),
        "there is no unique constraint on (user_id, name)"
    );
    assert_eq!(
        saved_query::list(&pool, &alice, None).await.unwrap().len(),
        2
    );
}

#[sqlx::test]
async fn list_is_user_scoped(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    saved_query::create(&pool, &alice, "alice's", serde_json::json!({}), None)
        .await
        .unwrap();
    saved_query::create(&pool, &bob, "bob's", serde_json::json!({}), None)
        .await
        .unwrap();

    let alice_list = saved_query::list(&pool, &alice, None).await.unwrap();
    assert_eq!(alice_list.len(), 1);
    assert_eq!(alice_list[0].name, "alice's");

    let bob_list = saved_query::list(&pool, &bob, None).await.unwrap();
    assert_eq!(bob_list.len(), 1);
    assert_eq!(bob_list[0].name, "bob's");
}

#[sqlx::test]
async fn list_can_be_narrowed_to_a_space(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let space = fubbik_db::repo::space::create(
        &pool,
        &alice,
        fubbik_db::repo::space::NewSpace {
            name: "Space".into(),
            kind: "notes".into(),
            description: None,
        },
        None,
    )
    .await
    .unwrap();
    saved_query::create(&pool, &alice, "global", serde_json::json!({}), None)
        .await
        .unwrap();
    saved_query::create(
        &pool,
        &alice,
        "scoped",
        serde_json::json!({}),
        Some(&space.id),
    )
    .await
    .unwrap();

    let all = saved_query::list(&pool, &alice, None).await.unwrap();
    assert_eq!(all.len(), 2);

    let scoped = saved_query::list(&pool, &alice, Some(&space.id))
        .await
        .unwrap();
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].name, "scoped");
}

/// The `user_id = $2` predicate in `delete`'s `WHERE` clause is the only
/// thing standing between this and deleting another user's row — the
/// route built on top never surfaces a 404 either way (see
/// `tests/search.rs`), so only a surviving-row assertion at this layer can
/// prove the guard is load-bearing.
#[sqlx::test]
async fn delete_is_user_scoped_and_leaves_the_victims_row_intact(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let alices = saved_query::create(&pool, &alice, "mine", serde_json::json!({}), None)
        .await
        .unwrap();

    // Bob names Alice's id.
    saved_query::delete(&pool, &bob, &alices.id).await.unwrap();

    let alice_list = saved_query::list(&pool, &alice, None).await.unwrap();
    assert_eq!(
        alice_list.len(),
        1,
        "Alice's saved query must survive Bob's delete call"
    );
}

#[sqlx::test]
async fn delete_removes_the_callers_own_row(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let created = saved_query::create(&pool, &alice, "mine", serde_json::json!({}), None)
        .await
        .unwrap();

    saved_query::delete(&pool, &alice, &created.id)
        .await
        .unwrap();

    assert!(
        saved_query::list(&pool, &alice, None)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test]
async fn delete_of_a_nonexistent_id_does_not_error(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    saved_query::delete(&pool, &alice, "no-such-id")
        .await
        .unwrap();
}

/// `created_at` is not unique — batch inserts routinely tie. `id ASC` is
/// the load-bearing tiebreaker, proven the same way
/// `notification.rs::list_breaks_created_at_ties_by_id` proves it: force
/// every row's `created_at` identical, `ANALYZE`, then confirm the order
/// matches ascending `id`, not query-plan chance.
#[sqlx::test]
async fn list_breaks_created_at_ties_by_id(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;

    for name in [
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
        "twenty",
    ] {
        saved_query::create(&pool, &alice, name, serde_json::json!({}), None)
            .await
            .unwrap();
    }

    sqlx::query!(
        "UPDATE saved_query SET created_at = now() WHERE user_id = $1",
        alice
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query!("ANALYZE saved_query")
        .execute(&pool)
        .await
        .unwrap();

    let expected_id_order: Vec<String> = sqlx::query_scalar!(
        "SELECT id FROM saved_query WHERE user_id = $1 ORDER BY id ASC",
        alice
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(expected_id_order.len(), 20);

    let first = saved_query::list(&pool, &alice, None).await.unwrap();
    let second = saved_query::list(&pool, &alice, None).await.unwrap();

    let first_ids: Vec<String> = first.iter().map(|s| s.id.clone()).collect();
    let second_ids: Vec<String> = second.iter().map(|s| s.id.clone()).collect();

    assert_eq!(
        first_ids, second_ids,
        "repeated calls over tied rows must return byte-identical order"
    );
    assert_eq!(
        first_ids, expected_id_order,
        "ties must be broken by ascending id, not left to query-plan chance"
    );
}
