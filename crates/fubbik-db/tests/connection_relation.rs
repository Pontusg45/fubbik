//! Repo-level tests for the `connection_relation` catalog.
//!
//! Same guard shape as `chunk_type.rs` — `list` scopes with `user_id = $1`,
//! and `update`/`delete` filter `WHERE id = $1 AND user_id = $2` and
//! nothing else, matching Node
//! (`packages/db/src/repository/vocabulary-catalog.ts:167,177`). Each guard
//! was verified by deleting it and watching a named test here go red:
//!
//! | guard removed | test that failed |
//! |---|---|
//! | `list`'s `user_id = $1` | `list_includes_builtin_and_own_but_excludes_other_users` |
//! | `update`'s `AND user_id = $2` | `update_is_scoped_to_owner` |
//! | `delete`'s `AND user_id = $2` | `delete_is_scoped_to_owner` |
//!
//! See `chunk_type.rs`'s module doc for why the HTTP-level cross-user tests
//! happen to catch the mutation guards too here (the service's unscoped
//! pre-check does not short-circuit a cross-user attempt, because the row
//! really does exist).
//!
//! Deliberately a separate file from `chunk_type.rs` because the two
//! catalogs are backed by two separate tables (`migrations/0001_init.sql:283`
//! and `:353`), not one table with a discriminator column.

use fubbik_db::repo::connection_relation::{self, ConnectionRelationPatch, NewConnectionRelation};
use fubbik_db::repo::user;

async fn seed_user(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "U", None).await.unwrap().id
}

fn bare(id: &str, label: &str) -> NewConnectionRelation {
    NewConnectionRelation {
        id: id.into(),
        label: label.into(),
        description: None,
        arrow_style: None,
        direction: None,
        color: None,
        inverse_of_id: None,
        display_order: None,
    }
}

#[sqlx::test]
async fn create_and_find_by_id_round_trip(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;

    // When
    let created = connection_relation::create(
        &pool,
        &uid,
        NewConnectionRelation {
            id: "mirrors".into(),
            label: "Mirrors".into(),
            description: Some("Two views of one thing".into()),
            arrow_style: Some("dotted".into()),
            direction: Some("bidirectional".into()),
            color: Some("#abcdef".into()),
            // A seeded built-in is a valid FK target for the self-reference.
            inverse_of_id: Some("related_to".into()),
            display_order: Some(9),
        },
    )
    .await
    .unwrap();

    // Then
    assert_eq!(created.id, "mirrors");
    assert_eq!(created.arrow_style, "dotted");
    assert_eq!(created.direction, "bidirectional");
    assert_eq!(created.color, "#abcdef");
    assert_eq!(created.inverse_of_id.as_deref(), Some("related_to"));
    assert_eq!(created.display_order, 9);
    assert!(!created.built_in);
    assert_eq!(created.user_id.as_deref(), Some(uid.as_str()));
    assert_eq!(created.space_id, None);

    let found = connection_relation::find_by_id(&pool, "mirrors")
        .await
        .unwrap()
        .expect("must round-trip");
    assert_eq!(found.label, "Mirrors");
}

/// Node's `??` defaults (`packages/db/src/repository/vocabulary-catalog.ts:133-137`).
/// `display_order` is again the interesting one — the column's own default
/// is `100`, Node passes `500`.
#[sqlx::test]
async fn create_applies_nodes_defaults_not_the_column_defaults(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    // When
    let created = connection_relation::create(&pool, &uid, bare("plain", "Plain"))
        .await
        .unwrap();

    // Then
    assert_eq!(created.arrow_style, "solid");
    assert_eq!(created.direction, "forward");
    assert_eq!(created.color, "#64748b");
    assert_eq!(created.display_order, 500);
    assert_eq!(created.inverse_of_id, None);
    assert_eq!(created.description, None);
}

#[sqlx::test]
async fn list_includes_builtin_and_own_but_excludes_other_users(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;

    connection_relation::create(&pool, &alice, bare("alices", "Alice's"))
        .await
        .unwrap();
    // When
    connection_relation::create(&pool, &bob, bare("bobs", "Bob's"))
        .await
        .unwrap();

    let ids: Vec<String> = connection_relation::list(&pool, &alice, None)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.id)
        .collect();
    // Then
    assert!(ids.contains(&"alices".to_string()));
    assert!(
        ids.contains(&"depends_on".to_string()),
        "must see the 13 seeded built-in relations"
    );
    assert!(!ids.contains(&"bobs".to_string()));
}

/// Same `OR`-not-`AND` behaviour as `chunk_type::list` — pinned here too so
/// both halves of the flagged Node quirk are covered.
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

    // When
    sqlx::query!(
        r#"INSERT INTO connection_relation (id, label, user_id, space_id)
           VALUES ('bobs_spaced', 'Bob''s', $1, $2)"#,
        bob,
        space_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let without: Vec<String> = connection_relation::list(&pool, &alice, None)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.id)
        .collect();
    // Then
    assert!(!without.contains(&"bobs_spaced".to_string()));

    let with: Vec<String> = connection_relation::list(&pool, &alice, Some(&space_id))
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.id)
        .collect();
    assert!(
        with.contains(&"bobs_spaced".to_string()),
        "Node's OR pulls the space-scoped row in regardless of owner"
    );
    assert!(with.len() > without.len());
}

