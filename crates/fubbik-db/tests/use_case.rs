//! Repository-level tests for `fubbik_db::repo::use_case`. Per the task
//! brief, cross-user guard tests live at *this* layer (not just the API
//! layer above it) because a service-level pre-check 404-ing first can hide
//! a removed SQL-level guard from an API test — see each test's doc comment
//! for which guard it proves and how it was verified load-bearing.

use fubbik_db::repo::{space, use_case, user};

async fn make_user(pool: &sqlx::PgPool, email: &str, name: &str) -> String {
    user::create(pool, email, name, None).await.unwrap().id
}

fn new_use_case(
    name: &str,
    space_id: Option<String>,
    parent_id: Option<String>,
) -> use_case::NewUseCase {
    use_case::NewUseCase {
        name: name.to_string(),
        description: None,
        space_id,
        parent_id,
    }
}

#[sqlx::test]
async fn create_and_find_by_id_round_trips(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;

    let created = use_case::create(&pool, &alice, new_use_case("Login flow", None, None))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(created.name, "Login flow");
    assert_eq!(created.order, 0);
    assert!(created.parent_id.is_none());

    let found = use_case::find_by_id(&pool, &alice, &created.id)
        .await
        .unwrap()
        .expect("must find the just-created row");
    assert_eq!(found.id, created.id);
}

/// Proves the `AND user_id = $2` guard in `find_by_id`'s SQL is load-bearing
/// by removing it mentally: without that predicate, Bob's lookup of
/// Alice's use case id would return `Some(..)` instead of `None`. This test
/// fails (returns `Some`) if that predicate is ever dropped from the query.
#[sqlx::test]
async fn find_by_id_is_user_scoped(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let bob = make_user(&pool, "c@d.test", "Bob").await;

    let created = use_case::create(&pool, &alice, new_use_case("Alice's use case", None, None))
        .await
        .unwrap()
        .unwrap();

    let bob_view = use_case::find_by_id(&pool, &bob, &created.id)
        .await
        .unwrap();
    assert!(
        bob_view.is_none(),
        "another user's find_by_id must not see this row"
    );

    let alice_view = use_case::find_by_id(&pool, &alice, &created.id)
        .await
        .unwrap();
    assert!(alice_view.is_some());
}

#[sqlx::test]
async fn create_with_owned_space_succeeds(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let the_space = space::create(
        &pool,
        &alice,
        space::NewSpace {
            name: "Alice's space".to_string(),
            kind: "notes".to_string(),
            description: None,
        },
        None,
    )
    .await
    .unwrap();

    let created = use_case::create(
        &pool,
        &alice,
        new_use_case("Scoped", Some(the_space.id.clone()), None),
    )
    .await
    .unwrap()
    .expect("creating in one's own space must succeed");
    assert_eq!(created.space_id.as_deref(), Some(the_space.id.as_str()));
}

/// Load-bearing proof for Fix 2's `use_case::create` guard: the `EXISTS`
/// predicate in its own SQL, not a service-level pre-check — a caller who
/// bypasses `use_cases::service::create` (as this repo-level test does)
/// must still be rejected. Node's `createUseCaseRepo` has no such guard at
/// all (`packages/db/src/repository/use-case.ts:17-22`); this is a
/// deliberate divergence, same shape as `collection::create`'s own
/// `space_id` guard (`tests/collection.rs::create_rejects_another_users_space_and_creates_nothing`).
/// The write must be rejected outright (`None`, not a row that then gets
/// cleaned up) and must leave both users' use cases completely unaffected.
#[sqlx::test]
async fn create_rejects_another_users_space_and_creates_nothing(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let bob = make_user(&pool, "c@d.test", "Bob").await;
    let alices_space = space::create(
        &pool,
        &alice,
        space::NewSpace {
            name: "Alice's space".to_string(),
            kind: "notes".to_string(),
            description: None,
        },
        None,
    )
    .await
    .unwrap();

    let result = use_case::create(
        &pool,
        &bob,
        new_use_case("hijack", Some(alices_space.id.clone()), None),
    )
    .await
    .unwrap();
    assert!(
        result.is_none(),
        "write against another user's space must be rejected"
    );

    let bobs_use_cases = use_case::list(&pool, &bob, None).await.unwrap();
    assert!(
        bobs_use_cases.is_empty(),
        "the rejected write must not have created a row at all"
    );
    let alices_use_cases = use_case::list(&pool, &alice, None).await.unwrap();
    assert!(alices_use_cases.is_empty(), "Alice gained nothing either");
}

