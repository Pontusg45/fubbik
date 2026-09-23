//! Repo-level tests for the `chunk_type` catalog.
//!
//! These cover the SQL-level guards directly. `list` scopes with
//! `user_id = $1`; `update`/`delete` both filter
//! `WHERE id = $1 AND user_id = $2`. Each was verified by deleting it and
//! watching a named test below go red:
//!
//! | guard removed | test that failed |
//! |---|---|
//! | `list`'s `user_id = $1` | `list_includes_builtin_and_own_but_excludes_other_users` ("must not leak another user's row") |
//! | `update`'s `AND user_id = $2` | `update_is_scoped_to_owner` |
//! | `delete`'s `AND user_id = $2` | `delete_is_scoped_to_owner` |
//!
//! Worth recording, because it differs from the `templates` domain's
//! situation: the `update`/`delete` guards here are *also* caught at the
//! HTTP layer (`fubbik-api/tests/vocabularies.rs::chunk_type_cross_user_
//! mutations_are_404_and_leave_the_row_alone` flips 404 -> 200 when the SQL
//! guard is dropped). The service's unscoped `find_by_id` pre-check does
//! **not** mask it, because in a cross-user attempt the row genuinely
//! exists — the pre-check passes and the request reaches SQL. These repo
//! tests are still the sharper instrument: they observe the guard directly
//! rather than through a status code.
//!
//! The `built_in` rejection is a *service*-layer check in Node (a 400), and
//! is tested at the HTTP level in `fubbik-api/tests/vocabularies.rs`.

use fubbik_db::repo::chunk_type::{self, ChunkTypePatch, NewChunkType};
use fubbik_db::repo::user;

async fn seed_user(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "U", None).await.unwrap().id
}

fn bare(id: &str, label: &str) -> NewChunkType {
    NewChunkType {
        id: id.into(),
        label: label.into(),
        description: None,
        icon: None,
        color: None,
        examples: None,
        display_order: None,
    }
}

#[sqlx::test]
async fn create_and_find_by_id_round_trip(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;

    // When
    let created = chunk_type::create(
        &pool,
        &uid,
        NewChunkType {
            id: "runbook".into(),
            label: "Runbook".into(),
            description: Some("Operational steps".into()),
            icon: Some("BookOpen".into()),
            color: Some("#123456".into()),
            examples: Some(vec!["Restart".into(), "Rollback".into()]),
            display_order: Some(7),
        },
    )
    .await
    .unwrap();

    // Then
    assert_eq!(created.id, "runbook");
    assert_eq!(created.label, "Runbook");
    assert_eq!(created.description.as_deref(), Some("Operational steps"));
    assert_eq!(created.icon.as_deref(), Some("BookOpen"));
    assert_eq!(created.color, "#123456");
    assert_eq!(created.examples.0, vec!["Restart", "Rollback"]);
    assert_eq!(created.display_order, 7);
    assert!(
        !created.built_in,
        "create must never produce a built-in row"
    );
    assert_eq!(created.user_id.as_deref(), Some(uid.as_str()));
    assert_eq!(
        created.space_id, None,
        "no route body carries spaceId, so created rows are always global"
    );

    let found = chunk_type::find_by_id(&pool, "runbook")
        .await
        .unwrap()
        .expect("must round-trip");
    assert_eq!(found.label, "Runbook");
}

/// The three `??` defaults Node's `createChunkType` applies
/// (`packages/db/src/repository/vocabulary-catalog.ts:65-67`). `500` is the
/// interesting one: the *column* default is `100`
/// (`migrations/0001_init.sql:290`), so deferring to Postgres here would
/// silently produce a different value than Node.
#[sqlx::test]
async fn create_applies_nodes_defaults_not_the_column_defaults(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    // When
    let created = chunk_type::create(&pool, &uid, bare("plain", "Plain"))
        .await
        .unwrap();

    // Then
    assert_eq!(created.color, "#8b5cf6");
    assert!(created.examples.0.is_empty());
    assert_eq!(
        created.display_order, 500,
        "Node passes 500 explicitly; the column default is 100"
    );
    assert_eq!(created.description, None);
    assert_eq!(created.icon, None);
}

/// Both halves of `WHERE built_in = true OR user_id = $1` are load-bearing:
/// dropping either breaks one of the assertions below. The seven built-ins
/// come from `migrations/0002_seed_reference_data.sql`.
#[sqlx::test]
async fn list_includes_builtin_and_own_but_excludes_other_users(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;

    chunk_type::create(&pool, &alice, bare("alices", "Alice's"))
        .await
        .unwrap();
    // When
    chunk_type::create(&pool, &bob, bare("bobs", "Bob's"))
        .await
        .unwrap();

    let ids: Vec<String> = chunk_type::list(&pool, &alice, None)
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.id)
        .collect();
    // Then
    assert!(ids.contains(&"alices".to_string()), "must see own row");
    assert!(
        ids.contains(&"convention".to_string()),
        "must see seeded built-ins"
    );
    assert!(
        !ids.contains(&"bobs".to_string()),
        "must not leak another user's row"
    );
}

