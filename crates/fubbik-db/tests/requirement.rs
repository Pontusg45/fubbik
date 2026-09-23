//! Repo-level tests for `fubbik_db::repo::requirement`.
//!
//! `search_titles` (Task 8's minimal slice) is tested first; everything
//! below it covers the full CRUD + chunk-link surface this task added.

use fubbik_db::repo::requirement::{
    self, ListParams, NewRequirement, RequirementPatch, RequirementStep, StepKeyword,
};
use fubbik_db::repo::{chunk, space, use_case, user};

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
    // Given
    let alice = seed_user(&pool).await;
    let bob = user::create(&pool, "bob-search-titles@b.test", "Bob", None)
        .await
        .unwrap()
        .id;

    seed_requirement(&pool, &alice, "The Great Authentication Flow").await;

    // When
    let bobs_view = requirement::search_titles(&pool, &bob, "Authentication", 10)
        .await
        .unwrap();
    // Then
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

// ---------------------------------------------------------------------
// CRUD
// ---------------------------------------------------------------------

fn gwt_steps() -> Vec<RequirementStep> {
    vec![
        RequirementStep {
            keyword: StepKeyword::Given,
            text: "a user".into(),
            params: None,
        },
        RequirementStep {
            keyword: StepKeyword::When,
            text: "they log in".into(),
            params: None,
        },
        RequirementStep {
            keyword: StepKeyword::Then,
            text: "they see the dashboard".into(),
            params: None,
        },
    ]
}

fn new_req(title: &str) -> NewRequirement {
    NewRequirement {
        title: title.to_string(),
        description: None,
        steps: gwt_steps(),
        priority: None,
        space_id: None,
        use_case_id: None,
        origin: "human".to_string(),
        review_status: "approved".to_string(),
    }
}

#[sqlx::test]
async fn create_and_find_by_id_round_trips(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    // When
    let created = requirement::create(&pool, &alice, new_req("Login"))
        .await
        .unwrap()
        .unwrap();
    // Then
    assert_eq!(created.title, "Login");
    assert_eq!(created.status, "untested");
    assert_eq!(created.order, 0);
    assert_eq!(created.steps.0.len(), 3);

    let found = requirement::find_by_id(&pool, &alice, &created.id)
        .await
        .unwrap()
        .expect("must find just-created row");
    assert_eq!(found.id, created.id);
}

/// Proves `create`'s `space_id` ownership guard: a `space_id` belonging to
/// another user makes the insert return `None`, not create a requirement
/// pointing at data the caller cannot see.
#[sqlx::test]
async fn create_rejects_a_space_id_the_caller_does_not_own(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let bob = user::create(&pool, "bob@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let bobs_space = space::create(
        &pool,
        &bob,
        space::NewSpace {
            name: "Bob's space".into(),
            kind: "wiki".into(),
            description: None,
        },
        None,
    )
    .await
    .unwrap()
    .id;

    let mut params = new_req("Cross-user");
    params.space_id = Some(bobs_space);
    // When
    let result = requirement::create(&pool, &alice, params).await.unwrap();
    // Then
    assert!(
        result.is_none(),
        "must reject a space_id Alice does not own"
    );
}

/// Same guard, for `use_case_id`.
#[sqlx::test]
async fn create_rejects_a_use_case_id_the_caller_does_not_own(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let bob = user::create(&pool, "bob2@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let bobs_uc = use_case::create(
        &pool,
        &bob,
        use_case::NewUseCase {
            name: "Bob's UC".into(),
            description: None,
            space_id: None,
            parent_id: None,
        },
    )
    .await
    .unwrap()
    .unwrap();

    let mut params = new_req("Cross-user UC");
    params.use_case_id = Some(bobs_uc.id);
    // When
    let result = requirement::create(&pool, &alice, params).await.unwrap();
    // Then
    assert!(
        result.is_none(),
        "must reject a use_case_id Alice does not own"
    );
}

