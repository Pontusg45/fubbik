//! `chunk_connection` links two chunks, `source_id` and `target_id`, and —
//! unlike `chunk_tag`/`chunk_space` — has an `id` of its own. Ownership
//! still derives entirely from the two parent `chunk` rows, though, so the
//! insert's ownership guard has the same "each direction gets its own
//! test" shape as those joins (`tests/tag.rs`, `tests/space.rs`), just with
//! both directions folded into a single row instead of two independent
//! parent rows.

use fubbik_core::error::AppError;
use fubbik_db::repo::{chunk, connection, user};

async fn seed(pool: &sqlx::PgPool, email: &str) -> String {
    user::create(pool, email, "U", None).await.unwrap().id
}

async fn a_chunk(pool: &sqlx::PgPool, uid: &str, title: &str) -> String {
    chunk::create(
        pool,
        uid,
        chunk::NewChunk {
            title: title.into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
        },
    )
    .await
    .unwrap()
    .id
}

#[sqlx::test]
async fn cannot_connect_from_another_users_chunk(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let bobs_chunk = a_chunk(&pool, &bob, "Bob's source").await;
    let alices_chunk = a_chunk(&pool, &alice, "Alice's target").await;

    // Alice tries to wire Bob's chunk (as source) to her own chunk.
    let id = fubbik_db::new_id();
    let created = connection::create(
        &pool,
        &id,
        &alice,
        &bobs_chunk,
        &alices_chunk,
        "related_to",
        "human",
        "approved",
    )
    .await
    .unwrap();
    assert!(
        created.is_none(),
        "must not create a connection from another user's chunk"
    );
    assert!(
        connection::find_by_id(&pool, &id).await.unwrap().is_none(),
        "no row may be created"
    );
}

#[sqlx::test]
async fn cannot_connect_to_another_users_chunk(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let alices_chunk = a_chunk(&pool, &alice, "Alice's source").await;
    let bobs_chunk = a_chunk(&pool, &bob, "Bob's target").await;

    // Alice tries to wire her own chunk (as source) to Bob's chunk.
    let id = fubbik_db::new_id();
    let created = connection::create(
        &pool,
        &id,
        &alice,
        &alices_chunk,
        &bobs_chunk,
        "related_to",
        "human",
        "approved",
    )
    .await
    .unwrap();
    assert!(
        created.is_none(),
        "must not create a connection to another user's chunk"
    );
    assert!(
        connection::find_by_id(&pool, &id).await.unwrap().is_none(),
        "no row may be created"
    );
}

#[sqlx::test]
async fn own_source_and_own_target_succeeds(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let source = a_chunk(&pool, &alice, "Source").await;
    let target = a_chunk(&pool, &alice, "Target").await;

    let id = fubbik_db::new_id();
    let created = connection::create(
        &pool,
        &id,
        &alice,
        &source,
        &target,
        "related_to",
        "human",
        "approved",
    )
    .await
    .unwrap()
    .expect("both chunks belong to the caller, so this must succeed");

    assert_eq!(created.source_id, source);
    assert_eq!(created.target_id, target);
    assert_eq!(created.relation, "related_to");
    assert_eq!(created.origin, "human");
    assert_eq!(created.review_status, "approved");
    assert_eq!(created.weight, 1);
    assert!(created.reviewed_by.is_none());
    assert!(created.reviewed_at.is_none());
}

#[sqlx::test]
async fn duplicate_source_target_relation_is_unique_violation_not_a_panic(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let source = a_chunk(&pool, &alice, "Source").await;
    let target = a_chunk(&pool, &alice, "Target").await;

    connection::create(
        &pool,
        &fubbik_db::new_id(),
        &alice,
        &source,
        &target,
        "related_to",
        "human",
        "approved",
    )
    .await
    .unwrap()
    .expect("first insert must succeed");

    let err = connection::create(
        &pool,
        &fubbik_db::new_id(),
        &alice,
        &source,
        &target,
        "related_to",
        "human",
        "approved",
    )
    .await
    .expect_err("duplicate (source_id, target_id, relation) must surface as an error");

    match err {
        AppError::Database(sqlx::Error::Database(db_err)) => {
            assert!(
                db_err.is_unique_violation(),
                "must be the unique-index violation, not some other database error"
            );
        }
        other => panic!("expected AppError::Database(unique violation), got {other:?}"),
    }
}

