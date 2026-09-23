//! HTTP-level tests for the `use-cases` domain.
//!
//! Cross-user SQL-level guards (`AND user_id = ..` on find/list/update/
//! delete/list_requirements) are proven load-bearing at the repository
//! level in `fubbik-db/tests/use_case.rs` — a service-layer 404 pre-check
//! would otherwise mask a removed SQL guard from a test at this layer, so
//! that's where those guard-removal proofs live. This file covers the five
//! HTTP routes: response shapes (bare arrays / bare rows, no envelope,
//! matching Node's `packages/api/src/use-cases/routes.ts`), status codes,
//! and the nesting-depth / self-parent validation rules end to end.

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

async fn list_use_cases(app: axum::Router, cookie: &str, query: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/use-cases{query}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn create_use_case(
    app: axum::Router,
    cookie: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::post("/api/use-cases")
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn update_use_case(
    app: axum::Router,
    cookie: &str,
    id: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::patch(format!("/api/use-cases/{id}"))
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn delete_use_case(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::delete(format!("/api/use-cases/{id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn use_case_requirements(
    app: axum::Router,
    cookie: &str,
    id: &str,
) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/use-cases/{id}/requirements"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_returns_201_and_bare_row(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-create@b.test", "Alice").await;

    // When
    let res = create_use_case(
        app.clone(),
        &cookie,
        serde_json::json!({ "name": "Checkout flow", "description": "buy stuff" }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = json_body(res).await;
    assert_eq!(body["name"], "Checkout flow");
    assert_eq!(body["description"], "buy stuff");
    assert_eq!(body["order"], 0);
    assert!(body["parentId"].is_null());
    assert!(
        body.get("childCount").is_none(),
        "POST response must be the bare row, not the list-item shape with childCount"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_returns_bare_array_with_counts_and_is_user_scoped(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-list@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-list@b.test", "Bob").await;

    create_use_case(
        app.clone(),
        &alice_cookie,
        serde_json::json!({ "name": "Alice's use case" }),
    )
    .await;
    create_use_case(
        app.clone(),
        &bob_cookie,
        serde_json::json!({ "name": "Bob's use case" }),
    )
    .await;

    // When
    let body = json_body(list_use_cases(app.clone(), &alice_cookie, "").await).await;
    // Then
    assert!(
        body.is_array(),
        "GET /api/use-cases must return a bare array"
    );
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"], "Alice's use case");
    assert_eq!(arr[0]["childCount"], 0);
    assert_eq!(arr[0]["requirementCount"], 0);

    let body = json_body(list_use_cases(app.clone(), &bob_cookie, "").await).await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"], "Bob's use case");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_with_nonexistent_parent_is_404(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-noparent@b.test", "Alice").await;

    // When
    let res = create_use_case(
        app.clone(),
        &cookie,
        serde_json::json!({ "name": "Orphan", "parentId": "does-not-exist" }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let body = json_body(res).await;
    assert_eq!(body["message"], "Parent use case not found");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_cannot_nest_more_than_one_level_deep(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-nest@b.test", "Alice").await;

    let grandparent = json_body(
        create_use_case(
            app.clone(),
            &cookie,
            serde_json::json!({ "name": "Grandparent" }),
        )
        .await,
    )
    .await;
    // When
    let parent_body = create_use_case(
        app.clone(),
        &cookie,
        serde_json::json!({ "name": "Parent", "parentId": grandparent["id"] }),
    )
    .await;
    // Then
    assert_eq!(parent_body.status(), StatusCode::CREATED);
    let parent = json_body(parent_body).await;

    // Parent already has a parent (Grandparent), so nesting a child under
    // it must be rejected.
    let res = create_use_case(
        app.clone(),
        &cookie,
        serde_json::json!({ "name": "Child", "parentId": parent["id"] }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = json_body(res).await;
    // `AppError::Validation`'s `Display` prepends "validation failed: " to
    // every message in this framework (see `fubbik_core::error::AppError`)
    // — an established, crate-wide convention — so this checks the
    // Node-sourced text is present rather than asserting byte-for-byte
    // equality.
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("Cannot nest more than one level deep")
    );
}

/// A non-owner cannot use their own use case ids as a parent reference on
/// someone else's create: the parent lookup is scoped by `user_id`, so
/// Bob's use case id passed by Alice as `parentId` must 404 as "not found",
/// not succeed cross-tenant.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_parent_lookup_is_user_scoped(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-parentscope@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-parentscope@b.test", "Bob").await;

    let bob_uc = json_body(
        create_use_case(
            app.clone(),
            &bob_cookie,
            serde_json::json!({ "name": "Bob's" }),
        )
        .await,
    )
    .await;

    // When
    let res = create_use_case(
        app.clone(),
        &alice_cookie,
        serde_json::json!({ "name": "Alice's child", "parentId": bob_uc["id"] }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// Phase 2e wave 1: `spaceId` ownership is now checked on create — Node's
/// `createUseCaseRepo` has no such guard at all. Bob passing Alice's space
/// id must 404 and create nothing; the mandatory SQL-level guard-removal
/// proof lives in
/// `fubbik-db/tests/use_case.rs::create_rejects_another_users_space_and_creates_nothing`.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_rejects_another_users_space_id(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-spacescope@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-spacescope@b.test", "Bob").await;

    // When
    let alice_space = json_body(
        app.clone()
            .oneshot(
                axum::http::Request::post("/api/spaces")
                    .header("cookie", &alice_cookie)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({ "name": "Alice's space", "kind": "notes" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;

    let res = create_use_case(
        app.clone(),
        &bob_cookie,
        serde_json::json!({ "name": "hijack", "spaceId": alice_space["id"] }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let bobs = json_body(list_use_cases(app, &bob_cookie, "").await).await;
    assert!(
        bobs.as_array().unwrap().is_empty(),
        "the rejected create must not have left a row behind"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_renames_and_returns_bare_row(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-update@b.test", "Alice").await;

    let created = json_body(
        create_use_case(
            app.clone(),
            &cookie,
            serde_json::json!({ "name": "Original" }),
        )
        .await,
    )
    .await;

    // When
    let res = update_use_case(
        app.clone(),
        &cookie,
        created["id"].as_str().unwrap(),
        serde_json::json!({ "name": "Renamed" }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["name"], "Renamed");
}

/// `description` is tri-state through the wire: omitted leaves it, explicit
/// `null` clears it.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_description_tri_state_over_http(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-desctri@b.test", "Alice").await;

    let created = json_body(
        create_use_case(
            app.clone(),
            &cookie,
            serde_json::json!({ "name": "Has desc", "description": "original" }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    // When
    // Omitted -> untouched.
    let body = json_body(
        update_use_case(app.clone(), &cookie, &id, serde_json::json!({ "order": 5 })).await,
    )
    .await;
    // Then
    assert_eq!(body["description"], "original");

    // Explicit null -> cleared.
    let body = json_body(
        update_use_case(
            app.clone(),
            &cookie,
            &id,
            serde_json::json!({ "description": null }),
        )
        .await,
    )
    .await;
    assert!(body["description"].is_null());
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_cannot_set_self_as_parent(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-selfparent@b.test", "Alice").await;

    let created = json_body(
        create_use_case(app.clone(), &cookie, serde_json::json!({ "name": "Solo" })).await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    // When
    let res = update_use_case(
        app.clone(),
        &cookie,
        &id,
        serde_json::json!({ "parentId": id }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = json_body(res).await;
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("Cannot set use case as its own parent")
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_update_is_404_and_leaves_victim_row_intact(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-crossupdate@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-crossupdate@b.test", "Bob").await;

    let created = json_body(
        create_use_case(
            app.clone(),
            &alice_cookie,
            serde_json::json!({ "name": "Alice's" }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    // When
    let res = update_use_case(
        app.clone(),
        &bob_cookie,
        &id,
        serde_json::json!({ "name": "Hijacked" }),
    )
    .await;
    // Then
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let body = json_body(list_use_cases(app.clone(), &alice_cookie, "").await).await;
    assert_eq!(body[0]["name"], "Alice's", "Alice's row must be unchanged");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_removes_the_use_case(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-delete@b.test", "Alice").await;

    let created = json_body(
        create_use_case(
            app.clone(),
            &cookie,
            serde_json::json!({ "name": "Doomed" }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    // When
    let res = delete_use_case(app.clone(), &cookie, &id).await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body, serde_json::json!({ "message": "Deleted" }));

    let body = json_body(list_use_cases(app.clone(), &cookie, "").await).await;
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn cross_user_delete_is_404_and_leaves_victim_row_in_place(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-crossdel@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-crossdel@b.test", "Bob").await;

    let created = json_body(
        create_use_case(
            app.clone(),
            &alice_cookie,
            serde_json::json!({ "name": "Alice's" }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    // When
    let res = delete_use_case(app.clone(), &bob_cookie, &id).await;
    // Then
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let body = json_body(list_use_cases(app.clone(), &alice_cookie, "").await).await;
    assert_eq!(
        body.as_array().unwrap().len(),
        1,
        "Alice's use case must survive"
    );
}

/// `GET /use-cases/{id}/requirements` returns a bare array of full
/// `requirement` rows, and is 404 for a use case that isn't the caller's,
/// even if it exists and belongs to someone else.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn requirements_endpoint_returns_bare_array_and_is_user_scoped(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-reqs@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-reqs@b.test", "Bob").await;

    let alice_id: String =
        sqlx::query_scalar!(r#"SELECT id FROM "user" WHERE email = 'alice-reqs@b.test'"#)
            .fetch_one(&pool)
            .await
            .unwrap();

    let created = json_body(
        create_use_case(
            app.clone(),
            &alice_cookie,
            serde_json::json!({ "name": "With reqs" }),
        )
        .await,
    )
    .await;
    let use_case_id = created["id"].as_str().unwrap().to_string();

    sqlx::query!(
        r#"INSERT INTO requirement (id, title, steps, user_id, use_case_id)
           VALUES ($1, 'Req one', '[]'::jsonb, $2, $3)"#,
        fubbik_db::new_id(),
        alice_id,
        use_case_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // When
    let res = use_case_requirements(app.clone(), &alice_cookie, &use_case_id).await;
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert!(body.is_array(), "must be a bare array");
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["title"], "Req one");

    // Bob does not own this use case, so it's a 404 for him even though it
    // exists.
    let res = use_case_requirements(app.clone(), &bob_cookie, &use_case_id).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unauthenticated_requests_are_401(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(state(pool));

    // When
    let res = app
        .clone()
        .oneshot(Request::get("/api/use-cases").body(Body::empty()).unwrap())
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let res = app
        .oneshot(
            Request::post("/api/use-cases")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"nope"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