#[sqlx::test]
async fn list_is_user_scoped(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let bob = make_user(&pool, "c@d.test", "Bob").await;

    use_case::create(&pool, &alice, new_use_case("Alice one", None, None))
        .await
        .unwrap();
    use_case::create(&pool, &bob, new_use_case("Bob one", None, None))
        .await
        .unwrap();

    let alice_list = use_case::list(&pool, &alice, None).await.unwrap();
    assert_eq!(alice_list.len(), 1);
    assert_eq!(alice_list[0].name, "Alice one");

    let bob_list = use_case::list(&pool, &bob, None).await.unwrap();
    assert_eq!(bob_list.len(), 1);
    assert_eq!(bob_list[0].name, "Bob one");
}

#[sqlx::test]
async fn list_filters_by_space_id_when_given(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let the_space = space::create(
        &pool,
        &alice,
        space::NewSpace {
            name: "Space One".to_string(),
            kind: "notes".to_string(),
            description: None,
        },
        None,
    )
    .await
    .unwrap();

    use_case::create(
        &pool,
        &alice,
        new_use_case("In space", Some(the_space.id.clone()), None),
    )
    .await
    .unwrap();
    use_case::create(&pool, &alice, new_use_case("No space", None, None))
        .await
        .unwrap();

    let filtered = use_case::list(&pool, &alice, Some(&the_space.id))
        .await
        .unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "In space");

    let all = use_case::list(&pool, &alice, None).await.unwrap();
    assert_eq!(all.len(), 2);
}

/// `("order", name)` orders correctly and `child_count`/`requirement_count`
/// are computed. Two use cases share `order = 0` (the default); `name ASC`
/// (unique per user) is what actually disambiguates them, matching Node's
/// `listUseCases` ordering exactly.
#[sqlx::test]
async fn list_orders_by_order_then_name_and_computes_counts(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;

    let parent = use_case::create(&pool, &alice, new_use_case("Zebra", None, None))
        .await
        .unwrap()
        .unwrap();
    use_case::create(
        &pool,
        &alice,
        new_use_case("Apple", None, Some(parent.id.clone())),
    )
    .await
    .unwrap();

    let list = use_case::list(&pool, &alice, None).await.unwrap();
    // Both share order = 0, so name ASC decides: "Apple" before "Zebra".
    assert_eq!(list[0].name, "Apple");
    assert_eq!(list[1].name, "Zebra");
    assert_eq!(list[1].child_count, 1, "Zebra has one child (Apple)");
    assert_eq!(list[0].child_count, 0);

    sqlx::query!(
        r#"INSERT INTO requirement (id, title, steps, user_id, use_case_id)
           VALUES ($1, 'Req', '[]'::jsonb, $2, $3)"#,
        fubbik_db::new_id(),
        alice,
        parent.id
    )
    .execute(&pool)
    .await
    .unwrap();

    let list = use_case::list(&pool, &alice, None).await.unwrap();
    let zebra = list.iter().find(|u| u.name == "Zebra").unwrap();
    assert_eq!(zebra.requirement_count, 1);
    let apple = list.iter().find(|u| u.name == "Apple").unwrap();
    assert_eq!(apple.requirement_count, 0);
}

/// `requirement_count` must only count the caller's own requirements, even
/// though `requirement.use_case_id` carries no per-user scoping of its own
/// at the FK level — matching Node's `listUseCases`, which computes counts
/// from a query additionally filtered by `eq(requirement.userId, userId)`.
#[sqlx::test]
async fn list_requirement_count_is_scoped_to_the_caller(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let bob = make_user(&pool, "c@d.test", "Bob").await;

    let alice_uc = use_case::create(&pool, &alice, new_use_case("Alice's UC", None, None))
        .await
        .unwrap()
        .unwrap();

    // A requirement belonging to Bob, but (contrived, to prove the scoping)
    // pointing at Alice's use case id.
    sqlx::query!(
        r#"INSERT INTO requirement (id, title, steps, user_id, use_case_id)
           VALUES ($1, 'Bob req', '[]'::jsonb, $2, $3)"#,
        fubbik_db::new_id(),
        bob,
        alice_uc.id
    )
    .execute(&pool)
    .await
    .unwrap();

    let alice_list = use_case::list(&pool, &alice, None).await.unwrap();
    assert_eq!(
        alice_list[0].requirement_count, 0,
        "Bob's requirement must not count toward Alice's use case"
    );
}