/// Proves `find_by_id`'s `user_id` scope is load-bearing: without the
/// `AND user_id = $2` predicate Bob's lookup of Alice's requirement would
/// return `Some`.
#[sqlx::test]
async fn find_by_id_is_user_scoped(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let bob = user::create(&pool, "bob3@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let created = requirement::create(&pool, &alice, new_req("Alice only"))
        .await
        .unwrap()
        .unwrap();

    // When
    let bobs_view = requirement::find_by_id(&pool, &bob, &created.id)
        .await
        .unwrap();
    // Then
    assert!(
        bobs_view.is_none(),
        "must not leak another user's requirement"
    );
}

#[sqlx::test]
async fn update_changes_only_provided_fields(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let created = requirement::create(&pool, &alice, new_req("Original"))
        .await
        .unwrap()
        .unwrap();

    // When
    let updated = requirement::update(
        &pool,
        &alice,
        &created.id,
        RequirementPatch {
            title: Some("Renamed".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .unwrap();
    // Then
    assert_eq!(updated.title, "Renamed");
    assert_eq!(
        updated.steps.0.len(),
        3,
        "steps must be untouched by a title-only patch"
    );
}

/// `description`/`priority`/`space_id`/`use_case_id` are tri-state:
/// `Some(None)` must clear the column, not leave it untouched.
#[sqlx::test]
async fn update_description_tri_state_clears_on_explicit_null(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let mut params = new_req("Has description");
    params.description = Some("original".into());
    // When
    let created = requirement::create(&pool, &alice, params)
        .await
        .unwrap()
        .unwrap();
    // Then
    assert_eq!(created.description.as_deref(), Some("original"));

    let cleared = requirement::update(
        &pool,
        &alice,
        &created.id,
        RequirementPatch {
            description: Some(None),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(cleared.description, None);
}

/// Proves `update`'s ownership scope: Bob's update of Alice's requirement
/// returns `None` AND leaves Alice's row unchanged.
#[sqlx::test]
async fn update_is_user_scoped_and_leaves_victim_unchanged(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let bob = user::create(&pool, "bob4@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let created = requirement::create(&pool, &alice, new_req("Victim"))
        .await
        .unwrap()
        .unwrap();

    // When
    let bobs_attempt = requirement::update(
        &pool,
        &bob,
        &created.id,
        RequirementPatch {
            title: Some("Hijacked".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    // Then
    assert!(bobs_attempt.is_none());

    let still_alices = requirement::find_by_id(&pool, &alice, &created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        still_alices.title, "Victim",
        "Bob's rejected update must not have touched Alice's row"
    );
}

#[sqlx::test]
async fn delete_is_user_scoped(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let bob = user::create(&pool, "bob5@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let created = requirement::create(&pool, &alice, new_req("Keep me"))
        .await
        .unwrap()
        .unwrap();

    // When
    let bobs_delete = requirement::delete(&pool, &bob, &created.id).await.unwrap();
    // Then
    assert!(
        !bobs_delete,
        "Bob must not be able to delete Alice's requirement"
    );
    assert!(
        requirement::find_by_id(&pool, &alice, &created.id)
            .await
            .unwrap()
            .is_some()
    );

    let alices_delete = requirement::delete(&pool, &alice, &created.id)
        .await
        .unwrap();
    assert!(alices_delete);
}

#[sqlx::test]
async fn update_status_is_user_scoped(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let bob = user::create(&pool, "bob6@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let created = requirement::create(&pool, &alice, new_req("Status target"))
        .await
        .unwrap()
        .unwrap();

    // When
    let bobs_attempt = requirement::update_status(&pool, &bob, &created.id, "passing")
        .await
        .unwrap();
    // Then
    assert!(bobs_attempt.is_none());

    let alices = requirement::update_status(&pool, &alice, &created.id, "passing")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(alices.status, "passing");
}

// ---------------------------------------------------------------------
// list / count: filters + tiebreaker stability
// ---------------------------------------------------------------------

fn empty_params(limit: i64) -> ListParams<'static> {
    ListParams {
        space_id: None,
        use_case_id: None,
        status: None,
        priority: None,
        origin: None,
        review_status: None,
        search: None,
        limit,
        offset: 0,
    }
}

#[sqlx::test]
async fn list_and_count_are_scoped_to_the_caller(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let bob = user::create(&pool, "bob7@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    requirement::create(&pool, &alice, new_req("Alice's"))
        .await
        .unwrap()
        .unwrap();
    requirement::create(&pool, &bob, new_req("Bob's"))
        .await
        .unwrap()
        .unwrap();

    // When
    let alices_list = requirement::list(&pool, &alice, &empty_params(50))
        .await
        .unwrap();
    // Then
    assert_eq!(alices_list.len(), 1);
    assert_eq!(alices_list[0].title, "Alice's");

    let alices_count = requirement::count(&pool, &alice, &empty_params(50))
        .await
        .unwrap();
    assert_eq!(alices_count, 1);
}

/// Forced-identical-sort-key stability: three requirements that all share
/// `order` (default `0`, never explicitly set) and the *same* `created_at`
/// (inserted in one batch via direct SQL, bypassing `now()` drift) must
/// still come back in a single, repeatable order — proving the `id ASC`
/// tiebreaker is genuinely load-bearing, not just decorative. Without it,
/// two identical calls over these exact rows could disagree.
#[sqlx::test]
async fn list_order_is_stable_when_order_and_created_at_tie(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let same_time =
        chrono::NaiveDateTime::parse_from_str("2024-01-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let mut ids = vec![];
    for title in ["C", "A", "B"] {
        let id = fubbik_db::new_id();
        sqlx::query!(
            r#"INSERT INTO requirement (id, title, steps, user_id, "order", created_at) VALUES ($1, $2, '[]'::jsonb, $3, 0, $4)"#,
            id,
            title,
            alice,
            same_time
        )
        .execute(&pool)
        .await
        .unwrap();
        ids.push(id);
    }
    ids.sort();

    let first = requirement::list(&pool, &alice, &empty_params(50))
        .await
        .unwrap();
    // When
    let second = requirement::list(&pool, &alice, &empty_params(50))
        .await
        .unwrap();
    let first_ids: Vec<String> = first.iter().map(|r| r.id.clone()).collect();
    let second_ids: Vec<String> = second.iter().map(|r| r.id.clone()).collect();
    // Then
    assert_eq!(
        first_ids, second_ids,
        "identical calls over tied sort keys must return identical order"
    );
    assert_eq!(
        first_ids, ids,
        "must be ordered by id ASC when order/created_at tie"
    );
}

#[sqlx::test]
async fn list_filters_by_status_and_search(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let a = requirement::create(&pool, &alice, new_req("Password reset flow"))
        .await
        .unwrap()
        .unwrap();
    requirement::create(&pool, &alice, new_req("Something else entirely"))
        .await
        .unwrap()
        .unwrap();
    requirement::update_status(&pool, &alice, &a.id, "passing")
        .await
        .unwrap();

    let mut params = empty_params(50);
    params.status = Some("passing");
    // When
    let results = requirement::list(&pool, &alice, &params).await.unwrap();
    // Then
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, a.id);

    let mut search_params = empty_params(50);
    search_params.search = Some("password");
    let search_results = requirement::list(&pool, &alice, &search_params)
        .await
        .unwrap();
    assert_eq!(search_results.len(), 1);
    assert_eq!(search_results[0].id, a.id);
}

// ---------------------------------------------------------------------
// set_chunks / get_chunks — the three-guard join table
// ---------------------------------------------------------------------

async fn seed_chunk(pool: &sqlx::PgPool, user_id: &str, title: &str) -> String {
    chunk::create(
        pool,
        user_id,
        chunk::NewChunk {
            title: title.into(),
            content: "c".into(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .id
}

#[sqlx::test]
async fn set_chunks_links_and_replaces(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let req = requirement::create(&pool, &alice, new_req("Linked"))
        .await
        .unwrap()
        .unwrap();
    let c1 = seed_chunk(&pool, &alice, "Chunk 1").await;
    let c2 = seed_chunk(&pool, &alice, "Chunk 2").await;

    // When
    let linked = requirement::set_chunks(&pool, &alice, &req.id, std::slice::from_ref(&c1))
        .await
        .unwrap();
    // Then
    assert_eq!(linked.len(), 1);
    assert_eq!(linked[0].chunk_id, c1);

    let chunks = requirement::get_chunks(&pool, &alice, &req.id)
        .await
        .unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].id, c1);

    // Replacing the set drops c1 and links c2 only.
    let replaced = requirement::set_chunks(&pool, &alice, &req.id, std::slice::from_ref(&c2))
        .await
        .unwrap();
    assert_eq!(replaced.len(), 1);
    assert_eq!(replaced[0].chunk_id, c2);
    let chunks_after = requirement::get_chunks(&pool, &alice, &req.id)
        .await
        .unwrap();
    assert_eq!(chunks_after.len(), 1);
    assert_eq!(chunks_after[0].id, c2);
}

/// Guard 1/3 (INSERT, parent A = `requirement`): Bob cannot attach chunks
/// to Alice's requirement. Proven load-bearing by removing the `r.user_id
/// = $1` predicate from `set_chunks`'s `INSERT ... SELECT` mentally: the
/// join would then succeed for any caller who names Alice's requirement
/// id, regardless of who owns it. With the guard, the insert returns zero
/// rows and Alice's link set stays empty.
#[sqlx::test]
async fn set_chunks_insert_guards_the_requirement_parent(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let bob = user::create(&pool, "bob8@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let req = requirement::create(&pool, &alice, new_req("Alice's requirement"))
        .await
        .unwrap()
        .unwrap();
    let bobs_chunk = seed_chunk(&pool, &bob, "Bob's chunk").await;
    // Give Bob a chunk of his own to attempt attaching — but the guard
    // under test is on the requirement side, so use an Alice-owned chunk
    // Bob has no reason to know about; what matters is Bob is not the
    // requirement's owner.
    let alices_chunk = seed_chunk(&pool, &alice, "Alice's chunk").await;
    let _ = bobs_chunk;

    // When
    let result = requirement::set_chunks(&pool, &bob, &req.id, &[alices_chunk])
        .await
        .unwrap();
    // Then
    assert_eq!(
        result.len(),
        0,
        "Bob must not be able to attach chunks to Alice's requirement"
    );

    let alices_view = requirement::get_chunks(&pool, &alice, &req.id)
        .await
        .unwrap();
    assert_eq!(
        alices_view.len(),
        0,
        "the rejected call must not have linked anything"
    );
}

/// Guard 2/3 (INSERT, parent B = `chunk`): Alice cannot attach Bob's chunk
/// to her own requirement. Proven load-bearing the same way: removing
/// `c.user_id = $1` from the join would let any caller link any user's
/// chunk id to their own requirement.
#[sqlx::test]
async fn set_chunks_insert_guards_the_chunk_parent(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let bob = user::create(&pool, "bob9@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let req = requirement::create(&pool, &alice, new_req("Alice's requirement"))
        .await
        .unwrap()
        .unwrap();
    let bobs_chunk = seed_chunk(&pool, &bob, "Bob's chunk").await;

    // When
    let result = requirement::set_chunks(&pool, &alice, &req.id, &[bobs_chunk])
        .await
        .unwrap();
    // Then
    assert_eq!(
        result.len(),
        0,
        "Alice must not be able to link Bob's chunk to her requirement"
    );
}

/// Guard 3/3 (the DELETE half of the replace-set) — the silent-data-loss
/// case the task brief calls out by name: without the `EXISTS (SELECT 1
/// FROM requirement r WHERE r.id = $1 AND r.user_id = $2)` guard on the
/// `DELETE`, a call for a requirement Bob does *not* own would still wipe
/// that requirement's real owner's (Alice's) existing chunk links, even
/// though the subsequent guarded `INSERT` correctly inserts nothing. This
/// test seeds Alice's requirement with a real link, then has Bob call
/// `set_chunks` against it; if the DELETE guard were ever removed, this
/// assertion (`chunks_after.len() == 1`) would fail because Alice's link
/// would have been deleted by Bob's rejected call.
#[sqlx::test]
async fn set_chunks_delete_guards_the_requirement_parent_no_silent_data_loss(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let bob = user::create(&pool, "bob10@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let req = requirement::create(&pool, &alice, new_req("Alice's requirement"))
        .await
        .unwrap()
        .unwrap();
    let alices_chunk = seed_chunk(&pool, &alice, "Alice's chunk").await;

    // When
    let existing =
        requirement::set_chunks(&pool, &alice, &req.id, std::slice::from_ref(&alices_chunk))
            .await
            .unwrap();
    // Then
    assert_eq!(existing.len(), 1);

    // Bob calls set_chunks against Alice's requirement id with an empty
    // (or any) chunk list — the DELETE runs unconditionally in the naive
    // version; the guard must stop it from touching Alice's row at all.
    let bobs_attempt = requirement::set_chunks(&pool, &bob, &req.id, &[])
        .await
        .unwrap();
    assert_eq!(bobs_attempt.len(), 0);

    let chunks_after = requirement::get_chunks(&pool, &alice, &req.id)
        .await
        .unwrap();
    assert_eq!(
        chunks_after.len(),
        1,
        "Bob's rejected call must not have deleted Alice's existing chunk link"
    );
    assert_eq!(chunks_after[0].id, alices_chunk);
}

// ---------------------------------------------------------------------
// bulk_update / bulk_delete / find_by_ids / set_order
// ---------------------------------------------------------------------

#[sqlx::test]
async fn bulk_update_is_user_scoped(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let bob = user::create(&pool, "bob11@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let alices_req = requirement::create(&pool, &alice, new_req("Mine"))
        .await
        .unwrap()
        .unwrap();
    let bobs_req = requirement::create(&pool, &bob, new_req("Bob's"))
        .await
        .unwrap()
        .unwrap();

    // When
    let affected = requirement::bulk_update(
        &pool,
        &alice,
        &[alices_req.id.clone(), bobs_req.id.clone()],
        requirement::BulkPatch {
            status: Some("passing".into()),
            use_case_id: None,
        },
    )
    .await
    .unwrap();
    // Then
    assert_eq!(
        affected, 1,
        "must only affect Alice's own row even though Bob's id was also in the list"
    );

    let bobs_row = requirement::find_by_id(&pool, &bob, &bobs_req.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        bobs_row.status, "untested",
        "Bob's row must be untouched by Alice's bulk update"
    );
}

#[sqlx::test]
async fn bulk_delete_is_user_scoped(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let bob = user::create(&pool, "bob12@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let alices_req = requirement::create(&pool, &alice, new_req("Mine"))
        .await
        .unwrap()
        .unwrap();
    let bobs_req = requirement::create(&pool, &bob, new_req("Bob's"))
        .await
        .unwrap()
        .unwrap();

    // When
    let affected = requirement::bulk_delete(&pool, &alice, &[alices_req.id, bobs_req.id.clone()])
        .await
        .unwrap();
    // Then
    assert_eq!(affected, 1);
    assert!(
        requirement::find_by_id(&pool, &bob, &bobs_req.id)
            .await
            .unwrap()
            .is_some(),
        "Bob's requirement must survive Alice's bulk delete"
    );
}

#[sqlx::test]
async fn find_by_ids_is_user_scoped(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let bob = user::create(&pool, "bob13@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let alices_req = requirement::create(&pool, &alice, new_req("Mine"))
        .await
        .unwrap()
        .unwrap();
    let bobs_req = requirement::create(&pool, &bob, new_req("Bob's"))
        .await
        .unwrap()
        .unwrap();

    // When
    let found = requirement::find_by_ids(&pool, &alice, &[alices_req.id.clone(), bobs_req.id])
        .await
        .unwrap();
    // Then
    assert_eq!(
        found.len(),
        1,
        "must not resolve Bob's id through Alice's scope"
    );
    assert_eq!(found[0].id, alices_req.id);
}

#[sqlx::test]
async fn set_order_applies_positions_and_is_user_scoped(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let bob = user::create(&pool, "bob14@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let a = requirement::create(&pool, &alice, new_req("A"))
        .await
        .unwrap()
        .unwrap();
    let b = requirement::create(&pool, &alice, new_req("B"))
        .await
        .unwrap()
        .unwrap();
    let bobs = requirement::create(&pool, &bob, new_req("Bob's"))
        .await
        .unwrap()
        .unwrap();

    requirement::set_order(
        &pool,
        &alice,
        &[b.id.clone(), a.id.clone(), bobs.id.clone()],
    )
    .await
    .unwrap();

    let a_after = requirement::find_by_id(&pool, &alice, &a.id)
        .await
        .unwrap()
        .unwrap();
    // When
    let b_after = requirement::find_by_id(&pool, &alice, &b.id)
        .await
        .unwrap()
        .unwrap();
    // Then
    assert_eq!(b_after.order, 0);
    assert_eq!(a_after.order, 1);

    let bobs_after = requirement::find_by_id(&pool, &bob, &bobs.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        bobs_after.order, 0,
        "Bob's row must be untouched even though its id was in Alice's reorder list"
    );
}

#[sqlx::test]
async fn stats_counts_by_status_and_is_user_scoped(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let bob = user::create(&pool, "bob15@b.test", "Bob", None)
        .await
        .unwrap()
        .id;
    let a = requirement::create(&pool, &alice, new_req("A"))
        .await
        .unwrap()
        .unwrap();
    requirement::create(&pool, &alice, new_req("B"))
        .await
        .unwrap()
        .unwrap();
    requirement::update_status(&pool, &alice, &a.id, "passing")
        .await
        .unwrap();
    requirement::create(&pool, &bob, new_req("Bob's"))
        .await
        .unwrap()
        .unwrap();

    // When
    let stats = requirement::stats(&pool, &alice, None).await.unwrap();
    // Then
    assert_eq!(stats.total, 2);
    assert_eq!(stats.passing, 1);
    assert_eq!(stats.untested, 1);
}

// ---------------------------------------------------------------------------
// requirements_for_chunks — the `requirements` array of `GET /api/chunks/{id}`
// ---------------------------------------------------------------------------

/// Returns one row per `(chunk, requirement)` link, carrying the join's
/// `chunkId` alongside the requirement's five-field slice.
#[sqlx::test]
async fn requirements_for_chunks_returns_one_row_per_link(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let chunk_a = seed_chunk(&pool, &alice, "Chunk A").await;
    let chunk_b = seed_chunk(&pool, &alice, "Chunk B").await;

    let login = requirement::create(&pool, &alice, new_req("Login"))
        .await
        .unwrap()
        .unwrap();
    let logout = requirement::create(&pool, &alice, new_req("Logout"))
        .await
        .unwrap()
        .unwrap();

    // `login` covers both chunks; `logout` covers only B.
    requirement::set_chunks(
        &pool,
        &alice,
        &login.id,
        &[chunk_a.clone(), chunk_b.clone()],
    )
    .await
    .unwrap();
    requirement::set_chunks(&pool, &alice, &logout.id, std::slice::from_ref(&chunk_b))
        .await
        .unwrap();

    let mut rows =
        requirement::requirements_for_chunks(&pool, &[chunk_a.clone(), chunk_b.clone()], &alice)
            .await
            .unwrap();
    // When
    // No ORDER BY (matching Node) — sort before asserting.
    rows.sort_by(|x, y| (&x.chunk_id, &x.title).cmp(&(&y.chunk_id, &y.title)));

    // Then
    assert_eq!(rows.len(), 3, "one row per link, not per requirement");

    let for_a: Vec<&str> = rows
        .iter()
        .filter(|r| r.chunk_id == chunk_a)
        .map(|r| r.title.as_str())
        .collect();
    assert_eq!(for_a, ["Login"]);

    let mut for_b: Vec<&str> = rows
        .iter()
        .filter(|r| r.chunk_id == chunk_b)
        .map(|r| r.title.as_str())
        .collect();
    for_b.sort_unstable();
    assert_eq!(for_b, ["Login", "Logout"]);

    // The projection's other three fields come through, including the
    // JSONB steps — a `Json<Vec<RequirementStep>>` decode failure would
    // surface here rather than as a 500 at runtime.
    let login_row = rows.iter().find(|r| r.title == "Login").unwrap();
    assert_eq!(login_row.status, "untested");
    assert_eq!(login_row.priority, None);
    assert_eq!(login_row.steps.0.len(), gwt_steps().len());
}

/// The `EXISTS (... c.user_id = $2)` guard is load-bearing: remove it and
/// this returns Alice's row to Bob. Proven at the fubbik-db layer, where
/// the guard is observed directly.
#[sqlx::test]
async fn requirements_for_chunks_is_scoped_through_the_chunks_owner(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    let bob = user::create(&pool, "bob-reqs-for-chunks@b.test", "Bob", None)
        .await
        .unwrap()
        .id;

    let alices_chunk = seed_chunk(&pool, &alice, "Alice's chunk").await;
    let req = requirement::create(&pool, &alice, new_req("Login"))
        .await
        .unwrap()
        .unwrap();
    // When
    requirement::set_chunks(&pool, &alice, &req.id, std::slice::from_ref(&alices_chunk))
        .await
        .unwrap();

    // Then
    assert_eq!(
        requirement::requirements_for_chunks(&pool, std::slice::from_ref(&alices_chunk), &alice)
            .await
            .unwrap()
            .len(),
        1,
        "the owner must see the link — otherwise the assertion below could \
         pass for the wrong reason"
    );
    assert!(
        requirement::requirements_for_chunks(&pool, std::slice::from_ref(&alices_chunk), &bob)
            .await
            .unwrap()
            .is_empty(),
        "Bob must not read the requirements linked to Alice's chunk"
    );
}

/// An empty id list short-circuits to an empty result without a round trip.
#[sqlx::test]
async fn requirements_for_chunks_with_no_ids_returns_empty(pool: sqlx::PgPool) {
    // Given
    let alice = seed_user(&pool).await;
    // When the operation is evaluated by the assertion.
    // Then
    assert!(
        requirement::requirements_for_chunks(&pool, &[], &alice)
            .await
            .unwrap()
            .is_empty()
    );
}
