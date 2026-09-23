//! HTTP-level tests for the `collections` domain.
//!
//! `collection::create`'s `space_id` ownership guard is proven load-bearing
//! at the repository level in `fubbik-db/tests/collection.rs` (both the
//! `EXISTS` SQL guard and, separately, the service-layer pre-check — see
//! that file and this one's `create_in_another_users_space_is_404` for
//! which layer catches what). This file covers the five HTTP routes:
//! response shapes (per `tests/fixtures/node-contract-2b/collections-*.json`),
//! status codes, PATCH's replace-not-merge `filter` semantics, and —
//! the highest-severity case in this slice — that
//! `GET /collections/{id}/chunks` cannot leak another user's chunks even
//! when the collection's stored filter names a tag or type they also use.

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
        rate_limiter: Default::default(),
        background: Default::default(),
    }
}

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

async fn seed_space(pool: &sqlx::PgPool, user_id: &str, name: &str) -> String {
    fubbik_db::repo::space::create(
        pool,
        user_id,
        fubbik_db::repo::space::NewSpace {
            name: name.into(),
            kind: "wiki".into(),
            description: None,
        },
        None,
    )
    .await
    .unwrap()
    .id
}

/// Seeds a chunk with a tag, directly via the repository — there is no
/// route in this port that assigns tags to a chunk yet, and the `tags`
/// domain has its own test suite.
async fn seed_tagged_chunk(pool: &sqlx::PgPool, user_id: &str, title: &str, tag_name: &str) {
    let c = fubbik_db::repo::chunk::create(
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
    .unwrap();
    let t = fubbik_db::repo::tag::create(pool, user_id, tag_name, None)
        .await
        .unwrap();
    fubbik_db::repo::tag::set_chunk_tags(pool, user_id, &c.id, &[t.id])
        .await
        .unwrap();
}

async fn create_collection(
    app: axum::Router,
    cookie: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::post("/api/collections")
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn update_collection(
    app: axum::Router,
    cookie: &str,
    id: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::patch(format!("/api/collections/{id}"))
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn delete_collection(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::delete(format!("/api/collections/{id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get_chunks(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/collections/{id}/chunks"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_returns_201_and_stores_the_filter_verbatim(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-create@b.test", "Alice").await;

    // When
    let res = create_collection(
        app.clone(),
        &cookie,
        serde_json::json!({
            "name": "Conventions",
            "description": "Pinned",
            "filter": { "type": "convention" }
        }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = json_body(res).await;
    assert_eq!(body["name"], "Conventions");
    assert_eq!(body["description"], "Pinned");
    assert_eq!(body["filter"], serde_json::json!({ "type": "convention" }));
    assert!(body["id"].is_string());
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_is_user_scoped(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-list@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-list@b.test", "Bob").await;

    create_collection(
        app.clone(),
        &alice_cookie,
        serde_json::json!({ "name": "Alice's", "filter": {} }),
    )
    .await;
    create_collection(
        app.clone(),
        &bob_cookie,
        serde_json::json!({ "name": "Bob's", "filter": {} }),
    )
    .await;

    // When
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/collections")
                .header("cookie", &alice_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let names: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Alice's"]);

    // Bob's own collection must also still be intact and visible from his
    // own view — a single-side check would pass even if Alice's list
    // handler had somehow mutated or dropped Bob's row.
    let res = app
        .oneshot(
            Request::get("/api/collections")
                .header("cookie", &bob_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let names: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Bob's"]);
}

/// Node has no ownership check on `spaceId` at create time at all — this
/// 404 is a deliberate Rust-side addition (see
/// `fubbik_db::repo::collection::create`'s doc comment). This test also
/// proves the service-layer pre-check (`space::find_by_id` in
/// `collections::service::create`) is reachable end to end; the repo's own
/// `EXISTS` guard is proven independently in
/// `fubbik-db/tests/collection.rs::create_rejects_another_users_space_and_creates_nothing`.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_in_another_users_space_is_404(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-space@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-space@b.test", "Bob").await;
    let bob_id = user_id_for_email(&pool, "bob-space@b.test").await;
    let bobs_space = seed_space(&pool, &bob_id, "bobs-space").await;

    // When
    let res = create_collection(
        app.clone(),
        &alice_cookie,
        serde_json::json!({ "name": "hijack", "filter": {}, "spaceId": bobs_space }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = app
        .oneshot(
            Request::get("/api/collections")
                .header("cookie", &bob_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(res).await;
    assert_eq!(
        body.as_array().unwrap().len(),
        0,
        "Bob must not have gained a collection from Alice's rejected create"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_replaces_filter_wholesale_and_leaves_omitted_fields(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-update@b.test", "Alice").await;
    let created = json_body(
        create_collection(
            app.clone(),
            &cookie,
            serde_json::json!({
                "name": "C",
                "description": "original",
                "filter": { "type": "convention", "tags": "x" }
            }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap();

    // When
    let res = update_collection(
        app.clone(),
        &cookie,
        id,
        serde_json::json!({ "filter": { "type": "note" } }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(
        body["filter"],
        serde_json::json!({ "type": "note" }),
        "PATCH filter must replace wholesale — the old `tags` key must be gone"
    );
    assert_eq!(
        body["description"], "original",
        "omitted description must be left untouched"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_on_another_users_collection_is_404_and_leaves_it_unchanged(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-cross-update@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-cross-update@b.test", "Bob").await;
    let created = json_body(
        create_collection(
            app.clone(),
            &bob_cookie,
            serde_json::json!({ "name": "Bob's", "filter": {} }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap();

    // When
    let res = update_collection(
        app.clone(),
        &alice_cookie,
        id,
        serde_json::json!({ "name": "hijacked" }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = app
        .oneshot(
            Request::get("/api/collections")
                .header("cookie", &bob_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(res).await;
    assert_eq!(
        body[0]["name"], "Bob's",
        "Bob's collection must be unchanged"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_returns_message_and_404s_on_second_call(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-delete@b.test", "Alice").await;
    let created = json_body(
        create_collection(
            app.clone(),
            &cookie,
            serde_json::json!({ "name": "C", "filter": {} }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    // When
    let res = delete_collection(app.clone(), &cookie, &id).await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body, serde_json::json!({ "message": "Deleted" }));

    let res = delete_collection(app.clone(), &cookie, &id).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_on_another_users_collection_is_404_and_leaves_it_intact(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-cross-delete@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-cross-delete@b.test", "Bob").await;
    let created = json_body(
        create_collection(
            app.clone(),
            &bob_cookie,
            serde_json::json!({ "name": "Bob's", "filter": {} }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    // When
    let res = delete_collection(app.clone(), &alice_cookie, &id).await;
    // Then
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = app
        .oneshot(
            Request::get("/api/collections")
                .header("cookie", &bob_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(res).await;
    assert_eq!(
        body.as_array().unwrap().len(),
        1,
        "Bob's collection must survive"
    );
}

/// `GET /collections/{id}/chunks` inherits the chunks
/// `{chunks, total, limit, offset}` envelope by delegating to the exact
/// same `chunks::service::list` used by `GET /api/chunks` — the one
/// endpoint in this slice that is NOT bare, per `tests/fixtures/
/// node-contract-2b/collections-chunks-filter-type.json`.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_chunks_returns_the_chunks_envelope_not_a_bare_array(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-envelope@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-envelope@b.test").await;
    seed_tagged_chunk(&pool, &user_id, "A convention", "unused").await;

    let created = json_body(
        create_collection(
            app.clone(),
            &cookie,
            serde_json::json!({ "name": "All notes", "filter": {} }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap();

    // When
    let res = get_chunks(app.clone(), &cookie, id).await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert!(
        body["chunks"].is_array(),
        "must be the envelope, not a bare array"
    );
    assert_eq!(body["total"], 1);
    assert_eq!(body["limit"], 50);
    assert_eq!(body["offset"], 0);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_chunks_on_another_users_collection_is_404(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-chunks-cross@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-chunks-cross@b.test", "Bob").await;
    let created = json_body(
        create_collection(
            app.clone(),
            &bob_cookie,
            serde_json::json!({ "name": "Bob's", "filter": {} }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap();

    // When
    let res = get_chunks(app.clone(), &alice_cookie, id).await;
    // Then
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// The highest-severity case in this slice: Alice's collection stores a
/// `tags` filter naming a tag whose *name* Bob also happens to use on his
/// own chunk. Evaluating the collection must never surface Bob's chunk —
/// the filter is caller-supplied data flowing into a query, and the only
/// thing standing between "saved search" and "cross-user data leak" is the
/// mandatory `user_id` scoping in `chunk::list`'s `WHERE` clause. See
/// `fubbik-db/tests/chunk.rs::tags_filter_cannot_leak_another_users_chunk_via_a_same_named_tag`
/// for the same property proven directly against the repository.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_chunks_cannot_leak_another_users_chunk_via_a_same_named_tag(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-leak@b.test", "Alice").await;
    signup(app.clone(), "bob-leak@b.test", "Bob").await;
    let alice_id = user_id_for_email(&pool, "alice-leak@b.test").await;
    let bob_id = user_id_for_email(&pool, "bob-leak@b.test").await;

    seed_tagged_chunk(&pool, &alice_id, "Alice's chunk", "shared").await;
    seed_tagged_chunk(&pool, &bob_id, "Bob's chunk", "shared").await;

    let created = json_body(
        create_collection(
            app.clone(),
            &alice_cookie,
            serde_json::json!({ "name": "Shared-tagged", "filter": { "tags": "shared" } }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap();

    // When
    let res = get_chunks(app.clone(), &alice_cookie, id).await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let chunks = body["chunks"].as_array().unwrap();
    assert_eq!(chunks.len(), 1, "must return exactly Alice's own chunk");
    assert_eq!(chunks[0]["title"], "Alice's chunk");
    assert_eq!(body["total"], 1);
}

/// Filter mapping end to end for the `type` key, matching
/// `tests/fixtures/node-contract-2b/collections-chunks-filter-type.json`'s
/// shape (this port's seed data differs, so only the mechanism — not the
/// literal fixture rows — is asserted).
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_chunks_filters_by_stored_type(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-type@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-type@b.test").await;

    fubbik_db::repo::chunk::create(
        &pool,
        &user_id,
        fubbik_db::repo::chunk::NewChunk {
            title: "A convention".into(),
            content: String::new(),
            chunk_type: "convention".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    fubbik_db::repo::chunk::create(
        &pool,
        &user_id,
        fubbik_db::repo::chunk::NewChunk {
            title: "A note".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let created = json_body(
        create_collection(
            app.clone(),
            &cookie,
            serde_json::json!({ "name": "Conventions", "filter": { "type": "convention" } }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap();

    // When
    let res = get_chunks(app.clone(), &cookie, id).await;
    let body = json_body(res).await;
    let chunks = body["chunks"].as_array().unwrap();
    // Then
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0]["title"], "A convention");
}

/// The bug this test exists to catch: Node's `getCollectionChunks` threads
/// `col.spaceId` into `listChunks` (`packages/api/src/collections/
/// service.ts:74`) — a collection pinned to a space must only surface that
/// space's chunks (plus global chunks with no space at all), never a
/// same-user chunk sitting in a *different* space. `total` must reflect the
/// narrowed count too, not the caller's whole chunk count.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_chunks_is_narrowed_to_the_collections_pinned_space(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-space-scope@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-space-scope@b.test").await;

    let pinned_space = seed_space(&pool, &user_id, "pinned").await;
    let other_space = seed_space(&pool, &user_id, "other").await;

    let in_space = fubbik_db::repo::chunk::create(
        &pool,
        &user_id,
        fubbik_db::repo::chunk::NewChunk {
            title: "In pinned space".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let in_other_space = fubbik_db::repo::chunk::create(
        &pool,
        &user_id,
        fubbik_db::repo::chunk::NewChunk {
            title: "In other space".into(),
            content: String::new(),
            chunk_type: "note".into(),
            rationale: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    fubbik_db::repo::space::set_chunk_spaces(
        &pool,
        &user_id,
        &in_space.id,
        std::slice::from_ref(&pinned_space),
    )
    .await
    .unwrap();
    fubbik_db::repo::space::set_chunk_spaces(
        &pool,
        &user_id,
        &in_other_space.id,
        std::slice::from_ref(&other_space),
    )
    .await
    .unwrap();

    let created = json_body(
        create_collection(
            app.clone(),
            &cookie,
            serde_json::json!({ "name": "Pinned", "filter": {}, "spaceId": pinned_space }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap();

    // When
    let res = get_chunks(app.clone(), &cookie, id).await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let chunks = body["chunks"].as_array().unwrap();
    assert_eq!(
        chunks.len(),
        1,
        "must exclude the same-user chunk pinned to a different space"
    );
    assert_eq!(chunks[0]["title"], "In pinned space");
    assert_eq!(
        body["total"], 1,
        "total must reflect the space-narrowed count, not the caller's whole chunk count"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unauthenticated_requests_are_401(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));

    // When
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/collections")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let res = app
        .oneshot(
            Request::post("/api/collections")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"x","filter":{}}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
