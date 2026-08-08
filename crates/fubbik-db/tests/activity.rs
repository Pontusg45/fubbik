//! There is no `activity::create` in the repository (see
//! `crates/fubbik-db/src/repo/activity.rs` — Node exposes no `POST
//! /activity` route either, only an internal `createActivity` other
//! domains call directly), so every test here seeds rows with a raw
//! `INSERT`, same approach `notification.rs`'s tests use.
//!
//! `list`'s `WHERE user_id = $1` predicate is Node's own scoping, faithfully
//! reproduced. The `AND EXISTS (SELECT 1 FROM space s WHERE s.id = ... AND
//! s.user_id = ...)` guard on the `space_id` filter is this port's own
//! addition — Node's equivalent filter is a bare `eq(activityLog.spaceId,
//! ...)` with no ownership check at all (see the module doc on
//! `fubbik_db::repo::activity` for why that's still safe in Node, and why
//! this port adds the guard anyway). Both predicates are proven
//! load-bearing below.

use fubbik_db::repo::activity::{self, ListParams};
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

#[allow(clippy::too_many_arguments)]
async fn seed_activity(
    pool: &sqlx::PgPool,
    user_id: &str,
    entity_type: &str,
    entity_id: &str,
    entity_title: Option<&str>,
    action: &str,
    space_id: Option<&str>,
) -> String {
    let id = fubbik_db::new_id();
    sqlx::query!(
        r#"INSERT INTO activity_log (id, user_id, entity_type, entity_id, entity_title, action, space_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        id,
        user_id,
        entity_type,
        entity_id,
        entity_title,
        action,
        space_id
    )
    .execute(pool)
    .await
    .unwrap();
    id
}

fn params() -> ListParams {
    ListParams::default()
}

#[sqlx::test]
async fn list_is_user_scoped(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;

    seed_activity(
        &pool,
        &alice,
        "chunk",
        "c1",
        Some("Alice's chunk"),
        "created",
        None,
    )
    .await;
    seed_activity(
        &pool,
        &bob,
        "chunk",
        "c2",
        Some("Bob's chunk"),
        "created",
        None,
    )
    .await;

    let alice_list = activity::list(&pool, &alice, &params()).await.unwrap();
    assert_eq!(alice_list.len(), 1);
    assert_eq!(alice_list[0].entity_title.as_deref(), Some("Alice's chunk"));

    let bob_list = activity::list(&pool, &bob, &params()).await.unwrap();
    assert_eq!(bob_list.len(), 1);
    assert_eq!(bob_list[0].entity_title.as_deref(), Some("Bob's chunk"));
}

#[sqlx::test]
async fn entity_type_filter_narrows_results(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;

    seed_activity(&pool, &alice, "chunk", "c1", None, "created", None).await;
    seed_activity(&pool, &alice, "requirement", "r1", None, "created", None).await;

    let mut p = params();
    p.entity_type = Some("requirement".into());
    let filtered = activity::list(&pool, &alice, &p).await.unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].entity_type, "requirement");
}

/// `action` and `entity_type` are unconstrained free text in Node (no
/// enum, no check constraint) — see the module doc on
/// `fubbik_db::repo::activity::Activity`. This proves the Rust repository
/// round-trips arbitrary strings rather than rejecting or coercing them.
#[sqlx::test]
async fn action_and_entity_type_are_free_text(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;

    seed_activity(
        &pool,
        &alice,
        "totally-made-up-entity",
        "x1",
        None,
        "totally-made-up-action",
        None,
    )
    .await;

    let listed = activity::list(&pool, &alice, &params()).await.unwrap();
    assert_eq!(listed[0].entity_type, "totally-made-up-entity");
    assert_eq!(listed[0].action, "totally-made-up-action");
}

/// The cross-user attack surface this port adds a guard for: a `spaceId`
/// belonging to another user must not be usable as a filter, even though
/// (per the module doc) it can never actually leak another user's rows
/// here since every row is already `user_id`-scoped. Proven below by
/// removing the `EXISTS` clause in `activity::list` and watching this test
/// fail — see the report for the exact failure message.
#[sqlx::test]
async fn space_id_filter_is_rejected_for_another_users_space(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let bobs_space = seed_space(&pool, &bob, "bobs-space").await;

    // Alice has an activity row that (implausibly, but the DB allows it —
    // space_id carries no FK-to-owner check of its own) points at Bob's
    // space id.
    seed_activity(
        &pool,
        &alice,
        "chunk",
        "c1",
        Some("Alice's row pointing at Bob's space"),
        "created",
        Some(&bobs_space),
    )
    .await;

    let mut p = params();
    p.space_id = Some(bobs_space.clone());
    let result = activity::list(&pool, &alice, &p).await.unwrap();
    assert!(
        result.is_empty(),
        "a spaceId belonging to another user must not be usable as a filter, even by its owner's row's actual creator"
    );
}

#[sqlx::test]
async fn space_id_filter_matches_the_callers_own_space(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let alices_space = seed_space(&pool, &alice, "alices-space").await;

    seed_activity(
        &pool,
        &alice,
        "chunk",
        "c1",
        Some("in space"),
        "created",
        Some(&alices_space),
    )
    .await;
    seed_activity(
        &pool,
        &alice,
        "chunk",
        "c2",
        Some("global"),
        "created",
        None,
    )
    .await;

    let mut p = params();
    p.space_id = Some(alices_space);
    let result = activity::list(&pool, &alice, &p).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].entity_title.as_deref(), Some("in space"));
}

/// Same bug class as `chunk::list`, `notification::list`, `tag::list`,
/// `tag_type::list`, and `space::list` (see their equivalent tests):
/// `ORDER BY created_at DESC` alone over tied rows is a query-plan
/// artifact, not a stable order. Activity rows are written in bursts (a
/// single mutation can fire several audit-log inserts), making ties
/// especially likely here — every row below shares the exact same
/// `created_at`, so only the `id ASC` tiebreaker can determine order.
#[sqlx::test]
async fn list_breaks_created_at_ties_by_id(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;

    for title in [
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
        seed_activity(&pool, &alice, "chunk", "c", Some(title), "created", None).await;
    }

    sqlx::query!(
        "UPDATE activity_log SET created_at = now() WHERE user_id = $1",
        alice
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query!("ANALYZE activity_log")
        .execute(&pool)
        .await
        .unwrap();

    let expected_id_order: Vec<String> = sqlx::query_scalar!(
        "SELECT id FROM activity_log WHERE user_id = $1 ORDER BY id ASC",
        alice
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(expected_id_order.len(), 20);

    let mut p = params();
    p.limit = 50;
    let first = activity::list(&pool, &alice, &p).await.unwrap();
    let second = activity::list(&pool, &alice, &p).await.unwrap();

    let first_ids: Vec<String> = first.iter().map(|a| a.id.clone()).collect();
    let second_ids: Vec<String> = second.iter().map(|a| a.id.clone()).collect();

    assert_eq!(
        first_ids, second_ids,
        "repeated calls over tied rows must return byte-identical order"
    );
    assert_eq!(
        first_ids, expected_id_order,
        "ties must be broken by ascending id, not left to query-plan chance"
    );
}
