//! HTTP-level tests for the `saved-graphs` domain
//! (`packages/api/src/saved-graphs/`). Repo-level guards (`user_id`
//! scoping, list ordering, tri-state `description`, no-op-doesn't-bump
//! semantics) are proven directly against SQL in
//! `fubbik-db/tests/saved_graph.rs` — this file covers the five HTTP
//! routes: response shapes, status codes, and validation messages.

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

async fn list_saved_graphs(
    app: axum::Router,
    cookie: &str,
    query: &str,
) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/saved-graphs{query}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn create_saved_graph(
    app: axum::Router,
    cookie: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::post("/api/saved-graphs")
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get_saved_graph(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/saved-graphs/{id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn update_saved_graph(
    app: axum::Router,
    cookie: &str,
    id: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::patch(format!("/api/saved-graphs/{id}"))
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn delete_saved_graph(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::delete(format!("/api/saved-graphs/{id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

fn a_graph_body() -> serde_json::Value {
    serde_json::json!({
        "name": "My Graph",
        "chunkIds": ["chunk-1", "chunk-2"],
        "positions": { "chunk-1": { "x": 1.0, "y": 2.0 } }
    })
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_returns_201_defaults_layout_and_trims_name(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-create@b.test", "Alice").await;

    let res = create_saved_graph(
        app.clone(),
        &cookie,
        serde_json::json!({
            "name": "  My Graph  ",
            "chunkIds": ["chunk-1", "chunk-2"],
            "positions": { "chunk-1": { "x": 1.0, "y": 2.0 } }
        }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = json_body(res).await;
    assert_eq!(body["name"], "My Graph", "name must be trimmed");
    assert_eq!(body["chunkIds"], serde_json::json!(["chunk-1", "chunk-2"]));
    assert_eq!(
        body["positions"],
        serde_json::json!({ "chunk-1": { "x": 1.0, "y": 2.0 } })
    );
    assert_eq!(
        body["layoutAlgorithm"], "force",
        "layoutAlgorithm must default to force when omitted"
    );
    assert!(body["id"].is_string());
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_rejects_blank_name(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-blank@b.test", "Alice").await;

    let mut body = a_graph_body();
    body["name"] = serde_json::json!("   ");
    let res = create_saved_graph(app.clone(), &cookie, body).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let json = json_body(res).await;
    assert_eq!(
        json["message"],
        "validation failed: Saved graph name is required"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_is_user_scoped(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-list@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-list@b.test", "Bob").await;

    let mut alice_body = a_graph_body();
    alice_body["name"] = serde_json::json!("Alice's");
    create_saved_graph(app.clone(), &alice_cookie, alice_body).await;
    let mut bob_body = a_graph_body();
    bob_body["name"] = serde_json::json!("Bob's");
    create_saved_graph(app.clone(), &bob_cookie, bob_body).await;

    let body = json_body(list_saved_graphs(app.clone(), &alice_cookie, "").await).await;
    let names: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Alice's"]);

    // Bob's own list must also still be intact — a single-side check would
    // pass even if Alice's list handler had somehow mutated or dropped
    // Bob's row.
    let body = json_body(list_saved_graphs(app.clone(), &bob_cookie, "").await).await;
    let names: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Bob's"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_filters_by_space_id_query_param(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-space@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-space@b.test").await;
    let space_id = seed_space(&pool, &user_id, "my-space").await;

    let mut in_space = a_graph_body();
    in_space["name"] = serde_json::json!("In space");
    in_space["spaceId"] = serde_json::json!(space_id);
    create_saved_graph(app.clone(), &cookie, in_space).await;
    let mut no_space = a_graph_body();
    no_space["name"] = serde_json::json!("No space");
    create_saved_graph(app.clone(), &cookie, no_space).await;

    let body = json_body(list_saved_graphs(app.clone(), &cookie, "").await).await;
    assert_eq!(body.as_array().unwrap().len(), 2);

    let body =
        json_body(list_saved_graphs(app.clone(), &cookie, &format!("?spaceId={space_id}")).await)
            .await;
    let names: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["In space"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_returns_the_saved_graph(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-get@b.test", "Alice").await;
    let created = json_body(create_saved_graph(app.clone(), &cookie, a_graph_body()).await).await;
    let id = created["id"].as_str().unwrap();

    let res = get_saved_graph(app.clone(), &cookie, id).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["id"], id);
    assert_eq!(body["name"], "My Graph");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_on_another_users_saved_graph_is_404(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-cross-get@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-cross-get@b.test", "Bob").await;
    let created =
        json_body(create_saved_graph(app.clone(), &bob_cookie, a_graph_body()).await).await;
    let id = created["id"].as_str().unwrap();

    let res = get_saved_graph(app.clone(), &alice_cookie, id).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_changes_only_given_fields_and_persists(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-update@b.test", "Alice").await;
    let created = json_body(create_saved_graph(app.clone(), &cookie, a_graph_body()).await).await;
    let id = created["id"].as_str().unwrap();

    let res = update_saved_graph(
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
        body["chunkIds"], created["chunkIds"],
        "omitted chunkIds must be untouched"
    );

    let refetched = json_body(get_saved_graph(app.clone(), &cookie, id).await).await;
    assert_eq!(refetched["name"], "Renamed", "update must persist");
}

/// `description` is tri-state on PATCH: omitted leaves it untouched,
/// explicit `null` clears it.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_description_tri_state(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-tristate@b.test", "Alice").await;
    let mut body = a_graph_body();
    body["description"] = serde_json::json!("has one");
    let created = json_body(create_saved_graph(app.clone(), &cookie, body).await).await;
    let id = created["id"].as_str().unwrap();
    assert_eq!(created["description"], "has one");

    // Omitted: untouched.
    let res = update_saved_graph(
        app.clone(),
        &cookie,
        id,
        serde_json::json!({ "name": "still original" }),
    )
    .await;
    let body = json_body(res).await;
    assert_eq!(body["description"], "has one");

    // Explicit null: cleared.
    let res = update_saved_graph(
        app.clone(),
        &cookie,
        id,
        serde_json::json!({ "description": null }),
    )
    .await;
    let body = json_body(res).await;
    assert_eq!(body["description"], serde_json::Value::Null);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_rejects_blank_name_with_distinct_message(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-update-blank@b.test", "Alice").await;
    let created = json_body(create_saved_graph(app.clone(), &cookie, a_graph_body()).await).await;
    let id = created["id"].as_str().unwrap();

    let res = update_saved_graph(
        app.clone(),
        &cookie,
        id,
        serde_json::json!({ "name": "  " }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = json_body(res).await;
    assert_eq!(
        body["message"],
        "validation failed: Saved graph name cannot be empty"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_on_another_users_saved_graph_is_404_and_leaves_it_unchanged(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-cross-update@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-cross-update@b.test", "Bob").await;
    let created =
        json_body(create_saved_graph(app.clone(), &bob_cookie, a_graph_body()).await).await;
    let id = created["id"].as_str().unwrap();

    let res = update_saved_graph(
        app.clone(),
        &alice_cookie,
        id,
        serde_json::json!({ "name": "hijacked" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let refetched = json_body(get_saved_graph(app.clone(), &bob_cookie, id).await).await;
    assert_eq!(refetched["name"], "My Graph", "Bob's row must be unchanged");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_returns_message_and_404s_on_second_call(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-delete@b.test", "Alice").await;
    let created = json_body(create_saved_graph(app.clone(), &cookie, a_graph_body()).await).await;
    let id = created["id"].as_str().unwrap().to_string();

    let res = delete_saved_graph(app.clone(), &cookie, &id).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body, serde_json::json!({ "message": "Deleted" }));

    let res = delete_saved_graph(app.clone(), &cookie, &id).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_on_another_users_saved_graph_is_404_and_leaves_it_intact(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-cross-delete@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-cross-delete@b.test", "Bob").await;
    let created =
        json_body(create_saved_graph(app.clone(), &bob_cookie, a_graph_body()).await).await;
    let id = created["id"].as_str().unwrap().to_string();

    let res = delete_saved_graph(app.clone(), &alice_cookie, &id).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = get_saved_graph(app.clone(), &bob_cookie, &id).await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "Bob's saved graph must survive Alice's rejected delete"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unauthenticated_requests_are_401(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/saved-graphs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let res = app
        .oneshot(
            Request::post("/api/saved-graphs")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "name": "x",
                        "chunkIds": [],
                        "positions": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
