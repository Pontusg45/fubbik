//! HTTP-level tests for the `plans` domain — the 10 core `routes.ts`
//! endpoints (list/detail/create/update/delete/duplicate/activity/links).
//!
//! Divergence #13's SQL guards are proven load-bearing at the repository
//! level in `fubbik-db/tests/plan.rs`; this file covers the HTTP routes
//! themselves: response shapes against `tests/fixtures/node-contract-2c/
//! plans-*.json`, status codes (every mutating endpoint is 200, never
//! 201), `status` validation living in the service layer (not a DTO enum),
//! the `description`/`spaceId` tri-state on `PATCH`, and that cross-user
//! access 404s **and leaves the victim's data unchanged** — a status-only
//! assertion would pass even if a rejected write had already mutated
//! something. Per the task brief: an API-level test here cannot prove a
//! removed repo-level SQL guard fails closed, because the service's own
//! `find_by_id` pre-check 404s first — that proof lives only in
//! `fubbik-db/tests/plan.rs`.

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

async fn get(app: axum::Router, cookie: &str, path: &str) -> axum::response::Response {
    app.oneshot(
        Request::get(path)
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn post(
    app: axum::Router,
    cookie: &str,
    path: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::post(path)
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn patch(
    app: axum::Router,
    cookie: &str,
    path: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::patch(path)
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn delete(app: axum::Router, cookie: &str, path: &str) -> axum::response::Response {
    app.oneshot(
        Request::delete(path)
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn create_plan(app: axum::Router, cookie: &str, title: &str) -> String {
    let body = json_body(
        post(
            app,
            cookie,
            "/api/plans",
            serde_json::json!({ "title": title }),
        )
        .await,
    )
    .await;
    body["id"].as_str().unwrap().to_string()
}

// ── Given tests (verbatim shape from the task brief) ───────────────────

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_plan_detail_is_enveloped(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-envelope@b.test", "Alice").await;
    let id = create_plan(app.clone(), &cookie, "My plan").await;

    let body = json_body(get(app.clone(), &cookie, &format!("/api/plans/{id}")).await).await;
    for key in ["plan", "requirements", "analyze", "tasks", "dependencies"] {
        assert!(
            body.get(key).is_some(),
            "detail envelope must carry `{key}`"
        );
    }
    assert_eq!(body["plan"]["title"], "My plan");
    for kind in ["chunk", "file", "risk", "assumption", "question"] {
        assert_eq!(
            body["analyze"][kind],
            serde_json::json!([]),
            "analyze.{kind} must always be present, even empty"
        );
    }
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_returns_200_not_201(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-create200@b.test", "Alice").await;

    let res = post(
        app.clone(),
        &cookie,
        "/api/plans",
        serde_json::json!({ "title": "x" }),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "Node returns 200 for every plans POST; it never sets 201"
    );
    let body = json_body(res).await;
    assert_eq!(body["title"], "x");
    assert_eq!(body["status"], "draft");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn patch_rejects_an_unknown_status(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-badstatus@b.test", "Alice").await;
    let id = create_plan(app.clone(), &cookie, "x").await;

    let res = patch(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}"),
        serde_json::json!({ "status": "bogus" }),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "status is validated in the service layer, not by a DB enum"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn detail_on_another_users_plan_is_404_and_leaves_it_intact(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-cross-detail@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-cross-detail@b.test", "Bob").await;
    let alice_id = user_id_for_email(&pool, "alice-cross-detail@b.test").await;
    let id = create_plan(app.clone(), &alice_cookie, "Alice's").await;

    let res = get(app.clone(), &bob_cookie, &format!("/api/plans/{id}")).await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "divergence #13: Node returns 200 here"
    );

    let row = fubbik_db::repo::plan::find_by_id(&pool, &alice_id, &id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.title, "Alice's", "the victim's plan must be untouched");
}

// ── list ─────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_is_bare_array_with_rollup_fields(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-rollups@b.test", "Alice").await;
    let id = create_plan(app.clone(), &cookie, "with rollups").await;

    let res = get(app.clone(), &cookie, "/api/plans").await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let arr = body
        .as_array()
        .expect("GET /api/plans must be a bare array");
    let row = arr.iter().find(|p| p["id"] == id).unwrap();
    assert_eq!(row["taskTotal"], 0);
    assert_eq!(row["taskDone"], 0);
    assert_eq!(row["nextAction"], serde_json::Value::Null);
    assert!(row["lastActivityAt"].is_string());
    assert!(row.get("codebaseName").is_some());
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_is_user_scoped(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-list@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-list@b.test", "Bob").await;
    create_plan(app.clone(), &alice_cookie, "alices").await;
    create_plan(app.clone(), &bob_cookie, "bobs").await;

    let body = json_body(get(app.clone(), &alice_cookie, "/api/plans").await).await;
    let titles: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["alices"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_rejects_an_unknown_status_filter(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-list-badstatus@b.test", "Alice").await;

    let res = get(app.clone(), &cookie, "/api/plans?status=bogus").await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

// ── create ───────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_rejects_whitespace_only_title(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-blanktitle@b.test", "Alice").await;

    let res = post(
        app.clone(),
        &cookie,
        "/api/plans",
        serde_json::json!({ "title": "   " }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_with_tasks_and_requirement_populates_detail(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-createtasks@b.test", "Alice").await;

    let req_owner = fubbik_db::repo::user::create(&pool, "req-owner@b.test", "R", None)
        .await
        .unwrap()
        .id;
    // Seed a requirement row directly — the requirements domain isn't part
    // of this task's scope, and `plan_requirement` carries no FK-visible
    // ownership check of its own (see `db::add_requirement`'s doc
    // comment), so any existing requirement id is enough to prove the link.
    let requirement_id = fubbik_db::new_id();
    sqlx::query!(
        r#"INSERT INTO requirement (id, title, steps, user_id) VALUES ($1, 'req', '[]'::jsonb, $2)"#,
        requirement_id,
        req_owner
    )
    .execute(&pool)
    .await
    .unwrap();

    let body = json_body(
        post(
            app.clone(),
            &cookie,
            "/api/plans",
            serde_json::json!({
                "title": "with children",
                "requirementIds": [requirement_id],
                "tasks": [{ "title": "task one", "acceptanceCriteria": ["do the thing"] }]
            }),
        )
        .await,
    )
    .await;
    let id = body["id"].as_str().unwrap().to_string();

    let detail = json_body(get(app.clone(), &cookie, &format!("/api/plans/{id}")).await).await;
    assert_eq!(detail["requirements"].as_array().unwrap().len(), 1);
    assert_eq!(detail["requirements"][0]["requirementId"], requirement_id);

    let tasks = detail["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["title"], "task one");
    assert_eq!(tasks[0]["status"], "pending");
    assert_eq!(
        tasks[0]["acceptanceCriteria"],
        serde_json::json!([{ "text": "do the thing", "done": false }])
    );
}

// ── update / tri-state ──────────────────────────────────────────────

/// The tri-state test the brief asks for: an explicit `null` clears
/// `description`, while a patch that omits it entirely leaves it
/// untouched.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_description_tri_state(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-tristate@b.test", "Alice").await;
    let created = json_body(
        post(
            app.clone(),
            &cookie,
            "/api/plans",
            serde_json::json!({ "title": "x", "description": "has one" }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    // Omitted `description` — untouched.
    let res = patch(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}"),
        serde_json::json!({ "title": "renamed" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["title"], "renamed");
    assert_eq!(
        body["description"], "has one",
        "omitted description must be left untouched"
    );

    // Explicit `null` — cleared.
    let res = patch(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}"),
        serde_json::json!({ "description": null }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["description"], serde_json::Value::Null);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_space_id_tri_state(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-space-tristate@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-space-tristate@b.test").await;
    let space_id = fubbik_db::repo::space::create(
        &pool,
        &user_id,
        fubbik_db::repo::space::NewSpace {
            name: "s".into(),
            kind: "wiki".into(),
            description: None,
        },
        None,
    )
    .await
    .unwrap()
    .id;

    let created = json_body(
        post(
            app.clone(),
            &cookie,
            "/api/plans",
            serde_json::json!({ "title": "x", "spaceId": space_id }),
        )
        .await,
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["spaceId"], space_id);

    // Omitted spaceId — untouched.
    let res = patch(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}"),
        serde_json::json!({ "title": "still there" }),
    )
    .await;
    let body = json_body(res).await;
    assert_eq!(body["spaceId"], space_id);

    // Explicit null — cleared.
    let res = patch(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}"),
        serde_json::json!({ "spaceId": null }),
    )
    .await;
    let body = json_body(res).await;
    assert_eq!(body["spaceId"], serde_json::Value::Null);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_completed_status_sets_and_clears_completed_at(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-completedat@b.test", "Alice").await;
    let id = create_plan(app.clone(), &cookie, "x").await;

    let res = patch(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}"),
        serde_json::json!({ "status": "completed" }),
    )
    .await;
    let body = json_body(res).await;
    assert_eq!(body["status"], "completed");
    assert!(
        body["completedAt"].is_string(),
        "entering completed must stamp completedAt"
    );

    let res = patch(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}"),
        serde_json::json!({ "status": "in_progress" }),
    )
    .await;
    let body = json_body(res).await;
    assert_eq!(body["status"], "in_progress");
    assert_eq!(
        body["completedAt"],
        serde_json::Value::Null,
        "leaving completed must clear completedAt"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_on_another_users_plan_is_404_and_leaves_it_intact(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-cross-update@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-cross-update@b.test", "Bob").await;
    let alice_id = user_id_for_email(&pool, "alice-cross-update@b.test").await;
    let id = create_plan(app.clone(), &alice_cookie, "bobs target").await;

    let res = patch(
        app.clone(),
        &bob_cookie,
        &format!("/api/plans/{id}"),
        serde_json::json!({ "title": "hijacked" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let row = fubbik_db::repo::plan::find_by_id(&pool, &alice_id, &id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.title, "bobs target", "Alice's plan must be unchanged");
}

// ── delete ───────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_returns_ok_true_and_404s_on_second_call(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-delete@b.test", "Alice").await;
    let id = create_plan(app.clone(), &cookie, "x").await;

    let res = delete(app.clone(), &cookie, &format!("/api/plans/{id}")).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body, serde_json::json!({ "ok": true }));

    let res = delete(app.clone(), &cookie, &format!("/api/plans/{id}")).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_on_another_users_plan_is_404_and_leaves_it_intact(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-cross-delete@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-cross-delete@b.test", "Bob").await;
    let alice_id = user_id_for_email(&pool, "alice-cross-delete@b.test").await;
    let id = create_plan(app.clone(), &alice_cookie, "alices").await;

    let res = delete(app.clone(), &bob_cookie, &format!("/api/plans/{id}")).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let row = fubbik_db::repo::plan::find_by_id(&pool, &alice_id, &id)
        .await
        .unwrap();
    assert!(
        row.is_some(),
        "Alice's plan must survive Bob's rejected delete"
    );
}

// ── duplicate ────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn duplicate_returns_200_and_only_the_new_plan(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-dup@b.test", "Alice").await;
    let id = create_plan(app.clone(), &cookie, "source").await;

    let res = post(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}/duplicate"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["title"], "source (copy)");
    assert_eq!(body["status"], "draft");
    assert_ne!(body["id"], id);
    assert!(
        body.get("tasks").is_none(),
        "duplicate's response is the new Plan row only, not its children"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn duplicate_on_another_users_plan_is_404(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-cross-dup@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-cross-dup@b.test", "Bob").await;
    let id = create_plan(app.clone(), &alice_cookie, "alices").await;

    let res = post(
        app.clone(),
        &bob_cookie,
        &format!("/api/plans/{id}/duplicate"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

// ── activity ─────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn activity_is_a_bare_array(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-activity@b.test", "Alice").await;
    let id = create_plan(app.clone(), &cookie, "x").await;

    let res = get(app.clone(), &cookie, &format!("/api/plans/{id}/activity")).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert!(body.is_array(), "activity must be a bare array");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn activity_on_another_users_plan_is_404(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-cross-activity@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-cross-activity@b.test", "Bob").await;
    let id = create_plan(app.clone(), &alice_cookie, "alices").await;

    let res = get(
        app.clone(),
        &bob_cookie,
        &format!("/api/plans/{id}/activity"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

// ── links ────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn add_link_defaults_system_and_label(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-link@b.test", "Alice").await;
    let id = create_plan(app.clone(), &cookie, "x").await;

    let res = post(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}/links"),
        serde_json::json!({ "url": "https://example.com" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["system"], "url");
    assert_eq!(body["label"], serde_json::Value::Null);
    assert_eq!(body["url"], "https://example.com");
    assert_eq!(body["planId"], id);

    let res = get(app.clone(), &cookie, &format!("/api/plans/{id}/links")).await;
    let body = json_body(res).await;
    assert_eq!(body.as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn add_link_honours_explicit_system_and_label(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-link-explicit@b.test", "Alice").await;
    let id = create_plan(app.clone(), &cookie, "x").await;

    let body = json_body(
        post(
            app.clone(),
            &cookie,
            &format!("/api/plans/{id}/links"),
            serde_json::json!({ "url": "https://example.com", "system": "github", "label": "PR" }),
        )
        .await,
    )
    .await;
    assert_eq!(body["system"], "github");
    assert_eq!(body["label"], "PR");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn remove_link_returns_ok_true_and_404_on_missing(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-unlink@b.test", "Alice").await;
    let id = create_plan(app.clone(), &cookie, "x").await;
    let link = json_body(
        post(
            app.clone(),
            &cookie,
            &format!("/api/plans/{id}/links"),
            serde_json::json!({ "url": "https://example.com" }),
        )
        .await,
    )
    .await;
    let link_id = link["id"].as_str().unwrap().to_string();

    let res = delete(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}/links/{link_id}"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await, serde_json::json!({ "ok": true }));

    let res = delete(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}/links/{link_id}"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn links_on_another_users_plan_is_404_and_leaves_it_intact(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-cross-link@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-cross-link@b.test", "Bob").await;
    let id = create_plan(app.clone(), &alice_cookie, "alices").await;

    let res = get(app.clone(), &bob_cookie, &format!("/api/plans/{id}/links")).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = post(
        app.clone(),
        &bob_cookie,
        &format!("/api/plans/{id}/links"),
        serde_json::json!({ "url": "https://evil.example.com" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let links = json_body(
        get(
            app.clone(),
            &alice_cookie,
            &format!("/api/plans/{id}/links"),
        )
        .await,
    )
    .await;
    assert_eq!(
        links.as_array().unwrap().len(),
        0,
        "Bob's rejected add must not have landed a link on Alice's plan"
    );
}

// ── requirement links ────────────────────────────────────────────────

async fn seed_requirement(pool: &sqlx::PgPool, user_id: &str) -> String {
    let id = fubbik_db::new_id();
    sqlx::query!(
        r#"INSERT INTO requirement (id, title, steps, user_id) VALUES ($1, 'req', '[]'::jsonb, $2)"#,
        id,
        user_id
    )
    .execute(pool)
    .await
    .unwrap();
    id
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn add_requirement_returns_bare_row(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-addreq@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-addreq@b.test").await;
    let id = create_plan(app.clone(), &cookie, "x").await;
    let requirement_id = seed_requirement(&pool, &user_id).await;

    let res = post(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}/requirements"),
        serde_json::json!({ "requirementId": requirement_id }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["requirementId"], requirement_id);
    assert_eq!(body["planId"], id);
    assert_eq!(body["order"], 0);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn remove_requirement_returns_ok_true_and_404_on_missing(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-removereq@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-removereq@b.test").await;
    let id = create_plan(app.clone(), &cookie, "x").await;
    let requirement_id = seed_requirement(&pool, &user_id).await;
    post(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}/requirements"),
        serde_json::json!({ "requirementId": requirement_id }),
    )
    .await;

    let res = delete(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}/requirements/{requirement_id}"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await, serde_json::json!({ "ok": true }));

    let res = delete(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}/requirements/{requirement_id}"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn reorder_requirements_leaves_unmentioned_rows_and_returns_ok_true(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-reorderreq@b.test", "Alice").await;
    let user_id = user_id_for_email(&pool, "alice-reorderreq@b.test").await;
    let id = create_plan(app.clone(), &cookie, "x").await;
    let ra = seed_requirement(&pool, &user_id).await;
    let rb = seed_requirement(&pool, &user_id).await;
    for rid in [&ra, &rb] {
        post(
            app.clone(),
            &cookie,
            &format!("/api/plans/{id}/requirements"),
            serde_json::json!({ "requirementId": rid }),
        )
        .await;
    }

    let res = post(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}/requirements/reorder"),
        serde_json::json!({ "requirementIds": [rb, ra] }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await, serde_json::json!({ "ok": true }));

    let detail = json_body(get(app.clone(), &cookie, &format!("/api/plans/{id}")).await).await;
    let reqs = detail["requirements"].as_array().unwrap();
    assert_eq!(reqs[0]["requirementId"], rb);
    assert_eq!(reqs[1]["requirementId"], ra);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn requirements_on_another_users_plan_is_404_and_leaves_it_intact(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-cross-req@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-cross-req@b.test", "Bob").await;
    let bob_id = user_id_for_email(&pool, "bob-cross-req@b.test").await;
    let id = create_plan(app.clone(), &alice_cookie, "alices").await;
    let requirement_id = seed_requirement(&pool, &bob_id).await;

    let res = post(
        app.clone(),
        &bob_cookie,
        &format!("/api/plans/{id}/requirements"),
        serde_json::json!({ "requirementId": requirement_id }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // No dedicated GET /requirements route in this domain — verify via the
    // plan detail envelope instead.
    let detail =
        json_body(get(app.clone(), &alice_cookie, &format!("/api/plans/{id}")).await).await;
    assert_eq!(
        detail["requirements"].as_array().unwrap().len(),
        0,
        "Bob's rejected add must not have landed a requirement link on Alice's plan"
    );
}

// ── analyze items ────────────────────────────────────────────────────

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn analyze_get_is_grouped_by_kind(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-analyzeget@b.test", "Alice").await;
    let id = create_plan(app.clone(), &cookie, "x").await;

    let res = get(app.clone(), &cookie, &format!("/api/plans/{id}/analyze")).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    for kind in ["chunk", "file", "risk", "assumption", "question"] {
        assert_eq!(body[kind], serde_json::json!([]));
    }
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_analyze_item_returns_bare_object(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-createanalyze@b.test", "Alice").await;
    let id = create_plan(app.clone(), &cookie, "x").await;

    let res = post(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}/analyze"),
        serde_json::json!({ "kind": "risk", "text": "a risk", "metadata": {"severity": "medium"} }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["kind"], "risk");
    assert_eq!(body["text"], "a risk");
    assert_eq!(body["order"], 0);
    assert_eq!(body["metadata"], serde_json::json!({"severity": "medium"}));
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_analyze_item_rejects_an_unknown_kind(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-badkind@b.test", "Alice").await;
    let id = create_plan(app.clone(), &cookie, "x").await;

    let res = post(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}/analyze"),
        serde_json::json!({ "kind": "bogus" }),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "kind is validated in the service layer, not by a DB enum"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_analyze_item_returns_the_updated_row(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-updateanalyze@b.test", "Alice").await;
    let id = create_plan(app.clone(), &cookie, "x").await;
    let created = json_body(
        post(
            app.clone(),
            &cookie,
            &format!("/api/plans/{id}/analyze"),
            serde_json::json!({ "kind": "assumption", "text": "initial" }),
        )
        .await,
    )
    .await;
    let item_id = created["id"].as_str().unwrap().to_string();

    let res = patch(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}/analyze/{item_id}"),
        serde_json::json!({ "text": "changed" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["text"], "changed");
    assert_eq!(
        body["kind"], "assumption",
        "kind must never change via PATCH"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn delete_analyze_item_returns_ok_true_and_404_on_missing(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-deleteanalyze@b.test", "Alice").await;
    let id = create_plan(app.clone(), &cookie, "x").await;
    let created = json_body(
        post(
            app.clone(),
            &cookie,
            &format!("/api/plans/{id}/analyze"),
            serde_json::json!({ "kind": "file", "filePath": "a.rs" }),
        )
        .await,
    )
    .await;
    let item_id = created["id"].as_str().unwrap().to_string();

    let res = delete(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}/analyze/{item_id}"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await, serde_json::json!({ "ok": true }));

    let res = delete(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}/analyze/{item_id}"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn reorder_analyze_items_leaves_unmentioned_rows_and_rejects_unknown_kind(
    pool: sqlx::PgPool,
) {
    let app = fubbik_api::router(state(pool.clone()));
    let cookie = signup(app.clone(), "alice-reorderanalyze@b.test", "Alice").await;
    let id = create_plan(app.clone(), &cookie, "x").await;
    let mut ids = vec![];
    for text in ["a", "b", "c"] {
        let created = json_body(
            post(
                app.clone(),
                &cookie,
                &format!("/api/plans/{id}/analyze"),
                serde_json::json!({ "kind": "risk", "text": text }),
            )
            .await,
        )
        .await;
        ids.push(created["id"].as_str().unwrap().to_string());
    }

    let res = post(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}/analyze/reorder"),
        serde_json::json!({ "kind": "bogus", "itemIds": [] }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let res = post(
        app.clone(),
        &cookie,
        &format!("/api/plans/{id}/analyze/reorder"),
        serde_json::json!({ "kind": "risk", "itemIds": [ids[1].clone(), ids[0].clone()] }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await, serde_json::json!({ "ok": true }));

    let analyze =
        json_body(get(app.clone(), &cookie, &format!("/api/plans/{id}/analyze")).await).await;
    let risks = analyze["risk"].as_array().unwrap();
    let c = risks.iter().find(|r| r["id"] == ids[2]).unwrap();
    assert_eq!(
        c["order"], 2,
        "the unmentioned third item must keep its original order"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn analyze_on_another_users_plan_is_404_and_leaves_it_intact(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone()));
    let alice_cookie = signup(app.clone(), "alice-cross-analyze@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-cross-analyze@b.test", "Bob").await;
    let id = create_plan(app.clone(), &alice_cookie, "alices").await;

    let res = get(
        app.clone(),
        &bob_cookie,
        &format!("/api/plans/{id}/analyze"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = post(
        app.clone(),
        &bob_cookie,
        &format!("/api/plans/{id}/analyze"),
        serde_json::json!({ "kind": "risk", "text": "evil" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let analyze = json_body(
        get(
            app.clone(),
            &alice_cookie,
            &format!("/api/plans/{id}/analyze"),
        )
        .await,
    )
    .await;
    assert_eq!(
        analyze["risk"].as_array().unwrap().len(),
        0,
        "Bob's rejected create must not have landed an item on Alice's plan"
    );
}

// ── auth ─────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unauthenticated_requests_are_401(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));

    let res = app
        .clone()
        .oneshot(Request::get("/api/plans").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let res = app
        .oneshot(
            Request::post("/api/plans")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"x"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
