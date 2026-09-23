//! Repo-level tests for `fubbik_db::repo::behavior_matrix`.
//!
//! Only `behavior_matrix` has a `user_id`; the other seven tables derive
//! ownership from it one to three hops away. Node's repository functions take
//! bare ids and trust the service to have checked — which, for the entire
//! cell surface, it did not. This port puts the chain in SQL, so these tests
//! observe the guard directly rather than through an HTTP status code.

use fubbik_db::repo::behavior_matrix::{self as bm, MatrixPatch, NewMatrix, NewRule, RulePatch};
use fubbik_db::repo::{requirement, user};

async fn seed_user(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "U", None).await.unwrap().id
}

async fn a_matrix(pool: &sqlx::PgPool, uid: &str, name: &str) -> String {
    bm::create(
        pool,
        uid,
        NewMatrix {
            name: name.into(),
            layer: "invariant".into(),
            description: None,
            space_id: None,
        },
    )
    .await
    .unwrap()
    .expect("a matrix with no space_id always inserts")
    .id
}

fn rule(title: &str) -> NewRule {
    NewRule {
        title: title.into(),
        description: None,
        category: None,
        rationale: None,
        alternatives: None,
        consequences: None,
        counterexample: None,
    }
}

/// Creates a rule, a dimension and their cell, returning
/// `(matrix_id, rule_id, dimension_id, cell_id)`.
async fn a_cell(pool: &sqlx::PgPool, uid: &str, name: &str) -> (String, String, String, String) {
    let m = a_matrix(pool, uid, name).await;
    let r = bm::create_rule(pool, &m, uid, rule("R"))
        .await
        .unwrap()
        .unwrap()
        .id;
    let d = bm::create_dimension(pool, &m, uid, "D")
        .await
        .unwrap()
        .unwrap()
        .id;
    let c = bm::create_cell(pool, &r, &d, &m, uid)
        .await
        .unwrap()
        .unwrap()
        .id;
    (m, r, d, c)
}

// ---------------------------------------------------------------------------
// Matrix
// ---------------------------------------------------------------------------

#[sqlx::test]
async fn matrix_crud_round_trips_and_is_user_scoped(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    // When
    let id = a_matrix(&pool, &alice, "Invariants").await;

    // Then
    assert!(
        bm::find_by_id(&pool, &id, &alice).await.unwrap().is_some(),
        "the owner must see it — otherwise the assertions below pass for the wrong reason"
    );
    assert!(bm::find_by_id(&pool, &id, &bob).await.unwrap().is_none());
    assert!(
        bm::update(
            &pool,
            &id,
            &bob,
            MatrixPatch {
                name: Some("Hijacked".into()),
                ..Default::default()
            }
        )
        .await
        .unwrap()
        .is_none()
    );
    assert!(bm::delete(&pool, &id, &bob).await.unwrap().is_none());

    // Alice's row survived Bob's rejected write.
    assert_eq!(
        bm::find_by_id(&pool, &id, &alice)
            .await
            .unwrap()
            .unwrap()
            .name,
        "Invariants"
    );
}

