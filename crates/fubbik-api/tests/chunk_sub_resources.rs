//! Cross-user isolation for the applies-to and file-ref sub-resources.
//!
//! Every user-scoped surface in this project has a cross-user test proving
//! `service::get`'s ownership check actually holds at the HTTP boundary —
//! these four endpoints (`GET|PUT .../applies-to`, `GET|PUT .../file-refs`)
//! did not. Drives real HTTP requests through the router with two real,
//! signed-up users so a future refactor that drops the `service::get` call
//! from a handler fails a test, not just a review.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn state(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState { pool, implicit_dev_session: false }
}

/// Signs up a fresh user and returns the `name=value` session cookie pair
/// from the `set-cookie` response header, matching the pattern in
/// `tests/auth.rs::cookie_pair`.
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

/// Creates a chunk as the user identified by `cookie` and returns its id.
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
    assert_eq!(res.status(), StatusCode::OK, "chunk creation must succeed");
    let body = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice::<serde_json::Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn put_applies_to(app: axum::Router, cookie: &str, id: &str, patterns: &[&str]) -> axum::response::Response {
    let body = serde_json::json!({ "patterns": patterns }).to_string();
    app.oneshot(
        Request::put(format!("/api/chunks/{id}/applies-to"))
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(body))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get_applies_to(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/chunks/{id}/applies-to"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn put_file_refs(app: axum::Router, cookie: &str, id: &str, paths: &[&str]) -> axum::response::Response {
    let body = serde_json::json!({ "paths": paths }).to_string();
    app.oneshot(
        Request::put(format!("/api/chunks/{id}/file-refs"))
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(body))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get_file_refs(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/chunks/{id}/file-refs"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn patterns_json(response: axum::response::Response) -> serde_json::Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_get_applies_to_is_404(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let alice_cookie = signup(app.clone(), "alice-ato-get@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-ato-get@b.test", "Bob").await;
    let chunk_id = create_chunk(app.clone(), &alice_cookie, "Alice's chunk").await;

    let res = get_applies_to(app.clone(), &bob_cookie, &chunk_id).await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "another user must not be able to read this chunk's applies-to patterns"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_put_applies_to_is_404_and_leaves_patterns_unchanged(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let alice_cookie = signup(app.clone(), "alice-ato-put@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-ato-put@b.test", "Bob").await;
    let chunk_id = create_chunk(app.clone(), &alice_cookie, "Alice's chunk").await;

    let seed = put_applies_to(app.clone(), &alice_cookie, &chunk_id, &["src/**/*.ts"]).await;
    assert_eq!(seed.status(), StatusCode::OK);

    let res = put_applies_to(app.clone(), &bob_cookie, &chunk_id, &["evil/**"]).await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "another user must not be able to overwrite this chunk's applies-to patterns"
    );

    // A 404 that still mutated the row would be worse than a 200: prove
    // Alice's patterns are byte-for-byte unchanged after Bob's rejected PUT.
    let after = patterns_json(get_applies_to(app.clone(), &alice_cookie, &chunk_id).await).await;
    let patterns: Vec<&str> = after
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["pattern"].as_str().unwrap())
        .collect();
    assert_eq!(patterns, vec!["src/**/*.ts"], "Alice's patterns must survive Bob's rejected PUT");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_get_file_refs_is_404(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let alice_cookie = signup(app.clone(), "alice-fr-get@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-fr-get@b.test", "Bob").await;
    let chunk_id = create_chunk(app.clone(), &alice_cookie, "Alice's chunk").await;

    let res = get_file_refs(app.clone(), &bob_cookie, &chunk_id).await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "another user must not be able to read this chunk's file refs"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_put_file_refs_is_404_and_leaves_refs_unchanged(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let alice_cookie = signup(app.clone(), "alice-fr-put@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-fr-put@b.test", "Bob").await;
    let chunk_id = create_chunk(app.clone(), &alice_cookie, "Alice's chunk").await;

    let seed = put_file_refs(app.clone(), &alice_cookie, &chunk_id, &["src/index.ts"]).await;
    assert_eq!(seed.status(), StatusCode::OK);

    let res = put_file_refs(app.clone(), &bob_cookie, &chunk_id, &["evil.ts"]).await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "another user must not be able to overwrite this chunk's file refs"
    );

    // A 404 that still mutated the row would be worse than a 200: prove
    // Alice's file refs are byte-for-byte unchanged after Bob's rejected PUT.
    let after = patterns_json(get_file_refs(app.clone(), &alice_cookie, &chunk_id).await).await;
    let paths: Vec<&str> = after
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, vec!["src/index.ts"], "Alice's file refs must survive Bob's rejected PUT");
}