#[sqlx::test]
async fn invalid_relation_is_foreign_key_violation_not_a_panic(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let source = a_chunk(&pool, &alice, "Source").await;
    let target = a_chunk(&pool, &alice, "Target").await;

    let err = connection::create(
        &pool,
        &fubbik_db::new_id(),
        &alice,
        &source,
        &target,
        "not_a_real_relation",
        "human",
        "approved",
    )
    .await
    .expect_err("an unrecognized relation must fail, not silently succeed");

    match err {
        AppError::Database(sqlx::Error::Database(db_err)) => {
            assert!(
                db_err.is_foreign_key_violation(),
                "must be the connection_relation FK violation, not some other database error"
            );
        }
        other => panic!("expected AppError::Database(fk violation), got {other:?}"),
    }
}

#[sqlx::test]
async fn delete_requires_at_least_one_endpoint_owned_by_caller(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;
    let alices_chunk = a_chunk(&pool, &alice, "Alice's").await;
    let bobs_chunk = a_chunk(&pool, &bob, "Bob's").await;
    let carol = seed(&pool, "e@f.test").await;

    // `connection::create` itself requires *both* endpoints to belong to
    // the caller (proven by the two tests above), so a mixed-ownership row
    // like this can never arise through it — it stands in for legacy/
    // imported data, which is exactly the scenario Node's `deleteConnection`
    // OR-check (`source || target`) exists to handle. Inserted directly,
    // bypassing that guard on purpose.
    let conn_id = fubbik_db::new_id();
    sqlx::query!(
        "INSERT INTO chunk_connection (id, source_id, target_id, relation) VALUES ($1, $2, $3, 'related_to')",
        conn_id,
        alices_chunk,
        bobs_chunk
    )
    .execute(&pool)
    .await
    .unwrap();
    let conn = connection::find_by_id(&pool, &conn_id)
        .await
        .unwrap()
        .expect("row was just inserted");

    // Carol owns neither endpoint: must not be able to delete it.
    let deleted = connection::delete(&pool, &carol, &conn.id).await.unwrap();
    assert!(
        !deleted,
        "a user owning neither endpoint must not be able to delete the connection"
    );
    assert!(
        connection::find_by_id(&pool, &conn.id)
            .await
            .unwrap()
            .is_some(),
        "the connection must still exist after the rejected delete"
    );

    // Alice owns only the source (not the target): delete must still
    // succeed — the rule is OR, not AND.
    let deleted = connection::delete(&pool, &alice, &conn.id).await.unwrap();
    assert!(
        deleted,
        "owning just one endpoint must be enough to delete the connection"
    );
    assert!(
        connection::find_by_id(&pool, &conn.id)
            .await
            .unwrap()
            .is_none()
    );
}

// ── count_for_chunks: the bulk variant `search::service` uses ─────────

#[sqlx::test]
async fn count_for_chunks_counts_both_directions_and_zero_fills_missing(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let a = a_chunk(&pool, &alice, "A").await;
    let b = a_chunk(&pool, &alice, "B").await;
    let c = a_chunk(&pool, &alice, "C").await;
    // a -> b, c -> a: a has 2 connections (one as source, one as target).
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
    .unwrap();
    connection::create(
        &pool,
        &fubbik_db::new_id(),
        &alice,
        &c,
        &a,
        "related_to",
        "human",
        "approved",
    )
    .await
    .unwrap();

    let rows = connection::count_for_chunks(&pool, &[a.clone(), b.clone(), c.clone()])
        .await
        .unwrap();
    let by_id: std::collections::HashMap<String, i64> =
        rows.into_iter().map(|r| (r.chunk_id, r.count)).collect();
    assert_eq!(by_id[&a], 2);
    assert_eq!(by_id[&b], 1);
    assert_eq!(by_id[&c], 1);
}

