//! HTTP-level tests for the `templates` domain (`GET/POST /api/templates`,
//! `PATCH/DELETE /api/templates/{id}`).
//!
//! The cross-user SQL ownership guards on `update`/`delete` are proven
//! load-bearing at the repo level in `fubbik-db/tests/template.rs` (an
//! API-level test can't distinguish "the SQL guard caught it" from "the
//! unscoped existence pre-check 404s and never reaches SQL" — both look
//! like a 404 from here). What THIS file proves that the repo tests
//! can't: the service-layer `is_built_in` rejection Node's
//! `updateTemplate`/`deleteTemplate` make
//! (`packages/api/src/templates/service.ts:62-64,73-75`) surfaces as a 400
//! with the exact copied message, not a 404 or a 500.

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

async fn seed_builtin(pool: &sqlx::PgPool, name: &str) -> String {
    let id = fubbik_db::new_id();
    sqlx::query!(
        r#"INSERT INTO chunk_template (id, name, type, content, is_built_in, user_id)
           VALUES ($1, $2, 'note', '', true, NULL)"#,
        id,
        name
    )
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn list_templates(app: axum::Router, cookie: &str) -> axum::response::Response {
    app.oneshot(
        Request::get("/api/templates")
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn create_template(
    app: axum::Router,
    cookie: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::post("/api/templates")
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn update_template(
    app: axum::Router,
    cookie: &str,
    id: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::patch(format!("/api/templates/{id}"))
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn delete_template(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::delete(format!("/api/templates/{id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

fn create_body(name: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "type": "reference",
        "content": "## Rationale"
    })
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_returns_bare_array_with_builtin_and_own_only(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-list@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-list@b.test", "Bob").await;

    create_template(app.clone(), &alice_cookie, create_body("Alice's")).await;
    create_template(app.clone(), &bob_cookie, create_body("Bob's")).await;
    seed_builtin(&pool, "Convention").await;

    let res = list_templates(app.clone(), &alice_cookie).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert!(
        body.is_array(),
        "GET /api/templates must return a bare array"
    );
    let names: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"Alice's"));
    assert!(names.contains(&"Convention"));
    assert!(
        !names.contains(&"Bob's"),
        "must not leak another user's template"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_returns_201_with_created_template(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-create@b.test", "Alice").await;

    let res = create_template(app.clone(), &cookie, create_body("New Template")).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = json_body(res).await;
    assert_eq!(body["name"], "New Template");
    assert_eq!(body["type"], "reference");
    assert_eq!(body["isBuiltIn"], false);
    assert_eq!(body["priority"], 0);
    assert!(body["id"].is_string());
}

/// Round-trips `matchRules`/`fieldMappings` through the exact camelCase
/// keys Node's Elysia schema uses.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_with_match_rules_round_trips_camel_case_shape(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-mr@b.test", "Alice").await;

    let mut body = create_body("With Rules");
    body["matchRules"] = serde_json::json!({
        "minScore": 1.5,
        "headings": [{"patterns": ["Rationale"], "match": "prefix", "level": 2, "required": true}],
        "frontmatter": [{"key": "status", "match": "exists"}]
    });
    body["fieldMappings"] = serde_json::json!([
        {"headings": ["Rationale"], "match": "contains", "target": "rationale"}
    ]);
    body["tags"] = serde_json::json!(["adr"]);
    body["priority"] = serde_json::json!(3);

    let res = create_template(app.clone(), &cookie, body).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let resp = json_body(res).await;
    assert_eq!(resp["matchRules"]["minScore"], 1.5);
    assert_eq!(resp["matchRules"]["headings"][0]["match"], "prefix");
    assert_eq!(resp["fieldMappings"][0]["target"], "rationale");
    assert_eq!(resp["tags"], serde_json::json!(["adr"]));
    assert_eq!(resp["priority"], 3);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_owner_succeeds_and_returns_updated_template(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-update@b.test", "Alice").await;

    let created =
        json_body(create_template(app.clone(), &cookie, create_body("Original")).await).await;
    let id = created["id"].as_str().unwrap();

    let res = update_template(
        app.clone(),
        &cookie,
        id,
        serde_json::json!({ "name": "Renamed" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["name"], "Renamed");
    assert_eq!(
        body["content"], "## Rationale",
        "omitted field must be left untouched"
    );
}

/// Explicit `null` on `description` clears it; omitting it entirely on a
/// later PATCH leaves the (now-null) value untouched — proves the DTO's
/// tri-state `deserialize_some` wiring reaches the repo's `CASE WHEN`
/// correctly end to end, not just at the repo layer directly.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_null_description_clears_it_and_omitted_leaves_it_untouched(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-null@b.test", "Alice").await;

    let mut body = create_body("HasDesc");
    body["description"] = serde_json::json!("original description");
    let created = json_body(create_template(app.clone(), &cookie, body).await).await;
    let id = created["id"].as_str().unwrap();

    let res = update_template(
        app.clone(),
        &cookie,
        id,
        serde_json::json!({ "description": null }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["description"], serde_json::Value::Null);

    let res = update_template(
        app.clone(),
        &cookie,
        id,
        serde_json::json!({ "name": "still there" }),
    )
    .await;
    let body = json_body(res).await;
    assert_eq!(
        body["description"],
        serde_json::Value::Null,
        "description was already cleared and must stay cleared when omitted from a later PATCH"
    );
}

/// The named test proving Node's built-in-rejection check
/// (`packages/api/src/templates/service.ts:62-64`) is replicated: PATCHing
/// a built-in template must 400 with the exact copied message, never 404
/// or a successful mutation. Removing `templates::service::update`'s
/// `is_built_in` check makes this test fail — the underlying row still
/// can't be mutated (its `user_id` is `NULL`, `template::update`'s own
/// `WHERE user_id = $2` can never match), but the response flips from 400
/// to 404, so this assertion on `StatusCode::BAD_REQUEST` catches it.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_rejects_built_in_template_with_400(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-builtin@b.test", "Alice").await;
    let builtin_id = seed_builtin(&pool, "Convention").await;

    let res = update_template(
        app.clone(),
        &cookie,
        &builtin_id,
        serde_json::json!({ "name": "hijacked" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = json_body(res).await;
    // `AppError::Validation`'s `Display` prepends "validation failed: " to
    // every message, a crate-wide convention (see `tags.rs`'s equivalent
    // comment) — so this checks the Node-sourced text is present rather
    // than asserting byte-for-byte equality against Node's raw message.
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("Cannot edit built-in templates")
    );

    let still_named: String =
        sqlx::query_scalar!("SELECT name FROM chunk_template WHERE id = $1", builtin_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(still_named, "Convention", "built-in row must be unmutated");
}

/// Same shape as `update_rejects_built_in_template_with_400`, for delete.
/// Named test proving Node's `deleteTemplate` built-in check
/// (`packages/api/src/templates/service.ts:73-75`) is replicated.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_rejects_built_in_template_with_400(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-builtin-del@b.test", "Alice").await;
    let builtin_id = seed_builtin(&pool, "Convention").await;

    let res = delete_template(app.clone(), &cookie, &builtin_id).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = json_body(res).await;
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("Cannot delete built-in templates")
    );

    let still_there: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM chunk_template WHERE id = $1"#,
        builtin_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(still_there, 1, "built-in row must not be deleted");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_cross_user_is_404_and_leaves_victim_unchanged(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-x@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-x@b.test", "Bob").await;

    let created =
        json_body(create_template(app.clone(), &alice_cookie, create_body("Alice's")).await).await;
    let id = created["id"].as_str().unwrap();

    let res = update_template(
        app.clone(),
        &bob_cookie,
        id,
        serde_json::json!({ "name": "hijacked" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let still_named: String =
        sqlx::query_scalar!("SELECT name FROM chunk_template WHERE id = $1", id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        still_named, "Alice's",
        "Alice's template must be unaffected"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_cross_user_is_404_and_leaves_victim_unchanged(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-y@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-y@b.test", "Bob").await;

    let created =
        json_body(create_template(app.clone(), &alice_cookie, create_body("Alice's")).await).await;
    let id = created["id"].as_str().unwrap();

    let res = delete_template(app.clone(), &bob_cookie, id).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let still_there: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM chunk_template WHERE id = $1"#,
        id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        still_there, 1,
        "Alice's template must not have been deleted"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_returns_message_deleted_and_removes_row(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-del@b.test", "Alice").await;

    let created =
        json_body(create_template(app.clone(), &cookie, create_body("Disposable")).await).await;
    let id = created["id"].as_str().unwrap();

    let res = delete_template(app.clone(), &cookie, id).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body, serde_json::json!({ "message": "Deleted" }));

    let still_there: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM chunk_template WHERE id = $1"#,
        id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(still_there, 0);
}
