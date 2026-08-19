//! Repository-level tests for `fubbik_db::repo::coverage`.
//!
//! Cross-user guards are tested at *this* layer, not only through HTTP,
//! because the API layer has no pre-check to 404 first for these two
//! endpoints — a coverage request always runs the query. That makes the SQL
//! `user_id` predicate the *only* thing separating callers, so it is the
//! thing that has to be asserted directly. Each guard test's doc comment
//! names the predicate it pins and what the failure looks like when that
//! predicate is deleted; all three were verified by actually removing the
//! predicate and watching the named test go red.

use fubbik_db::repo::{chunk, coverage, space, user};

async fn make_user(pool: &sqlx::PgPool, email: &str, name: &str) -> String {
    user::create(pool, email, name, None).await.unwrap().id
}

async fn make_chunk(pool: &sqlx::PgPool, user_id: &str, title: &str) -> String {
    chunk::create(
        pool,
        user_id,
        chunk::NewChunk {
            title: title.to_string(),
            content: String::new(),
            chunk_type: "note".to_string(),
            rationale: None,
        },
    )
    .await
    .unwrap()
    .id
}

async fn make_space(pool: &sqlx::PgPool, user_id: &str, name: &str) -> String {
    space::create(
        pool,
        user_id,
        space::NewSpace {
            name: name.to_string(),
            kind: "code".to_string(),
            description: None,
        },
        None,
    )
    .await
    .unwrap()
    .id
}

async fn add_chunk_to_space(pool: &sqlx::PgPool, chunk_id: &str, space_id: &str) {
    sqlx::query!(
        "INSERT INTO chunk_space (chunk_id, space_id) VALUES ($1, $2)",
        chunk_id,
        space_id
    )
    .execute(pool)
    .await
    .unwrap();
}

