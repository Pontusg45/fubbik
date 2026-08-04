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
