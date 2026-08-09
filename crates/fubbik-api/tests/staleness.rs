//! HTTP-level tests for the `staleness` domain.
//!
//! Routes live under `/api/chunks/...`, not `/api/staleness/...`
//! (`packages/api/src/staleness/routes.ts`). Two response-shape quirks
//! captured from the running Node server
//! (`tests/fixtures/node-contract-2c/chunks-stale.json`,
//! `chunks-stale-count.json`) are load-bearing here:
//!
//! 1. `GET /api/chunks/stale` returns a **bare array**.
//! 2. `GET /api/chunks/stale/count` returns a **bare number** as
//!    `text/plain`, not `{"count": N}` — see `routes::stale_count`'s doc
//!    comment for why Phase 2b's `{count:N}` precedent doesn't apply here.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn state(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState {
        pool,
        implicit_dev_session: false,
    }
}

/// Signs up a fresh user and returns the `name=value` session cookie pair,
/// matching the pattern in `tests/favorites.rs::signup`.
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
    if body.is_empty() {
        return serde_json::Value::Null;
    }
    serde_json::from_slice(&body).unwrap()
}

async fn text_body(response: axum::response::Response) -> String {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(body.to_vec()).unwrap()
}

async fn user_id_for_email(pool: &sqlx::PgPool, email: &str) -> String {
    sqlx::query_scalar!(r#"SELECT id FROM "user" WHERE email = $1"#, email)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn seed_chunk(pool: &sqlx::PgPool, user_id: &str, title: &str) -> String {
    fubbik_db::repo::chunk::create(
        pool,
        user_id,
        fubbik_db::repo::chunk::NewChunk {
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

async fn seed_flag(pool: &sqlx::PgPool, chunk_id: &str, reason: &str) -> String {
    let id = fubbik_db::new_id();
    sqlx::query!(
        "INSERT INTO chunk_staleness (id, chunk_id, reason) VALUES ($1, $2, $3)",
        id,
        chunk_id,
        reason
    )
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn seed_duplicate_flag(pool: &sqlx::PgPool, chunk_id: &str, related_chunk_id: &str) {
    let id = fubbik_db::new_id();
    sqlx::query!(
        "INSERT INTO chunk_staleness (id, chunk_id, reason, related_chunk_id) \
         VALUES ($1, $2, 'diverged_duplicate', $3)",
        id,
        chunk_id,
        related_chunk_id
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn get_stale(app: axum::Router, cookie: &str) -> axum::response::Response {
    app.oneshot(
        Request::get("/api/chunks/stale")
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get_stale_count(app: axum::Router, cookie: &str) -> axum::response::Response {
    app.oneshot(
        Request::get("/api/chunks/stale/count")
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn dismiss(app: axum::Router, cookie: &str, flag_id: &str) -> axum::response::Response {
    app.oneshot(
        Request::post(format!("/api/chunks/{flag_id}/dismiss-staleness"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn suppress_duplicate(
    app: axum::Router,
    cookie: &str,
    chunk_id_a: &str,
    chunk_id_b: &str,
) -> axum::response::Response {
    app.oneshot(
        Request::post("/api/chunks/suppress-duplicate")
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(format!(
                r#"{{"chunkIdA":"{chunk_id_a}","chunkIdB":"{chunk_id_b}"}}"#
            )))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn scan_age(
    app: axum::Router,
    cookie: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::post("/api/chunks/stale/scan-age")
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_returns_bare_array_and_is_user_scoped(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-list@b.test", "Alice").await;
    signup(app.clone(), "bob-list@b.test", "Bob").await;
    let alice_id = user_id_for_email(&pool, "alice-list@b.test").await;
    let bob_id = user_id_for_email(&pool, "bob-list@b.test").await;

    let alices_chunk = seed_chunk(&pool, &alice_id, "Alice's chunk").await;
    let bobs_chunk = seed_chunk(&pool, &bob_id, "Bob's chunk").await;
    seed_flag(&pool, &alices_chunk, "age").await;
    seed_flag(&pool, &bobs_chunk, "age").await;

    let res = get_stale(app.clone(), &alice_cookie).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert!(
        body.is_array(),
        "GET /api/chunks/stale must return a bare array"
    );
    let ids: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["chunkId"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec![alices_chunk.as_str()]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn count_returns_a_bare_number_as_text_plain(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-count@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-count@b.test").await;

    let res = get_stale_count(app.clone(), &cookie).await;
    assert_eq!(res.status(), StatusCode::OK);
    let content_type = res
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        content_type.starts_with("text/plain"),
        "count must be text/plain, not application/json — got {content_type}"
    );
    let body = text_body(res).await;
    assert_eq!(body, "0", "count with no flags must literally be \"0\"");

    let chunk_id = seed_chunk(&pool, &user_id, "A chunk").await;
    seed_flag(&pool, &chunk_id, "age").await;
    seed_flag(&pool, &chunk_id, "upstream_impact").await;

    let res = get_stale_count(app.clone(), &cookie).await;
    let body = text_body(res).await;
    assert_eq!(body, "2");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn dismiss_by_owner_succeeds(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-dismiss@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-dismiss@b.test").await;
    let chunk_id = seed_chunk(&pool, &user_id, "A chunk").await;
    let flag_id = seed_flag(&pool, &chunk_id, "age").await;

    let res = dismiss(app.clone(), &cookie, &flag_id).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["command"], "UPDATE");
    assert_eq!(body["rowCount"], 1);
    assert_eq!(body["oid"], serde_json::Value::Null);
    assert_eq!(body["rows"], serde_json::json!([]));
    assert_eq!(body["fields"], serde_json::json!([]));

    let list_body = json_body(get_stale(app.clone(), &cookie).await).await;
    assert_eq!(list_body.as_array().unwrap().len(), 0);
}

/// Divergence #14: Node's `dismissStaleFlag` has no ownership check at
/// all. This port's HTTP layer must answer 404 and leave the victim's flag
/// untouched.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn dismiss_on_another_users_flag_is_404_and_leaves_it_untouched(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-cross@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-cross@b.test", "Bob").await;
    let alice_id = user_id_for_email(&pool, "alice-cross@b.test").await;
    let alices_chunk = seed_chunk(&pool, &alice_id, "Alice's chunk").await;
    let flag_id = seed_flag(&pool, &alices_chunk, "age").await;

    let res = dismiss(app.clone(), &bob_cookie, &flag_id).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let alice_list = json_body(get_stale(app.clone(), &alice_cookie).await).await;
    assert_eq!(
        alice_list.as_array().unwrap().len(),
        1,
        "alice's flag must be unchanged after bob's failed attempt"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn dismiss_nonexistent_flag_is_404(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-404@b.test", "Alice").await;

    let res = dismiss(app.clone(), &cookie, "no-such-flag").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn suppress_duplicate_hides_both_directions(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-suppress@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-suppress@b.test").await;
    let a = seed_chunk(&pool, &user_id, "A").await;
    let b = seed_chunk(&pool, &user_id, "B").await;
    seed_duplicate_flag(&pool, &a, &b).await;
    seed_duplicate_flag(&pool, &b, &a).await;

    let res = suppress_duplicate(app.clone(), &cookie, &a, &b).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["command"], "UPDATE");
    assert_eq!(body["rowCount"], 2);

    let list_body = json_body(get_stale(app.clone(), &cookie).await).await;
    assert_eq!(
        list_body.as_array().unwrap().len(),
        0,
        "suppress must hide both directions of the pair"
    );
}

/// This port's own guard on `suppress_duplicate` (Node's equivalent takes
/// no `userId` at all — a global, cross-user write). Bob owns neither
/// chunk in the pair.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn suppress_duplicate_requires_owning_both_chunks(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-guard@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-guard@b.test", "Bob").await;
    let alice_id = user_id_for_email(&pool, "alice-guard@b.test").await;
    let a = seed_chunk(&pool, &alice_id, "A").await;
    let b = seed_chunk(&pool, &alice_id, "B").await;
    seed_duplicate_flag(&pool, &a, &b).await;
    seed_duplicate_flag(&pool, &b, &a).await;

    let res = suppress_duplicate(app.clone(), &bob_cookie, &a, &b).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let alice_list = json_body(get_stale(app.clone(), &alice_cookie).await).await;
    assert_eq!(
        alice_list.as_array().unwrap().len(),
        2,
        "alice's duplicate flags must be untouched by bob's failed attempt"
    );
}

/// `scan-age` runs both detectors and returns the *sum*. With
/// `requirement_chunk` empty in this workspace, every eligible chunk also
/// flags as `requirement_uncovered` — one stale chunk here yields
/// `flagged: 2` (one `age` flag, one `requirement_uncovered` flag), not 1.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn scan_age_sums_both_detectors(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-scan@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-scan@b.test").await;
    let chunk_id = seed_chunk(&pool, &user_id, "Old chunk").await;
    sqlx::query("UPDATE chunk SET updated_at = now() - interval '200 days' WHERE id = $1")
        .bind(&chunk_id)
        .execute(&pool)
        .await
        .unwrap();

    let res = scan_age(app.clone(), &cookie, serde_json::json!({})).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(
        body,
        serde_json::json!({ "flagged": 2 }),
        "one chunk stale by both age (>90d) and uncovered (>30d) criteria must sum to 2"
    );

    // Idempotent at the route level too.
    let res = scan_age(app.clone(), &cookie, serde_json::json!({})).await;
    let body = json_body(res).await;
    assert_eq!(body, serde_json::json!({ "flagged": 0 }));
}

/// `thresholdDays` in the request body only overrides the age detector —
/// the uncovered detector's 30-day default is never touched by it. A
/// chunk updated 40 days ago is not old enough for a 90-day age threshold,
/// but is old enough to be flagged uncovered (>30d default), so `flagged`
/// must be 1, not 0 and not 2.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn scan_age_threshold_days_only_overrides_the_age_detector(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-threshold@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-threshold@b.test").await;
    let chunk_id = seed_chunk(&pool, &user_id, "Chunk").await;
    sqlx::query("UPDATE chunk SET updated_at = now() - interval '40 days' WHERE id = $1")
        .bind(&chunk_id)
        .execute(&pool)
        .await
        .unwrap();

    let res = scan_age(
        app.clone(),
        &cookie,
        serde_json::json!({ "thresholdDays": 200 }),
    )
    .await;
    let body = json_body(res).await;
    assert_eq!(
        body,
        serde_json::json!({ "flagged": 1 }),
        "thresholdDays=200 must suppress the age flag but not the uncovered flag's fixed 30-day default"
    );
}
