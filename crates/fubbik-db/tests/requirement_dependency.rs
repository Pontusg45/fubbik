//! Repo-level tests for `fubbik_db::repo::requirement_dependency`.

use fubbik_db::repo::requirement::{self, NewRequirement, RequirementStep, StepKeyword};
use fubbik_db::repo::{requirement_dependency as dep, user};

async fn seed_user(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "U", None).await.unwrap().id
}

async fn seed_req(pool: &sqlx::PgPool, user_id: &str, title: &str) -> String {
    requirement::create(
        pool,
        user_id,
        NewRequirement {
            title: title.to_string(),
            description: None,
            steps: vec![
                RequirementStep {
                    keyword: StepKeyword::Given,
                    text: "x".into(),
                    params: None,
                },
                RequirementStep {
                    keyword: StepKeyword::When,
                    text: "y".into(),
                    params: None,
                },
                RequirementStep {
                    keyword: StepKeyword::Then,
                    text: "z".into(),
                    params: None,
                },
            ],
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
    .id
}

#[sqlx::test]
async fn add_and_get_round_trips_both_directions(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let a = seed_req(&pool, &alice, "A").await;
    let b = seed_req(&pool, &alice, "B").await;

    let inserted = dep::add(&pool, &a, &b).await.unwrap();
    assert!(inserted);

    let a_deps = dep::get(&pool, &a).await.unwrap();
    assert_eq!(a_deps.depends_on.len(), 1);
    assert_eq!(a_deps.depends_on[0].id, b);
    assert_eq!(a_deps.depended_on_by.len(), 0);

    let b_deps = dep::get(&pool, &b).await.unwrap();
    assert_eq!(b_deps.depended_on_by.len(), 1);
    assert_eq!(b_deps.depended_on_by[0].id, a);
}

/// Re-adding the same edge is a silent no-op (`ON CONFLICT DO NOTHING`),
/// matching Node's `.onConflictDoNothing()`.
#[sqlx::test]
async fn add_is_idempotent(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let a = seed_req(&pool, &alice, "A").await;
    let b = seed_req(&pool, &alice, "B").await;

    assert!(dep::add(&pool, &a, &b).await.unwrap());
    assert!(
        !dep::add(&pool, &a, &b).await.unwrap(),
        "second insert of the same edge must affect zero rows"
    );

    let deps = dep::get(&pool, &a).await.unwrap();
    assert_eq!(
        deps.depends_on.len(),
        1,
        "must not have duplicated the edge"
    );
}

#[sqlx::test]
async fn remove_deletes_the_edge(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let a = seed_req(&pool, &alice, "A").await;
    let b = seed_req(&pool, &alice, "B").await;
    dep::add(&pool, &a, &b).await.unwrap();

    let removed = dep::remove(&pool, &a, &b).await.unwrap();
    assert!(removed);
    assert_eq!(dep::get(&pool, &a).await.unwrap().depends_on.len(), 0);
}

/// `no_self_dependency` is a DB-level CHECK constraint
/// (`crates/fubbik-db/migrations/0001_init.sql`), not an application-level
/// guard — same as Node. This proves the constraint actually exists and
/// rejects the insert.
#[sqlx::test]
async fn self_dependency_is_rejected_by_the_database_check_constraint(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let a = seed_req(&pool, &alice, "A").await;

    let result = dep::add(&pool, &a, &a).await;
    assert!(
        result.is_err(),
        "the DB's no_self_dependency CHECK must reject requirement_id == depends_on_id"
    );
}

#[sqlx::test]
async fn check_circular_detects_a_would_be_cycle(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let a = seed_req(&pool, &alice, "A").await;
    let b = seed_req(&pool, &alice, "B").await;
    let c = seed_req(&pool, &alice, "C").await;

    // a -> b -> c
    dep::add(&pool, &a, &b).await.unwrap();
    dep::add(&pool, &b, &c).await.unwrap();

    // `a` already (transitively) depends on `c` (a -> b -> c), so adding
    // `c -> a` (check_circular(requirement_id=c, depends_on_id=a)) would
    // close the cycle a -> b -> c -> a and must be flagged.
    let would_cycle = dep::check_circular(&pool, &c, &a).await.unwrap();
    assert!(
        would_cycle,
        "adding c -> a must be detected as closing the a -> b -> c -> a cycle"
    );

    // Non-cyclic addition should not be flagged.
    let d = seed_req(&pool, &alice, "D").await;
    let unrelated = dep::check_circular(&pool, &d, &a).await.unwrap();
    assert!(!unrelated, "d depending on a is not a cycle");
}

#[sqlx::test]
async fn transitive_walks_ancestors_descendants_and_edges(pool: sqlx::PgPool) {
    let alice = seed_user(&pool, "a@b.test").await;
    let a = seed_req(&pool, &alice, "A").await;
    let b = seed_req(&pool, &alice, "B").await;
    let c = seed_req(&pool, &alice, "C").await;

    // a -> b -> c (a depends on b, b depends on c)
    dep::add(&pool, &a, &b).await.unwrap();
    dep::add(&pool, &b, &c).await.unwrap();

    let from_b = dep::transitive(&pool, &b).await.unwrap();
    let ancestor_ids: Vec<String> = from_b.ancestors.iter().map(|r| r.id.clone()).collect();
    let descendant_ids: Vec<String> = from_b.descendants.iter().map(|r| r.id.clone()).collect();
    assert_eq!(
        ancestor_ids,
        vec![c.clone()],
        "b's ancestors (what b depends on) must include c"
    );
    assert_eq!(
        descendant_ids,
        vec![a.clone()],
        "b's descendants (what depends on b) must include a"
    );
    assert!(from_b.edges.iter().any(|e| e.source == a && e.target == b));
    assert!(from_b.edges.iter().any(|e| e.source == b && e.target == c));
}