#[sqlx::test]
async fn update_applies_only_provided_fields(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let created = use_case::create(&pool, &alice, new_use_case("Original", None, None))
        .await
        .unwrap()
        .unwrap();

    let updated = use_case::update(
        &pool,
        &alice,
        &created.id,
        use_case::UseCasePatch {
            name: Some("Renamed".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .expect("row must be found and updated");
    assert_eq!(updated.name, "Renamed");
    assert_eq!(updated.description, None);
}

/// `description` is tri-state: `Some(None)` must clear a previously-set
/// value, distinct from omitting the field entirely (`None`, which must
/// leave it untouched).
#[sqlx::test]
async fn update_description_tri_state(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let mut new = new_use_case("Has description", None, None);
    new.description = Some("original".to_string());
    let created = use_case::create(&pool, &alice, new).await.unwrap().unwrap();
    assert_eq!(created.description.as_deref(), Some("original"));

    // Omitted -> untouched.
    let untouched = use_case::update(
        &pool,
        &alice,
        &created.id,
        use_case::UseCasePatch {
            name: Some("still has description".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(untouched.description.as_deref(), Some("original"));

    // Explicit null -> cleared.
    let cleared = use_case::update(
        &pool,
        &alice,
        &created.id,
        use_case::UseCasePatch {
            description: Some(None),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert!(cleared.description.is_none());
}

/// Proves the `AND user_id = $2` guard in `update`'s SQL is load-bearing:
/// Bob's attempt to rename Alice's use case must return `None`, and
/// Alice's row must be provably unchanged afterward.
#[sqlx::test]
async fn update_is_user_scoped_and_leaves_the_victims_row_intact(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let bob = make_user(&pool, "c@d.test", "Bob").await;

    let created = use_case::create(&pool, &alice, new_use_case("Alice's", None, None))
        .await
        .unwrap()
        .unwrap();

    let result = use_case::update(
        &pool,
        &bob,
        &created.id,
        use_case::UseCasePatch {
            name: Some("Hijacked".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(
        result.is_none(),
        "Bob must not be able to update Alice's use case"
    );

    let still_alices = use_case::find_by_id(&pool, &alice, &created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        still_alices.name, "Alice's",
        "Alice's row must be unchanged after Bob's rejected update"
    );
}

/// A fully-omitted patch (no fields set) must not run the `UPDATE` at all —
/// so `updated_at` stays exactly as it was, matching Node's short-circuit
/// (`packages/db/src/repository/use-case.ts:103-109`), unlike
/// `collection::update`, whose `UPDATE` always runs.
#[sqlx::test]
async fn update_with_no_fields_does_not_touch_updated_at(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let created = use_case::create(&pool, &alice, new_use_case("Static", None, None))
        .await
        .unwrap()
        .unwrap();

    let result = use_case::update(
        &pool,
        &alice,
        &created.id,
        use_case::UseCasePatch::default(),
    )
    .await
    .unwrap()
    .expect("must still find and return the row");

    assert_eq!(
        result.updated_at.0, created.updated_at.0,
        "updated_at must be untouched by a fully-omitted patch"
    );
}

#[sqlx::test]
async fn delete_removes_the_row(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let created = use_case::create(&pool, &alice, new_use_case("Doomed", None, None))
        .await
        .unwrap()
        .unwrap();

    assert!(use_case::delete(&pool, &alice, &created.id).await.unwrap());
    assert!(
        use_case::find_by_id(&pool, &alice, &created.id)
            .await
            .unwrap()
            .is_none()
    );
}

/// Proves the `AND user_id = $2` guard in `delete`'s SQL is load-bearing:
/// Bob's delete of Alice's use case must report `false` (no row affected)
/// and Alice's row must survive.
#[sqlx::test]
async fn delete_is_user_scoped_and_leaves_the_victims_row_intact(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let bob = make_user(&pool, "c@d.test", "Bob").await;

    let created = use_case::create(&pool, &alice, new_use_case("Alice's", None, None))
        .await
        .unwrap()
        .unwrap();

    let deleted = use_case::delete(&pool, &bob, &created.id).await.unwrap();
    assert!(!deleted, "Bob must not be able to delete Alice's use case");

    assert!(
        use_case::find_by_id(&pool, &alice, &created.id)
            .await
            .unwrap()
            .is_some(),
        "Alice's use case must survive Bob's rejected delete"
    );
}

/// Deleting a use case nulls out dependent `requirement.use_case_id` rows
/// via the database's own `ON DELETE SET NULL` — see this module's doc
/// comment on `fubbik_db::repo::use_case` for the full writeup and how it
/// differs from Node's app-level unlink on a *successful* delete (same
/// end state) versus a *rejected* one (see the next test).
#[sqlx::test]
async fn delete_nulls_dependent_requirements_via_db_cascade(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let created = use_case::create(&pool, &alice, new_use_case("Parent UC", None, None))
        .await
        .unwrap()
        .unwrap();

    let req_id = fubbik_db::new_id();
    sqlx::query!(
        r#"INSERT INTO requirement (id, title, steps, user_id, use_case_id)
           VALUES ($1, 'Req', '[]'::jsonb, $2, $3)"#,
        req_id,
        alice,
        created.id
    )
    .execute(&pool)
    .await
    .unwrap();

    assert!(use_case::delete(&pool, &alice, &created.id).await.unwrap());

    let use_case_id: Option<String> =
        sqlx::query_scalar!("SELECT use_case_id FROM requirement WHERE id = $1", req_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        use_case_id.is_none(),
        "requirement.use_case_id must be nulled by ON DELETE SET NULL, not left dangling"
    );
}

/// The behavioural divergence from Node called out in this module's doc
/// comment: Node's app-level unlink `UPDATE` runs unconditionally,
/// *before* the ownership-scoped `DELETE`, so a non-owner's rejected
/// delete attempt still nulls out the real owner's requirement links in
/// Node. Relying purely on the DB's `ON DELETE SET NULL` does not
/// reproduce that — the cascade only fires when the row is actually
/// deleted. This proves the fix: after Bob's rejected delete of Alice's
/// use case, Alice's requirement link must still point at the (still
/// alive) use case.
#[sqlx::test]
async fn delete_by_non_owner_does_not_unlink_the_owners_requirements(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let bob = make_user(&pool, "c@d.test", "Bob").await;

    let created = use_case::create(&pool, &alice, new_use_case("Alice's UC", None, None))
        .await
        .unwrap()
        .unwrap();

    let req_id = fubbik_db::new_id();
    sqlx::query!(
        r#"INSERT INTO requirement (id, title, steps, user_id, use_case_id)
           VALUES ($1, 'Req', '[]'::jsonb, $2, $3)"#,
        req_id,
        alice,
        created.id
    )
    .execute(&pool)
    .await
    .unwrap();

    let deleted = use_case::delete(&pool, &bob, &created.id).await.unwrap();
    assert!(!deleted);

    let use_case_id: Option<String> =
        sqlx::query_scalar!("SELECT use_case_id FROM requirement WHERE id = $1", req_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        use_case_id.as_deref(),
        Some(created.id.as_str()),
        "a rejected cross-user delete must not unlink the owner's requirement"
    );
}

#[sqlx::test]
async fn list_requirements_scopes_by_use_case_and_user(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let bob = make_user(&pool, "c@d.test", "Bob").await;

    let alice_uc = use_case::create(&pool, &alice, new_use_case("Alice UC", None, None))
        .await
        .unwrap()
        .unwrap();
    let bob_uc = use_case::create(&pool, &bob, new_use_case("Bob UC", None, None))
        .await
        .unwrap()
        .unwrap();

    sqlx::query!(
        r#"INSERT INTO requirement (id, title, steps, user_id, use_case_id)
           VALUES ($1, 'Alice req', '[]'::jsonb, $2, $3)"#,
        fubbik_db::new_id(),
        alice,
        alice_uc.id
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query!(
        r#"INSERT INTO requirement (id, title, steps, user_id, use_case_id)
           VALUES ($1, 'Bob req', '[]'::jsonb, $2, $3)"#,
        fubbik_db::new_id(),
        bob,
        bob_uc.id
    )
    .execute(&pool)
    .await
    .unwrap();

    let alice_reqs = use_case::list_requirements(&pool, &alice, &alice_uc.id)
        .await
        .unwrap();
    assert_eq!(alice_reqs.len(), 1);
    assert_eq!(alice_reqs[0].title, "Alice req");

    // Cross-user: Bob passing Alice's use_case_id must see nothing, proving
    // the `AND user_id = $2` guard in `list_requirements` is load-bearing.
    let bob_view_of_alice_uc = use_case::list_requirements(&pool, &bob, &alice_uc.id)
        .await
        .unwrap();
    assert!(
        bob_view_of_alice_uc.is_empty(),
        "another user must not see requirements through someone else's use case id"
    );
}