/// Node's `spaceId` condition is `OR`-ed, not `AND`-ed
/// (`packages/db/src/repository/vocabulary-catalog.ts:18-25`), so passing
/// it *adds* space-scoped rows to the caller's own rather than restricting
/// to them — including rows another user created against that space. This
/// pins that behaviour so a future "fix" toward `AND` is a deliberate,
/// visible change rather than a silent one.
#[sqlx::test]
async fn list_space_id_widens_rather_than_filters(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    let space_id = fubbik_db::new_id();
    sqlx::query!(
        r#"INSERT INTO space (id, name, kind, user_id) VALUES ($1, 'S', 'code', $2)"#,
        space_id,
        alice
    )
    .execute(&pool)
    .await
    .unwrap();

    chunk_type::create(&pool, &alice, bare("alices", "Alice's"))
        .await
        .unwrap();
    // When
    // A space-scoped row owned by Bob — nothing in this API can create one,
    // so it is inserted directly.
    sqlx::query!(
        r#"INSERT INTO chunk_type (id, label, user_id, space_id) VALUES ('bobs_spaced', 'Bob''s', $1, $2)"#,
        bob,
        space_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let without: Vec<String> = chunk_type::list(&pool, &alice, None)
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.id)
        .collect();
    // Then
    assert!(!without.contains(&"bobs_spaced".to_string()));

    let with: Vec<String> = chunk_type::list(&pool, &alice, Some(&space_id))
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.id)
        .collect();
    assert!(
        with.contains(&"alices".to_string()),
        "own rows must still be present — spaceId widens, it does not filter"
    );
    assert!(
        with.contains(&"bobs_spaced".to_string()),
        "Node's OR pulls in the space-scoped row regardless of who owns it"
    );
    assert!(with.len() > without.len(), "the result set only ever grows");
}

/// `ORDER BY display_order ASC, id ASC` is Node's own ordering. This forces
/// a genuine `display_order` tie so the `id ASC` tiebreaker is what is
/// actually being exercised — with distinct orders the test would pass even
/// if the tiebreaker were missing.
#[sqlx::test]
async fn list_breaks_display_order_ties_by_id(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    // Inserted in deliberately non-alphabetical order, all sharing order 42.
    for id in ["zebra", "mango", "apple", "quince", "banana"] {
        // When
        chunk_type::create(
            &pool,
            &uid,
            NewChunkType {
                display_order: Some(42),
                ..bare(id, id)
            },
        )
        .await
        .unwrap();
    }

    let tied: Vec<String> = chunk_type::list(&pool, &uid, None)
        .await
        .unwrap()
        .into_iter()
        .filter(|t| t.display_order == 42)
        .map(|t| t.id)
        .collect();
    // Then
    assert_eq!(
        tied,
        vec!["apple", "banana", "mango", "quince", "zebra"],
        "identical display_order must fall back to id ASC"
    );

    // And the primary sort key still dominates the tiebreaker.
    let all = chunk_type::list(&pool, &uid, None).await.unwrap();
    let orders: Vec<i32> = all.iter().map(|t| t.display_order).collect();
    let mut sorted = orders.clone();
    sorted.sort_unstable();
    assert_eq!(orders, sorted, "display_order must be the primary key");
}

