//! HTTP-level tests for the `workspaces` domain.
//!
//! `workspace_space`'s three write-side guards (workspace ownership on
//! attach, space ownership on attach, workspace ownership on the detach's
//! DELETE) are proven load-bearing at the repository level in
//! `fubbik-db/tests/workspace.rs` — each guard removed and its named test
//! watched to fail, then restored. This file covers the seven HTTP routes
//! themselves: response shapes (per `tests/fixtures/node-contract-2b/
//! workspaces-*.json`), status codes, and that the two ownership 404s at
//! `POST /api/workspaces/{id}/spaces` (`Workspace` vs `Space`) are
//! distinguishable end to end, matching `_mutating.md`.

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

async fn user_id_for_email(pool: &sqlx::PgPool, email: &str) -> String {
    sqlx::query_scalar!(r#"SELECT id FROM "user" WHERE email = $1"#, email)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Seeds a space directly via the repository — there is no reason to go
/// through HTTP for this, and the `spaces` domain has its own test suite.
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

async fn create_workspace(
    app: axum::Router,
    cookie: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::post("/api/workspaces")
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get_workspace(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(format!("/api/workspaces/{id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn update_workspace(
    app: axum::Router,
    cookie: &str,
    id: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::patch(format!("/api/workspaces/{id}"))
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn delete_workspace(app: axum::Router, cookie: &str, id: &str) -> axum::response::Response {
    app.oneshot(
        Request::delete(format!("/api/workspaces/{id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn add_space(
    app: axum::Router,
    cookie: &str,
    workspace_id: &str,
    space_id: &str,
) -> axum::response::Response {
    app.oneshot(
        Request::post(format!("/api/workspaces/{workspace_id}/spaces"))
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({ "spaceId": space_id }).to_string(),
            ))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn remove_space(
    app: axum::Router,
    cookie: &str,
    workspace_id: &str,
    space_id: &str,
) -> axum::response::Response {
    app.oneshot(
        Request::delete(format!("/api/workspaces/{workspace_id}/spaces/{space_id}"))
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_returns_201_and_the_row(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-create@b.test", "Alice").await;

    let res = create_workspace(
        app.clone(),
        &cookie,
        serde_json::json!({ "name": "fubbik-platform", "description": "everything" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = json_body(res).await;
    assert_eq!(body["name"], "fubbik-platform");
    assert_eq!(body["description"], "everything");
    assert!(body["id"].is_string());
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_rejects_whitespace_only_name(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-blank@b.test", "Alice").await;

    let res = create_workspace(app.clone(), &cookie, serde_json::json!({ "name": "   " })).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_is_user_scoped(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-list@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-list@b.test", "Bob").await;

    create_workspace(
        app.clone(),
        &alice_cookie,
        serde_json::json!({ "name": "alices" }),
    )
    .await;
    create_workspace(
        app.clone(),
        &bob_cookie,
        serde_json::json!({ "name": "bobs" }),
    )
    .await;

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/workspaces")
                .header("cookie", &alice_cookie)
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
        .map(|w| w["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["alices"]);

    // Bob's own workspace must also still be intact and visible from his
    // own view — a single-side check would pass even if Alice's list
    // handler had somehow mutated or dropped Bob's row.
    let res = app
        .oneshot(
            Request::get("/api/workspaces")
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
        .map(|w| w["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["bobs"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn detail_includes_flattened_fields_and_spaces_array(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-detail@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-detail@b.test").await;

    let created = json_body(
        create_workspace(
            app.clone(),
            &cookie,
            serde_json::json!({ "name": "platform" }),
        )
        .await,
    )
    .await;
    let workspace_id = created["id"].as_str().unwrap().to_string();
    let space_id = seed_space(&pool, &user_id, "fubbik").await;
    add_space(app.clone(), &cookie, &workspace_id, &space_id).await;

    let res = get_workspace(app.clone(), &cookie, &workspace_id).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(
        body["id"], workspace_id,
        "workspace fields must be at the top level, not nested"
    );
    assert_eq!(body["name"], "platform");
    let spaces = body["spaces"].as_array().unwrap();
    assert_eq!(spaces.len(), 1);
    assert_eq!(spaces[0]["id"], space_id);
    assert_eq!(spaces[0]["name"], "fubbik");
    assert!(
        spaces[0].get("description").is_none(),
        "spaces entries must be the bare {{id,name,kind}} summary, not a full Space row"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_on_another_users_workspace_is_404(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-cross-get@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-cross-get@b.test", "Bob").await;

    let created = json_body(
        create_workspace(
            app.clone(),
            &bob_cookie,
            serde_json::json!({ "name": "bobs" }),
        )
        .await,
    )
    .await;
    let workspace_id = created["id"].as_str().unwrap();

    let res = get_workspace(app.clone(), &alice_cookie, workspace_id).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_sets_fields_and_clears_description_on_explicit_null(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-update@b.test", "Alice").await;
    let created = json_body(
        create_workspace(
            app.clone(),
            &cookie,
            serde_json::json!({ "name": "platform", "description": "has one" }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap();

    let res = update_workspace(
        app.clone(),
        &cookie,
        id,
        serde_json::json!({ "name": "renamed" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["name"], "renamed");
    assert_eq!(
        body["description"], "has one",
        "omitted description must be left untouched"
    );

    let res = update_workspace(
        app.clone(),
        &cookie,
        id,
        serde_json::json!({ "description": null }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["description"], serde_json::Value::Null);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_rejects_blank_name_but_not_omitted_name(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-update-blank@b.test", "Alice").await;
    let created = json_body(
        create_workspace(
            app.clone(),
            &cookie,
            serde_json::json!({ "name": "platform" }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap();

    let res = update_workspace(
        app.clone(),
        &cookie,
        id,
        serde_json::json!({ "name": "  " }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let res = update_workspace(app.clone(), &cookie, id, serde_json::json!({})).await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "an empty patch is a no-op 200, not a 400"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_on_another_users_workspace_is_404_and_leaves_it_unchanged(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-cross-update@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-cross-update@b.test", "Bob").await;
    let created = json_body(
        create_workspace(
            app.clone(),
            &bob_cookie,
            serde_json::json!({ "name": "bobs" }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap();

    let res = update_workspace(
        app.clone(),
        &alice_cookie,
        id,
        serde_json::json!({ "name": "hijacked" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = get_workspace(app.clone(), &bob_cookie, id).await;
    let body = json_body(res).await;
    assert_eq!(body["name"], "bobs", "Bob's workspace must be unchanged");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_returns_message_and_404s_on_second_call(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-delete@b.test", "Alice").await;
    let created = json_body(
        create_workspace(
            app.clone(),
            &cookie,
            serde_json::json!({ "name": "platform" }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    let res = delete_workspace(app.clone(), &cookie, &id).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body, serde_json::json!({ "message": "Deleted" }));

    let res = delete_workspace(app.clone(), &cookie, &id).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_on_another_users_workspace_is_404_and_leaves_it_intact(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-cross-delete@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-cross-delete@b.test", "Bob").await;
    let created = json_body(
        create_workspace(
            app.clone(),
            &bob_cookie,
            serde_json::json!({ "name": "bobs" }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    let res = delete_workspace(app.clone(), &alice_cookie, &id).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = get_workspace(app.clone(), &bob_cookie, &id).await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "Bob's workspace must survive Alice's rejected delete"
    );
}

/// Node's `addSpaceToWorkspace` distinguishes the two ownership failures
/// with different `resource` values (`Workspace` vs `Space`) — this proves
/// both are reachable end to end, not just that /some/ 404 comes back.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn add_space_to_another_users_workspace_is_404(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-add-wsx@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-add-wsx@b.test", "Bob").await;
    let alice_id = user_id_for_email(&pool, "alice-add-wsx@b.test").await;

    let bobs_ws = json_body(
        create_workspace(
            app.clone(),
            &bob_cookie,
            serde_json::json!({ "name": "bobs-ws" }),
        )
        .await,
    )
    .await;
    let bobs_ws_id = bobs_ws["id"].as_str().unwrap();
    let alices_space = seed_space(&pool, &alice_id, "alices-space").await;

    let res = add_space(app.clone(), &alice_cookie, bobs_ws_id, &alices_space).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let detail = json_body(get_workspace(app.clone(), &bob_cookie, bobs_ws_id).await).await;
    assert_eq!(
        detail["spaces"].as_array().unwrap().len(),
        0,
        "Bob's workspace must not have gained Alice's space"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn add_another_users_space_to_my_workspace_is_404(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-add-spx@b.test", "Alice").await;
    signup(app.clone(), "bob-add-spx@b.test", "Bob").await;
    let bob_id = user_id_for_email(&pool, "bob-add-spx@b.test").await;

    let alices_ws = json_body(
        create_workspace(
            app.clone(),
            &alice_cookie,
            serde_json::json!({ "name": "alices-ws" }),
        )
        .await,
    )
    .await;
    let alices_ws_id = alices_ws["id"].as_str().unwrap();
    let bobs_space = seed_space(&pool, &bob_id, "bobs-space").await;

    let res = add_space(app.clone(), &alice_cookie, alices_ws_id, &bobs_space).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let detail = json_body(get_workspace(app.clone(), &alice_cookie, alices_ws_id).await).await;
    assert_eq!(detail["spaces"].as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn add_space_returns_201_and_the_link(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-add-ok@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-add-ok@b.test").await;
    let ws = json_body(
        create_workspace(
            app.clone(),
            &cookie,
            serde_json::json!({ "name": "platform" }),
        )
        .await,
    )
    .await;
    let ws_id = ws["id"].as_str().unwrap();
    let space_id = seed_space(&pool, &user_id, "fubbik").await;

    let res = add_space(app.clone(), &cookie, ws_id, &space_id).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = json_body(res).await;
    assert_eq!(body["workspaceId"], ws_id);
    assert_eq!(body["spaceId"], space_id);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn remove_space_removes_the_association(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-remove-ok@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-remove-ok@b.test").await;
    let ws = json_body(
        create_workspace(
            app.clone(),
            &cookie,
            serde_json::json!({ "name": "platform" }),
        )
        .await,
    )
    .await;
    let ws_id = ws["id"].as_str().unwrap().to_string();
    let space_id = seed_space(&pool, &user_id, "fubbik").await;
    add_space(app.clone(), &cookie, &ws_id, &space_id).await;

    let res = remove_space(app.clone(), &cookie, &ws_id, &space_id).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body, serde_json::json!({ "message": "Deleted" }));

    let detail = json_body(get_workspace(app.clone(), &cookie, &ws_id).await).await;
    assert_eq!(detail["spaces"].as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn remove_space_never_linked_is_404(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-remove-never@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-remove-never@b.test").await;
    let ws = json_body(
        create_workspace(
            app.clone(),
            &cookie,
            serde_json::json!({ "name": "platform" }),
        )
        .await,
    )
    .await;
    let ws_id = ws["id"].as_str().unwrap().to_string();
    let space_id = seed_space(&pool, &user_id, "fubbik").await;

    let res = remove_space(app.clone(), &cookie, &ws_id, &space_id).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// The end-to-end version of `fubbik-db/tests/workspace.rs
/// ::rejected_remove_does_not_wipe_the_victims_existing_association`: Bob
/// cannot remove a space from Alice's workspace by naming her ids, and her
/// association survives his rejected call.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn remove_space_from_another_users_workspace_is_404_and_leaves_it_intact(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-remove-cross@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-remove-cross@b.test", "Bob").await;
    let alice_id = user_id_for_email(&pool, "alice-remove-cross@b.test").await;

    let alices_ws = json_body(
        create_workspace(
            app.clone(),
            &alice_cookie,
            serde_json::json!({ "name": "alices-ws" }),
        )
        .await,
    )
    .await;
    let alices_ws_id = alices_ws["id"].as_str().unwrap().to_string();
    let alices_space = seed_space(&pool, &alice_id, "alices-space").await;
    add_space(app.clone(), &alice_cookie, &alices_ws_id, &alices_space).await;

    let res = remove_space(app.clone(), &bob_cookie, &alices_ws_id, &alices_space).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let detail = json_body(get_workspace(app.clone(), &alice_cookie, &alices_ws_id).await).await;
    assert_eq!(
        detail["spaces"].as_array().unwrap().len(),
        1,
        "Alice's association must survive Bob's rejected remove"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unauthenticated_requests_are_401(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let res = app
        .clone()
        .oneshot(Request::get("/api/workspaces").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let res = app
        .oneshot(
            Request::post("/api/workspaces")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"x"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
