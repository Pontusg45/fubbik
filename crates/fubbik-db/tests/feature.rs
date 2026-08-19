//! Repository tests for the `features` domain.
//!
//! Node leaves five of these queries entirely unscoped (`getSpacesForFeature`,
//! `setFeatureSpaces`, `getDeltasForChunk`, `getDeltasForFeature`,
//! `upsertDelta`/`deleteDelta`), relying on service-layer pre-checks. This
//! port pushes `user_id` into every statement, so the tests below have to
//! live *here*, at the repository layer: an API-level test cannot tell a
//! working SQL guard from a service pre-check that 404s first.
//!
//! The two behaviours worth reading the file for:
//! - `merge_is_atomic_under_forced_failure` — the whole point of the merge
//!   transaction.
//! - `deltas_for_chunk_orders_ascending_so_highest_priority_wins` — delta
//!   precedence, exercised with two deltas touching the *same* field.

use fubbik_db::repo::feature::{FeaturePatch, ListParams};
use fubbik_db::repo::{chunk, feature, space, user};

async fn seed_user(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "U", None).await.unwrap().id
}

async fn a_chunk(pool: &sqlx::PgPool, uid: &str, title: &str) -> String {
    chunk::create(
        pool,
        uid,
        chunk::NewChunk {
            title: title.into(),
            content: "base content".into(),
            chunk_type: "note".into(),
            rationale: None,
        },
    )
    .await
    .unwrap()
    .id
}

