//! HTTP-level tests for the `favorites` domain.
//!
//! Node's captured contract (`tests/fixtures/node-contract-2b/favorites-list.json`,
//! `_mutating.md`) is the source of truth for three surprising behaviors
//! reproduced deliberately, not "fixed":
//!
//! 1. `POST /api/favorites` on an already-favorited chunk returns **201**
//!    with body **`null`** — Node's `Effect.tap` sets the status before
//!    ever inspecting the (possibly-`null`) insert result.
//! 2. `DELETE /api/favorites/{chunkId}` **never 404s** — it always answers
//!    `{"message":"Deleted"}` with status 200, whether or not a row
//!    existed.
//! 3. `GET /api/favorites` returns a **bare array**, not the
//!    `{chunks,total,limit,offset}` envelope the chunks domain uses.

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
/// `tests/tag_types.rs::signup`.
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

async fn user_id_for_email(pool: &sqlx::PgPool, email: &str) -> String {
    sqlx::query_scalar!(r#"SELECT id FROM "user" WHERE email = $1"#, email)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Seeds a chunk directly via the repository (there is no reason to go
/// through HTTP for this — favorites don't create chunks).
async fn seed_chunk(pool: &sqlx::PgPool, user_id: &str, title: &str) -> String {
    fubbik_db::repo::chunk::create(
        pool,
        user_id,
        fubbik_db::repo::chunk::NewChunk {
            title: title.into(),
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

async fn list_favorites(app: axum::Router, cookie: &str) -> axum::response::Response {
    app.oneshot(
        Request::get("/api/favorites")
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn add_favorite(app: axum::Router, cookie: &str, chunk_id: &str) -> axum::response::Response {
    app.oneshot(
        Request::post("/api/favorites")
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(format!(r#"{{"chunkId":"{chunk_id}"}}"#)))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn remove_favorite(
    app: axum::Router,
    cookie: &str,
    chunk_id: &str,
) -> axum::response::Response {
    app.oneshot(
        Request::delete(format!("/api/favorites/{chunk_id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn reorder_favorites(
    app: axum::Router,
    cookie: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::put("/api/favorites/reorder")
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
    let bob_cookie = signup(app.clone(), "bob-list@b.test", "Bob").await;
    let alice_id = user_id_for_email(&pool, "alice-list@b.test").await;
    let bob_id = user_id_for_email(&pool, "bob-list@b.test").await;

    let alices_chunk = seed_chunk(&pool, &alice_id, "Alice's chunk").await;
    let bobs_chunk = seed_chunk(&pool, &bob_id, "Bob's chunk").await;
    add_favorite(app.clone(), &alice_cookie, &alices_chunk).await;
    add_favorite(app.clone(), &bob_cookie, &bobs_chunk).await;

    let res = list_favorites(app.clone(), &alice_cookie).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert!(
        body.is_array(),
        "GET /api/favorites must return a bare array, not the {{chunks,...}} envelope"
    );
    let chunk_ids: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["chunkId"].as_str().unwrap())
        .collect();
    assert_eq!(chunk_ids, vec![alices_chunk.as_str()]);

    // Bob's own favorite must also still be intact and visible from his own
    // view — a status-only/single-side check would pass even if Alice's
    // list handler had somehow mutated or dropped Bob's row.
    let body = json_body(list_favorites(app, &bob_cookie).await).await;
    let chunk_ids: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["chunkId"].as_str().unwrap())
        .collect();
    assert_eq!(chunk_ids, vec![bobs_chunk.as_str()]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn add_favorite_returns_201_and_the_row(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-add@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-add@b.test").await;
    let chunk_id = seed_chunk(&pool, &user_id, "Alice's chunk").await;

    let res = add_favorite(app.clone(), &cookie, &chunk_id).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = json_body(res).await;
    assert_eq!(body["chunkId"], chunk_id);
    assert_eq!(body["userId"], user_id);
    assert_eq!(body["order"], 0);
}

/// Adding a second favorite must append at the end of the caller's
/// current ordering (`order` computed app-side as `max(order) + 1`),
/// matching Node's `addFavorite`.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn add_favorite_appends_at_the_end_of_the_current_order(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-append@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-append@b.test").await;
    let c1 = seed_chunk(&pool, &user_id, "One").await;
    let c2 = seed_chunk(&pool, &user_id, "Two").await;

    let body = json_body(add_favorite(app.clone(), &cookie, &c1).await).await;
    assert_eq!(body["order"], 0);

    let body = json_body(add_favorite(app.clone(), &cookie, &c2).await).await;
    assert_eq!(body["order"], 1);
}

/// The one deliberately preserved Node quirk this domain is built around:
/// favoriting an already-favorited chunk answers 201 with a `null` body,
/// not an error and not the existing row.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn add_favorite_duplicate_returns_201_with_null_body(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-dup@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-dup@b.test").await;
    let chunk_id = seed_chunk(&pool, &user_id, "Alice's chunk").await;

    let res = add_favorite(app.clone(), &cookie, &chunk_id).await;
    assert_eq!(res.status(), StatusCode::CREATED);

    let res = add_favorite(app.clone(), &cookie, &chunk_id).await;
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "status must still be 201 on a duplicate favorite, not 200/409"
    );
    let body = json_body(res).await;
    assert_eq!(
        body,
        serde_json::Value::Null,
        "duplicate favorite must respond with a literal null body"
    );

    // Only one row must exist — this was a no-op, not an idempotent echo.
    let list_body = json_body(list_favorites(app.clone(), &cookie).await).await;
    assert_eq!(list_body.as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn add_favorite_on_another_users_chunk_is_404_and_creates_nothing(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-cross@b.test", "Alice").await;
    let bob_id = {
        signup(app.clone(), "bob-cross@b.test", "Bob").await;
        user_id_for_email(&pool, "bob-cross@b.test").await
    };
    let bobs_chunk = seed_chunk(&pool, &bob_id, "Bob's chunk").await;

    let res = add_favorite(app.clone(), &alice_cookie, &bobs_chunk).await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "favoriting another user's chunk must 404"
    );

    // No favorite was created for anyone.
    let alice_list = json_body(list_favorites(app.clone(), &alice_cookie).await).await;
    assert_eq!(alice_list.as_array().unwrap().len(), 0);

    let remaining: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM user_favorite WHERE chunk_id = $1"#,
        bobs_chunk
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        remaining, 0,
        "Bob's chunk must not have been favorited by anyone"
    );
}

/// Deliberate divergence from every other delete in this codebase's Phase
/// 2b slice: this one never 404s, cross-user or otherwise. The test still
/// proves the victim's data is untouched — just via a 200, not a 404.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn remove_never_404s_even_cross_user_and_leaves_victim_data_unchanged(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-remove@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-remove@b.test", "Bob").await;
    let alice_id = user_id_for_email(&pool, "alice-remove@b.test").await;
    let alices_chunk = seed_chunk(&pool, &alice_id, "Alice's chunk").await;
    add_favorite(app.clone(), &alice_cookie, &alices_chunk).await;

    // Bob tries to remove Alice's favorite by naming her chunk id.
    let res = remove_favorite(app.clone(), &bob_cookie, &alices_chunk).await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "DELETE /api/favorites/{{chunkId}} must never 404, even cross-user"
    );
    let body = json_body(res).await;
    assert_eq!(body, serde_json::json!({ "message": "Deleted" }));

    // Alice's favorite must be untouched.
    let alice_list = json_body(list_favorites(app.clone(), &alice_cookie).await).await;
    assert_eq!(alice_list.as_array().unwrap().len(), 1);

    // Removing a chunk that was never favorited at all is also 200, not 404.
    let res = remove_favorite(app.clone(), &alice_cookie, "no-such-chunk-id").await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body, serde_json::json!({ "message": "Deleted" }));
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn remove_removes_the_callers_own_favorite(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-remove-own@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-remove-own@b.test").await;
    let chunk_id = seed_chunk(&pool, &user_id, "Alice's chunk").await;
    add_favorite(app.clone(), &cookie, &chunk_id).await;

    let res = remove_favorite(app.clone(), &cookie, &chunk_id).await;
    assert_eq!(res.status(), StatusCode::OK);

    let list = json_body(list_favorites(app.clone(), &cookie).await).await;
    assert_eq!(list.as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn reorder_applies_partial_updates_and_ignores_foreign_chunk_ids(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-reorder@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-reorder@b.test", "Bob").await;
    let alice_id = user_id_for_email(&pool, "alice-reorder@b.test").await;
    let bob_id = user_id_for_email(&pool, "bob-reorder@b.test").await;

    let c1 = seed_chunk(&pool, &alice_id, "One").await;
    let c2 = seed_chunk(&pool, &alice_id, "Two").await;
    let bobs_chunk = seed_chunk(&pool, &bob_id, "Bob's").await;
    add_favorite(app.clone(), &alice_cookie, &c1).await;
    add_favorite(app.clone(), &alice_cookie, &c2).await;
    add_favorite(app.clone(), &bob_cookie, &bobs_chunk).await;

    let res = reorder_favorites(
        app.clone(),
        &alice_cookie,
        serde_json::json!([
            { "chunkId": c1, "order": 50 },
            // Bob's chunk id is not one of Alice's favorites — this entry
            // must silently touch zero rows, not error the whole request.
            { "chunkId": bobs_chunk, "order": 99 },
        ]),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body, serde_json::json!({ "message": "Reordered" }));

    let alice_list = json_body(list_favorites(app.clone(), &alice_cookie).await).await;
    let by_chunk: std::collections::HashMap<String, i64> = alice_list
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["chunkId"].as_str().unwrap().to_string(),
                f["order"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(by_chunk[&c1], 50);
    assert_eq!(
        by_chunk[&c2], 1,
        "c2 was not mentioned, so its order must be unchanged"
    );

    // Bob's favorite must be entirely unaffected by Alice's reorder call.
    let bob_list = json_body(list_favorites(app.clone(), &bob_cookie).await).await;
    assert_eq!(bob_list[0]["order"], 0);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unauthenticated_requests_are_401(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let res = app
        .clone()
        .oneshot(Request::get("/api/favorites").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let res = app
        .oneshot(
            Request::post("/api/favorites")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"chunkId":"x"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