/// `description` is tri-state: absent leaves it, explicit null clears it.
/// All three transitions in sequence — asserting only "set" would pass
/// against a two-state implementation.
#[sqlx::test]
async fn matrix_description_is_tri_state(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let id = a_matrix(&pool, &uid, "M").await;

    // When
    bm::update(
        &pool,
        &id,
        &uid,
        MatrixPatch {
            description: Some(Some("why".into())),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    // Then
    assert_eq!(
        bm::find_by_id(&pool, &id, &uid)
            .await
            .unwrap()
            .unwrap()
            .description
            .as_deref(),
        Some("why")
    );

    // Absent — must not clear.
    bm::update(
        &pool,
        &id,
        &uid,
        MatrixPatch {
            name: Some("Renamed".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        bm::find_by_id(&pool, &id, &uid)
            .await
            .unwrap()
            .unwrap()
            .description
            .as_deref(),
        Some("why"),
        "omitting description must leave it alone"
    );

    // Explicit null — must clear.
    bm::update(
        &pool,
        &id,
        &uid,
        MatrixPatch {
            description: Some(None),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        bm::find_by_id(&pool, &id, &uid)
            .await
            .unwrap()
            .unwrap()
            .description,
        None
    );
}

#[sqlx::test]
async fn create_rejects_a_space_the_caller_does_not_own(pool: sqlx::PgPool) {
    // Given
    use fubbik_db::repo::space;
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let bobs_space = space::create(
        &pool,
        &bob,
        space::NewSpace {
            name: "bob".into(),
            kind: "code".into(),
            description: None,
        },
        None,
    )
    .await
    .unwrap()
    .id;

    // When
    let created = bm::create(
        &pool,
        &alice,
        NewMatrix {
            name: "M".into(),
            layer: "invariant".into(),
            description: None,
            space_id: Some(bobs_space),
        },
    )
    .await
    .unwrap();
    // Then
    assert!(
        created.is_none(),
        "a foreign spaceId must insert nothing, not a dangling matrix"
    );
}

#[sqlx::test]
async fn list_filters_by_space_and_layer_and_is_scoped(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    a_matrix(&pool, &alice, "A").await;
    bm::create(
        &pool,
        &alice,
        NewMatrix {
            name: "B".into(),
            layer: "contract".into(),
            description: None,
            space_id: None,
        },
    )
    .await
    .unwrap();
    a_matrix(&pool, &bob, "Bobs").await;

    // When
    let all = bm::list(&pool, &alice, None, None).await.unwrap();
    // Then
    assert_eq!(all.len(), 2, "Bob's matrix must not appear");
    assert_eq!(all[0].name, "A", "ordered by name");

    let contracts = bm::list(&pool, &alice, None, Some("contract"))
        .await
        .unwrap();
    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts[0].name, "B");
}

// ---------------------------------------------------------------------------
// Dimensions and rules
// ---------------------------------------------------------------------------

/// `order` is assigned `max + 1` inside the INSERT, so three adds land 0,1,2.
#[sqlx::test]
async fn dimensions_and_rules_get_sequential_order(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let m = a_matrix(&pool, &uid, "M").await;

    for name in ["first", "second", "third"] {
        bm::create_dimension(&pool, &m, &uid, name)
            .await
            .unwrap()
            .unwrap();
        bm::create_rule(&pool, &m, &uid, rule(name))
            .await
            .unwrap()
            .unwrap();
    }

    // When
    let dims = bm::dimensions_for_matrix(&pool, &m, &uid).await.unwrap();
    // Then
    assert_eq!(dims.iter().map(|d| d.order).collect::<Vec<_>>(), [0, 1, 2]);
    assert_eq!(
        dims.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(),
        ["first", "second", "third"]
    );

    let rules = bm::rules_for_matrix(&pool, &m, &uid).await.unwrap();
    assert_eq!(rules.iter().map(|r| r.order).collect::<Vec<_>>(), [0, 1, 2]);
}

#[sqlx::test]
async fn dimension_and_rule_writes_are_scoped_to_the_matrix_owner(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let m = a_matrix(&pool, &alice, "M").await;
    let d = bm::create_dimension(&pool, &m, &alice, "D")
        .await
        .unwrap()
        .unwrap()
        .id;
    // When
    let r = bm::create_rule(&pool, &m, &alice, rule("R"))
        .await
        .unwrap()
        .unwrap()
        .id;

    // Then
    assert!(
        bm::create_dimension(&pool, &m, &bob, "Bobs")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        bm::create_rule(&pool, &m, &bob, rule("Bobs"))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        bm::update_dimension(&pool, &d, &m, &bob, "Hijacked")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        bm::delete_dimension(&pool, &d, &m, &bob)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        bm::delete_rule(&pool, &r, &m, &bob)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        bm::rules_for_matrix(&pool, &m, &bob)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        bm::dimensions_for_matrix(&pool, &m, &bob)
            .await
            .unwrap()
            .is_empty()
    );

    // Alice's rows are untouched.
    assert_eq!(
        bm::dimensions_for_matrix(&pool, &m, &alice).await.unwrap()[0].name,
        "D"
    );
    assert_eq!(
        bm::rules_for_matrix(&pool, &m, &alice).await.unwrap().len(),
        1
    );
}

/// **Pins a deliberate divergence.** Node's `reorderRules(ids)` renumbers
/// whatever ids it is handed, checking only that the *matrix* belongs to the
/// caller — so owning any matrix lets you renumber another's rows. Here the
/// UPDATE is constrained to the matrix, and foreign ids are skipped.
#[sqlx::test]
async fn reorder_ignores_ids_from_another_matrix(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let mine = a_matrix(&pool, &uid, "Mine").await;
    let other = a_matrix(&pool, &uid, "Other").await;

    let a = bm::create_rule(&pool, &mine, &uid, rule("a"))
        .await
        .unwrap()
        .unwrap()
        .id;
    let b = bm::create_rule(&pool, &mine, &uid, rule("b"))
        .await
        .unwrap()
        .unwrap()
        .id;
    let foreign = bm::create_rule(&pool, &other, &uid, rule("foreign"))
        .await
        .unwrap()
        .unwrap()
        .id;
    let foreign_order_before = bm::rules_for_matrix(&pool, &other, &uid).await.unwrap()[0].order;

    // When
    // Reverse mine, and try to sneak the other matrix's rule into the list.
    let affected = bm::reorder_rules(&pool, &mine, &uid, &[b.clone(), foreign.clone(), a.clone()])
        .await
        .unwrap();
    // Then
    assert_eq!(
        affected, 2,
        "only the two rules that belong to this matrix are renumbered"
    );

    let reordered = bm::rules_for_matrix(&pool, &mine, &uid).await.unwrap();
    assert_eq!(reordered[0].id, b, "b moved to the front");
    assert_eq!(reordered[1].id, a);

    assert_eq!(
        bm::rules_for_matrix(&pool, &other, &uid).await.unwrap()[0].order,
        foreign_order_before,
        "a rule in another matrix must not be renumbered by this call"
    );
}

#[sqlx::test]
async fn reorder_is_scoped_to_the_matrix_owner(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let m = a_matrix(&pool, &alice, "M").await;
    let a = bm::create_rule(&pool, &m, &alice, rule("a"))
        .await
        .unwrap()
        .unwrap()
        .id;
    // When
    let b = bm::create_rule(&pool, &m, &alice, rule("b"))
        .await
        .unwrap()
        .unwrap()
        .id;

    // Then
    assert_eq!(
        bm::reorder_rules(&pool, &m, &bob, &[b.clone(), a.clone()])
            .await
            .unwrap(),
        0,
        "Bob must renumber nothing"
    );
    let rules = bm::rules_for_matrix(&pool, &m, &alice).await.unwrap();
    assert_eq!(
        rules[0].id, a,
        "Alice's order survives Bob's rejected reorder"
    );
}

/// Every one of the six nullable rule fields is tri-state, and `title` is
/// not. Enumerated rather than sampled: they are generated by one macro-like
/// block of SQL, so a mistake in one is a mistake in all six.
#[sqlx::test]
async fn rule_patch_is_tri_state_for_every_nullable_field(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let m = a_matrix(&pool, &uid, "M").await;
    let r = bm::create_rule(
        &pool,
        &m,
        &uid,
        NewRule {
            title: "T".into(),
            description: Some("d".into()),
            category: Some("c".into()),
            rationale: Some("ra".into()),
            alternatives: Some("al".into()),
            consequences: Some("co".into()),
            counterexample: Some("cx".into()),
        },
    )
    .await
    .unwrap()
    .unwrap()
    .id;

    // An unrelated edit must leave all six alone.
    bm::update_rule(
        &pool,
        &r,
        &m,
        &uid,
        RulePatch {
            title: Some("T2".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    // When
    let kept = bm::find_rule(&pool, &r, &m, &uid).await.unwrap().unwrap();
    // Then
    assert_eq!(kept.title, "T2");
    for (label, got) in [
        ("description", &kept.description),
        ("category", &kept.category),
        ("rationale", &kept.rationale),
        ("alternatives", &kept.alternatives),
        ("consequences", &kept.consequences),
        ("counterexample", &kept.counterexample),
    ] {
        assert!(got.is_some(), "{label} must survive an unrelated PATCH");
    }

    // Explicit nulls must clear all six.
    bm::update_rule(
        &pool,
        &r,
        &m,
        &uid,
        RulePatch {
            description: Some(None),
            category: Some(None),
            rationale: Some(None),
            alternatives: Some(None),
            consequences: Some(None),
            counterexample: Some(None),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let cleared = bm::find_rule(&pool, &r, &m, &uid).await.unwrap().unwrap();
    for (label, got) in [
        ("description", &cleared.description),
        ("category", &cleared.category),
        ("rationale", &cleared.rationale),
        ("alternatives", &cleared.alternatives),
        ("consequences", &cleared.consequences),
        ("counterexample", &cleared.counterexample),
    ] {
        assert!(got.is_none(), "an explicit null must clear {label}");
    }
    assert_eq!(
        cleared.title, "T2",
        "title has no null variant and must be untouched"
    );
}

#[sqlx::test]
async fn rule_versions_are_newest_first_and_scoped(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let m = a_matrix(&pool, &alice, "M").await;
    let r = bm::create_rule(&pool, &m, &alice, rule("R"))
        .await
        .unwrap()
        .unwrap()
        .id;

    for title in ["v1", "v2"] {
        bm::insert_rule_version(
            &pool,
            &r,
            &m,
            &alice,
            &bm::BehaviorRuleSnapshot {
                title: title.into(),
                description: None,
                category: None,
                rationale: None,
                alternatives: None,
                consequences: None,
                counterexample: None,
            },
        )
        .await
        .unwrap()
        .expect("the owner can snapshot their own rule");
    }

    // When
    let versions = bm::rule_versions(&pool, &r, &m, &alice).await.unwrap();
    // Then
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0].snapshot.0.title, "v2", "newest first");
    assert_eq!(versions[0].changed_by.as_deref(), Some(alice.as_str()));

    assert!(
        bm::rule_versions(&pool, &r, &m, &bob)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        bm::insert_rule_version(
            &pool,
            &r,
            &m,
            &bob,
            &bm::BehaviorRuleSnapshot {
                title: "hijack".into(),
                description: None,
                category: None,
                rationale: None,
                alternatives: None,
                consequences: None,
                counterexample: None,
            }
        )
        .await
        .unwrap()
        .is_none(),
        "Bob must not append to Alice's rule history"
    );
}

// ---------------------------------------------------------------------------
// Cells — the surface Node shipped with no authorization at all
// ---------------------------------------------------------------------------

#[sqlx::test]
async fn cell_reads_and_writes_are_scoped_to_the_matrix_owner(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    // When
    let (m, r, d, c) = a_cell(&pool, &alice, "M").await;

    // Then
    assert!(
        bm::find_cell_by_id(&pool, &c, &m, &alice)
            .await
            .unwrap()
            .is_some(),
        "the owner must see the cell"
    );
    assert!(
        bm::find_cell_by_id(&pool, &c, &m, &bob)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        bm::find_cell(&pool, &r, &d, &m, &bob)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        bm::delete_cell(&pool, &c, &m, &bob)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        bm::code_for_cell(&pool, &c, &m, &bob)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        bm::requirements_for_cell(&pool, &c, &m, &bob)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        bm::test_results_for_cell(&pool, &c, &m, &bob)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        bm::link_cell_code(&pool, &c, "file", "src/x.rs", &m, &bob)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        bm::record_test_result(&pool, &c, "t", "pass", None, &m, &bob)
            .await
            .unwrap()
            .is_none()
    );

    // Alice's cell survived every rejected write.
    assert!(
        bm::find_cell_by_id(&pool, &c, &m, &alice)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        bm::code_for_cell(&pool, &c, &m, &alice)
            .await
            .unwrap()
            .is_empty()
    );
}

/// The subtle case: the caller owns BOTH matrices, but passes a `cell_id`
/// from the wrong one. A user-only check would let this through — the
/// `matrix_id` hop is what closes it.
#[sqlx::test]
async fn a_cell_id_from_another_matrix_is_rejected(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let (mine, _, _, _) = a_cell(&pool, &uid, "Mine").await;
    // When
    let (_, _, _, other_cell) = a_cell(&pool, &uid, "Other").await;

    // Then
    assert!(
        bm::find_cell_by_id(&pool, &other_cell, &mine, &uid)
            .await
            .unwrap()
            .is_none(),
        "a cell from another matrix must not resolve under this matrix's id"
    );
    assert!(
        bm::link_cell_code(&pool, &other_cell, "file", "src/x.rs", &mine, &uid)
            .await
            .unwrap()
            .is_none(),
        "nor be writable through it"
    );
}

/// Both ends of a cell are checked on create: pairing your own rule with a
/// dimension from elsewhere is rejected.
#[sqlx::test]
async fn create_cell_requires_rule_and_dimension_in_the_same_matrix(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let mine = a_matrix(&pool, &uid, "Mine").await;
    let other = a_matrix(&pool, &uid, "Other").await;
    let my_rule = bm::create_rule(&pool, &mine, &uid, rule("R"))
        .await
        .unwrap()
        .unwrap()
        .id;
    // When
    let foreign_dim = bm::create_dimension(&pool, &other, &uid, "D")
        .await
        .unwrap()
        .unwrap()
        .id;

    // Then
    assert!(
        bm::create_cell(&pool, &my_rule, &foreign_dim, &mine, &uid)
            .await
            .unwrap()
            .is_none(),
        "a dimension from another matrix must not form a cell here"
    );
}

/// Linking a requirement checks the requirement's owner too, not just the
/// cell's — Node checked neither end.
#[sqlx::test]
async fn linking_a_requirement_checks_both_ends(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let (m, _, _, c) = a_cell(&pool, &alice, "M").await;

    // When
    let bobs_req = requirement::create(
        &pool,
        &bob,
        requirement::NewRequirement {
            title: "Bob's".into(),
            description: None,
            steps: vec![],
            priority: None,
            space_id: None,
            use_case_id: None,
            origin: "human".into(),
            review_status: "approved".into(),
        },
    )
    .await
    .unwrap()
    .unwrap()
    .id;

    // Then
    assert!(
        bm::link_cell_requirement(&pool, &c, &bobs_req, &m, &alice)
            .await
            .unwrap()
            .is_none(),
        "Alice must not attach Bob's requirement to her cell"
    );

    let hers = requirement::create(
        &pool,
        &alice,
        requirement::NewRequirement {
            title: "Alice's".into(),
            description: None,
            steps: vec![],
            priority: None,
            space_id: None,
            use_case_id: None,
            origin: "human".into(),
            review_status: "approved".into(),
        },
    )
    .await
    .unwrap()
    .unwrap()
    .id;
    assert!(
        bm::link_cell_requirement(&pool, &c, &hers, &m, &alice)
            .await
            .unwrap()
            .is_some(),
        "her own requirement links fine — otherwise the assertion above proves nothing"
    );
    assert_eq!(
        bm::cell_requirement_count(&pool, &c, &m, &alice)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        bm::requirements_for_cell(&pool, &c, &m, &alice)
            .await
            .unwrap()[0]
            .title,
        "Alice's"
    );
}

/// The view's five per-cell counts, each fed a distinct number so a query
/// that crossed two of them would produce a visibly wrong total.
#[sqlx::test]
async fn matrix_view_counts_requirements_code_and_tests_per_cell(pool: sqlx::PgPool) {
    // Given
    let uid = seed_user(&pool, "a@b.test").await;
    let (m, _, _, c) = a_cell(&pool, &uid, "M").await;

    let req = requirement::create(
        &pool,
        &uid,
        requirement::NewRequirement {
            title: "R".into(),
            description: None,
            steps: vec![],
            priority: None,
            space_id: None,
            use_case_id: None,
            origin: "human".into(),
            review_status: "approved".into(),
        },
    )
    .await
    .unwrap()
    .unwrap()
    .id;
    bm::link_cell_requirement(&pool, &c, &req, &m, &uid)
        .await
        .unwrap()
        .unwrap();

    // 2 code links, 3 passing tests, 1 failing — all different, so a query
    // that read the wrong table would not accidentally agree.
    for r in ["src/a.rs", "src/b.rs"] {
        bm::link_cell_code(&pool, &c, "file", r, &m, &uid)
            .await
            .unwrap()
            .unwrap();
    }
    for t in ["t1", "t2", "t3"] {
        bm::record_test_result(&pool, &c, t, "pass", None, &m, &uid)
            .await
            .unwrap()
            .unwrap();
    }
    bm::record_test_result(&pool, &c, "t4", "fail", Some("boom"), &m, &uid)
        .await
        .unwrap()
        .unwrap();

    // When
    let cells = bm::matrix_view_cells(&pool, &m, &uid).await.unwrap();
    // Then
    assert_eq!(cells.len(), 1);
    let cell = &cells[0];
    assert_eq!(cell.requirement_count, 1);
    assert_eq!(
        cell.failing_count, 0,
        "the requirement is 'untested', not 'failing'"
    );
    assert_eq!(cell.code_count, 2);
    assert_eq!(cell.passing_test_count, 3);
    assert_eq!(cell.failing_test_count, 1);

    // Flip the requirement to failing — only failing_count moves.
    requirement::update_status(&pool, &uid, &req, "failing")
        .await
        .unwrap();
    let after = bm::matrix_view_cells(&pool, &m, &uid).await.unwrap();
    assert_eq!(after[0].failing_count, 1);
    assert_eq!(
        after[0].requirement_count, 1,
        "still counted once, not twice"
    );
    assert_eq!(after[0].code_count, 2, "unrelated counts must not move");
}

#[sqlx::test]
async fn matrix_view_is_scoped(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    // When
    let (m, _, _, _) = a_cell(&pool, &alice, "M").await;

    // Then
    assert_eq!(
        bm::matrix_view_cells(&pool, &m, &alice)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        bm::matrix_view_cells(&pool, &m, &bob)
            .await
            .unwrap()
            .is_empty()
    );
}

/// The reverse lookup's three-way match, reproduced from Node: exact ref,
/// ref-as-suffix-of-path, and a `path::symbol` link.
#[sqlx::test]
async fn behaviors_for_path_matches_exact_suffix_and_symbol_refs(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool, "a@b.test").await;
    let bob = seed_user(&pool, "c@d.test").await;
    let (m, _, _, c) = a_cell(&pool, &alice, "M").await;

    bm::link_cell_code(&pool, &c, "file", "src/deep/thing.rs", &m, &alice)
        .await
        .unwrap()
        .unwrap();
    bm::link_cell_code(&pool, &c, "file", "thing.rs", &m, &alice)
        .await
        .unwrap()
        .unwrap();
    bm::link_cell_code(&pool, &c, "symbol", "src/deep/thing.rs::run", &m, &alice)
        .await
        .unwrap()
        .unwrap();
    bm::link_cell_code(&pool, &c, "file", "src/unrelated.rs", &m, &alice)
        .await
        .unwrap()
        .unwrap();

    let mut hits = bm::behaviors_for_path(&pool, &alice, "src/deep/thing.rs")
        .await
        .unwrap();
    // When
    hits.sort_by(|a, b| a.code_ref.cmp(&b.code_ref));
    let refs: Vec<&str> = hits.iter().map(|h| h.code_ref.as_str()).collect();
    // Then
    assert_eq!(
        refs,
        ["src/deep/thing.rs", "src/deep/thing.rs::run", "thing.rs"],
        "exact, symbol and suffix refs all match; the unrelated one does not"
    );
    assert_eq!(hits[0].matrix_name, "M");
    assert_eq!(hits[0].dimension_name, "D");

    assert!(
        bm::behaviors_for_path(&pool, &bob, "src/deep/thing.rs")
            .await
            .unwrap()
            .is_empty(),
        "the reverse lookup is scoped to the matrix owner"
    );
}
