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

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn state(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState {
        pool,
        implicit_dev_session: false,
        better_auth_secret: "test-secret".into(),
    }
}

/// Signs up a fresh user and returns the `name=value` session cookie pair
/// from the `set-cookie` response header, matching the pattern in
/// `tests/connections.rs::signup`.
async fn signup(app: axum::Router, email: &str, name: &str) -> String {
    let res = app
        .oneshot(
            Request::post("/api/auth/sign-up/email")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"email":"{email}","password":"hunter22","name":"{name}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "signup must succeed");
    res.headers()
        .get("set-cookie")
        .expect("signup should set a session cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

async fn json_body(response: axum::response::Response) -> serde_json::Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

async fn create_chunk(app: axum::Router, cookie: &str, title: &str) -> String {
    let res = app
        .oneshot(
            Request::post("/api/chunks")
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(format!(r#"{{"title":"{title}"}}"#)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "chunk creation must succeed"
    );
    json_body(res).await["id"].as_str().unwrap().to_string()
}

async fn create_connection(app: axum::Router, cookie: &str, source_id: &str, target_id: &str) {
    let body = serde_json::json!({
        "sourceId": source_id,
        "targetId": target_id,
        "relation": "related_to",
    });
    let res = app
        .oneshot(
            Request::post("/api/connections")
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "connection creation must succeed"
    );
}

async fn create_tag(app: axum::Router, cookie: &str, name: &str) {
    let res = app
        .oneshot(
            Request::post("/api/tags")
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(format!(r#"{{"name":"{name}"}}"#)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "tag creation must succeed"
    );
}

async fn get_stats(app: axum::Router, cookie: &str) -> axum::response::Response {
    app.oneshot(
        Request::get("/api/stats")
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

/// Seeds two users with different chunk/connection/tag counts and asserts
/// each sees only their own totals. Passes only if every count in
/// `fubbik_db::repo::stats::get_stats` carries its own `WHERE user_id`
/// (directly for `chunk`/`tag`, through the parent chunk for
/// `chunk_connection`).
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn counts_are_scoped_per_user(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let alice = signup(app.clone(), "alice-stats@b.test", "Alice").await;
    let bob = signup(app.clone(), "bob-stats@b.test", "Bob").await;

    // Alice: 3 chunks, 1 connection between two of them, 2 tags.
    let a1 = create_chunk(app.clone(), &alice, "A1").await;
    let a2 = create_chunk(app.clone(), &alice, "A2").await;
    let _a3 = create_chunk(app.clone(), &alice, "A3").await;
    create_connection(app.clone(), &alice, &a1, &a2).await;
    create_tag(app.clone(), &alice, "alice-tag-1").await;
    create_tag(app.clone(), &alice, "alice-tag-2").await;

    // Bob: 5 chunks, 2 connections, 1 tag — deliberately different counts
    // in every dimension, so a swapped or unscoped query would be caught.
    let b1 = create_chunk(app.clone(), &bob, "B1").await;
    let b2 = create_chunk(app.clone(), &bob, "B2").await;
    let b3 = create_chunk(app.clone(), &bob, "B3").await;
    let _b4 = create_chunk(app.clone(), &bob, "B4").await;
    let _b5 = create_chunk(app.clone(), &bob, "B5").await;
    create_connection(app.clone(), &bob, &b1, &b2).await;
    create_connection(app.clone(), &bob, &b2, &b3).await;
    create_tag(app.clone(), &bob, "bob-tag-1").await;

    let res = get_stats(app.clone(), &alice).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        json_body(res).await,
        serde_json::json!({ "chunks": 3, "connections": 1, "tags": 2 }),
        "Alice must see only her own totals, not a mix of hers and Bob's"
    );

    let res = get_stats(app.clone(), &bob).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        json_body(res).await,
        serde_json::json!({ "chunks": 5, "connections": 2, "tags": 1 }),
        "Bob's rows existing alongside Alice's must not leak into her totals or vice versa"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unauthenticated_request_is_401(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let res = app
        .oneshot(Request::get("/api/stats").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

/// Matches the captured fixture's field set exactly
/// (`tests/fixtures/node-contract/stats.json`): `chunks`, `connections`,
/// `tags` — no more, no less.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn response_shape_matches_fixture_field_set(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "alice-shape@b.test", "Alice").await;
    let res = get_stats(app.clone(), &alice).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let obj = body.as_object().unwrap();
    let mut keys: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
    keys.sort();
    assert_eq!(keys, vec!["chunks", "connections", "tags"]);
}
