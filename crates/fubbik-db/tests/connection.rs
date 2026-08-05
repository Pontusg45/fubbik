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
