use fubbik_db::repo::{tag_type, user};

#[sqlx::test]
async fn crud_round_trips_and_is_user_scoped(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let bob = user::create(&pool, "c@d.test", "Bob", None)
        .await
        .unwrap()
        .id;

    let t = tag_type::create(&pool, &alice, "Topic", Some("#ff0000"), None)
        .await
        .unwrap();
    assert_eq!(t.name, "Topic");
    assert_eq!(t.color, "#ff0000");

    // Bob cannot see or touch Alice's tag type.
    assert!(tag_type::list(&pool, &bob).await.unwrap().is_empty());
    assert!(
        tag_type::update(&pool, &bob, &t.id, Some("Hijacked"), None, None)
            .await
            .unwrap()
            .is_none()
    );
    assert!(!tag_type::delete(&pool, &bob, &t.id).await.unwrap());

    // Alice's row is untouched by Bob's attempts.
    let still = tag_type::list(&pool, &alice).await.unwrap();
    assert_eq!(still.len(), 1);
    assert_eq!(still[0].name, "Topic");

    assert!(tag_type::delete(&pool, &alice, &t.id).await.unwrap());
    assert!(tag_type::list(&pool, &alice).await.unwrap().is_empty());
}

#[sqlx::test]
async fn create_without_color_falls_back_to_db_default(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;

    let t = tag_type::create(&pool, &alice, "Topic", None, None)
        .await
        .unwrap();
    assert_eq!(t.color, "#8b5cf6");
}

/// Same bug class as `chunk::list`, `tag::list`, and `space::list` (see
/// their equivalent tests): `ORDER BY created_at ASC` alone over tied rows
/// is a query-plan artifact. Every tag type here shares the exact same
/// `created_at`, so only the `id ASC` tiebreaker can determine order.
#[sqlx::test]
async fn list_breaks_created_at_ties_by_id(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;

    for name in ["one", "two", "three", "four", "five"] {
        tag_type::create(&pool, &alice, name, None, None)
            .await
            .unwrap();
    }

    sqlx::query!(
        "UPDATE tag_type SET created_at = now() WHERE user_id = $1",
        alice
    )
    .execute(&pool)
    .await
    .unwrap();

    let expected_id_order: Vec<String> = sqlx::query_scalar!(
        "SELECT id FROM tag_type WHERE user_id = $1 ORDER BY id ASC",
        alice
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(expected_id_order.len(), 5);

    let first = tag_type::list(&pool, &alice).await.unwrap();
    let second = tag_type::list(&pool, &alice).await.unwrap();

    let first_ids: Vec<String> = first.iter().map(|t| t.id.clone()).collect();
    let second_ids: Vec<String> = second.iter().map(|t| t.id.clone()).collect();

    assert_eq!(
        first_ids, second_ids,
        "repeated calls over tied rows must return byte-identical order"
    );
    assert_eq!(
        first_ids, expected_id_order,
        "ties must be broken by ascending id, not left to query-plan chance"
    );
}
