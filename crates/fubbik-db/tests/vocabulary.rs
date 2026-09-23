//! Repository-level tests for `fubbik_db::repo::vocabulary`. Covers CRUD,
//! the auto-seed-on-first-entry behaviour, and — the highest-severity
//! cases in this file — that every function's `EXISTS`-based space
//! ownership guard is independently load-bearing at the SQL layer, proven
//! by calling the repo function directly with the *wrong* `user_id` (i.e.
//! simulating the service-layer `verify_space_ownership` pre-check having
//! been skipped or removed entirely).

use fubbik_db::repo::vocabulary::{
    self, NewVocabularyEntry, NewVocabularyEntryItem, VocabularyPatch,
};
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

#[sqlx::test]
async fn create_entry_lowercases_word_and_round_trips(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let sid = seed_space(&pool, &uid, "Space").await;

    // When
    let created = vocabulary::create_entry(
        &pool,
        &uid,
        NewVocabularyEntry {
            id: fubbik_db::new_id(),
            word: "ClICk".into(),
            category: "action".into(),
            expects: Some(vec!["target".into()]),
            space_id: sid.clone(),
        },
    )
    .await
    .unwrap()
    .expect("owned space must insert");

    // Then
    assert_eq!(created.word, "click");
    assert_eq!(created.category, "action");
    assert_eq!(created.expects.unwrap().0, vec!["target".to_string()]);
    assert_eq!(created.space_id, sid);
    assert_eq!(
        created.definition, None,
        "definition is never set by this domain"
    );
}

#[sqlx::test]
async fn create_entry_in_a_foreign_space_inserts_nothing(pool: sqlx::PgPool) {
    // Given
    let owner = seed_user(&pool, "owner@b.test").await;
    let attacker = seed_user(&pool, "attacker@b.test").await;
    let sid = seed_space(&pool, &owner, "Owner space").await;

    // When
    let result = vocabulary::create_entry(
        &pool,
        &attacker,
        NewVocabularyEntry {
            id: fubbik_db::new_id(),
            word: "click".into(),
            category: "action".into(),
            expects: None,
            space_id: sid.clone(),
        },
    )
    .await
    .unwrap();

    // Then
    assert!(
        result.is_none(),
        "guard: create_entry must not insert into a space the caller does not own"
    );
    let entries = vocabulary::list(&pool, &owner, &sid).await.unwrap();
    assert!(
        entries.is_empty(),
        "attacker's insert attempt must not have landed"
    );
}

#[sqlx::test]
async fn count_and_seed_modifiers(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let sid = seed_space(&pool, &uid, "Space").await;

    // When the operation is evaluated by the assertion.
    // Then
    assert_eq!(vocabulary::count(&pool, &uid, &sid).await.unwrap(), 0);

    let seeded = vocabulary::seed_modifiers(&pool, &uid, &sid).await.unwrap();
    assert_eq!(
        seeded.len(),
        16,
        "all 16 standard modifiers should be seeded once"
    );
    assert!(seeded.iter().all(|e| e.category == "modifier"));

    assert_eq!(vocabulary::count(&pool, &uid, &sid).await.unwrap(), 16);

    // Re-seeding is idempotent: ON CONFLICT DO NOTHING means a second call
    // inserts (and returns) nothing new.
    let reseeded = vocabulary::seed_modifiers(&pool, &uid, &sid).await.unwrap();
    assert!(reseeded.is_empty());
    assert_eq!(vocabulary::count(&pool, &uid, &sid).await.unwrap(), 16);
}

#[sqlx::test]
async fn seed_modifiers_in_a_foreign_space_inserts_nothing(pool: sqlx::PgPool) {
    // Given
    let owner = seed_user(&pool, "owner@b.test").await;
    let attacker = seed_user(&pool, "attacker@b.test").await;
    let sid = seed_space(&pool, &owner, "Owner space").await;

    let seeded = vocabulary::seed_modifiers(&pool, &attacker, &sid)
        .await
        .unwrap();
    // When the operation is evaluated by the assertion.
    // Then
    assert!(
        seeded.is_empty(),
        "guard: seed_modifiers must not touch a space the caller does not own"
    );
    assert_eq!(vocabulary::count(&pool, &owner, &sid).await.unwrap(), 0);
}

#[sqlx::test]
async fn create_entries_bulk_skips_conflicts_and_returns_only_inserted_rows(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let sid = seed_space(&pool, &uid, "Space").await;

    vocabulary::create_entry(
        &pool,
        &uid,
        NewVocabularyEntry {
            id: fubbik_db::new_id(),
            word: "click".into(),
            category: "action".into(),
            expects: None,
            space_id: sid.clone(),
        },
    )
    .await
    .unwrap()
    .unwrap();

    // When
    let created = vocabulary::create_entries(
        &pool,
        &uid,
        &sid,
        vec![
            NewVocabularyEntryItem {
                id: fubbik_db::new_id(),
                word: "click".into(), // conflicts with the existing (space, category, lower(word)) row
                category: "action".into(),
                expects: None,
            },
            NewVocabularyEntryItem {
                id: fubbik_db::new_id(),
                word: "Button".into(),
                category: "target".into(),
                expects: None,
            },
        ],
    )
    .await
    .unwrap();

    // Then
    assert_eq!(
        created.len(),
        1,
        "the conflicting row must be silently skipped"
    );
    assert_eq!(created[0].word, "button");
}

