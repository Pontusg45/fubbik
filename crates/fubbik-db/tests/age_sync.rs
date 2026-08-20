//! Integration tests for Task 10: projecting `chunk_connection` into the
//! AGE graph on create/delete (`connection::create`/`connection::delete`),
//! plus the one-time, idempotent backfill (`age::backfill_connections`)
//! for rows that predate write-time projection.
//!
//! Every test skips (rather than failing) when AGE isn't available in the
//! target database — the same pattern `tests/age_queries.rs` already uses.

use fubbik_db::age;
use fubbik_db::repo::{chunk, connection, user};

async fn seed_user(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "U", None).await.unwrap().id
}

async fn seed_chunk(pool: &sqlx::PgPool, user_id: &str) -> String {
    chunk::create(
        pool,
        user_id,
        chunk::NewChunk {
            title: "A chunk".into(),
            content: String::new(),
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
async fn creating_a_connection_projects_an_edge(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable — skipping");
        return;
    }
    let alice = seed_user(&pool, "alice-sync-create@b.test").await;
    let a = seed_chunk(&pool, &alice).await;
    let b = seed_chunk(&pool, &alice).await;

    connection::create(
        &pool,
        &fubbik_db::new_id(),
        &alice,
        &a,
        &b,
        "related_to",
        "human",
        "approved",
    )
    .await
    .unwrap()
    .expect("both endpoints belong to the caller, so the insert must succeed");

    let ids = age::get_neighborhood(&pool, &a, 1).await.unwrap();
    assert!(
        ids.contains(&b),
        "creating a connection must project a `connects` edge into the graph"
    );
    assert_eq!(
        age::count_edges_between(&pool, &a, &b).await.unwrap(),
        1,
        "exactly one edge must exist between the two endpoints"
    );
}

#[sqlx::test]
async fn deleting_a_connection_removes_the_edge(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable — skipping");
        return;
    }
    let alice = seed_user(&pool, "alice-sync-delete@b.test").await;
    let a = seed_chunk(&pool, &alice).await;
    let b = seed_chunk(&pool, &alice).await;
    let id = fubbik_db::new_id();
    connection::create(
        &pool,
        &id,
        &alice,
        &a,
        &b,
        "related_to",
        "human",
        "approved",
    )
    .await
    .unwrap()
    .expect("both endpoints belong to the caller, so the insert must succeed");
    assert_eq!(age::count_edges_between(&pool, &a, &b).await.unwrap(), 1);

    let deleted = connection::delete(&pool, &alice, &id).await.unwrap();
    assert!(
        deleted,
        "the caller owns both endpoints, so delete must succeed"
    );

    let ids = age::get_neighborhood(&pool, &a, 1).await.unwrap();
    assert!(
        !ids.contains(&b),
        "deleting a connection must remove its edge"
    );
    assert_eq!(
        age::count_edges_between(&pool, &a, &b).await.unwrap(),
        0,
        "no edge may survive the delete"
    );
}

/// The whole justification for `MERGE` over `CREATE` in `create_edge`:
/// running the backfill twice against the same pre-existing row must not
/// leave two parallel edges behind.
#[sqlx::test]
async fn backfill_is_idempotent(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable — skipping");
        return;
    }
    let alice = seed_user(&pool, "alice-sync-backfill@b.test").await;
    let a = seed_chunk(&pool, &alice).await;
    let b = seed_chunk(&pool, &alice).await;
    // Insert a row directly, bypassing write-time projection, to simulate
    // data that existed before this port started projecting connections.
    sqlx::query!(
        "INSERT INTO chunk_connection (id, source_id, target_id, relation) VALUES ($1, $2, $3, 'related_to')",
        fubbik_db::new_id(),
        a,
        b
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        age::count_edges_between(&pool, &a, &b).await.unwrap(),
        0,
        "the row was inserted directly, so no edge exists yet"
    );

    let first = age::backfill_connections(&pool).await.unwrap();
    let second = age::backfill_connections(&pool).await.unwrap();
    assert_eq!(first, 1, "one pre-existing row must be walked");

    let edges = age::count_edges_between(&pool, &a, &b).await.unwrap();
    assert_eq!(
        edges, 1,
        "re-running the backfill must not duplicate the edge (ran twice, second call walked {second} row(s))"
    );
}

/// A second, independently-related connection row must also survive a
/// double backfill run without its edge count climbing past one — proves
/// idempotency isn't an artifact of there being only a single row.
#[sqlx::test]
async fn backfill_projects_every_preexisting_row_without_duplication(pool: sqlx::PgPool) {
    if !age::is_available(&pool).await {
        eprintln!("AGE unavailable — skipping");
        return;
    }
    let alice = seed_user(&pool, "alice-sync-backfill-multi@b.test").await;
    let a = seed_chunk(&pool, &alice).await;
    let b = seed_chunk(&pool, &alice).await;
    let c = seed_chunk(&pool, &alice).await;
    sqlx::query!(
        "INSERT INTO chunk_connection (id, source_id, target_id, relation) VALUES ($1, $2, $3, 'related_to')",
        fubbik_db::new_id(),
        a,
        b
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query!(
        "INSERT INTO chunk_connection (id, source_id, target_id, relation) VALUES ($1, $2, $3, 'depends_on')",
        fubbik_db::new_id(),
        b,
        c
    )
    .execute(&pool)
    .await
    .unwrap();

    let first = age::backfill_connections(&pool).await.unwrap();
    assert_eq!(first, 2, "both pre-existing rows must be walked");
    age::backfill_connections(&pool).await.unwrap();

    assert_eq!(age::count_edges_between(&pool, &a, &b).await.unwrap(), 1);
    assert_eq!(age::count_edges_between(&pool, &b, &c).await.unwrap(), 1);
}