/// Forces a genuine `display_order` tie so `id ASC` is what is actually
/// under test. The seeded built-ins all use distinct orders, so without
/// these rows the tiebreaker would never be reached.
#[sqlx::test]
async fn list_breaks_display_order_ties_by_id(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    for id in ["zeta", "mu", "alpha", "sigma", "beta"] {
        // When
        connection_relation::create(
            &pool,
            &uid,
            NewConnectionRelation {
                display_order: Some(42),
                ..bare(id, id)
            },
        )
        .await
        .unwrap();
    }

    let tied: Vec<String> = connection_relation::list(&pool, &uid, None)
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.display_order == 42)
        .map(|r| r.id)
        .collect();
    // Then
    assert_eq!(tied, vec!["alpha", "beta", "mu", "sigma", "zeta"]);

    let orders: Vec<i32> = connection_relation::list(&pool, &uid, None)
        .await
        .unwrap()
        .iter()
        .map(|r| r.display_order)
        .collect();
    let mut sorted = orders.clone();
    sorted.sort_unstable();
    assert_eq!(orders, sorted, "display_order must be the primary key");
}

/// `update`'s `WHERE user_id = $2` guard — removing it turns this red.
#[sqlx::test]
async fn update_is_scoped_to_owner(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    connection_relation::create(
        &pool,
        &alice,
        NewConnectionRelation {
            description: Some("original".into()),
            ..bare("alices", "Alice's")
        },
    )
    .await
    .unwrap();

    // When
    let result = connection_relation::update(
        &pool,
        &bob,
        "alices",
        ConnectionRelationPatch {
            label: Some("hijacked".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    // Then
    assert!(result.is_none(), "Bob must not update Alice's relation");

    let untouched = connection_relation::find_by_id(&pool, "alices")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(untouched.label, "Alice's");
    assert_eq!(untouched.description.as_deref(), Some("original"));
}

#[sqlx::test]
async fn update_cannot_touch_a_seeded_builtin_even_at_the_repo_layer(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    // When
    let result = connection_relation::update(
        &pool,
        &uid,
        "depends_on",
        ConnectionRelationPatch {
            label: Some("hijacked".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    // Then
    assert!(result.is_none());
    assert_eq!(
        connection_relation::find_by_id(&pool, "depends_on")
            .await
            .unwrap()
            .unwrap()
            .label,
        "Depends on"
    );
}

/// Tri-state on both `description` and `inverse_of_id`, plus `updated_at`
/// moving.
#[sqlx::test]
async fn update_owner_succeeds_with_tri_state_and_touches_updated_at(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let created = connection_relation::create(
        &pool,
        &uid,
        NewConnectionRelation {
            description: Some("desc".into()),
            inverse_of_id: Some("related_to".into()),
            display_order: Some(3),
            ..bare("mine", "Mine")
        },
    )
    .await
    .unwrap();

    // When
    let updated = connection_relation::update(
        &pool,
        &uid,
        "mine",
        ConnectionRelationPatch {
            label: Some("Renamed".into()),
            description: Some(None),
            arrow_style: Some("dashed".into()),
            direction: None,
            color: None,
            inverse_of_id: None,
            display_order: None,
        },
    )
    .await
    .unwrap()
    .expect("owner update must succeed");

    // Then
    assert_eq!(updated.label, "Renamed");
    assert_eq!(updated.description, None, "explicit null must clear");
    assert_eq!(updated.arrow_style, "dashed");
    assert_eq!(updated.direction, "forward", "omitted stays untouched");
    assert_eq!(
        updated.inverse_of_id.as_deref(),
        Some("related_to"),
        "omitted stays untouched"
    );
    assert_eq!(updated.display_order, 3);
    assert_eq!(updated.created_at, created.created_at);
    assert!(updated.updated_at >= created.updated_at);

    // And an explicit null on inverse_of_id does clear it.
    let cleared = connection_relation::update(
        &pool,
        &uid,
        "mine",
        ConnectionRelationPatch {
            inverse_of_id: Some(None),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(cleared.inverse_of_id, None);
}

#[sqlx::test]
async fn delete_is_scoped_to_owner(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    let bob = seed_user(&pool, "bob@b.test").await;
    connection_relation::create(&pool, &alice, bare("alices", "Alice's"))
        .await
        .unwrap();

    // When
    let deleted = connection_relation::delete(&pool, &bob, "alices")
        .await
        .unwrap();
    // Then
    assert!(!deleted, "Bob must not delete Alice's relation");
    assert!(
        connection_relation::find_by_id(&pool, "alices")
            .await
            .unwrap()
            .is_some()
    );
}

/// Documents the absence of an `AND built_in = false` clause, matching Node
/// (`packages/db/src/repository/vocabulary-catalog.ts:173-181`) — see the
/// equivalent test in `chunk_type.rs` for the full reasoning.
#[sqlx::test]
async fn delete_has_no_built_in_clause_matching_node(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    // When
    sqlx::query!(
        r#"INSERT INTO connection_relation (id, label, built_in, user_id)
           VALUES ('adversarial', 'A', true, $1)"#,
        alice
    )
    .execute(&pool)
    .await
    .unwrap();

    // Then
    assert!(
        connection_relation::delete(&pool, &alice, "adversarial")
            .await
            .unwrap(),
        "Node's DELETE filters on user_id only; the 400 protecting real \
         built-ins lives in the service layer"
    );
}

#[sqlx::test]
async fn delete_removes_owned_row(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    // When
    connection_relation::create(&pool, &uid, bare("disposable", "Disposable"))
        .await
        .unwrap();

    // Then
    assert!(
        connection_relation::delete(&pool, &uid, "disposable")
            .await
            .unwrap()
    );
    assert!(
        connection_relation::find_by_id(&pool, "disposable")
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test]
async fn find_by_id_is_unscoped_by_design(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "alice@b.test").await;
    // When
    connection_relation::create(&pool, &alice, bare("alices", "Alice's"))
        .await
        .unwrap();

    // Then
    // No user_id parameter to pass at all -- that is the point.
    assert!(
        connection_relation::find_by_id(&pool, "alices")
            .await
            .unwrap()
            .is_some()
    );
}