/// Inserted straight into the table rather than through
/// `requirement::create` so tests can pin `status`, `priority` and
/// `space_id` exactly without depending on that function's defaulting rules.
async fn make_requirement(
    pool: &sqlx::PgPool,
    user_id: &str,
    title: &str,
    status: &str,
    priority: Option<&str>,
    space_id: Option<&str>,
) -> String {
    let id = fubbik_db::new_id();
    sqlx::query!(
        r#"INSERT INTO requirement (id, title, steps, user_id, status, priority, space_id)
           VALUES ($1, $2, '[]'::jsonb, $3, $4, $5, $6)"#,
        id,
        title,
        user_id,
        status,
        priority,
        space_id
    )
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn link(pool: &sqlx::PgPool, requirement_id: &str, chunk_id: &str) {
    sqlx::query!(
        "INSERT INTO requirement_chunk (requirement_id, chunk_id) VALUES ($1, $2)",
        requirement_id,
        chunk_id
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn archive(pool: &sqlx::PgPool, chunk_id: &str) {
    sqlx::query!(
        "UPDATE chunk SET archived_at = now() WHERE id = $1",
        chunk_id
    )
    .execute(pool)
    .await
    .unwrap();
}

/// Row order is unspecified (no `ORDER BY` in Node or here — see the repo
/// module doc), so every assertion sorts first. A test that asserted a
/// positional order would be asserting a query-plan artifact.
fn titles_with_counts(rows: &[coverage::ChunkCoverageRow]) -> Vec<(String, i64)> {
    let mut v: Vec<(String, i64)> = rows
        .iter()
        .map(|r| (r.title.clone(), r.requirement_count))
        .collect();
    v.sort();
    v
}

// ---------------------------------------------------------------------
// get_chunk_coverage
// ---------------------------------------------------------------------

/// The `LEFT JOIN` is what puts zero-requirement chunks in the result at
/// all. Downgrading it to an inner join would drop `Lonely` entirely and the
/// report would claim 100% coverage — so this asserts the uncovered chunk is
/// present *with* a zero count, not merely that the covered ones are right.
#[sqlx::test]
async fn counts_requirements_per_chunk_and_keeps_zero_count_chunks(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;

    let popular = make_chunk(&pool, &alice, "Popular").await;
    let single = make_chunk(&pool, &alice, "Single").await;
    let _lonely = make_chunk(&pool, &alice, "Lonely").await;

    let r1 = make_requirement(&pool, &alice, "R1", "passing", None, None).await;
    let r2 = make_requirement(&pool, &alice, "R2", "failing", None, None).await;
    link(&pool, &r1, &popular).await;
    link(&pool, &r2, &popular).await;
    link(&pool, &r1, &single).await;

    let rows = coverage::get_chunk_coverage(&pool, &alice, None)
        .await
        .unwrap();

    assert_eq!(
        titles_with_counts(&rows),
        vec![
            ("Lonely".to_string(), 0),
            ("Popular".to_string(), 2),
            ("Single".to_string(), 1),
        ]
    );
}

/// Pins `WHERE c.user_id = $1` in `get_chunk_coverage`. Verified
/// load-bearing: with that predicate deleted this test fails with 3 rows
/// instead of 1 for Bob, leaking Alice's chunk titles through an endpoint
/// that scans the whole `chunk` table by design.
#[sqlx::test]
async fn chunk_coverage_is_user_scoped(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let bob = make_user(&pool, "c@d.test", "Bob").await;

    let alices = make_chunk(&pool, &alice, "Alice secret").await;
    make_chunk(&pool, &alice, "Alice other").await;
    make_chunk(&pool, &bob, "Bob chunk").await;

    let r = make_requirement(&pool, &alice, "R", "passing", None, None).await;
    link(&pool, &r, &alices).await;

    let bobs_view = coverage::get_chunk_coverage(&pool, &bob, None)
        .await
        .unwrap();
    assert_eq!(
        titles_with_counts(&bobs_view),
        vec![("Bob chunk".to_string(), 0)],
        "another user's chunks must not appear in the coverage scan"
    );

    let alices_view = coverage::get_chunk_coverage(&pool, &alice, None)
        .await
        .unwrap();
    assert_eq!(
        alices_view.len(),
        2,
        "the guard must not break the query for the owner"
    );
}

#[sqlx::test]
async fn chunk_coverage_excludes_archived_chunks(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    make_chunk(&pool, &alice, "Live").await;
    let gone = make_chunk(&pool, &alice, "Archived").await;
    archive(&pool, &gone).await;

    let rows = coverage::get_chunk_coverage(&pool, &alice, None)
        .await
        .unwrap();
    assert_eq!(titles_with_counts(&rows), vec![("Live".to_string(), 0)]);
}

#[sqlx::test]
async fn chunk_coverage_filters_by_codebase_id(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let backend = make_space(&pool, &alice, "backend").await;
    let frontend = make_space(&pool, &alice, "frontend").await;

    let api = make_chunk(&pool, &alice, "API notes").await;
    let ui = make_chunk(&pool, &alice, "UI notes").await;
    let _global = make_chunk(&pool, &alice, "Global notes").await;
    add_chunk_to_space(&pool, &api, &backend).await;
    add_chunk_to_space(&pool, &ui, &frontend).await;

    let rows = coverage::get_chunk_coverage(&pool, &alice, Some(&backend))
        .await
        .unwrap();
    assert_eq!(
        titles_with_counts(&rows),
        vec![("API notes".to_string(), 0)],
        "space filter must exclude other spaces and space-less chunks"
    );

    let unfiltered = coverage::get_chunk_coverage(&pool, &alice, None)
        .await
        .unwrap();
    assert_eq!(unfiltered.len(), 3, "no filter means every chunk");
}

/// The space filter is an `EXISTS` subquery rather than Node's `innerJoin`.
/// This pins the equivalence: a chunk in two spaces with two requirements
/// must report `2`, not `4`. A naive rewrite that joined `chunk_space` into
/// the same `FROM` as `requirement_chunk` would double every count here
/// while still passing every other test in this file.
#[sqlx::test]
async fn chunk_coverage_space_filter_does_not_inflate_counts(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let backend = make_space(&pool, &alice, "backend").await;
    let shared = make_space(&pool, &alice, "shared").await;

    let c = make_chunk(&pool, &alice, "Shared chunk").await;
    add_chunk_to_space(&pool, &c, &backend).await;
    add_chunk_to_space(&pool, &c, &shared).await;

    let r1 = make_requirement(&pool, &alice, "R1", "passing", None, None).await;
    let r2 = make_requirement(&pool, &alice, "R2", "passing", None, None).await;
    link(&pool, &r1, &c).await;
    link(&pool, &r2, &c).await;

    let rows = coverage::get_chunk_coverage(&pool, &alice, Some(&backend))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "one chunk, not one row per space membership");
    assert_eq!(rows[0].requirement_count, 2);
}

// ---------------------------------------------------------------------
// get_chunk_coverage_matrix
// ---------------------------------------------------------------------

#[sqlx::test]
async fn coverage_matrix_pairs_chunks_with_requirements(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let c = make_chunk(&pool, &alice, "Auth chunk").await;
    make_chunk(&pool, &alice, "Unlinked chunk").await;
    let r = make_requirement(&pool, &alice, "Login works", "failing", None, None).await;
    link(&pool, &r, &c).await;

    let rows = coverage::get_chunk_coverage_matrix(&pool, &alice, None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "only linked pairs appear in the matrix");
    assert_eq!(rows[0].chunk_id, c);
    assert_eq!(rows[0].chunk_title, "Auth chunk");
    assert_eq!(rows[0].requirement_id, r);
    assert_eq!(rows[0].requirement_title, "Login works");
    assert_eq!(rows[0].requirement_status, "failing");
}

/// Pins `WHERE c.user_id = $1` in `get_chunk_coverage_matrix` — a separate
/// predicate in a separate statement from the one
/// `chunk_coverage_is_user_scoped` covers, so it needs its own test.
/// Verified load-bearing: deleting it makes Bob's matrix come back with one
/// row (Alice's chunk title and requirement title) instead of empty.
#[sqlx::test]
async fn coverage_matrix_is_user_scoped(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let bob = make_user(&pool, "c@d.test", "Bob").await;

    let c = make_chunk(&pool, &alice, "Alice chunk").await;
    let r = make_requirement(&pool, &alice, "Alice requirement", "passing", None, None).await;
    link(&pool, &r, &c).await;

    let bobs_view = coverage::get_chunk_coverage_matrix(&pool, &bob, None)
        .await
        .unwrap();
    assert!(
        bobs_view.is_empty(),
        "another user's chunk/requirement pairs must not appear"
    );

    let alices_view = coverage::get_chunk_coverage_matrix(&pool, &alice, None)
        .await
        .unwrap();
    assert_eq!(alices_view.len(), 1);
}

#[sqlx::test]
async fn coverage_matrix_excludes_archived_and_filters_by_codebase_id(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let backend = make_space(&pool, &alice, "backend").await;

    let live = make_chunk(&pool, &alice, "Live").await;
    let gone = make_chunk(&pool, &alice, "Archived").await;
    let elsewhere = make_chunk(&pool, &alice, "Elsewhere").await;
    add_chunk_to_space(&pool, &live, &backend).await;
    add_chunk_to_space(&pool, &gone, &backend).await;

    let r = make_requirement(&pool, &alice, "R", "untested", None, None).await;
    link(&pool, &r, &live).await;
    link(&pool, &r, &gone).await;
    link(&pool, &r, &elsewhere).await;
    archive(&pool, &gone).await;

    let rows = coverage::get_chunk_coverage_matrix(&pool, &alice, Some(&backend))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].chunk_title, "Live");
}

