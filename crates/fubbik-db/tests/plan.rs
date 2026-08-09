//! `plan` repo layer — see `fubbik_db::repo::plan`'s module doc for
//! divergence #13 (the largest ownership divergence in this port: Node's
//! `getPlan(id)` selects by id alone, so any authenticated user can read
//! and write any other user's plan). Every guard proven load-bearing here
//! is named explicitly in each repo function's doc comment.

use fubbik_db::repo::chunk::{self, NewChunk};
use fubbik_db::repo::plan::{self, ListFilter};
use fubbik_db::repo::space::{self, NewSpace};
use fubbik_db::repo::user;
use sqlx::PgPool;

async fn seed_user(pool: &PgPool, email: &str) -> String {
    user::create(pool, email, "U", None).await.unwrap().id
}

async fn seed_space(pool: &PgPool, user_id: &str, name: &str) -> String {
    space::create(
        pool,
        user_id,
        NewSpace {
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

async fn seed_requirement(pool: &PgPool, user_id: &str) -> String {
    let id = fubbik_db::new_id();
    sqlx::query!(
        r#"INSERT INTO requirement (id, title, steps, user_id) VALUES ($1, 'req', '[]'::jsonb, $2)"#,
        id,
        user_id
    )
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn seed_chunk(pool: &PgPool, user_id: &str, title: &str) -> String {
    chunk::create(
        pool,
        user_id,
        NewChunk {
            title: title.into(),
            content: "content".into(),
            chunk_type: "note".into(),
            rationale: None,
        },
    )
    .await
    .unwrap()
    .id
}

// ── Given tests (verbatim from the task brief) ─────────────────────────

#[sqlx::test]
async fn find_by_id_is_user_scoped(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "Alice's plan", None, None)
        .await
        .unwrap();

    let got = plan::find_by_id(&pool, &bob, &p.id).await.unwrap();
    assert!(
        got.is_none(),
        "bob must not read alice's plan by id — Node allows this, the port must not"
    );

    let still = plan::find_by_id(&pool, &alice, &p.id).await.unwrap();
    assert!(still.is_some(), "alice must still read her own plan");
}

#[sqlx::test]
async fn update_is_user_scoped_and_leaves_the_victims_row_intact(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "Original", None, None)
        .await
        .unwrap();

    let res = plan::update(&pool, &bob, &p.id, Some("Hijacked"), None, None)
        .await
        .unwrap();
    assert!(res.is_none(), "bob's update must not match any row");

    let after = plan::find_by_id(&pool, &alice, &p.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        after.title, "Original",
        "alice's title must be unchanged after bob's rejected update"
    );
}

#[sqlx::test]
async fn list_breaks_created_at_ties_by_id(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    for i in 0..20 {
        plan::create(&pool, &alice, &format!("plan {i}"), None, None)
            .await
            .unwrap();
    }
    sqlx::query("UPDATE plan SET created_at = now()")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("ANALYZE plan").execute(&pool).await.unwrap();

    let a: Vec<String> = plan::list(&pool, &alice, Default::default())
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.id)
        .collect();
    let mut expected = a.clone();
    expected.sort();
    assert_eq!(
        a, expected,
        "ties on created_at must be broken by ascending id, not left to query-plan chance"
    );
}

// ── Additional CRUD coverage ─────────────────────────────────────────────

#[sqlx::test]
async fn create_and_find_by_id_round_trip(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let sp = seed_space(&pool, &alice, "code").await;

    let created = plan::create(
        &pool,
        &alice,
        "Ship the thing",
        Some("a description"),
        Some(&sp),
    )
    .await
    .unwrap();

    assert_eq!(created.title, "Ship the thing");
    assert_eq!(created.description.as_deref(), Some("a description"));
    assert_eq!(created.status, "draft", "status must default to draft");
    assert_eq!(created.user_id, alice);
    assert_eq!(created.space_id.as_deref(), Some(sp.as_str()));
    assert!(created.completed_at.is_none());

    let found = plan::find_by_id(&pool, &alice, &created.id)
        .await
        .unwrap()
        .expect("must find own plan");
    assert_eq!(found.id, created.id);
}

#[sqlx::test]
async fn list_is_user_scoped(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    plan::create(&pool, &alice, "alices plan", None, None)
        .await
        .unwrap();
    plan::create(&pool, &bob, "bobs plan", None, None)
        .await
        .unwrap();

    let alice_list = plan::list(&pool, &alice, ListFilter::default())
        .await
        .unwrap();
    assert_eq!(alice_list.len(), 1);
    assert_eq!(alice_list[0].title, "alices plan");
}

#[sqlx::test]
async fn list_excludes_archived_by_default_and_include_archived_returns_it(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let active = plan::create(&pool, &alice, "active", None, None)
        .await
        .unwrap();
    let archived = plan::create(&pool, &alice, "archived", None, None)
        .await
        .unwrap();
    plan::update(&pool, &alice, &archived.id, None, None, Some("archived"))
        .await
        .unwrap();

    let default_list = plan::list(&pool, &alice, ListFilter::default())
        .await
        .unwrap();
    let default_ids: Vec<&str> = default_list.iter().map(|p| p.id.as_str()).collect();
    assert!(default_ids.contains(&active.id.as_str()));
    assert!(
        !default_ids.contains(&archived.id.as_str()),
        "archived plans must be excluded by default"
    );

    let with_archived = plan::list(
        &pool,
        &alice,
        ListFilter {
            include_archived: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let with_archived_ids: Vec<&str> = with_archived.iter().map(|p| p.id.as_str()).collect();
    assert!(with_archived_ids.contains(&archived.id.as_str()));

    // An explicit status filter bypasses the archived exclusion entirely,
    // matching Node's `if (!filter.includeArchived && !filter.status)`.
    let status_filtered = plan::list(
        &pool,
        &alice,
        ListFilter {
            status: Some("archived".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(status_filtered.len(), 1);
    assert_eq!(status_filtered[0].id, archived.id);
}

#[sqlx::test]
async fn list_filters_by_space_id(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let sp1 = seed_space(&pool, &alice, "space-one").await;
    let sp2 = seed_space(&pool, &alice, "space-two").await;
    plan::create(&pool, &alice, "in space one", None, Some(&sp1))
        .await
        .unwrap();
    plan::create(&pool, &alice, "in space two", None, Some(&sp2))
        .await
        .unwrap();

    let filtered = plan::list(
        &pool,
        &alice,
        ListFilter {
            space_id: Some(sp1.clone()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "in space one");
}

#[sqlx::test]
async fn update_with_no_fields_is_a_reselect_and_does_not_bump_updated_at(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let created = plan::create(&pool, &alice, "Original", None, None)
        .await
        .unwrap();

    let updated = plan::update(&pool, &alice, &created.id, None, None, None)
        .await
        .unwrap()
        .expect("no-op patch must still find the row");
    assert_eq!(updated.updated_at, created.updated_at);
}

/// `update`'s no-op reselect branch (all three of `title`/`description`/
/// `status` are `None`) carries its own `WHERE id = $1 AND user_id = $2`
/// guard, entirely separate from the `UPDATE` branch's — see `plan::update`'s
/// doc comment. `update_is_user_scoped_and_leaves_the_victims_row_intact`
/// only exercises the `UPDATE` branch (its patch sets `title`), so it cannot
/// prove this one holds; this test sends an empty patch specifically to
/// route through the `SELECT`-only branch instead.
#[sqlx::test]
async fn update_with_no_fields_is_user_scoped(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let alices_plan = plan::create(&pool, &alice, "alices plan", None, None)
        .await
        .unwrap();

    let result = plan::update(&pool, &bob, &alices_plan.id, None, None, None)
        .await
        .unwrap();
    assert!(
        result.is_none(),
        "bob's no-op patch must not find alice's plan"
    );

    let still = plan::find_by_id(&pool, &alice, &alices_plan.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(still.title, "alices plan", "alice's plan must be untouched");
}

#[sqlx::test]
async fn delete_is_user_scoped(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let alices_plan = plan::create(&pool, &alice, "alices plan", None, None)
        .await
        .unwrap();

    let deleted = plan::delete(&pool, &bob, &alices_plan.id).await.unwrap();
    assert!(!deleted, "bob must not be able to delete alice's plan");
    assert!(
        plan::find_by_id(&pool, &alice, &alices_plan.id)
            .await
            .unwrap()
            .is_some(),
        "alice's plan must survive bob's rejected delete"
    );
}

#[sqlx::test]
async fn delete_removes_the_callers_own_plan(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "to delete", None, None)
        .await
        .unwrap();

    let deleted = plan::delete(&pool, &alice, &p.id).await.unwrap();
    assert!(deleted);
    assert!(
        plan::find_by_id(&pool, &alice, &p.id)
            .await
            .unwrap()
            .is_none()
    );
}

// ── duplicate ────────────────────────────────────────────────────────────

#[sqlx::test]
async fn duplicate_is_user_scoped(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let alices_plan = plan::create(&pool, &alice, "alices plan", None, None)
        .await
        .unwrap();

    let result = plan::duplicate(&pool, &bob, &alices_plan.id).await.unwrap();
    assert!(
        result.is_none(),
        "bob must not be able to duplicate alice's plan"
    );

    let bobs_plans = plan::list(&pool, &bob, ListFilter::default())
        .await
        .unwrap();
    assert!(
        bobs_plans.is_empty(),
        "bob's rejected duplicate must not create any plan owned by bob"
    );

    let alices_own_duplicate = plan::duplicate(&pool, &alice, &alices_plan.id)
        .await
        .unwrap();
    assert!(
        alices_own_duplicate.is_some(),
        "alice must still be able to duplicate her own plan"
    );
}

#[sqlx::test]
async fn duplicate_copies_children_and_resets_task_status(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let source = plan::create(&pool, &alice, "Source Plan", Some("desc"), None)
        .await
        .unwrap();

    // A requirement, linked via plan_requirement (no repo layer for this
    // table yet — inserted directly, matching the technique used elsewhere
    // in this crate's tests to construct fixtures ahead of their own repo
    // module, e.g. `workspace.rs`'s direct `workspace_space` insert).
    let requirement_id = fubbik_db::new_id();
    sqlx::query(
        "INSERT INTO requirement (id, title, steps, user_id) VALUES ($1, 'req', '[]'::jsonb, $2)",
    )
    .bind(&requirement_id)
    .bind(&alice)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(r#"INSERT INTO plan_requirement (id, plan_id, requirement_id, "order") VALUES ($1, $2, $3, 3)"#)
        .bind(fubbik_db::new_id())
        .bind(&source.id)
        .bind(&requirement_id)
        .execute(&pool)
        .await
        .unwrap();

    // An analyze item.
    sqlx::query(
        r#"INSERT INTO plan_analyze_item (id, plan_id, kind, "order", text) VALUES ($1, $2, 'risk', 0, 'a risk')"#,
    )
    .bind(fubbik_db::new_id())
    .bind(&source.id)
    .execute(&pool)
    .await
    .unwrap();

    // Two tasks, one blocked on the other, plus a chunk link on the first.
    let chunk_id = seed_chunk(&pool, &alice, "linked chunk").await;
    let task_a = fubbik_db::new_id();
    let task_b = fubbik_db::new_id();
    sqlx::query(
        r#"INSERT INTO plan_task (id, plan_id, title, "order", status) VALUES ($1, $2, 'Task A', 0, 'done')"#,
    )
    .bind(&task_a)
    .bind(&source.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO plan_task (id, plan_id, title, "order", status) VALUES ($1, $2, 'Task B', 1, 'blocked')"#,
    )
    .bind(&task_b)
    .bind(&source.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO plan_task_chunk (id, task_id, chunk_id, relation) VALUES ($1, $2, $3, 'context')")
        .bind(fubbik_db::new_id())
        .bind(&task_a)
        .bind(&chunk_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO plan_task_dependency (id, task_id, depends_on_task_id) VALUES ($1, $2, $3)",
    )
    .bind(fubbik_db::new_id())
    .bind(&task_b)
    .bind(&task_a)
    .execute(&pool)
    .await
    .unwrap();

    let copy = plan::duplicate(&pool, &alice, &source.id)
        .await
        .unwrap()
        .expect("duplicate of an owned plan must succeed");

    assert_eq!(copy.title, "Source Plan (copy)");
    assert_eq!(copy.description.as_deref(), Some("desc"));
    assert_eq!(copy.status, "draft");
    assert_ne!(copy.id, source.id);

    let req_rows: Vec<(String, i32)> = sqlx::query_as(
        r#"SELECT requirement_id, "order" FROM plan_requirement WHERE plan_id = $1"#,
    )
    .bind(&copy.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(req_rows, vec![(requirement_id, 3)]);

    let analyze_rows: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT kind, text FROM plan_analyze_item WHERE plan_id = $1")
            .bind(&copy.id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        analyze_rows,
        vec![("risk".to_string(), Some("a risk".to_string()))]
    );

    let task_rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, title, status FROM plan_task WHERE plan_id = $1 ORDER BY \"order\"",
    )
    .bind(&copy.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(task_rows.len(), 2);
    assert_eq!(task_rows[0].1, "Task A");
    assert_eq!(task_rows[1].1, "Task B");
    assert_eq!(
        task_rows[0].2, "pending",
        "duplicated tasks must reset to pending regardless of source status"
    );
    assert_eq!(task_rows[1].2, "pending");
    let new_task_a = &task_rows[0].0;
    let new_task_b = &task_rows[1].0;
    assert_ne!(new_task_a, &task_a);
    assert_ne!(new_task_b, &task_b);

    let chunk_link: Vec<(String, String)> =
        sqlx::query_as("SELECT task_id, chunk_id FROM plan_task_chunk WHERE task_id = $1")
            .bind(new_task_a)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(chunk_link, vec![(new_task_a.clone(), chunk_id.clone())]);

    let dep_rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT task_id, depends_on_task_id FROM plan_task_dependency WHERE task_id = $1",
    )
    .bind(new_task_b)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(dep_rows, vec![(new_task_b.clone(), new_task_a.clone())]);

    // Source rows must be untouched.
    let source_tasks: Vec<(String,)> =
        sqlx::query_as("SELECT status FROM plan_task WHERE plan_id = $1 AND status = 'done'")
            .bind(&source.id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        source_tasks.len(),
        1,
        "source task status must be untouched"
    );
}

// ── Requirement links + analyze items (Task 5) ──────────────────────────

// ── Given tests (verbatim from the task brief) ───────────────────────

#[sqlx::test]
async fn reorder_leaves_unmentioned_rows_untouched(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let a = plan::create_analyze_item(&pool, &alice, &p.id, "risk", None, None, Some("a"), None)
        .await
        .unwrap();
    let b = plan::create_analyze_item(&pool, &alice, &p.id, "risk", None, None, Some("b"), None)
        .await
        .unwrap();
    let c = plan::create_analyze_item(&pool, &alice, &p.id, "risk", None, None, Some("c"), None)
        .await
        .unwrap();
    let c_order_before = c.order;

    // mention only a and b, swapped
    plan::reorder_analyze_items(&pool, &alice, &p.id, "risk", &[b.id.clone(), a.id.clone()])
        .await
        .unwrap();

    let items = plan::list_analyze_items(&pool, &alice, &p.id)
        .await
        .unwrap();
    let c_after = items.iter().find(|i| i.id == c.id).unwrap();
    assert_eq!(
        c_after.order, c_order_before,
        "a row absent from the reorder request must keep its original order, not be renumbered"
    );
}

#[sqlx::test]
async fn analyze_kind_accepts_only_the_five_known_kinds(pool: PgPool) {
    // Validation lives in the SERVICE layer, matching Node. The DB column is free text.
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let raw =
        sqlx::query("INSERT INTO plan_analyze_item (id, plan_id, kind) VALUES ($1,$2,'nonsense')")
            .bind("x")
            .bind(&p.id)
            .execute(&pool)
            .await;
    assert!(
        raw.is_ok(),
        "the DB must NOT constrain kind — Node has no CHECK and no enum here"
    );
}

#[sqlx::test]
async fn cannot_add_a_requirement_to_another_users_plan(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let r = seed_requirement(&pool, &alice).await;

    let res = plan::add_requirement(&pool, &bob, &p.id, &r).await.unwrap();
    assert!(
        res.is_none(),
        "ownership derives from plan.user_id, guarded in SQL"
    );

    let links = plan::list_requirements(&pool, &alice, &p.id).await.unwrap();
    assert!(
        links.is_empty(),
        "the victim's plan must have gained no requirement link"
    );
}

// ── Additional coverage ──────────────────────────────────────────────

#[sqlx::test]
async fn cannot_create_an_analyze_item_for_another_users_plan(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();

    let res =
        plan::create_analyze_item(&pool, &bob, &p.id, "risk", None, None, Some("evil"), None).await;
    assert!(
        res.is_err(),
        "ownership derives from plan.user_id, guarded in SQL"
    );

    let items = plan::list_analyze_items(&pool, &alice, &p.id)
        .await
        .unwrap();
    assert!(
        items.is_empty(),
        "the victim's plan must have gained no analyze item"
    );
}

#[sqlx::test]
async fn remove_requirement_returns_true_then_false(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let r = seed_requirement(&pool, &alice).await;
    plan::add_requirement(&pool, &alice, &p.id, &r)
        .await
        .unwrap();

    let first = plan::remove_requirement(&pool, &alice, &p.id, &r)
        .await
        .unwrap();
    assert!(first, "the link existed and must be removed");

    let second = plan::remove_requirement(&pool, &alice, &p.id, &r)
        .await
        .unwrap();
    assert!(!second, "a second removal of the same link must be a no-op");
}

#[sqlx::test]
async fn cannot_remove_another_users_requirement_link(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let r = seed_requirement(&pool, &alice).await;
    plan::add_requirement(&pool, &alice, &p.id, &r)
        .await
        .unwrap();

    let res = plan::remove_requirement(&pool, &bob, &p.id, &r)
        .await
        .unwrap();
    assert!(!res, "bob does not own the plan; the link must survive");

    let links = plan::list_requirements(&pool, &alice, &p.id).await.unwrap();
    assert_eq!(links.len(), 1, "the victim's link must be untouched");
}

#[sqlx::test]
async fn reorder_requirements_leaves_unmentioned_rows_untouched(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let ra = seed_requirement(&pool, &alice).await;
    let rb = seed_requirement(&pool, &alice).await;
    let rc = seed_requirement(&pool, &alice).await;
    plan::add_requirement(&pool, &alice, &p.id, &ra)
        .await
        .unwrap();
    plan::add_requirement(&pool, &alice, &p.id, &rb)
        .await
        .unwrap();
    let c = plan::add_requirement(&pool, &alice, &p.id, &rc)
        .await
        .unwrap()
        .unwrap();
    let c_order_before = c.order;

    plan::reorder_requirements(&pool, &alice, &p.id, &[rb.clone(), ra.clone()])
        .await
        .unwrap();

    let links = plan::list_requirements(&pool, &alice, &p.id).await.unwrap();
    let c_after = links.iter().find(|l| l.requirement_id == rc).unwrap();
    assert_eq!(
        c_after.order, c_order_before,
        "unmentioned requirement link must keep its order"
    );
}

#[sqlx::test]
async fn update_analyze_item_round_trip_and_leaves_kind_unchanged(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let item = plan::create_analyze_item(
        &pool,
        &alice,
        &p.id,
        "risk",
        None,
        None,
        Some("initial"),
        None,
    )
    .await
    .unwrap();

    let updated = plan::update_analyze_item(
        &pool,
        &alice,
        &p.id,
        &item.id,
        Some("changed"),
        Some(serde_json::json!({"severity": "high"})),
        None,
        None,
    )
    .await
    .unwrap()
    .expect("owned item must update");

    assert_eq!(updated.text.as_deref(), Some("changed"));
    assert_eq!(updated.kind, "risk", "kind must never change via PATCH");
    assert_eq!(updated.metadata.0, serde_json::json!({"severity": "high"}));
}

#[sqlx::test]
async fn cannot_update_another_users_analyze_item(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let item =
        plan::create_analyze_item(&pool, &alice, &p.id, "risk", None, None, Some("mine"), None)
            .await
            .unwrap();

    let res = plan::update_analyze_item(
        &pool,
        &bob,
        &p.id,
        &item.id,
        Some("hijacked"),
        None,
        None,
        None,
    )
    .await
    .unwrap();
    assert!(res.is_none(), "bob does not own the plan");

    let items = plan::list_analyze_items(&pool, &alice, &p.id)
        .await
        .unwrap();
    assert_eq!(
        items[0].text.as_deref(),
        Some("mine"),
        "the victim's item must be untouched"
    );
}

#[sqlx::test]
async fn delete_analyze_item_returns_true_then_false(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let item = plan::create_analyze_item(&pool, &alice, &p.id, "risk", None, None, Some("x"), None)
        .await
        .unwrap();

    let first = plan::delete_analyze_item(&pool, &alice, &p.id, &item.id)
        .await
        .unwrap();
    assert!(first);
    let second = plan::delete_analyze_item(&pool, &alice, &p.id, &item.id)
        .await
        .unwrap();
    assert!(!second);
}

#[sqlx::test]
async fn cannot_delete_another_users_analyze_item(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let item =
        plan::create_analyze_item(&pool, &alice, &p.id, "risk", None, None, Some("mine"), None)
            .await
            .unwrap();

    let res = plan::delete_analyze_item(&pool, &bob, &p.id, &item.id)
        .await
        .unwrap();
    assert!(!res, "bob does not own the plan");

    let items = plan::list_analyze_items(&pool, &alice, &p.id)
        .await
        .unwrap();
    assert_eq!(items.len(), 1, "the victim's item must survive");
}

#[sqlx::test]
async fn create_analyze_item_order_is_scoped_per_kind(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let r1 = plan::create_analyze_item(&pool, &alice, &p.id, "risk", None, None, Some("r1"), None)
        .await
        .unwrap();
    let f1 =
        plan::create_analyze_item(&pool, &alice, &p.id, "file", None, Some("a.rs"), None, None)
            .await
            .unwrap();
    let r2 = plan::create_analyze_item(&pool, &alice, &p.id, "risk", None, None, Some("r2"), None)
        .await
        .unwrap();

    assert_eq!(r1.order, 0);
    assert_eq!(
        f1.order, 0,
        "a different kind starts its own order sequence at 0"
    );
    assert_eq!(r2.order, 1);
}

// ── Ordering stability (tie-break by id) ─────────────────────────────

#[sqlx::test]
async fn list_analyze_items_breaks_order_ties_by_id(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    for i in 0..20 {
        plan::create_analyze_item(
            &pool,
            &alice,
            &p.id,
            "risk",
            None,
            None,
            Some(&format!("r{i}")),
            None,
        )
        .await
        .unwrap();
    }
    sqlx::query(r#"UPDATE plan_analyze_item SET "order" = 0 WHERE plan_id = $1"#)
        .bind(&p.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("ANALYZE plan_analyze_item")
        .execute(&pool)
        .await
        .unwrap();

    let items = plan::list_analyze_items(&pool, &alice, &p.id)
        .await
        .unwrap();
    let ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
    let mut expected = ids.clone();
    expected.sort();
    assert_eq!(
        ids, expected,
        "ties on kind+order must be broken by ascending id, not left to query-plan chance"
    );
}

#[sqlx::test]
async fn list_requirements_breaks_order_ties_by_id(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    for _ in 0..20 {
        let r = seed_requirement(&pool, &alice).await;
        plan::add_requirement(&pool, &alice, &p.id, &r)
            .await
            .unwrap();
    }
    sqlx::query(r#"UPDATE plan_requirement SET "order" = 0 WHERE plan_id = $1"#)
        .bind(&p.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("ANALYZE plan_requirement")
        .execute(&pool)
        .await
        .unwrap();

    let links = plan::list_requirements(&pool, &alice, &p.id).await.unwrap();
    let ids: Vec<String> = links.iter().map(|l| l.id.clone()).collect();
    let mut expected = ids.clone();
    expected.sort();
    assert_eq!(
        ids, expected,
        "ties on order must be broken by ascending id, not left to query-plan chance"
    );
}

// ── Tasks (Task 6) ───────────────────────────────────────────────────

async fn seed_task(pool: &PgPool, user_id: &str, plan_id: &str, title: &str) -> plan::PlanTask {
    plan::create_task(
        pool,
        user_id,
        plan_id,
        title,
        None,
        serde_json::json!([]),
        None,
    )
    .await
    .unwrap()
    .expect("owned plan must accept a new task")
}

// ── B1: list_tasks order tiebreaker ─────────────────────────────────

#[sqlx::test]
async fn list_tasks_breaks_order_ties_by_id(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    for i in 0..20 {
        seed_task(&pool, &alice, &p.id, &format!("t{i}")).await;
    }
    sqlx::query(r#"UPDATE plan_task SET "order" = 0 WHERE plan_id = $1"#)
        .bind(&p.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("ANALYZE plan_task")
        .execute(&pool)
        .await
        .unwrap();

    let tasks = plan::list_tasks(&pool, &alice, &p.id).await.unwrap();
    let ids: Vec<String> = tasks.iter().map(|t| t.id.clone()).collect();
    let mut expected = ids.clone();
    expected.sort();
    assert_eq!(
        ids, expected,
        "ties on order must be broken by ascending id, not left to query-plan chance"
    );
}

#[sqlx::test]
async fn find_task_by_id_is_user_scoped(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let t = seed_task(&pool, &alice, &p.id, "a").await;

    let bobs_view = plan::find_task_by_id(&pool, &bob, &p.id, &t.id)
        .await
        .unwrap();
    assert!(bobs_view.is_none(), "bob must not read alice's task by id");

    let alices_view = plan::find_task_by_id(&pool, &alice, &p.id, &t.id)
        .await
        .unwrap();
    assert!(alices_view.is_some(), "alice must still read her own task");
}

#[sqlx::test]
async fn create_task_defaults_status_to_pending_and_appends_order(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();

    let a = plan::create_task(
        &pool,
        &alice,
        &p.id,
        "a",
        Some("desc"),
        serde_json::json!([{"text": "x", "done": false}]),
        Some(serde_json::json!({"k": "v"})),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(a.status, "pending");
    assert_eq!(a.order, 0);
    assert_eq!(a.metadata.0, serde_json::json!({"k": "v"}));

    let b = seed_task(&pool, &alice, &p.id, "b").await;
    assert_eq!(b.order, 1);
    assert_eq!(
        b.metadata.0,
        serde_json::json!({}),
        "metadata defaults to {{}} when omitted"
    );
}

#[sqlx::test]
async fn cannot_create_a_task_for_another_users_plan(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();

    let res = plan::create_task(
        &pool,
        &bob,
        &p.id,
        "evil",
        None,
        serde_json::json!([]),
        None,
    )
    .await
    .unwrap();
    assert!(
        res.is_none(),
        "ownership derives from plan.user_id, guarded in SQL"
    );

    let tasks = plan::list_tasks(&pool, &alice, &p.id).await.unwrap();
    assert!(
        tasks.is_empty(),
        "the victim's plan must have gained no task"
    );
}

#[sqlx::test]
async fn update_task_round_trip_and_no_op_is_a_reselect(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let t = seed_task(&pool, &alice, &p.id, "original").await;

    let updated = plan::update_task(
        &pool,
        &alice,
        &p.id,
        &t.id,
        Some("changed"),
        Some(None),
        None,
        None,
        Some("in_progress"),
    )
    .await
    .unwrap()
    .expect("owned task must update");
    assert_eq!(updated.title, "changed");
    assert!(
        updated.description.is_none(),
        "explicit null must clear description"
    );
    assert_eq!(updated.status, "in_progress");

    let noop = plan::update_task(&pool, &alice, &p.id, &t.id, None, None, None, None, None)
        .await
        .unwrap()
        .expect("no-op patch must still find the row");
    assert_eq!(
        noop.updated_at, updated.updated_at,
        "a no-op patch must not bump updated_at"
    );
}

#[sqlx::test]
async fn cannot_update_another_users_task(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let t = seed_task(&pool, &alice, &p.id, "mine").await;

    let res = plan::update_task(
        &pool,
        &bob,
        &p.id,
        &t.id,
        Some("hijacked"),
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    assert!(res.is_none(), "bob does not own the plan");

    let still = plan::find_task_by_id(&pool, &alice, &p.id, &t.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(still.title, "mine", "the victim's task must be untouched");
}

#[sqlx::test]
async fn cannot_delete_another_users_task(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let t = seed_task(&pool, &alice, &p.id, "mine").await;

    let res = plan::delete_task(&pool, &bob, &p.id, &t.id).await.unwrap();
    assert!(!res, "bob does not own the plan; the task must survive");

    assert!(
        plan::find_task_by_id(&pool, &alice, &p.id, &t.id)
            .await
            .unwrap()
            .is_some(),
        "alice's task must survive bob's rejected delete"
    );

    let deleted = plan::delete_task(&pool, &alice, &p.id, &t.id)
        .await
        .unwrap();
    assert!(deleted, "alice can delete her own task");
}

#[sqlx::test]
async fn reorder_tasks_leaves_unmentioned_rows_untouched(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let a = seed_task(&pool, &alice, &p.id, "a").await;
    let b = seed_task(&pool, &alice, &p.id, "b").await;
    let c = seed_task(&pool, &alice, &p.id, "c").await;
    let c_order_before = c.order;

    plan::reorder_tasks(&pool, &alice, &p.id, &[b.id.clone(), a.id.clone()])
        .await
        .unwrap();

    let tasks = plan::list_tasks(&pool, &alice, &p.id).await.unwrap();
    let c_after = tasks.iter().find(|t| t.id == c.id).unwrap();
    assert_eq!(
        c_after.order, c_order_before,
        "a row absent from the reorder request must keep its original order, not be renumbered"
    );
}

#[sqlx::test]
async fn cannot_reorder_another_users_tasks(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let a = seed_task(&pool, &alice, &p.id, "a").await;
    let b = seed_task(&pool, &alice, &p.id, "b").await;

    // bob's reorder call runs against alice's plan id; every UPDATE inside
    // it matches zero rows because the ownership guard fails.
    plan::reorder_tasks(&pool, &bob, &p.id, &[b.id.clone(), a.id.clone()])
        .await
        .unwrap();

    let tasks = plan::list_tasks(&pool, &alice, &p.id).await.unwrap();
    let a_after = tasks.iter().find(|t| t.id == a.id).unwrap();
    let b_after = tasks.iter().find(|t| t.id == b.id).unwrap();
    assert_eq!(
        a_after.order, a.order,
        "bob's rejected reorder must not touch alice's task order"
    );
    assert_eq!(
        b_after.order, b.order,
        "bob's rejected reorder must not touch alice's task order"
    );
}

// ── B2: list_task_chunks_with_titles ownership guard ────────────────

#[sqlx::test]
async fn list_task_chunks_with_titles_is_user_scoped(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let t = seed_task(&pool, &alice, &p.id, "a").await;
    let chunk_id = seed_chunk(&pool, &alice, "c").await;
    plan::add_task_chunk(&pool, &alice, &p.id, &t.id, &chunk_id, "context")
        .await
        .unwrap();

    let bobs_view = plan::list_task_chunks_with_titles(&pool, &bob, &p.id, &t.id)
        .await
        .unwrap();
    assert!(
        bobs_view.is_empty(),
        "bob must not read alice's task-chunk links, even by a correct task id"
    );

    let alices_view = plan::list_task_chunks_with_titles(&pool, &alice, &p.id, &t.id)
        .await
        .unwrap();
    assert_eq!(alices_view.len(), 1);
    assert_eq!(alices_view[0].chunk_title.as_deref(), Some("c"));
}

#[sqlx::test]
async fn cannot_add_a_chunk_to_another_users_task(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let t = seed_task(&pool, &alice, &p.id, "a").await;
    let chunk_id = seed_chunk(&pool, &bob, "evil-chunk").await;

    let res = plan::add_task_chunk(&pool, &bob, &p.id, &t.id, &chunk_id, "context")
        .await
        .unwrap();
    assert!(res.is_none(), "ownership derives through plan_task -> plan");

    let links = plan::list_task_chunks_with_titles(&pool, &alice, &p.id, &t.id)
        .await
        .unwrap();
    assert!(
        links.is_empty(),
        "the victim's task must have gained no chunk link"
    );
}

#[sqlx::test]
async fn cannot_remove_another_users_task_chunk_link(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let t = seed_task(&pool, &alice, &p.id, "a").await;
    let chunk_id = seed_chunk(&pool, &alice, "c").await;
    let link = plan::add_task_chunk(&pool, &alice, &p.id, &t.id, &chunk_id, "context")
        .await
        .unwrap()
        .unwrap();

    let res = plan::remove_task_chunk(&pool, &bob, &p.id, &t.id, &link.id)
        .await
        .unwrap();
    assert!(!res, "bob does not own the plan; the link must survive");

    let links = plan::list_task_chunks_with_titles(&pool, &alice, &p.id, &t.id)
        .await
        .unwrap();
    assert_eq!(links.len(), 1, "the victim's link must be untouched");
}

// ── Task dependencies ────────────────────────────────────────────────

#[sqlx::test]
async fn cannot_add_a_dependency_to_another_users_task(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let a = seed_task(&pool, &alice, &p.id, "a").await;
    let b = seed_task(&pool, &alice, &p.id, "b").await;

    let res = plan::add_task_dependency(&pool, &bob, &p.id, &b.id, &a.id)
        .await
        .unwrap();
    assert!(res.is_none(), "ownership derives through plan_task -> plan");

    let deps = plan::list_task_dependencies(&pool, &alice, &p.id)
        .await
        .unwrap();
    assert!(
        deps.is_empty(),
        "the victim's task must have gained no dependency"
    );
}

#[sqlx::test]
async fn cannot_remove_another_users_task_dependency(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let a = seed_task(&pool, &alice, &p.id, "a").await;
    let b = seed_task(&pool, &alice, &p.id, "b").await;
    let dep = plan::add_task_dependency(&pool, &alice, &p.id, &b.id, &a.id)
        .await
        .unwrap()
        .unwrap();

    let res = plan::remove_task_dependency(&pool, &bob, &p.id, &b.id, &dep.id)
        .await
        .unwrap();
    assert!(
        !res,
        "bob does not own the plan; the dependency must survive"
    );

    let deps = plan::list_task_dependencies(&pool, &alice, &p.id)
        .await
        .unwrap();
    assert_eq!(deps.len(), 1, "the victim's dependency must be untouched");
}

// ── Task external links ──────────────────────────────────────────────

#[sqlx::test]
async fn list_task_links_is_user_scoped(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let t = seed_task(&pool, &alice, &p.id, "a").await;
    plan::add_task_link(
        &pool,
        &alice,
        &p.id,
        &t.id,
        "url",
        "https://alice.example",
        None,
    )
    .await
    .unwrap();

    let bobs_view = plan::list_task_links(&pool, &bob, &p.id, &t.id)
        .await
        .unwrap();
    assert!(
        bobs_view.is_empty(),
        "bob must not read alice's task links, even by a correct task id"
    );

    let alices_view = plan::list_task_links(&pool, &alice, &p.id, &t.id)
        .await
        .unwrap();
    assert_eq!(alices_view.len(), 1);
}

#[sqlx::test]
async fn cannot_add_a_link_to_another_users_task(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let t = seed_task(&pool, &alice, &p.id, "a").await;

    let res = plan::add_task_link(
        &pool,
        &bob,
        &p.id,
        &t.id,
        "url",
        "https://evil.example",
        None,
    )
    .await
    .unwrap();
    assert!(res.is_none(), "ownership derives through plan_task -> plan");

    let links = plan::list_task_links(&pool, &alice, &p.id, &t.id)
        .await
        .unwrap();
    assert!(
        links.is_empty(),
        "the victim's task must have gained no link"
    );
}

#[sqlx::test]
async fn cannot_remove_another_users_task_link(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let t = seed_task(&pool, &alice, &p.id, "a").await;
    let link = plan::add_task_link(
        &pool,
        &alice,
        &p.id,
        &t.id,
        "url",
        "https://alice.example",
        None,
    )
    .await
    .unwrap()
    .unwrap();

    let res = plan::remove_task_link(&pool, &bob, &p.id, &t.id, &link.id)
        .await
        .unwrap();
    assert!(!res, "bob does not own the plan; the link must survive");

    let links = plan::list_task_links(&pool, &alice, &p.id, &t.id)
        .await
        .unwrap();
    assert_eq!(links.len(), 1, "the victim's link must be untouched");
}

// ── Divergence #12: mark_task_done_and_unblock, the first transaction ──
// Given tests (verbatim semantics from the task brief, adapted to this
// port's actual `create_task`/`add_task_dependency` signatures).

#[sqlx::test]
async fn marking_done_and_unblocking_is_atomic(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let a = seed_task(&pool, &alice, &p.id, "a").await;
    let b = seed_task(&pool, &alice, &p.id, "b").await;
    plan::add_task_dependency(&pool, &alice, &p.id, &b.id, &a.id)
        .await
        .unwrap();
    plan::update_task(
        &pool,
        &alice,
        &p.id,
        &b.id,
        None,
        None,
        None,
        None,
        Some("blocked"),
    )
    .await
    .unwrap();

    let unblocked = plan::mark_task_done_and_unblock(&pool, &alice, &p.id, &a.id)
        .await
        .unwrap();
    assert_eq!(unblocked, vec![b.id.clone()]);

    let tasks = plan::list_tasks(&pool, &alice, &p.id).await.unwrap();
    assert_eq!(tasks.iter().find(|t| t.id == a.id).unwrap().status, "done");
    assert_eq!(
        tasks.iter().find(|t| t.id == b.id).unwrap().status,
        "pending"
    );
}

#[sqlx::test]
async fn only_blocked_dependents_are_unblocked(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let a = seed_task(&pool, &alice, &p.id, "a").await;
    let b = seed_task(&pool, &alice, &p.id, "b").await;
    plan::add_task_dependency(&pool, &alice, &p.id, &b.id, &a.id)
        .await
        .unwrap();
    plan::update_task(
        &pool,
        &alice,
        &p.id,
        &b.id,
        None,
        None,
        None,
        None,
        Some("in_progress"),
    )
    .await
    .unwrap();

    plan::mark_task_done_and_unblock(&pool, &alice, &p.id, &a.id)
        .await
        .unwrap();

    let tasks = plan::list_tasks(&pool, &alice, &p.id).await.unwrap();
    assert_eq!(
        tasks.iter().find(|t| t.id == b.id).unwrap().status,
        "in_progress",
        "only dependents in exactly 'blocked' are moved to 'pending'"
    );
}

#[sqlx::test]
async fn cannot_mark_another_users_task_done(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let a = seed_task(&pool, &alice, &p.id, "a").await;

    let unblocked = plan::mark_task_done_and_unblock(&pool, &bob, &p.id, &a.id)
        .await
        .unwrap();
    assert!(
        unblocked.is_empty(),
        "bob does not own the plan; nothing to unblock"
    );

    let still = plan::find_task_by_id(&pool, &alice, &p.id, &a.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        still.status, "pending",
        "bob's rejected mark-done must not have flipped alice's task to done"
    );
}

// ── B3: permanent guard tests for four functions review previously
// proved load-bearing only with temporary, since-removed probes ─────────

#[sqlx::test]
async fn list_requirements_is_user_scoped(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let r = seed_requirement(&pool, &alice).await;
    plan::add_requirement(&pool, &alice, &p.id, &r)
        .await
        .unwrap();

    let bobs_view = plan::list_requirements(&pool, &bob, &p.id).await.unwrap();
    assert!(
        bobs_view.is_empty(),
        "bob must not read alice's requirement links, even by a correct plan id"
    );

    let alices_view = plan::list_requirements(&pool, &alice, &p.id).await.unwrap();
    assert_eq!(alices_view.len(), 1);
}

#[sqlx::test]
async fn list_analyze_items_is_user_scoped(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    plan::create_analyze_item(&pool, &alice, &p.id, "risk", None, None, Some("x"), None)
        .await
        .unwrap();

    let bobs_view = plan::list_analyze_items(&pool, &bob, &p.id).await.unwrap();
    assert!(
        bobs_view.is_empty(),
        "bob must not read alice's analyze items, even by a correct plan id"
    );

    let alices_view = plan::list_analyze_items(&pool, &alice, &p.id)
        .await
        .unwrap();
    assert_eq!(alices_view.len(), 1);
}

#[sqlx::test]
async fn cannot_reorder_another_users_requirements(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let ra = seed_requirement(&pool, &alice).await;
    let rb = seed_requirement(&pool, &alice).await;
    let a = plan::add_requirement(&pool, &alice, &p.id, &ra)
        .await
        .unwrap()
        .unwrap();
    let b = plan::add_requirement(&pool, &alice, &p.id, &rb)
        .await
        .unwrap()
        .unwrap();

    plan::reorder_requirements(&pool, &bob, &p.id, &[rb.clone(), ra.clone()])
        .await
        .unwrap();

    let links = plan::list_requirements(&pool, &alice, &p.id).await.unwrap();
    let a_after = links.iter().find(|l| l.id == a.id).unwrap();
    let b_after = links.iter().find(|l| l.id == b.id).unwrap();
    assert_eq!(
        a_after.order, a.order,
        "bob's rejected reorder must not touch alice's links"
    );
    assert_eq!(
        b_after.order, b.order,
        "bob's rejected reorder must not touch alice's links"
    );
}

#[sqlx::test]
async fn cannot_reorder_another_users_analyze_items(pool: PgPool) {
    let alice = seed_user(&pool, "alice").await;
    let bob = seed_user(&pool, "bob").await;
    let p = plan::create(&pool, &alice, "p", None, None).await.unwrap();
    let a = plan::create_analyze_item(&pool, &alice, &p.id, "risk", None, None, Some("a"), None)
        .await
        .unwrap();
    let b = plan::create_analyze_item(&pool, &alice, &p.id, "risk", None, None, Some("b"), None)
        .await
        .unwrap();

    plan::reorder_analyze_items(&pool, &bob, &p.id, "risk", &[b.id.clone(), a.id.clone()])
        .await
        .unwrap();

    let items = plan::list_analyze_items(&pool, &alice, &p.id)
        .await
        .unwrap();
    let a_after = items.iter().find(|i| i.id == a.id).unwrap();
    let b_after = items.iter().find(|i| i.id == b.id).unwrap();
    assert_eq!(
        a_after.order, a.order,
        "bob's rejected reorder must not touch alice's items"
    );
    assert_eq!(
        b_after.order, b.order,
        "bob's rejected reorder must not touch alice's items"
    );
}
