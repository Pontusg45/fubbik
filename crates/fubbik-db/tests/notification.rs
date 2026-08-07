//! There is no `notification::create` in the repository (see
//! `crates/fubbik-db/src/repo/notification.rs` — the Node HTTP surface
//! never exposes a create route either, only internal service calls this
//! codebase's Rust port does not need to reproduce for this slice), so
//! every test here seeds rows with a raw `INSERT` directly, the same
//! approach `tag_type.rs`'s FK test uses for the `tag` table.

use fubbik_db::repo::{notification, user};

async fn seed(
    pool: &sqlx::PgPool,
    user_id: &str,
    notification_type: &str,
    title: &str,
    read: bool,
) -> String {
    let id = fubbik_db::new_id();
    sqlx::query!(
        r#"INSERT INTO notification (id, user_id, type, title, message, link_to, read)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        id,
        user_id,
        notification_type,
        title,
        "a message",
        Option::<&str>::None,
        read
    )
    .execute(pool)
    .await
    .unwrap();
    id
}

#[sqlx::test]
async fn list_is_user_scoped(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let bob = user::create(&pool, "c@d.test", "Bob", None)
        .await
        .unwrap()
        .id;

    seed(&pool, &alice, "stale_chunks", "Alice's notification", false).await;
    seed(&pool, &bob, "stale_chunks", "Bob's notification", false).await;

    let alice_list = notification::list(&pool, &alice, false, 50).await.unwrap();
    assert_eq!(alice_list.len(), 1);
    assert_eq!(alice_list[0].title, "Alice's notification");

    let bob_list = notification::list(&pool, &bob, false, 50).await.unwrap();
    assert_eq!(bob_list.len(), 1);
    assert_eq!(bob_list[0].title, "Bob's notification");
}

#[sqlx::test]
async fn list_unread_only_filters_out_read_rows(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;

    seed(&pool, &alice, "stale_chunks", "Unread one", false).await;
    seed(&pool, &alice, "stale_chunks", "Already read", true).await;

    let all = notification::list(&pool, &alice, false, 50).await.unwrap();
    assert_eq!(all.len(), 2);

    let unread = notification::list(&pool, &alice, true, 50).await.unwrap();
    assert_eq!(unread.len(), 1);
    assert_eq!(unread[0].title, "Unread one");
}

#[sqlx::test]
async fn count_unread_counts_only_unread_and_only_the_caller(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let bob = user::create(&pool, "c@d.test", "Bob", None)
        .await
        .unwrap()
        .id;

    seed(&pool, &alice, "stale_chunks", "Unread one", false).await;
    seed(&pool, &alice, "stale_chunks", "Unread two", false).await;
    seed(&pool, &alice, "stale_chunks", "Already read", true).await;
    seed(&pool, &bob, "stale_chunks", "Bob's unread", false).await;

    assert_eq!(notification::count_unread(&pool, &alice).await.unwrap(), 2);
    assert_eq!(notification::count_unread(&pool, &bob).await.unwrap(), 1);
}

#[sqlx::test]
async fn mark_read_on_another_users_notification_affects_nothing_and_returns_none(
    pool: sqlx::PgPool,
) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let bob = user::create(&pool, "c@d.test", "Bob", None)
        .await
        .unwrap()
        .id;

    let id = seed(&pool, &alice, "stale_chunks", "Alice's notification", false).await;

    let result = notification::mark_read(&pool, &bob, &id).await.unwrap();
    assert!(
        result.is_none(),
        "marking another user's notification must return None, not succeed"
    );

    // Confirm the row is untouched, not just that the call reported failure.
    let still_unread = notification::list(&pool, &alice, true, 50).await.unwrap();
    assert_eq!(still_unread.len(), 1);
    assert_eq!(still_unread[0].id, id);
    assert!(!still_unread[0].read);
}

#[sqlx::test]
async fn mark_read_marks_the_callers_own_notification(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;

    let id = seed(&pool, &alice, "stale_chunks", "Alice's notification", false).await;

    let updated = notification::mark_read(&pool, &alice, &id)
        .await
        .unwrap()
        .expect("own notification must be found and updated");
    assert!(updated.read);

    assert_eq!(notification::count_unread(&pool, &alice).await.unwrap(), 0);
}