/// `update`'s `WHERE user_id = $2` guard. Removing that clause makes this
/// test red at the repo layer — an API test cannot catch it, because the
/// service's unscoped `find_by_id` pre-check 404s first either way.
#[sqlx::test]
async fn update_is_scoped_to_owner(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    chunk_type::create(
        &pool,
        &alice,
        NewChunkType {
            description: Some("original".into()),
            ..bare("alices", "Alice's")
        },
    )
    .await
    .unwrap();

    // When
    let result = chunk_type::update(
        &pool,
        &bob,
        "alices",
        ChunkTypePatch {
            label: Some("hijacked".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    // Then
    assert!(result.is_none(), "Bob must not update Alice's chunk type");

    let untouched = chunk_type::find_by_id(&pool, "alices")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(untouched.label, "Alice's");
    assert_eq!(untouched.description.as_deref(), Some("original"));
}

/// The `user_id` guard also protects seeded built-ins for free: every
/// built-in row has `user_id IS NULL`, which no caller's id can equal.
#[sqlx::test]
async fn update_cannot_touch_a_seeded_builtin_even_at_the_repo_layer(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    // When
    let result = chunk_type::update(
        &pool,
        &uid,
        "convention",
        ChunkTypePatch {
            label: Some("hijacked".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    // Then
    assert!(result.is_none());

    let still = chunk_type::find_by_id(&pool, "convention")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(still.label, "Convention");
}

/// Tri-state patch semantics: `Some(None)` clears, `None` leaves untouched,
/// `Some(Some(v))` sets — plus `updated_at` moving while `created_at` does
/// not.
#[sqlx::test]
async fn update_owner_succeeds_with_tri_state_and_touches_updated_at(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let created = chunk_type::create(
        &pool,
        &uid,
        NewChunkType {
            description: Some("desc".into()),
            icon: Some("Icon".into()),
            examples: Some(vec!["one".into()]),
            display_order: Some(3),
            ..bare("mine", "Mine")
        },
    )
    .await
    .unwrap();

    // When
    let updated = chunk_type::update(
        &pool,
        &uid,
        "mine",
        ChunkTypePatch {
            label: Some("Renamed".into()),
            description: Some(None),
            icon: None,
            color: None,
            examples: Some(vec!["two".into(), "three".into()]),
            display_order: None,
        },
    )
    .await
    .unwrap()
    .expect("owner update must succeed");

    // Then
    assert_eq!(updated.label, "Renamed");
    assert_eq!(updated.description, None, "explicit null must clear");
    assert_eq!(
        updated.icon.as_deref(),
        Some("Icon"),
        "omitted field must stay untouched"
    );
    assert_eq!(updated.examples.0, vec!["two", "three"]);
    assert_eq!(updated.display_order, 3, "omitted field stays untouched");
    assert_eq!(updated.color, "#8b5cf6");
    assert_eq!(updated.created_at, created.created_at);
    assert!(
        updated.updated_at >= created.updated_at,
        "Drizzle's $onUpdate bumps updated_at on every .set()"
    );
}

/// `delete`'s `WHERE user_id = $2` guard, same shape as the update proof.
#[sqlx::test]
async fn delete_is_scoped_to_owner(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    chunk_type::create(&pool, &alice, bare("alices", "Alice's"))
        .await
        .unwrap();

    // When
    let deleted = chunk_type::delete(&pool, &bob, "alices").await.unwrap();
    // Then
    assert!(!deleted, "Bob must not delete Alice's chunk type");
    assert!(
        chunk_type::find_by_id(&pool, "alices")
            .await
            .unwrap()
            .is_some()
    );
}

/// Deliberately documents that this port has **no** `AND built_in = false`
/// in the `DELETE`, because Node has none either
/// (`packages/db/src/repository/vocabulary-catalog.ts:102-110`) — unlike
/// `template::delete`, which does. The row constructed here (`built_in =
/// true` *and* a real `user_id`) is a state the app cannot itself produce;
/// it exists only to make the absence of that clause observable, so that
/// adding one later shows up as a deliberate divergence from Node rather
/// than an invisible one.
#[sqlx::test]
async fn delete_has_no_built_in_clause_matching_node(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    sqlx::query!(
        r#"INSERT INTO chunk_type (id, label, built_in, user_id) VALUES ('adversarial', 'A', true, $1)"#,
        alice
    )
    .execute(&pool)
    .await
    .unwrap();

    // When
    let deleted = chunk_type::delete(&pool, &alice, "adversarial")
        .await
        .unwrap();
    // Then
    assert!(
        deleted,
        "Node's DELETE filters on user_id only — a built_in row with a matching \
         user_id is deletable at the repo layer. The 400 that protects real \
         built-ins comes from the service layer (tested in fubbik-api)."
    );
}

#[sqlx::test]
async fn delete_removes_owned_row(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    // When
    chunk_type::create(&pool, &uid, bare("disposable", "Disposable"))
        .await
        .unwrap();

    // Then
    assert!(chunk_type::delete(&pool, &uid, "disposable").await.unwrap());
    assert!(
        chunk_type::find_by_id(&pool, "disposable")
            .await
            .unwrap()
            .is_none()
    );
}

/// `find_by_id` is unscoped by design, matching Node's
/// `findChunkTypeById` — which is why the ownership guard has to live in
/// `update`/`delete`'s own `WHERE`.
#[sqlx::test]
async fn find_by_id_is_unscoped_by_design(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    // When
    chunk_type::create(&pool, &alice, bare("alices", "Alice's"))
        .await
        .unwrap();

    // Then
    // No user_id parameter to pass at all -- that is the point.
    assert!(
        chunk_type::find_by_id(&pool, "alices")
            .await
            .unwrap()
            .is_some()
    );
}