// ---------------------------------------------------------------------
// get_traceability_matrix
// ---------------------------------------------------------------------

/// Pins `WHERE user_id = $1` in `get_traceability_matrix`. Verified
/// load-bearing: without it Bob's traceability report lists Alice's
/// requirement titles.
#[sqlx::test]
async fn traceability_is_user_scoped(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let bob = make_user(&pool, "c@d.test", "Bob").await;

    make_requirement(
        &pool,
        &alice,
        "Alice requirement",
        "passing",
        Some("must"),
        None,
    )
    .await;
    make_requirement(&pool, &bob, "Bob requirement", "untested", None, None).await;

    let bobs_view = coverage::get_traceability_matrix(&pool, &bob, None)
        .await
        .unwrap();
    assert_eq!(bobs_view.len(), 1);
    assert_eq!(bobs_view[0].title, "Bob requirement");
    assert_eq!(bobs_view[0].status, "untested");
    assert!(bobs_view[0].priority.is_none(), "priority is nullable text");

    let alices_view = coverage::get_traceability_matrix(&pool, &alice, None)
        .await
        .unwrap();
    assert_eq!(alices_view.len(), 1);
    assert_eq!(alices_view[0].priority.as_deref(), Some("must"));
}

/// Traceability filters on `requirement.space_id` **directly**, not through
/// `chunk_space` like the two coverage queries. A requirement with no
/// `space_id` is therefore excluded by any space filter, even if its linked
/// chunks live in that space.
#[sqlx::test]
async fn traceability_filters_on_requirement_space_id(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let backend = make_space(&pool, &alice, "backend").await;
    let frontend = make_space(&pool, &alice, "frontend").await;

    make_requirement(
        &pool,
        &alice,
        "Backend req",
        "passing",
        None,
        Some(&backend),
    )
    .await;
    make_requirement(
        &pool,
        &alice,
        "Frontend req",
        "passing",
        None,
        Some(&frontend),
    )
    .await;

    let spaceless = make_requirement(&pool, &alice, "Global req", "passing", None, None).await;
    let c = make_chunk(&pool, &alice, "Backend chunk").await;
    add_chunk_to_space(&pool, &c, &backend).await;
    link(&pool, &spaceless, &c).await;

    let rows = coverage::get_traceability_matrix(&pool, &alice, Some(&backend))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Backend req");

    let unfiltered = coverage::get_traceability_matrix(&pool, &alice, None)
        .await
        .unwrap();
    assert_eq!(unfiltered.len(), 3);
}

/// `requirement` has no `archived_at` column, so unlike the coverage
/// queries there is nothing for traceability to filter out — this pins that
/// the absence is intentional rather than an omission, by asserting a
/// requirement whose linked chunk is archived still shows up.
#[sqlx::test]
async fn traceability_has_no_archived_filter(pool: sqlx::PgPool) {
    let alice = make_user(&pool, "a@b.test", "Alice").await;
    let c = make_chunk(&pool, &alice, "Archived chunk").await;
    let r = make_requirement(&pool, &alice, "Still listed", "passing", None, None).await;
    link(&pool, &r, &c).await;
    archive(&pool, &c).await;

    let rows = coverage::get_traceability_matrix(&pool, &alice, None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Still listed");
}