#[sqlx::test]
async fn count_for_chunks_of_a_chunk_with_no_connections_is_zero(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let lonely = a_chunk(&pool, &alice, "Lonely").await;

    let rows = connection::count_for_chunks(&pool, std::slice::from_ref(&lonely))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "the id must still appear, with count 0");
    assert_eq!(rows[0].chunk_id, lonely);
    assert_eq!(rows[0].count, 0);
}

#[sqlx::test]
async fn count_for_chunks_of_empty_input_returns_empty_without_querying(pool: sqlx::PgPool) {
    let rows = connection::count_for_chunks(&pool, &[]).await.unwrap();
    assert!(rows.is_empty());
}

// ---------------------------------------------------------------------------
// connections_for_chunk — the `connections` array of `GET /api/chunks/{id}`
// ---------------------------------------------------------------------------

/// Inserts a connection and returns its id, panicking on the `Ok(None)`
/// ownership rejection so a mis-seeded test fails loudly here rather than
/// as a confusing empty result later.
async fn connect(pool: &sqlx::PgPool, uid: &str, source: &str, target: &str) -> String {
    let id = fubbik_db::new_id();
    connection::create(
        pool,
        &id,
        uid,
        source,
        target,
        "related_to",
        "human",
        "draft",
    )
    .await
    .unwrap()
    .expect("both chunks belong to uid, so the ownership guard must pass")
    .id
}

/// Both directions are returned, and `title` is always the **other** end's
/// title — not the subject's. A query that joined the subject instead would
/// still return the right number of rows, so the titles are what makes this
/// test able to fail.
#[sqlx::test]
async fn connections_for_chunk_returns_both_directions_with_the_other_ends_title(
    pool: sqlx::PgPool,
) {
    let uid = seed(&pool, "a@b.test").await;
    let subject = a_chunk(&pool, &uid, "Subject").await;
    let outgoing = a_chunk(&pool, &uid, "Outgoing neighbour").await;
    let incoming = a_chunk(&pool, &uid, "Incoming neighbour").await;

    connect(&pool, &uid, &subject, &outgoing).await;
    connect(&pool, &uid, &incoming, &subject).await;

    let mut rows = connection::connections_for_chunk(&pool, &subject, &uid)
        .await
        .unwrap();
    // The query has no ORDER BY (matching Node), so sort before asserting.
    rows.sort_by(|a, b| a.title.cmp(&b.title));

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].title.as_deref(), Some("Incoming neighbour"));
    assert_eq!(rows[0].source_id, incoming);
    assert_eq!(rows[0].target_id, subject);
    assert_eq!(rows[1].title.as_deref(), Some("Outgoing neighbour"));
    assert_eq!(rows[1].source_id, subject);
    assert_eq!(rows[1].target_id, outgoing);
}

