use fubbik_db::repo::{session, user};

#[sqlx::test]
async fn session_round_trips_and_expires(pool: sqlx::PgPool) {
    let u = user::create(&pool, "a@b.test", "Alice", Some("hash"))
        .await
        .unwrap();

    let token = session::create(&pool, &u.id, chrono::Duration::days(7))
        .await
        .unwrap();

    let found = session::find_valid(&pool, &token).await.unwrap();
    assert_eq!(found.unwrap().id, u.id);

    let expired = session::create(&pool, &u.id, chrono::Duration::seconds(-1))
        .await
        .unwrap();
    assert!(session::find_valid(&pool, &expired).await.unwrap().is_none());
}
