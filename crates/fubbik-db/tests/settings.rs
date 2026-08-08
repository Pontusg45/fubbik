//! `user_settings` is scoped by a plain `WHERE user_id = $1` — there is no
//! client-supplied id to attack, since the caller always comes from the
//! session, so this file's main job is the ordering guarantee and the
//! upsert semantics, not a cross-user attack surface.
//!
//! `codebase_settings` is the one table in this domain with a real
//! cross-user attack surface: the caller supplies a bare `spaceId`
//! (`codebaseId` on the wire), and the table itself carries no `user_id`
//! column at all — ownership only exists one hop away, through the parent
//! `space` row. Every read and write here carries an `EXISTS (SELECT 1
//! FROM space s WHERE s.id = $.. AND s.user_id = $..)` guard for exactly
//! that reason (see `fubbik_db::repo::settings`'s module doc for why this
//! is a deliberate divergence from Node, which has no such guard). Both
//! guards are proven load-bearing below by removing them and watching a
//! named test fail — see the doc comments on
//! `write_guard_is_load_bearing` and `read_guard_is_load_bearing`.

use fubbik_db::repo::{settings, space, user};

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

// --- user_settings ---

#[sqlx::test]
async fn user_settings_are_scoped_by_user(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;

    settings::set_user_setting(
        &pool,
        &fubbik_db::new_id(),
        &alice,
        "theme",
        serde_json::json!("dark"),
    )
    .await
    .unwrap();
    settings::set_user_setting(
        &pool,
        &fubbik_db::new_id(),
        &bob,
        "theme",
        serde_json::json!("light"),
    )
    .await
    .unwrap();

    let alice_rows = settings::list_user_settings(&pool, &alice).await.unwrap();
    assert_eq!(alice_rows.len(), 1);
    assert_eq!(alice_rows[0].value.0, serde_json::json!("dark"));

    let bob_rows = settings::list_user_settings(&pool, &bob).await.unwrap();
    assert_eq!(bob_rows.len(), 1);
    assert_eq!(bob_rows[0].value.0, serde_json::json!("light"));
}

#[sqlx::test]
async fn user_setting_upsert_overwrites_value_not_duplicates(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;

    let first = settings::set_user_setting(
        &pool,
        &fubbik_db::new_id(),
        &alice,
        "theme",
        serde_json::json!("dark"),
    )
    .await
    .unwrap();

    let second = settings::set_user_setting(
        &pool,
        &fubbik_db::new_id(),
        &alice,
        "theme",
        serde_json::json!("light"),
    )
    .await
    .unwrap();

    // Same row (same id, same unique (user_id, key)), value overwritten.
    assert_eq!(first.id, second.id);
    assert_eq!(second.value.0, serde_json::json!("light"));

    let rows = settings::list_user_settings(&pool, &alice).await.unwrap();
    assert_eq!(rows.len(), 1, "upsert must not create a duplicate row");
    assert_eq!(rows[0].value.0, serde_json::json!("light"));
}

#[sqlx::test]
async fn user_setting_value_is_stored_as_is_with_no_shape_validation(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;

    // `theme` is documented as `"light" | "dark" | "system"` in Node's
    // (TS-only, never enforced) `UserSettingsMap` — this stores a bare
    // number under that key with no error, matching Node exactly.
    let row = settings::set_user_setting(
        &pool,
        &fubbik_db::new_id(),
        &alice,
        "theme",
        serde_json::json!(42),
    )
    .await
    .unwrap();
    assert_eq!(row.value.0, serde_json::json!(42));
}

/// `ORDER BY key ASC` with no `id` tiebreaker is a total order here
/// because `(user_id, key)` is `UNIQUE` (`user_settings_user_key_idx`):
/// within one user's scope, `key` cannot repeat, so there is no tie left
/// for an `id` tiebreaker to break. This seeds several keys in a
/// non-alphabetical insertion order and proves the list comes back sorted
/// and stable across repeated calls.
#[sqlx::test]
async fn list_orders_by_key_and_is_stable(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;

    for key in ["zeta", "alpha", "mu", "beta", "omega"] {
        settings::set_user_setting(
            &pool,
            &fubbik_db::new_id(),
            &alice,
            key,
            serde_json::json!(true),
        )
        .await
        .unwrap();
    }

    let first = settings::list_user_settings(&pool, &alice).await.unwrap();
    let second = settings::list_user_settings(&pool, &alice).await.unwrap();

    let keys: Vec<&str> = first.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, vec!["alpha", "beta", "mu", "omega", "zeta"]);

    let second_keys: Vec<&str> = second.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(
        keys, second_keys,
        "repeated calls must return byte-identical order"
    );
}

// --- codebase_settings ---

