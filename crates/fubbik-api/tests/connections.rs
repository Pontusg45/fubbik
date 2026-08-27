//! HTTP-level tests for the `connections` domain.
//!
//! Node's captured contract (`tests/fixtures/node-contract/_mutating.md`,
//! "Connections") is the source of truth: `POST /api/connections` returns
//! the bare inserted row at 201; `DELETE /api/connections/{id}` returns
//! `{ "message": "Deleted" }` at 200, and 404s both when the id doesn't
//! exist and when neither the connection's source nor target chunk
//! resolves for the caller.
//!
//! `chunk_connection` is the first join in this slice where **both**
//! ownership checks apply to a single row (source and target), rather than
//! two independent parent rows — so both directions get their own
//! cross-user test here too, exercised through the router rather than the
//! repo layer (that half lives in `fubbik-db/tests/connection.rs`).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn state(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState {
        pool,
        implicit_dev_session: false,
        better_auth_secret: "test-secret".into(),
        ai: fubbik_ai::OllamaClient::new("http://127.0.0.1:1"),
    }
}

/// Signs up a fresh user and returns the `name=value` session cookie pair
/// from the `set-cookie` response header, matching the pattern in
/// `tests/tags.rs::signup`.
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
    let body = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice::<serde_json::Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn json_body(response: axum::response::Response) -> serde_json::Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

async fn create_connection(
    app: axum::Router,
    cookie: &str,
    source_id: &str,
    target_id: &str,
    relation: &str,
) -> axum::response::Response {
    let body = serde_json::json!({
        "sourceId": source_id,
        "targetId": target_id,
        "relation": relation,
    });
    app.oneshot(
        Request::post("/api/connections")
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn delete_connection(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::delete(format!("/api/connections/{id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_then_delete_round_trip(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-conn@b.test", "Alice").await;
    let source = create_chunk(app.clone(), &cookie, "Source").await;
    let target = create_chunk(app.clone(), &cookie, "Target").await;

    let res = create_connection(app.clone(), &cookie, &source, &target, "related_to").await;
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "create must return 201, matching Node's ctx.set.status = 201"
    );
    let created = json_body(res).await;
    assert_eq!(created["sourceId"], source);
    assert_eq!(created["targetId"], target);
    assert_eq!(created["relation"], "related_to");
    assert_eq!(created["origin"], "human");
    assert_eq!(created["reviewStatus"], "approved");
    assert_eq!(created["weight"], 1);
    assert_eq!(created["reviewedBy"], serde_json::Value::Null);
    assert_eq!(created["reviewedAt"], serde_json::Value::Null);
    let id = created["id"].as_str().unwrap().to_string();

    let res = delete_connection(app.clone(), &cookie, &id).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        json_body(res).await,
        serde_json::json!({"message": "Deleted"})
    );

    // A second delete of the now-gone id must 404, not succeed again.
    let res = delete_connection(app.clone(), &cookie, &id).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn origin_ai_defaults_review_status_to_draft(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-origin@b.test", "Alice").await;
    let source = create_chunk(app.clone(), &cookie, "Source").await;
    let target = create_chunk(app.clone(), &cookie, "Target").await;

    let body = serde_json::json!({
        "sourceId": source,
        "targetId": target,
        "relation": "related_to",
        "origin": "ai",
    });
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/connections")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let created = json_body(res).await;
    assert_eq!(created["origin"], "ai");
    assert_eq!(created["reviewStatus"], "draft");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cannot_connect_to_self(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-self@b.test", "Alice").await;
    let chunk = create_chunk(app.clone(), &cookie, "Solo").await;

    let res = create_connection(app.clone(), &cookie, &chunk, &chunk, "related_to").await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn foreign_source_is_rejected(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice_cookie = signup(app.clone(), "alice-fsrc@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-fsrc@b.test", "Bob").await;
    let bobs_chunk = create_chunk(app.clone(), &bob_cookie, "Bob's").await;
    let alices_chunk = create_chunk(app.clone(), &alice_cookie, "Alice's").await;

    // Alice tries to connect FROM Bob's chunk to her own.
    let res = create_connection(
        app.clone(),
        &alice_cookie,
        &bobs_chunk,
        &alices_chunk,
        "related_to",
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "a foreign source chunk must be rejected"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn foreign_target_is_rejected(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice_cookie = signup(app.clone(), "alice-ftgt@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-ftgt@b.test", "Bob").await;
    let alices_chunk = create_chunk(app.clone(), &alice_cookie, "Alice's").await;
    let bobs_chunk = create_chunk(app.clone(), &bob_cookie, "Bob's").await;

    // Alice tries to connect her own chunk TO Bob's.
    let res = create_connection(
        app.clone(),
        &alice_cookie,
        &alices_chunk,
        &bobs_chunk,
        "related_to",
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "a foreign target chunk must be rejected"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn duplicate_connection_is_conflict_not_500(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-dupe@b.test", "Alice").await;
    let source = create_chunk(app.clone(), &cookie, "Source").await;
    let target = create_chunk(app.clone(), &cookie, "Target").await;

    let res = create_connection(app.clone(), &cookie, &source, &target, "related_to").await;
    assert_eq!(res.status(), StatusCode::CREATED);

    let res = create_connection(app.clone(), &cookie, &source, &target, "related_to").await;
    assert_eq!(
        res.status(),
        StatusCode::CONFLICT,
        "a duplicate (source, target, relation) must be 409, not a raw 500"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn invalid_relation_fails_cleanly(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-badrel@b.test", "Alice").await;
    let source = create_chunk(app.clone(), &cookie, "Source").await;
    let target = create_chunk(app.clone(), &cookie, "Target").await;

    let res = create_connection(
        app.clone(),
        &cookie,
        &source,
        &target,
        "not_a_real_relation",
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "an unrecognized relation must fail cleanly, not surface the raw FK error as a 500"
    );
}

/// The one place this domain's ownership rule is `OR`, not `AND`: deleting
/// a connection only requires the caller to own *one* of its two
/// endpoints. This test proves the other half — a caller owning *neither*
/// endpoint gets 404 and the connection survives untouched.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_delete_is_404_and_leaves_connection_present(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice_cookie = signup(app.clone(), "alice-del@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-del@b.test", "Bob").await;
    let source = create_chunk(app.clone(), &alice_cookie, "Source").await;
    let target = create_chunk(app.clone(), &alice_cookie, "Target").await;

    let created = json_body(
        create_connection(app.clone(), &alice_cookie, &source, &target, "related_to").await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    // Bob owns neither endpoint.
    let res = delete_connection(app.clone(), &bob_cookie, &id).await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "a user owning neither endpoint must not be able to delete the connection"
    );

    // The victim's connection must still be deletable by its rightful
    // owner afterward, proving Bob's rejected attempt did not touch it.
    let res = delete_connection(app.clone(), &alice_cookie, &id).await;
    assert_eq!(res.status(), StatusCode::OK);
}