async fn a_space(pool: &sqlx::PgPool, uid: &str, name: &str) -> String {
    space::create(
        pool,
        uid,
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

async fn a_feature(pool: &sqlx::PgPool, uid: &str, name: &str, priority: i32) -> String {
    let id = fubbik_db::new_id();
    feature::create(pool, &id, uid, name, None, priority, None)
        .await
        .unwrap()
        .id
}

fn json(v: serde_json::Value) -> serde_json::Value {
    v
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

#[sqlx::test]
async fn list_is_user_scoped(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    a_feature(&pool, &alice, "alice-feature", 1).await;
    a_feature(&pool, &bob, "bob-feature", 1).await;

    let rows = feature::list(&pool, &alice, &ListParams::default())
        .await
        .unwrap();
    let names: Vec<&str> = rows.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, vec!["alice-feature"]);

    // Bob's own row must still be intact and visible from his side — a
    // one-sided check would pass even if Alice's query had eaten it.
    let bobs = feature::list(&pool, &bob, &ListParams::default())
        .await
        .unwrap();
    assert_eq!(bobs.len(), 1);
}

/// `ORDER BY priority ASC`. **No tiebreaker test is possible here**, and
/// that is a property of the schema, not an omission: `feature_user_priority_idx`
/// is `UNIQUE (user_id, priority)`, so two of one user's features can never
/// share a sort key. The `, id ASC` in the query is therefore unreachable
/// by construction — this test asserts the insert attempt actually fails,
/// so the claim is verified rather than assumed.
#[sqlx::test]
async fn list_orders_by_priority_which_cannot_tie(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    a_feature(&pool, &alice, "third", 30).await;
    a_feature(&pool, &alice, "first", 10).await;
    a_feature(&pool, &alice, "second", 20).await;

    let rows = feature::list(&pool, &alice, &ListParams::default())
        .await
        .unwrap();
    let names: Vec<&str> = rows.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, vec!["first", "second", "third"]);

    let dupe = fubbik_db::new_id();
    let err = feature::create(&pool, &dupe, &alice, "tie-attempt", None, 10, None).await;
    assert!(
        err.is_err(),
        "a duplicate (user_id, priority) must be rejected — if this ever \
         succeeds, the ordering above needs a real tiebreaker test"
    );
}

#[sqlx::test]
async fn list_counts_deltas_per_feature(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let f = a_feature(&pool, &alice, "f", 1).await;
    let empty = a_feature(&pool, &alice, "empty", 2).await;
    let c1 = a_chunk(&pool, &alice, "c1").await;
    let c2 = a_chunk(&pool, &alice, "c2").await;
    for c in [&c1, &c2] {
        feature::upsert_delta(
            &pool,
            &fubbik_db::new_id(),
            c,
            &f,
            &alice,
            &json(serde_json::json!({"title": "x"})),
        )
        .await
        .unwrap()
        .expect("own chunk + own feature");
    }

    let rows = feature::list(&pool, &alice, &ListParams::default())
        .await
        .unwrap();
    assert_eq!(rows.iter().find(|r| r.id == f).unwrap().delta_count, 2);
    assert_eq!(rows.iter().find(|r| r.id == empty).unwrap().delta_count, 0);
}

/// Node's space filter is not a join: a feature with **no** space
/// association is global and shows up under every `spaceId`; only features
/// linked to some *other* space are hidden.
#[sqlx::test]
async fn list_space_filter_keeps_unlinked_features_global(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let s1 = a_space(&pool, &alice, "s1").await;
    let s2 = a_space(&pool, &alice, "s2").await;
    let in_s1 = a_feature(&pool, &alice, "in-s1", 1).await;
    let in_s2 = a_feature(&pool, &alice, "in-s2", 2).await;
    a_feature(&pool, &alice, "global", 3).await;
    feature::set_spaces(&pool, &in_s1, &alice, std::slice::from_ref(&s1))
        .await
        .unwrap();
    feature::set_spaces(&pool, &in_s2, &alice, std::slice::from_ref(&s2))
        .await
        .unwrap();

    let names = |rows: Vec<fubbik_db::repo::feature::FeatureListItem>| -> Vec<String> {
        rows.into_iter().map(|f| f.name).collect()
    };

    let under_s1 = feature::list(
        &pool,
        &alice,
        &ListParams {
            space_id: Some(&s1),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(names(under_s1), vec!["in-s1", "global"]);

    // An empty string is "no filter", not "match the empty space id".
    let empty_filter = feature::list(
        &pool,
        &alice,
        &ListParams {
            space_id: Some(""),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(empty_filter.len(), 3);
}

#[sqlx::test]
async fn list_status_and_search_filters(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let archived = a_feature(&pool, &alice, "Dark Mode", 1).await;
    a_feature(&pool, &alice, "light mode", 2).await;
    feature::update(
        &pool,
        &archived,
        &alice,
        FeaturePatch {
            status: Some("archived".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let by_status = feature::list(
        &pool,
        &alice,
        &ListParams {
            status: Some("archived"),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(by_status.len(), 1);
    assert_eq!(by_status[0].name, "Dark Mode");

    // ILIKE — case-insensitive substring, exactly Drizzle's `ilike`.
    let by_search = feature::list(
        &pool,
        &alice,
        &ListParams {
            search: Some("MODE"),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(by_search.len(), 2);

    // Empty string is truthy-checked away in Node, so it filters nothing.
    let empty = feature::list(
        &pool,
        &alice,
        &ListParams {
            status: Some(""),
            search: Some(""),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(empty.len(), 2);
}

// ---------------------------------------------------------------------------
// find / update / delete scoping
// ---------------------------------------------------------------------------

#[sqlx::test]
async fn find_update_and_delete_are_user_scoped(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let bobs = a_feature(&pool, &bob, "bobs-feature", 1).await;

    assert!(
        feature::find_by_id(&pool, &bobs, &alice)
            .await
            .unwrap()
            .is_none()
    );

    let hijack = feature::update(
        &pool,
        &bobs,
        &alice,
        FeaturePatch {
            name: Some("pwned".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(hijack.is_none(), "must not update another user's feature");

    assert!(
        feature::delete(&pool, &bobs, &alice)
            .await
            .unwrap()
            .is_none(),
        "must not delete another user's feature"
    );

    // The victim's row is untouched, name and all.
    let still = feature::find_by_id(&pool, &bobs, &bob)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(still.name, "bobs-feature");
}

/// `description`/`color` are tri-state; an explicit `Some(None)` clears.
#[sqlx::test]
async fn update_tri_state_clears_and_empty_patch_is_a_no_op(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let id = fubbik_db::new_id();
    let created = feature::create(&pool, &id, &alice, "f", Some("desc"), 1, Some("#fff"))
        .await
        .unwrap();
    assert_eq!(created.description.as_deref(), Some("desc"));

    let cleared = feature::update(
        &pool,
        &id,
        &alice,
        FeaturePatch {
            description: Some(None),
            color: Some(None),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(cleared.description, None);
    assert_eq!(cleared.color, None);

    // A fully-omitted patch re-selects rather than issuing a no-op UPDATE,
    // so `updated_at` does not move. (Node throws `No values to set` here —
    // see `feature::update`'s doc comment.)
    let untouched = feature::update(&pool, &id, &alice, FeaturePatch::default())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(untouched.updated_at, cleared.updated_at);
}

#[sqlx::test]
async fn name_conflict_excludes_self_and_is_user_scoped(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let a1 = a_feature(&pool, &alice, "shared-name", 1).await;
    let a2 = a_feature(&pool, &alice, "other", 2).await;
    a_feature(&pool, &bob, "shared-name", 1).await;

    // Renaming a2 to a1's name conflicts.
    assert!(
        feature::name_conflict(&pool, &a2, &alice, "shared-name")
            .await
            .unwrap()
    );
    // a1 keeping its own name does not.
    assert!(
        !feature::name_conflict(&pool, &a1, &alice, "shared-name")
            .await
            .unwrap()
    );
    // Bob owning the same name is not Alice's problem.
    assert!(
        !feature::name_conflict(&pool, &a2, &alice, "bob-only")
            .await
            .unwrap()
    );
}

#[sqlx::test]
async fn max_priority_is_user_scoped_and_zero_when_empty(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    assert_eq!(feature::max_priority(&pool, &alice).await.unwrap(), 0);

    a_feature(&pool, &bob, "bobs", 99).await;
    assert_eq!(
        feature::max_priority(&pool, &alice).await.unwrap(),
        0,
        "Bob's priority must not leak into Alice's auto-assignment"
    );

    a_feature(&pool, &alice, "alices", 4).await;
    assert_eq!(feature::max_priority(&pool, &alice).await.unwrap(), 4);
}

// ---------------------------------------------------------------------------
// reorder / shiftPriorities — Node bug, reproduced
// ---------------------------------------------------------------------------

/// Node's `shiftPriorities(userId, p, "up")` is a single bulk
/// `UPDATE ... SET priority = priority + 1 WHERE priority >= p`. With a
/// contiguous run that violates the non-deferrable `UNIQUE (user_id,
/// priority)` index mid-statement. This is a real Node bug, kept.
#[sqlx::test]
async fn shift_priorities_up_hits_nodes_unique_violation_on_a_contiguous_run(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    a_feature(&pool, &alice, "p1", 1).await;
    a_feature(&pool, &alice, "p2", 2).await;
    a_feature(&pool, &alice, "p3", 3).await;

    let err = feature::shift_priorities_up(&pool, &alice, 1).await;
    assert!(
        err.is_err(),
        "reproducing Node: shifting 1,2,3 up collides with itself"
    );

    // Nothing moved — the failed statement is atomic on its own.
    let rows = feature::list(&pool, &alice, &ListParams::default())
        .await
        .unwrap();
    assert_eq!(
        rows.iter().map(|r| r.priority).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
}

/// The path that does work: a gap-free run is the pathological case, a
/// sparse one is fine. This is also the shape the API's own reorder test
/// exercises.
#[sqlx::test]
async fn shift_priorities_up_succeeds_on_a_sparse_run(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    a_feature(&pool, &alice, "p10", 10).await;
    a_feature(&pool, &alice, "p20", 20).await;
    a_feature(&pool, &bob, "bob-p10", 10).await;

    feature::shift_priorities_up(&pool, &alice, 10)
        .await
        .unwrap();

    let rows = feature::list(&pool, &alice, &ListParams::default())
        .await
        .unwrap();
    assert_eq!(
        rows.iter().map(|r| r.priority).collect::<Vec<_>>(),
        vec![11, 21]
    );
    // Bob's priority must not have been shifted by Alice's reorder.
    let bobs = feature::list(&pool, &bob, &ListParams::default())
        .await
        .unwrap();
    assert_eq!(bobs[0].priority, 10);
}

// ---------------------------------------------------------------------------
// feature_space
// ---------------------------------------------------------------------------

#[sqlx::test]
async fn set_spaces_replaces_wholesale_and_refuses_foreign_spaces(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let f = a_feature(&pool, &alice, "f", 1).await;
    let mine = a_space(&pool, &alice, "mine").await;
    let also_mine = a_space(&pool, &alice, "also-mine").await;
    let bobs_space = a_space(&pool, &bob, "bobs").await;

    feature::set_spaces(&pool, &f, &alice, &[mine.clone(), bobs_space.clone()])
        .await
        .unwrap();
    let linked = feature::spaces_for_feature(&pool, &f, &alice)
        .await
        .unwrap();
    assert_eq!(
        linked.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        vec![mine.as_str()],
        "a space owned by someone else must be skipped, not linked"
    );

    // Replacement, not append.
    feature::set_spaces(&pool, &f, &alice, std::slice::from_ref(&also_mine))
        .await
        .unwrap();
    let linked = feature::spaces_for_feature(&pool, &f, &alice)
        .await
        .unwrap();
    assert_eq!(
        linked.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        vec![also_mine.as_str()]
    );

    // Empty list clears.
    feature::set_spaces(&pool, &f, &alice, &[]).await.unwrap();
    assert!(
        feature::spaces_for_feature(&pool, &f, &alice)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test]
async fn set_spaces_cannot_wipe_another_users_associations(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let bobs_feature = a_feature(&pool, &bob, "bobs", 1).await;
    let bobs_space = a_space(&pool, &bob, "bobs-space").await;
    feature::set_spaces(
        &pool,
        &bobs_feature,
        &bob,
        std::slice::from_ref(&bobs_space),
    )
    .await
    .unwrap();

    // Alice aims set_spaces at Bob's feature. Node's version (no userId at
    // all) would delete his rows.
    feature::set_spaces(&pool, &bobs_feature, &alice, &[])
        .await
        .unwrap();

    let still = feature::spaces_for_feature(&pool, &bobs_feature, &bob)
        .await
        .unwrap();
    assert_eq!(still.len(), 1, "Bob's association must survive");
}

#[sqlx::test]
async fn spaces_for_feature_is_scoped_through_the_feature_owner(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let bobs_feature = a_feature(&pool, &bob, "bobs", 1).await;
    let bobs_space = a_space(&pool, &bob, "bobs-space").await;
    feature::set_spaces(&pool, &bobs_feature, &bob, &[bobs_space])
        .await
        .unwrap();

    assert!(
        feature::spaces_for_feature(&pool, &bobs_feature, &alice)
            .await
            .unwrap()
            .is_empty(),
        "Alice must not read Bob's feature/space links"
    );
}

// ---------------------------------------------------------------------------
// user_active_feature
// ---------------------------------------------------------------------------

#[sqlx::test]
async fn active_features_round_trip_and_replace(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let f1 = a_feature(&pool, &alice, "f1", 1).await;
    let f2 = a_feature(&pool, &alice, "f2", 2).await;

    assert!(
        feature::active_feature_ids(&pool, &alice)
            .await
            .unwrap()
            .is_empty()
    );

    feature::set_active_features(&pool, &alice, &[f1.clone(), f2.clone()])
        .await
        .unwrap();
    let mut got = feature::active_feature_ids(&pool, &alice).await.unwrap();
    got.sort();
    let mut want = vec![f1.clone(), f2.clone()];
    want.sort();
    assert_eq!(got, want);

    // Replacement, not union.
    feature::set_active_features(&pool, &alice, std::slice::from_ref(&f2))
        .await
        .unwrap();
    assert_eq!(
        feature::active_feature_ids(&pool, &alice).await.unwrap(),
        vec![f2.clone()]
    );

    // Empty clears.
    feature::set_active_features(&pool, &alice, &[])
        .await
        .unwrap();
    assert!(
        feature::active_feature_ids(&pool, &alice)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test]
async fn set_active_features_drops_foreign_ids_in_sql(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let mine = a_feature(&pool, &alice, "mine", 1).await;
    let bobs = a_feature(&pool, &bob, "bobs", 1).await;

    feature::set_active_features(&pool, &alice, &[mine.clone(), bobs.clone()])
        .await
        .unwrap();

    assert_eq!(
        feature::active_feature_ids(&pool, &alice).await.unwrap(),
        vec![mine],
        "Bob's feature must not become active for Alice"
    );
    assert!(
        feature::active_feature_ids(&pool, &bob)
            .await
            .unwrap()
            .is_empty(),
        "and Bob's own active set must be untouched"
    );
}

// ---------------------------------------------------------------------------
// deltas
// ---------------------------------------------------------------------------

#[sqlx::test]
async fn upsert_delta_requires_owning_both_the_chunk_and_the_feature(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let alices_chunk = a_chunk(&pool, &alice, "alices").await;
    let bobs_chunk = a_chunk(&pool, &bob, "bobs").await;
    let alices_feature = a_feature(&pool, &alice, "alices", 1).await;
    let bobs_feature = a_feature(&pool, &bob, "bobs", 1).await;
    let d = json(serde_json::json!({"title": "overlay"}));

    // Alice's feature, Bob's chunk.
    assert!(
        feature::upsert_delta(
            &pool,
            &fubbik_db::new_id(),
            &bobs_chunk,
            &alices_feature,
            &alice,
            &d
        )
        .await
        .unwrap()
        .is_none()
    );
    // Bob's feature, Alice's chunk.
    assert!(
        feature::upsert_delta(
            &pool,
            &fubbik_db::new_id(),
            &alices_chunk,
            &bobs_feature,
            &alice,
            &d
        )
        .await
        .unwrap()
        .is_none()
    );
    // Nothing was written anywhere.
    assert!(
        feature::deltas_for_feature(&pool, &alices_feature, &alice)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        feature::deltas_for_feature(&pool, &bobs_feature, &bob)
            .await
            .unwrap()
            .is_empty()
    );

    // Both hers: works.
    let ok = feature::upsert_delta(
        &pool,
        &fubbik_db::new_id(),
        &alices_chunk,
        &alices_feature,
        &alice,
        &d,
    )
    .await
    .unwrap()
    .expect("own chunk + own feature");
    assert_eq!(ok.delta.0, d);
}

/// A delta holds **only** the changed fields, and an upsert replaces it
/// wholesale rather than merging — a `{content}` write after a `{title}`
/// write leaves `title` gone.
#[sqlx::test]
async fn delta_is_sparse_and_replaced_wholesale(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let c = a_chunk(&pool, &alice, "c").await;
    let f = a_feature(&pool, &alice, "f", 1).await;

    let first = feature::upsert_delta(
        &pool,
        &fubbik_db::new_id(),
        &c,
        &f,
        &alice,
        &json(serde_json::json!({"title": "New Title"})),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        first
            .delta
            .0
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        vec!["title"],
        "only the changed field is stored — not a whole chunk"
    );

    let second = feature::upsert_delta(
        &pool,
        &fubbik_db::new_id(),
        &c,
        &f,
        &alice,
        &json(serde_json::json!({"content": "New Content"})),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        second.id, first.id,
        "upsert on (chunk, feature), not insert"
    );
    assert_eq!(
        second.delta.0,
        serde_json::json!({"content": "New Content"})
    );

    assert_eq!(
        feature::deltas_for_chunk(&pool, &c, &alice)
            .await
            .unwrap()
            .len(),
        1
    );
}

/// **Delta precedence.** `deltas_for_chunk` returns ascending priority, so
/// folding the rows in the order they arrive — Node's
/// `Object.assign(base, ...sortedAscending)` — leaves the *highest*
/// priority value in place. Both deltas here touch the same field, so the
/// precedence is genuinely exercised: reverse the ordering and this fails.
#[sqlx::test]
async fn deltas_for_chunk_orders_ascending_so_highest_priority_wins(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let c = a_chunk(&pool, &alice, "base title").await;
    let low = a_feature(&pool, &alice, "low", 1).await;
    let high = a_feature(&pool, &alice, "high", 9).await;

    // Insert the high-priority delta FIRST so insertion order cannot be
    // mistaken for priority order.
    feature::upsert_delta(
        &pool,
        &fubbik_db::new_id(),
        &c,
        &high,
        &alice,
        &json(serde_json::json!({"title": "from high", "summary": "only high"})),
    )
    .await
    .unwrap()
    .unwrap();
    feature::upsert_delta(
        &pool,
        &fubbik_db::new_id(),
        &c,
        &low,
        &alice,
        &json(serde_json::json!({"title": "from low", "content": "only low"})),
    )
    .await
    .unwrap()
    .unwrap();

    let rows = feature::deltas_for_chunk(&pool, &c, &alice).await.unwrap();
    assert_eq!(
        rows.iter().map(|r| r.feature_priority).collect::<Vec<_>>(),
        vec![1, 9],
        "ascending priority"
    );

    // Fold exactly as `resolveChunk` does.
    let mut resolved = serde_json::Map::new();
    resolved.insert("title".into(), serde_json::json!("base title"));
    for row in &rows {
        for (k, v) in row.delta.0.as_object().unwrap() {
            resolved.insert(k.clone(), v.clone());
        }
    }
    assert_eq!(
        resolved["title"], "from high",
        "higher priority must win a same-field conflict"
    );
    // Non-conflicting fields from both features survive.
    assert_eq!(resolved["content"], "only low");
    assert_eq!(resolved["summary"], "only high");
}

/// Unlike `list`, a priority tie *is* reachable here: `UNIQUE (user_id,
/// priority)` does not stop two features owned by **different** users from
/// both being priority 1, and `chunk_feature_delta` has no user column of
/// its own. Forcing that tie is the only way to exercise the `id`
/// tiebreaker, so the two deltas are inserted directly.
#[sqlx::test]
async fn deltas_for_chunk_breaks_priority_ties_by_id(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let c = a_chunk(&pool, &alice, "c").await;
    let alices = a_feature(&pool, &alice, "alices", 1).await;
    let bobs = a_feature(&pool, &bob, "bobs", 1).await;

    // Insert descending by id so a missing tiebreaker would likely surface
    // them the other way round.
    for (id, feature_id) in [("delta-zzz", &bobs), ("delta-aaa", &alices)] {
        sqlx::query(
            "INSERT INTO chunk_feature_delta (id, chunk_id, feature_id, delta)
             VALUES ($1, $2, $3, '{\"title\":\"x\"}'::jsonb)",
        )
        .bind(id)
        .bind(&c)
        .bind(feature_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    let rows = feature::deltas_for_chunk(&pool, &c, &alice).await.unwrap();
    assert_eq!(
        rows.iter().map(|r| r.feature_priority).collect::<Vec<_>>(),
        vec![1, 1],
        "the sort keys really are tied"
    );
    assert_eq!(
        rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["delta-aaa", "delta-zzz"],
        "id ASC is the tiebreaker"
    );
}

/// The guard Node does not have at all: `GET /chunks/{id}/deltas` discards
/// the session there.
#[sqlx::test]
async fn deltas_for_chunk_is_scoped_to_the_chunk_owner(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let bobs_chunk = a_chunk(&pool, &bob, "bobs secret").await;
    let bobs_feature = a_feature(&pool, &bob, "bobs", 1).await;
    feature::upsert_delta(
        &pool,
        &fubbik_db::new_id(),
        &bobs_chunk,
        &bobs_feature,
        &bob,
        &json(serde_json::json!({"content": "confidential overlay"})),
    )
    .await
    .unwrap()
    .unwrap();

    assert!(
        feature::deltas_for_chunk(&pool, &bobs_chunk, &alice)
            .await
            .unwrap()
            .is_empty(),
        "Alice must not read the overlay text of Bob's chunk"
    );
    assert_eq!(
        feature::deltas_for_chunk(&pool, &bobs_chunk, &bob)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[sqlx::test]
async fn deltas_for_feature_is_scoped_and_carries_the_chunk_title(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let c = a_chunk(&pool, &alice, "Chunk Title").await;
    let f = a_feature(&pool, &alice, "f", 1).await;
    feature::upsert_delta(
        &pool,
        &fubbik_db::new_id(),
        &c,
        &f,
        &alice,
        &json(serde_json::json!({"title": "x"})),
    )
    .await
    .unwrap()
    .unwrap();

    let mine = feature::deltas_for_feature(&pool, &f, &alice)
        .await
        .unwrap();
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].chunk_title, "Chunk Title");

    assert!(
        feature::deltas_for_feature(&pool, &f, &bob)
            .await
            .unwrap()
            .is_empty(),
        "Bob must not read Alice's feature deltas"
    );
}

#[sqlx::test]
async fn delete_delta_is_scoped_through_the_feature_owner(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let c = a_chunk(&pool, &bob, "bobs").await;
    let f = a_feature(&pool, &bob, "bobs", 1).await;
    feature::upsert_delta(
        &pool,
        &fubbik_db::new_id(),
        &c,
        &f,
        &bob,
        &json(serde_json::json!({"title": "x"})),
    )
    .await
    .unwrap()
    .unwrap();

    assert!(
        feature::delete_delta(&pool, &c, &f, &alice)
            .await
            .unwrap()
            .is_none(),
        "Alice must not delete Bob's delta"
    );
    assert_eq!(
        feature::deltas_for_feature(&pool, &f, &bob)
            .await
            .unwrap()
            .len(),
        1,
        "and the row must still be there"
    );

    assert!(
        feature::delete_delta(&pool, &c, &f, &bob)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        feature::deltas_for_feature(&pool, &f, &bob)
            .await
            .unwrap()
            .is_empty()
    );
}

// ---------------------------------------------------------------------------
// merge
// ---------------------------------------------------------------------------

#[sqlx::test]
async fn merge_applies_deltas_snapshots_versions_and_marks_merged(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let c1 = a_chunk(&pool, &alice, "original one").await;
    let c2 = a_chunk(&pool, &alice, "original two").await;
    let f = a_feature(&pool, &alice, "f", 1).await;

    for (c, d) in [
        (&c1, serde_json::json!({"title": "merged one"})),
        (
            &c2,
            serde_json::json!({"content": "merged content", "summary": "s"}),
        ),
    ] {
        feature::upsert_delta(&pool, &fubbik_db::new_id(), c, &f, &alice, &d)
            .await
            .unwrap()
            .unwrap();
    }

    let deltas = feature::deltas_for_feature(&pool, &f, &alice)
        .await
        .unwrap();
    let pairs: Vec<(String, serde_json::Value)> = deltas
        .into_iter()
        .map(|d| (d.chunk_id, d.delta.0))
        .collect();
    let affected = feature::merge_feature_deltas(&pool, &f, &alice, &pairs)
        .await
        .unwrap();
    assert_eq!(affected.len(), 2);

    // Base chunks rewritten, and only the fields the delta named.
    let one = chunk::find_by_id(&pool, &alice, &c1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(one.title, "merged one");
    assert_eq!(one.content, "base content", "an absent key must not clear");
    let two = chunk::find_by_id(&pool, &alice, &c2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(two.title, "original two");
    assert_eq!(two.content, "merged content");
    assert_eq!(two.summary.as_deref(), Some("s"));

    // A pre-merge version snapshot exists for each.
    let versions = fubbik_db::repo::chunk_version::list_for_chunk(&pool, &c1, &alice)
        .await
        .unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].version, 1);
    assert_eq!(versions[0].title, "original one", "snapshot is pre-edit");

    // Deltas gone, feature merged.
    assert!(
        feature::deltas_for_feature(&pool, &f, &alice)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        feature::find_by_id(&pool, &f, &alice)
            .await
            .unwrap()
            .unwrap()
            .status,
        "merged"
    );
}

/// Merge writes to four tables per delta. Forcing a failure *after* the
/// first chunk has already been versioned and rewritten is the only way to
/// tell a transaction from a loop of independent statements.
///
/// The trigger is a second delta whose `title` is JSON `null`: the `UPDATE`
/// then assigns NULL to `chunk.title`, which is `NOT NULL`. (Node reaches
/// the identical failure — Drizzle's `.set({title: null})` writes the same
/// NULL.)
#[sqlx::test]
async fn merge_is_atomic_under_forced_failure(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let good = a_chunk(&pool, &alice, "untouched title").await;
    let poison = a_chunk(&pool, &alice, "poison title").await;
    let f = a_feature(&pool, &alice, "f", 1).await;

    for (c, d) in [
        (&good, serde_json::json!({"title": "would-be new title"})),
        (&poison, serde_json::json!({"title": null})),
    ] {
        feature::upsert_delta(&pool, &fubbik_db::new_id(), c, &f, &alice, &d)
            .await
            .unwrap()
            .unwrap();
    }

    // Order matters: the good chunk must be processed first, so its version
    // insert and update are already in the transaction when the poison row
    // blows up.
    let pairs = vec![
        (
            good.clone(),
            serde_json::json!({"title": "would-be new title"}),
        ),
        (poison.clone(), serde_json::json!({"title": null})),
    ];
    let result = feature::merge_feature_deltas(&pool, &f, &alice, &pairs).await;
    assert!(result.is_err(), "the NOT NULL violation must propagate");

    // NOTHING may have been written.
    let good_chunk = chunk::find_by_id(&pool, &alice, &good)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        good_chunk.title, "untouched title",
        "the first chunk's update must have rolled back"
    );
    assert!(
        fubbik_db::repo::chunk_version::list_for_chunk(&pool, &good, &alice)
            .await
            .unwrap()
            .is_empty(),
        "the first chunk's version snapshot must have rolled back"
    );
    assert_eq!(
        feature::deltas_for_feature(&pool, &f, &alice)
            .await
            .unwrap()
            .len(),
        2,
        "no delta may have been deleted"
    );
    assert_eq!(
        feature::find_by_id(&pool, &f, &alice)
            .await
            .unwrap()
            .unwrap()
            .status,
        "inactive",
        "the feature must not have been marked merged"
    );
}

/// A delta pointing at a chunk the caller does not own is skipped (Node's
/// `if (!existing) continue`, plus this port's `user_id` guard), and the
/// victim's chunk is left alone.
#[sqlx::test]
async fn merge_skips_chunks_the_caller_does_not_own(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let bobs_chunk = a_chunk(&pool, &bob, "bobs title").await;
    let f = a_feature(&pool, &alice, "f", 1).await;

    let pairs = vec![(bobs_chunk.clone(), serde_json::json!({"title": "hijacked"}))];
    let affected = feature::merge_feature_deltas(&pool, &f, &alice, &pairs)
        .await
        .unwrap();
    assert!(affected.is_empty());

    let untouched = chunk::find_by_id(&pool, &bob, &bobs_chunk)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(untouched.title, "bobs title");
    assert!(
        fubbik_db::repo::chunk_version::list_for_chunk(&pool, &bobs_chunk, &bob)
            .await
            .unwrap()
            .is_empty()
    );
}

/// Merge must not flip another user's feature to `merged`, nor delete their
/// deltas, when aimed at their id.
#[sqlx::test]
async fn merge_is_user_scoped_on_the_feature_side(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let bobs_chunk = a_chunk(&pool, &bob, "bobs").await;
    let bobs_feature = a_feature(&pool, &bob, "bobs", 1).await;
    feature::upsert_delta(
        &pool,
        &fubbik_db::new_id(),
        &bobs_chunk,
        &bobs_feature,
        &bob,
        &json(serde_json::json!({"title": "bobs overlay"})),
    )
    .await
    .unwrap()
    .unwrap();

    feature::merge_feature_deltas(&pool, &bobs_feature, &alice, &[])
        .await
        .unwrap();

    let still = feature::find_by_id(&pool, &bobs_feature, &bob)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(still.status, "inactive", "Bob's feature must not be merged");
    assert_eq!(
        feature::deltas_for_feature(&pool, &bobs_feature, &bob)
            .await
            .unwrap()
            .len(),
        1,
        "Bob's delta must not be deleted"
    );
}