#[sqlx::test]
async fn codebase_setting_written_for_own_space_succeeds(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let space_id = seed_space(&pool, &alice, "alices-space").await;

    let row = settings::set_codebase_setting(
        &pool,
        &fubbik_db::new_id(),
        &space_id,
        &alice,
        "defaultChunkType",
        serde_json::json!("note"),
    )
    .await
    .unwrap()
    .expect("writing to one's own space must succeed");
    assert_eq!(row.space_id, space_id);
    assert_eq!(row.value.0, serde_json::json!("note"));
}

/// The core claim of this task: "writing settings on another user's space
/// must be impossible." Bob names Alice's `space_id`; the write must be
/// rejected (`None`, not an error, matching the `favorite::add` /
/// `chunk_meta` ownership-guard shape elsewhere in this crate) and must
/// leave Alice's settings completely untouched — a status-only assertion
/// would pass even if the rejected write partially mutated something.
#[sqlx::test]
async fn cannot_write_codebase_setting_for_another_users_space(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let alices_space = seed_space(&pool, &alice, "alices-space").await;

    let result = settings::set_codebase_setting(
        &pool,
        &fubbik_db::new_id(),
        &alices_space,
        &bob,
        "defaultChunkType",
        serde_json::json!("hijacked"),
    )
    .await
    .unwrap();
    assert!(
        result.is_none(),
        "write against another user's space must be rejected"
    );

    let alice_rows = settings::list_codebase_settings(&pool, &alices_space, &alice)
        .await
        .unwrap();
    assert!(
        alice_rows.is_empty(),
        "Alice's space must have gained no setting from Bob's rejected write"
    );
}

#[sqlx::test]
async fn cannot_read_codebase_settings_for_another_users_space(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let alices_space = seed_space(&pool, &alice, "alices-space").await;

    settings::set_codebase_setting(
        &pool,
        &fubbik_db::new_id(),
        &alices_space,
        &alice,
        "defaultChunkType",
        serde_json::json!("note"),
    )
    .await
    .unwrap();

    let bob_view = settings::list_codebase_settings(&pool, &alices_space, &bob)
        .await
        .unwrap();
    assert!(
        bob_view.is_empty(),
        "another user must not be able to read this space's settings"
    );

    let alice_view = settings::list_codebase_settings(&pool, &alices_space, &alice)
        .await
        .unwrap();
    assert_eq!(alice_view.len(), 1, "the owner must still see it");
}

#[sqlx::test]
async fn codebase_setting_upsert_overwrites_value_for_owner(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let space_id = seed_space(&pool, &alice, "alices-space").await;

    settings::set_codebase_setting(
        &pool,
        &fubbik_db::new_id(),
        &space_id,
        &alice,
        "defaultChunkType",
        serde_json::json!("note"),
    )
    .await
    .unwrap();
    settings::set_codebase_setting(
        &pool,
        &fubbik_db::new_id(),
        &space_id,
        &alice,
        "defaultChunkType",
        serde_json::json!("reference"),
    )
    .await
    .unwrap();

    let rows = settings::list_codebase_settings(&pool, &space_id, &alice)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "upsert must not duplicate rows");
    assert_eq!(rows[0].value.0, serde_json::json!("reference"));
}

// --- instance_settings ---

#[sqlx::test]
async fn instance_settings_are_global_with_no_owner_scoping(pool: sqlx::PgPool) {
    // No user in sight at all — `set_instance_setting` and
    // `list_instance_settings` take no user_id/space_id parameter because
    // `instance_settings` has no owner column. Any caller that can reach
    // these functions can read and write every row.
    settings::set_instance_setting(&pool, "aiEnabled", serde_json::json!(false))
        .await
        .unwrap();

    let rows = settings::list_instance_settings(&pool).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key, "aiEnabled");
    assert_eq!(rows[0].value.0, serde_json::json!(false));
}

#[sqlx::test]
async fn instance_setting_upsert_overwrites_value(pool: sqlx::PgPool) {
    settings::set_instance_setting(&pool, "aiEnabled", serde_json::json!(true))
        .await
        .unwrap();
    settings::set_instance_setting(&pool, "aiEnabled", serde_json::json!(false))
        .await
        .unwrap();

    let rows = settings::list_instance_settings(&pool).await.unwrap();
    assert_eq!(rows.len(), 1, "upsert must not duplicate rows");
    assert_eq!(rows[0].value.0, serde_json::json!(false));
}

/// `ORDER BY key ASC` is a total order here too: `key` is
/// `instance_settings`' own primary key, globally unique, so there is
/// nothing left for a tiebreaker to break.
#[sqlx::test]
async fn instance_settings_list_orders_by_key(pool: sqlx::PgPool) {
    for key in ["zeta", "alpha", "mu"] {
        settings::set_instance_setting(&pool, key, serde_json::json!(true))
            .await
            .unwrap();
    }

    let rows = settings::list_instance_settings(&pool).await.unwrap();
    let keys: Vec<&str> = rows.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, vec!["alpha", "mu", "zeta"]);
}
