//! Repo-level tests for `fubbik_db::repo::requirement` — currently just
//! `search_titles`, the minimal module Task 8 added to back
//! `GET /api/search/autocomplete?field=requirement` (see that module's
//! doc comment: there is no full requirements domain ported yet).

use fubbik_db::repo::{requirement, user};

async fn seed_user(pool: &sqlx::PgPool) -> String {
    user::create(pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id
}

async fn seed_requirement(pool: &sqlx::PgPool, user_id: &str, title: &str) -> String {
    let id = fubbik_db::new_id();
    sqlx::query!(
        r#"INSERT INTO requirement (id, title, steps, user_id) VALUES ($1, $2, '[]'::jsonb, $3)"#,
        id,
        title,
        user_id
    )
    .execute(pool)
    .await
    .unwrap();
    id
}

/// Divergence #17 (Phase 2c task 8b): Node's `searchRequirementTitles` has
/// no `user_id` filter either, leaking every user's requirement titles
/// through the same autocomplete endpoint. Proves the added guard: Bob's
/// query for Alice's title-matching prefix comes back empty, and Alice
/// still sees her own row.
#[sqlx::test]
async fn search_titles_never_returns_another_users_requirement(pool: sqlx::PgPool) {
    let alice = seed_user(&pool).await;
    let bob = user::create(&pool, "bob-search-titles@b.test", "Bob", None)
        .await
        .unwrap()
        .id;

    seed_requirement(&pool, &alice, "The Great Authentication Flow").await;

    let bobs_view = requirement::search_titles(&pool, &bob, "Authentication", 10)
        .await
        .unwrap();
    assert_eq!(
        bobs_view.len(),
        0,
        "must not surface another user's requirement title"
    );

    let alices_view = requirement::search_titles(&pool, &alice, "Authentication", 10)
        .await
        .unwrap();
    assert_eq!(
        alices_view.len(),
        1,
        "the guard must not break the query for the owner"
    );
    assert_eq!(alices_view[0].title, "The Great Authentication Flow");
}