/// **Pins a Node quirk, deliberately reproduced.** The `chunk_space` LEFT
/// JOIN has no aggregation, so a neighbour in N spaces yields N rows for
/// the same edge. `getChunkDetail` feeds `connections.length` into
/// `computeHealthScore`, so this also inflates the connectivity score.
///
/// If someone later adds `DISTINCT` or aggregates the space names, this
/// test fails and forces the change to be made on both stacks at once
/// rather than silently diverging.
#[sqlx::test]
async fn connections_for_chunk_multiplies_rows_per_space(pool: sqlx::PgPool) {
    use fubbik_db::repo::space;

    let uid = seed(&pool, "a@b.test").await;
    let subject = a_chunk(&pool, &uid, "Subject").await;
    let neighbour = a_chunk(&pool, &uid, "Neighbour").await;
    connect(&pool, &uid, &subject, &neighbour).await;

    // One edge, one space on the neighbour: one row.
    let s1 = space::create(
        &pool,
        &uid,
        space::NewSpace {
            name: "alpha".into(),
            kind: "code".into(),
            description: None,
        },
        None,
    )
    .await
    .unwrap();
    space::set_chunk_spaces(&pool, &uid, &neighbour, std::slice::from_ref(&s1.id))
        .await
        .unwrap();
    let rows = connection::connections_for_chunk(&pool, &subject, &uid)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].codebase_name.as_deref(), Some("alpha"));

    // Same single edge, neighbour now in two spaces: TWO rows.
    let s2 = space::create(
        &pool,
        &uid,
        space::NewSpace {
            name: "beta".into(),
            kind: "code".into(),
            description: None,
        },
        None,
    )
    .await
    .unwrap();
    space::set_chunk_spaces(&pool, &uid, &neighbour, &[s1.id, s2.id])
        .await
        .unwrap();

    let mut rows = connection::connections_for_chunk(&pool, &subject, &uid)
        .await
        .unwrap();
    rows.sort_by(|a, b| a.codebase_name.cmp(&b.codebase_name));
    assert_eq!(
        rows.len(),
        2,
        "one edge must yield one row per space of the other end — Node's \
         un-aggregated chunk_space join, reproduced"
    );
    assert_eq!(rows[0].codebase_name.as_deref(), Some("alpha"));
    assert_eq!(rows[1].codebase_name.as_deref(), Some("beta"));
    assert_eq!(
        rows[0].id, rows[1].id,
        "both rows must describe the SAME edge — this is duplication, not \
         two connections"
    );
}

/// The `EXISTS (... c.user_id = $2)` guard is load-bearing: removing it
/// from the SQL flips this test from an empty result to two rows, because
/// nothing else in this function checks ownership. Proven at the
/// **fubbik-db** layer, which observes the guard directly rather than
/// through an HTTP status code.
#[sqlx::test]
async fn connections_for_chunk_is_user_scoped(pool: sqlx::PgPool) {
    let alice = seed(&pool, "a@b.test").await;
    let bob = seed(&pool, "c@d.test").await;

    let subject = a_chunk(&pool, &alice, "Alice's subject").await;
    let neighbour = a_chunk(&pool, &alice, "Alice's neighbour").await;
    connect(&pool, &alice, &subject, &neighbour).await;

    assert_eq!(
        connection::connections_for_chunk(&pool, &subject, &alice)
            .await
            .unwrap()
            .len(),
        1,
        "the owner must see the connection — otherwise the scoping \
         assertion below could pass for the wrong reason"
    );
    assert!(
        connection::connections_for_chunk(&pool, &subject, &bob)
            .await
            .unwrap()
            .is_empty(),
        "Bob must not be able to read Alice's chunk's connections"
    );
}

/// A dangling edge — the other end deleted — yields a row with a null
/// title rather than disappearing. That is what the LEFT JOIN buys, and the
/// detail page's connection list has to cope with it.
#[sqlx::test]
async fn connections_for_chunk_keeps_a_dangling_edge_with_a_null_title(pool: sqlx::PgPool) {
    let uid = seed(&pool, "a@b.test").await;
    let subject = a_chunk(&pool, &uid, "Subject").await;
    let neighbour = a_chunk(&pool, &uid, "Doomed").await;
    connect(&pool, &uid, &subject, &neighbour).await;

    chunk::delete(&pool, &uid, &neighbour).await.unwrap();

    let rows = connection::connections_for_chunk(&pool, &subject, &uid)
        .await
        .unwrap();
    // The edge itself cascades with the chunk, so the expected result is
    // no rows at all — asserted here so the cascade is documented rather
    // than assumed.
    assert!(
        rows.is_empty(),
        "chunk_connection cascades off chunk, so deleting an end removes \
         the edge outright"
    );
}