/// The bulk-write case this task calls out explicitly: `mark_all_read` has
/// no per-row id check, only a `WHERE user_id = $1` filter. Forgetting that
/// filter would silently mark another user's notifications read with no
/// error — a destructive cross-tenant write. This seeds unread
/// notifications for two users, calls `mark_all_read` as Alice, and asserts
/// Bob's are still unread.
#[sqlx::test]
async fn mark_all_read_does_not_touch_another_users_rows(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let bob = user::create(&pool, "c@d.test", "Bob", None)
        .await
        .unwrap()
        .id;

    seed(&pool, &alice, "stale_chunks", "Alice one", false).await;
    seed(&pool, &alice, "stale_chunks", "Alice two", false).await;
    seed(&pool, &bob, "stale_chunks", "Bob one", false).await;
    seed(&pool, &bob, "stale_chunks", "Bob two", false).await;

    notification::mark_all_read(&pool, &alice).await.unwrap();

    assert_eq!(
        notification::count_unread(&pool, &alice).await.unwrap(),
        0,
        "all of Alice's notifications must now be read"
    );
    assert_eq!(
        notification::count_unread(&pool, &bob).await.unwrap(),
        2,
        "Bob's notifications must be untouched by Alice's mark_all_read"
    );
}

#[sqlx::test]
async fn delete_on_another_users_notification_returns_false_and_leaves_it(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;
    let bob = user::create(&pool, "c@d.test", "Bob", None)
        .await
        .unwrap()
        .id;

    let id = seed(&pool, &alice, "stale_chunks", "Alice's notification", false).await;

    assert!(!notification::delete(&pool, &bob, &id).await.unwrap());

    let still_there = notification::list(&pool, &alice, false, 50).await.unwrap();
    assert_eq!(still_there.len(), 1);
    assert_eq!(still_there[0].id, id);
}

#[sqlx::test]
async fn delete_removes_the_callers_own_notification(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;

    let id = seed(&pool, &alice, "stale_chunks", "Alice's notification", false).await;

    assert!(notification::delete(&pool, &alice, &id).await.unwrap());
    assert!(
        notification::list(&pool, &alice, false, 50)
            .await
            .unwrap()
            .is_empty()
    );
}

/// `notification.type` is unconstrained free text in Node (no enum, no
/// check constraint) — see the module doc on `Notification`. This proves
/// the Rust repository round-trips an arbitrary string rather than
/// rejecting or coercing it.
#[sqlx::test]
async fn notification_type_is_free_text(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;

    seed(&pool, &alice, "totally-made-up-type", "Freeform", false).await;

    let listed = notification::list(&pool, &alice, false, 50).await.unwrap();
    assert_eq!(listed[0].notification_type, "totally-made-up-type");
}

/// Same bug class as `chunk::list`, `tag::list`, `tag_type::list`, and
/// `space::list` (see their equivalent tests): `ORDER BY created_at DESC`
/// alone over tied rows is a query-plan artifact, not a stable order.
/// Every notification here shares the exact same `created_at`, so only the
/// `id ASC` tiebreaker can determine order.
#[sqlx::test]
async fn list_breaks_created_at_ties_by_id(pool: sqlx::PgPool) {
    let alice = user::create(&pool, "a@b.test", "Alice", None)
        .await
        .unwrap()
        .id;

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
        seed(&pool, &alice, "stale_chunks", title, false).await;
    }

    sqlx::query!(
        "UPDATE notification SET created_at = now() WHERE user_id = $1",
        alice
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query!("ANALYZE notification")
        .execute(&pool)
        .await
        .unwrap();

    let expected_id_order: Vec<String> = sqlx::query_scalar!(
        "SELECT id FROM notification WHERE user_id = $1 ORDER BY id ASC",
        alice
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(expected_id_order.len(), 20);

    let first = notification::list(&pool, &alice, false, 50).await.unwrap();
    let second = notification::list(&pool, &alice, false, 50).await.unwrap();

    let first_ids: Vec<String> = first.iter().map(|n| n.id.clone()).collect();
    let second_ids: Vec<String> = second.iter().map(|n| n.id.clone()).collect();

    assert_eq!(
        first_ids, second_ids,
        "repeated calls over tied rows must return byte-identical order"
    );
    assert_eq!(
        first_ids, expected_id_order,
        "ties must be broken by ascending id, not left to query-plan chance"
    );
}