#[sqlx::test]
async fn create_entries_empty_input_short_circuits(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let sid = seed_space(&pool, &uid, "Space").await;
    // When
    let created = vocabulary::create_entries(&pool, &uid, &sid, vec![])
        .await
        .unwrap();
    // Then
    assert!(created.is_empty());
}

#[sqlx::test]
async fn create_entries_in_a_foreign_space_inserts_nothing(pool: sqlx::PgPool) {
    // Given
    let owner = seed_user(&pool, "owner@b.test").await;
    let attacker = seed_user(&pool, "attacker@b.test").await;
    let sid = seed_space(&pool, &owner, "Owner space").await;

    // When
    let created = vocabulary::create_entries(
        &pool,
        &attacker,
        &sid,
        vec![NewVocabularyEntryItem {
            id: fubbik_db::new_id(),
            word: "click".into(),
            category: "action".into(),
            expects: None,
        }],
    )
    .await
    .unwrap();

    // Then
    assert!(
        created.is_empty(),
        "guard: create_entries must not insert into a space the caller does not own"
    );
    assert!(
        vocabulary::list(&pool, &owner, &sid)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test]
async fn list_returns_empty_for_a_space_the_caller_does_not_own(pool: sqlx::PgPool) {
    // Given
    let owner = seed_user(&pool, "owner@b.test").await;
    let attacker = seed_user(&pool, "attacker@b.test").await;
    let sid = seed_space(&pool, &owner, "Owner space").await;

    vocabulary::create_entry(
        &pool,
        &owner,
        NewVocabularyEntry {
            id: fubbik_db::new_id(),
            word: "click".into(),
            category: "action".into(),
            expects: None,
            space_id: sid.clone(),
        },
    )
    .await
    .unwrap()
    .unwrap();

    // When
    // Direct repo call with the attacker's user_id — this is what the
    // service layer's `verify_space_ownership` pre-check exists to
    // prevent from ever running with real data behind it. Proving this
    // returns empty (not the owner's entry) shows the `EXISTS` guard is a
    // second, independent line of defense, not decorative.
    let leaked = vocabulary::list(&pool, &attacker, &sid).await.unwrap();
    // Then
    assert!(
        leaked.is_empty(),
        "guard: list must not leak another user's space's vocabulary"
    );

    let owners_view = vocabulary::list(&pool, &owner, &sid).await.unwrap();
    assert_eq!(owners_view.len(), 1);
}

#[sqlx::test]
async fn list_orders_by_category_then_word_then_id(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let sid = seed_space(&pool, &uid, "Space").await;

    for (word, category) in [
        ("zebra", "target"),
        ("apple", "target"),
        ("submit", "action"),
        ("click", "action"),
    ] {
        vocabulary::create_entry(
            &pool,
            &uid,
            NewVocabularyEntry {
                id: fubbik_db::new_id(),
                word: word.into(),
                category: category.into(),
                expects: None,
                space_id: sid.clone(),
            },
        )
        .await
        .unwrap()
        .unwrap();
    }

    // When
    let listed = vocabulary::list(&pool, &uid, &sid).await.unwrap();
    let pairs: Vec<(&str, &str)> = listed
        .iter()
        .map(|e| (e.category.as_str(), e.word.as_str()))
        .collect();
    // Then
    assert_eq!(
        pairs,
        vec![
            ("action", "click"),
            ("action", "submit"),
            ("target", "apple"),
            ("target", "zebra"),
        ]
    );
}

/// `(space_id, category, lower(word))` carries a unique index
/// (`vocabulary_space_word_cat_idx`), so two rows in the same space can
/// never share an identical `(category, word)` sort key — there is no way
/// to construct a genuine forced-tie fixture for this table without
/// violating that constraint (unlike `notification`/`collection`, which
/// have no such uniqueness on their sort keys). The `, id ASC` tiebreaker
/// is kept anyway for this port's own total-ordering convention and as a
/// defensive default; this test instead proves the ordering is stable and
/// deterministic across repeated calls with real, non-tied data.
#[sqlx::test]
async fn list_ordering_is_stable_across_repeated_calls(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let sid = seed_space(&pool, &uid, "Space").await;

    for word in ["mango", "kiwi", "fig", "date", "cherry"] {
        // When
        vocabulary::create_entry(
            &pool,
            &uid,
            NewVocabularyEntry {
                id: fubbik_db::new_id(),
                word: word.into(),
                category: "target".into(),
                expects: None,
                space_id: sid.clone(),
            },
        )
        .await
        .unwrap()
        .unwrap();
    }

    let first: Vec<String> = vocabulary::list(&pool, &uid, &sid)
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.word)
        .collect();
    let second: Vec<String> = vocabulary::list(&pool, &uid, &sid)
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.word)
        .collect();

    // Then
    assert_eq!(first, second);
    assert_eq!(first, vec!["cherry", "date", "fig", "kiwi", "mango"]);
}

