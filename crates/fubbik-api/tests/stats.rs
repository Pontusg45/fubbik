//! HTTP-level tests for the `stats` domain.
//!
//! Node's captured contract (`tests/fixtures/node-contract/stats.json`) is
//! the source of truth: `GET /api/stats` returns a **bare object** —
//! `{chunks, connections, tags}` — not an array, and not the
//! `{chunks,total,limit,offset}` envelope the chunks *list* endpoint uses.
//!
//! Every field here is an aggregate count, which makes an unscoped query
//! the natural mistake: `SELECT count(*) FROM chunk` reads as "obviously
//! correct" until you notice it counts every user's rows, not just the
//! caller's. The test below is the one the brief calls out explicitly:
//! seed two users with *different* counts in every dimension and assert
//! each sees their own totals, not zero for one and everything for the
//! other — a single-user test would pass even with completely unscoped
//! queries.

mod common;

use axum::http::{Method, StatusCode};
use common::{TestApp, TestUser};

async fn create_chunk(app: &TestApp, user: &TestUser, title: &str) -> String {
    let res = app
        .post(user, "/api/chunks", serde_json::json!({ "title": title }))
        .await;
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "chunk creation must succeed"
    );
    TestApp::json(res).await["id"].as_str().unwrap().to_string()
}

async fn create_connection(app: &TestApp, user: &TestUser, source_id: &str, target_id: &str) {
    let body = serde_json::json!({
        "sourceId": source_id,
        "targetId": target_id,
        "relation": "related_to",
    });
    let res = app.post(user, "/api/connections", body).await;
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "connection creation must succeed"
    );
}

async fn create_tag(app: &TestApp, user: &TestUser, name: &str) {
    let res = app
        .post(user, "/api/tags", serde_json::json!({ "name": name }))
        .await;
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "tag creation must succeed"
    );
}

/// Seeds two users with different chunk/connection/tag counts and asserts
/// each sees only their own totals. Passes only if every count in
/// `fubbik_db::repo::stats::get_stats` carries its own `WHERE user_id`
/// (directly for `chunk`/`tag`, through the parent chunk for
/// `chunk_connection`).
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn counts_are_scoped_per_user(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);

    let alice = app.signup("alice-stats@b.test", "Alice").await;
    let bob = app.signup("bob-stats@b.test", "Bob").await;

    // Alice: 3 chunks, 1 connection between two of them, 2 tags.
    let a1 = create_chunk(&app, &alice, "A1").await;
    let a2 = create_chunk(&app, &alice, "A2").await;
    let _a3 = create_chunk(&app, &alice, "A3").await;
    create_connection(&app, &alice, &a1, &a2).await;
    create_tag(&app, &alice, "alice-tag-1").await;
    create_tag(&app, &alice, "alice-tag-2").await;

    // Bob: 5 chunks, 2 connections, 1 tag — deliberately different counts
    // in every dimension, so a swapped or unscoped query would be caught.
    let b1 = create_chunk(&app, &bob, "B1").await;
    let b2 = create_chunk(&app, &bob, "B2").await;
    let b3 = create_chunk(&app, &bob, "B3").await;
    let _b4 = create_chunk(&app, &bob, "B4").await;
    let _b5 = create_chunk(&app, &bob, "B5").await;
    create_connection(&app, &bob, &b1, &b2).await;
    create_connection(&app, &bob, &b2, &b3).await;
    create_tag(&app, &bob, "bob-tag-1").await;

    let res = app.get(&alice, "/api/stats").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        TestApp::json(res).await,
        serde_json::json!({ "chunks": 3, "connections": 1, "tags": 2 }),
        "Alice must see only her own totals, not a mix of hers and Bob's"
    );

    let res = app.get(&bob, "/api/stats").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        TestApp::json(res).await,
        serde_json::json!({ "chunks": 5, "connections": 2, "tags": 1 }),
        "Bob's rows existing alongside Alice's must not leak into her totals or vice versa"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unauthenticated_request_is_401(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);
    let res = app.request(None, Method::GET, "/api/stats", None).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

/// Matches the captured fixture's field set exactly
/// (`tests/fixtures/node-contract/stats.json`): `chunks`, `connections`,
/// `tags` — no more, no less.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn response_shape_matches_fixture_field_set(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);
    let alice = app.signup("alice-shape@b.test", "Alice").await;
    let res = app.get(&alice, "/api/stats").await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = TestApp::json(res).await;
    let obj = body.as_object().unwrap();
    let mut keys: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
    keys.sort();
    assert_eq!(keys, vec!["chunks", "connections", "tags"]);
}