#[sqlx::test]
async fn get_by_id_is_unscoped_like_node(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let sid = seed_space(&pool, &uid, "Space").await;
    let created = vocabulary::create_entry(
        &pool,
        &uid,
        NewVocabularyEntry {
            id: fubbik_db::new_id(),
            word: "click".into(),
            category: "action".into(),
            expects: None,
            space_id: sid,
        },
    )
    .await
    .unwrap()
    .unwrap();

    // When
    let found = vocabulary::get_by_id(&pool, &created.id)
        .await
        .unwrap()
        .unwrap();
    // Then
    assert_eq!(found.id, created.id);

    assert!(
        vocabulary::get_by_id(&pool, "does-not-exist")
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test]
async fn update_sets_only_provided_fields_and_always_bumps_updated_at(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let sid = seed_space(&pool, &uid, "Space").await;
    let created = vocabulary::create_entry(
        &pool,
        &uid,
        NewVocabularyEntry {
            id: fubbik_db::new_id(),
            word: "click".into(),
            category: "action".into(),
            expects: Some(vec!["target".into()]),
            space_id: sid,
        },
    )
    .await
    .unwrap()
    .unwrap();

    // When
    // All-omitted patch: word/category/expects untouched, updated_at still bumps.
    let untouched = vocabulary::update(&pool, &uid, &created.id, VocabularyPatch::default())
        .await
        .unwrap()
        .unwrap();
    // Then
    assert_eq!(untouched.word, "click");
    assert_eq!(untouched.category, "action");
    assert!(untouched.updated_at.0 >= created.updated_at.0);

    let patched = vocabulary::update(
        &pool,
        &uid,
        &created.id,
        VocabularyPatch {
            word: Some("TAP".into()),
            category: None,
            expects: Some(vec![]),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(patched.word, "tap", "word is lower-cased on update too");
    assert_eq!(patched.category, "action", "category left untouched");
    assert_eq!(
        patched.expects.unwrap().0,
        Vec::<String>::new(),
        "an explicit empty array clears expects to [] (not null)"
    );
}

#[sqlx::test]
async fn update_cannot_touch_another_users_entry_via_a_guessed_id(pool: sqlx::PgPool) {
    // Given
    let owner = seed_user(&pool, "owner@b.test").await;
    let attacker = seed_user(&pool, "attacker@b.test").await;
    let sid = seed_space(&pool, &owner, "Owner space").await;
    let created = vocabulary::create_entry(
        &pool,
        &owner,
        NewVocabularyEntry {
            id: fubbik_db::new_id(),
            word: "click".into(),
            category: "action".into(),
            expects: None,
            space_id: sid,
        },
    )
    .await
    .unwrap()
    .unwrap();

    // When
    // Direct repo call with the attacker's user_id, id guessed/known —
    // simulates the service-layer `get_by_id` + `verify_space_ownership`
    // sequence having been bypassed entirely.
    let result = vocabulary::update(
        &pool,
        &attacker,
        &created.id,
        VocabularyPatch {
            word: Some("hacked".into()),
            category: None,
            expects: None,
        },
    )
    .await
    .unwrap();
    // Then
    assert!(
        result.is_none(),
        "guard: update must not touch another user's entry"
    );

    let unchanged = vocabulary::get_by_id(&pool, &created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.word, "click", "victim's entry must be unchanged");
}

#[sqlx::test]
async fn delete_removes_the_row_and_reports_whether_it_existed(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let sid = seed_space(&pool, &uid, "Space").await;
    // When
    let created = vocabulary::create_entry(
        &pool,
        &uid,
        NewVocabularyEntry {
            id: fubbik_db::new_id(),
            word: "click".into(),
            category: "action".into(),
            expects: None,
            space_id: sid,
        },
    )
    .await
    .unwrap()
    .unwrap();

    // Then
    assert!(vocabulary::delete(&pool, &uid, &created.id).await.unwrap());
    assert!(
        vocabulary::get_by_id(&pool, &created.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        !vocabulary::delete(&pool, &uid, &created.id).await.unwrap(),
        "already gone"
    );
}

#[sqlx::test]
async fn delete_cannot_touch_another_users_entry_via_a_guessed_id(pool: sqlx::PgPool) {
    // Given
    let owner = seed_user(&pool, "owner@b.test").await;
    let attacker = seed_user(&pool, "attacker@b.test").await;
    let sid = seed_space(&pool, &owner, "Owner space").await;
    let created = vocabulary::create_entry(
        &pool,
        &owner,
        NewVocabularyEntry {
            id: fubbik_db::new_id(),
            word: "click".into(),
            category: "action".into(),
            expects: None,
            space_id: sid,
        },
    )
    .await
    .unwrap()
    .unwrap();

    // When
    let deleted = vocabulary::delete(&pool, &attacker, &created.id)
        .await
        .unwrap();
    // Then
    assert!(
        !deleted,
        "guard: delete must not remove another user's entry"
    );
    assert!(
        vocabulary::get_by_id(&pool, &created.id)
            .await
            .unwrap()
            .is_some(),
        "victim's entry must still exist"
    );
}
