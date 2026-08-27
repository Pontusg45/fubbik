//! HTTP-level tests for the matrices domain — 25 endpoints.
//!
//! The SQL ownership guards are proven at the repository layer
//! (`fubbik-db/tests/behavior_matrix.rs`), where a removed guard is observed
//! directly rather than through a status code. This file proves the wiring:
//! status codes, response shapes, the rule-history snapshot, and the computed
//! view's status derivation end to end.
//!
//! Cross-user cases are still worth having here, because the thing they
//! protect against is a *route* forgetting to pass the matrix id or the
//! session — which is exactly what Node did on all nine cell endpoints, and
//! which no repository test can see.

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

async fn send(
    app: axum::Router,
    cookie: &str,
    method: &str,
    path: &str,
    body: serde_json::Value,
) -> axum::response::Response {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header("cookie", cookie);
    if !body.is_null() {
        req = req.header("content-type", "application/json");
    }
    let body = if body.is_null() {
        Body::empty()
    } else {
        Body::from(body.to_string())
    };
    app.oneshot(req.body(body).unwrap()).await.unwrap()
}

async fn get(app: axum::Router, cookie: &str, path: &str) -> axum::response::Response {
    send(app, cookie, "GET", path, serde_json::Value::Null).await
}

async fn a_matrix(app: axum::Router, cookie: &str, name: &str) -> String {
    let res = send(
        app,
        cookie,
        "POST",
        "/api/matrices",
        serde_json::json!({ "name": name, "layer": "invariant" }),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::CREATED,
        "Node answers 201 on create"
    );
    json_body(res).await["id"].as_str().unwrap().to_string()
}

/// Builds a matrix with one rule, one dimension and their cell.
async fn a_cell(app: axum::Router, cookie: &str, name: &str) -> (String, String) {
    let m = a_matrix(app.clone(), cookie, name).await;
    let rule = json_body(
        send(
            app.clone(),
            cookie,
            "POST",
            &format!("/api/matrices/{m}/rules"),
            serde_json::json!({ "title": "R" }),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let dim = json_body(
        send(
            app.clone(),
            cookie,
            "POST",
            &format!("/api/matrices/{m}/dimensions"),
            serde_json::json!({ "name": "D" }),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let toggled = json_body(
        send(
            app,
            cookie,
            "PUT",
            &format!("/api/matrices/{m}/cells"),
            serde_json::json!({ "ruleId": rule, "dimensionId": dim }),
        )
        .await,
    )
    .await;
    assert_eq!(toggled["action"], "created");
    (m, toggled["cell"]["id"].as_str().unwrap().to_string())
}

// ---------------------------------------------------------------------------
// Matrix
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn matrix_crud_and_detail_shape(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let id = a_matrix(app.clone(), &cookie, "Invariants").await;

    send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/matrices/{id}/dimensions"),
        serde_json::json!({ "name": "happy path" }),
    )
    .await;
    send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/matrices/{id}/rules"),
        serde_json::json!({ "title": "No orphans" }),
    )
    .await;

    // `GET /matrices/{id}` is `{matrix, dimensions, rules}`, not a flat row.
    let detail = json_body(get(app.clone(), &cookie, &format!("/api/matrices/{id}")).await).await;
    assert_eq!(detail["matrix"]["name"], "Invariants");
    assert_eq!(detail["dimensions"].as_array().unwrap().len(), 1);
    assert_eq!(detail["rules"][0]["title"], "No orphans");

    // The list is a bare array.
    let list = json_body(get(app.clone(), &cookie, "/api/matrices").await).await;
    assert!(list.is_array());
    assert_eq!(list.as_array().unwrap().len(), 1);

    let res = send(
        app.clone(),
        &cookie,
        "DELETE",
        &format!("/api/matrices/{id}"),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["message"], "Deleted");
    assert_eq!(
        get(app, &cookie, &format!("/api/matrices/{id}"))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_rejects_an_unknown_layer(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let res = send(
        app,
        &cookie,
        "POST",
        "/api/matrices",
        serde_json::json!({ "name": "M", "layer": "vibes" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

/// Every matrix route 404s for another user. Enumerated rather than sampled:
/// each is a separate handler, and the thing being checked is that each one
/// passes the session at all.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn every_matrix_route_is_404_for_another_user(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "a@b.test", "Alice").await;
    let bob = signup(app.clone(), "c@d.test", "Bob").await;
    let (m, cell) = a_cell(app.clone(), &alice, "Alice's").await;
    let rule =
        json_body(get(app.clone(), &alice, &format!("/api/matrices/{m}")).await).await["rules"][0]
            ["id"]
            .as_str()
            .unwrap()
            .to_string();

    for (method, path, body) in [
        ("GET", format!("/api/matrices/{m}"), serde_json::Value::Null),
        (
            "GET",
            format!("/api/matrices/{m}/view"),
            serde_json::Value::Null,
        ),
        (
            "PATCH",
            format!("/api/matrices/{m}"),
            serde_json::json!({ "name": "X" }),
        ),
        (
            "DELETE",
            format!("/api/matrices/{m}"),
            serde_json::Value::Null,
        ),
        (
            "POST",
            format!("/api/matrices/{m}/dimensions"),
            serde_json::json!({ "name": "X" }),
        ),
        (
            "POST",
            format!("/api/matrices/{m}/rules"),
            serde_json::json!({ "title": "X" }),
        ),
        (
            "GET",
            format!("/api/matrices/{m}/rules/{rule}/history"),
            serde_json::Value::Null,
        ),
        (
            "PATCH",
            format!("/api/matrices/{m}/rules/{rule}"),
            serde_json::json!({ "title": "X" }),
        ),
        (
            "DELETE",
            format!("/api/matrices/{m}/rules/{rule}"),
            serde_json::Value::Null,
        ),
        (
            "POST",
            format!("/api/matrices/{m}/rules/reorder"),
            serde_json::json!({ "ruleIds": [] }),
        ),
        (
            "POST",
            format!("/api/matrices/{m}/dimensions/reorder"),
            serde_json::json!({ "dimensionIds": [] }),
        ),
        (
            "GET",
            format!("/api/matrices/{m}/cells/{cell}/requirements"),
            serde_json::Value::Null,
        ),
        (
            "GET",
            format!("/api/matrices/{m}/cells/{cell}/code"),
            serde_json::Value::Null,
        ),
        (
            "GET",
            format!("/api/matrices/{m}/cells/{cell}/test-results"),
            serde_json::Value::Null,
        ),
        (
            "POST",
            format!("/api/matrices/{m}/cells/{cell}/code"),
            serde_json::json!({ "kind": "file", "ref": "x.rs" }),
        ),
        (
            "POST",
            format!("/api/matrices/{m}/cells/{cell}/test-results"),
            serde_json::json!({ "testRef": "t", "status": "pass" }),
        ),
        (
            "DELETE",
            format!("/api/matrices/{m}/cells/{cell}/code/whatever"),
            serde_json::Value::Null,
        ),
        (
            "DELETE",
            format!("/api/matrices/{m}/cells/{cell}/requirements/whatever"),
            serde_json::Value::Null,
        ),
    ] {
        let res = send(app.clone(), &bob, method, &path, body).await;
        assert_eq!(
            res.status(),
            StatusCode::NOT_FOUND,
            "{method} {path} must 404 for a stranger, got {}",
            res.status()
        );
    }

    // Alice's matrix is intact after every one of Bob's attempts.
    let detail = json_body(get(app, &alice, &format!("/api/matrices/{m}")).await).await;
    assert_eq!(detail["matrix"]["name"], "Alice's");
    assert_eq!(detail["rules"].as_array().unwrap().len(), 1);
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

/// Editing a rule snapshots its pre-edit state, and the snapshot carries all
/// seven fields — not just the ones that changed.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn updating_a_rule_snapshots_the_previous_version(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let m = a_matrix(app.clone(), &cookie, "M").await;

    let rule = json_body(
        send(
            app.clone(),
            &cookie,
            "POST",
            &format!("/api/matrices/{m}/rules"),
            serde_json::json!({
                "title": "Original", "description": "d", "category": "c",
                "rationale": "why", "alternatives": "alt",
                "consequences": "co", "counterexample": "cx"
            }),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let res = send(
        app.clone(),
        &cookie,
        "PATCH",
        &format!("/api/matrices/{m}/rules/{rule}"),
        serde_json::json!({ "title": "Revised", "rationale": null }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let updated = json_body(res).await;
    assert_eq!(updated["title"], "Revised");
    assert_eq!(
        updated["rationale"],
        serde_json::Value::Null,
        "an explicit null clears the field"
    );
    assert_eq!(
        updated["description"], "d",
        "an omitted field is left alone, not cleared"
    );

    let history = json_body(
        get(
            app,
            &cookie,
            &format!("/api/matrices/{m}/rules/{rule}/history"),
        )
        .await,
    )
    .await;
    assert_eq!(history.as_array().unwrap().len(), 1);
    let snap = &history[0]["snapshot"];
    assert_eq!(
        snap["title"], "Original",
        "the snapshot is the PRE-edit state"
    );
    assert_eq!(snap["rationale"], "why");
    assert_eq!(snap["counterexample"], "cx");
}

/// A rejected PATCH must leave no history behind — validation runs before the
/// snapshot.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn a_rejected_rule_patch_writes_no_history(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let m = a_matrix(app.clone(), &cookie, "M").await;
    let rule = json_body(
        send(
            app.clone(),
            &cookie,
            "POST",
            &format!("/api/matrices/{m}/rules"),
            serde_json::json!({ "title": "T" }),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let res = send(
        app.clone(),
        &cookie,
        "PATCH",
        &format!("/api/matrices/{m}/rules/{rule}"),
        serde_json::json!({ "title": "x".repeat(201) }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let history = json_body(
        get(
            app,
            &cookie,
            &format!("/api/matrices/{m}/rules/{rule}/history"),
        )
        .await,
    )
    .await;
    assert_eq!(
        history.as_array().unwrap().len(),
        0,
        "a rejected edit must not appear in history"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn reordering_rules_renumbers_them(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let m = a_matrix(app.clone(), &cookie, "M").await;

    let mut ids = Vec::new();
    for title in ["a", "b", "c"] {
        ids.push(
            json_body(
                send(
                    app.clone(),
                    &cookie,
                    "POST",
                    &format!("/api/matrices/{m}/rules"),
                    serde_json::json!({ "title": title }),
                )
                .await,
            )
            .await["id"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }

    let res = send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/matrices/{m}/rules/reorder"),
        serde_json::json!({ "ruleIds": [ids[2], ids[0], ids[1]] }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["message"], "Reordered");

    let titles: Vec<String> = json_body(get(app, &cookie, &format!("/api/matrices/{m}")).await)
        .await["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["title"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(titles, ["c", "a", "b"]);
}

// ---------------------------------------------------------------------------
// Cells
// ---------------------------------------------------------------------------

/// Toggling twice creates then deletes; the response says which happened.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn toggling_a_cell_creates_then_deletes(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let (m, cell) = a_cell(app.clone(), &cookie, "M").await;

    let detail = json_body(get(app.clone(), &cookie, &format!("/api/matrices/{m}")).await).await;
    let rule = detail["rules"][0]["id"].as_str().unwrap().to_string();
    let dim = detail["dimensions"][0]["id"].as_str().unwrap().to_string();

    let res = send(
        app.clone(),
        &cookie,
        "PUT",
        &format!("/api/matrices/{m}/cells"),
        serde_json::json!({ "ruleId": rule, "dimensionId": dim }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let toggled = json_body(res).await;
    assert_eq!(toggled["action"], "deleted");
    assert_eq!(toggled["cell"]["id"], cell.as_str());
}

/// A cell with requirements linked refuses to be toggled off, and the message
/// names the count.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn a_cell_with_requirements_cannot_be_toggled_off(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let (m, cell) = a_cell(app.clone(), &cookie, "M").await;

    let req = json_body(
        send(
            app.clone(),
            &cookie,
            "POST",
            "/api/requirements",
            serde_json::json!({
                "title": "Users can log in",
                "steps": [
                    { "keyword": "given", "text": "a user" },
                    { "keyword": "when", "text": "they log in" },
                    { "keyword": "then", "text": "they see the dashboard" }
                ]
            }),
        )
        .await,
    )
    .await["requirement"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let res = send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/matrices/{m}/cells/{cell}/requirements"),
        serde_json::json!({ "requirementId": req }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);

    let detail = json_body(get(app.clone(), &cookie, &format!("/api/matrices/{m}")).await).await;
    let res = send(
        app.clone(),
        &cookie,
        "PUT",
        &format!("/api/matrices/{m}/cells"),
        serde_json::json!({
            "ruleId": detail["rules"][0]["id"],
            "dimensionId": detail["dimensions"][0]["id"]
        }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let msg = json_body(res).await["message"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        msg.contains('1'),
        "the message must name how many links block the toggle, got: {msg}"
    );

    // Unlink, then the toggle succeeds — otherwise the assertion above could
    // pass because toggling is broken generally.
    let res = send(
        app.clone(),
        &cookie,
        "DELETE",
        &format!("/api/matrices/{m}/cells/{cell}/requirements/{req}"),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res).await["message"], "Unlinked");

    let res = send(
        app,
        &cookie,
        "PUT",
        &format!("/api/matrices/{m}/cells"),
        serde_json::json!({
            "ruleId": detail["rules"][0]["id"],
            "dimensionId": detail["dimensions"][0]["id"]
        }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn code_links_round_trip_and_reject_a_bad_kind(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let (m, cell) = a_cell(app.clone(), &cookie, "M").await;
    let path = format!("/api/matrices/{m}/cells/{cell}/code");

    let res = send(
        app.clone(),
        &cookie,
        "POST",
        &path,
        serde_json::json!({ "kind": "vandalises", "ref": "src/x.rs" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let res = send(
        app.clone(),
        &cookie,
        "POST",
        &path,
        serde_json::json!({ "kind": "symbol", "ref": "  src/x.rs::run  " }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    assert_eq!(
        json_body(res).await["ref"],
        "src/x.rs::run",
        "the ref is trimmed"
    );

    let links = json_body(get(app.clone(), &cookie, &path).await).await;
    assert_eq!(links.as_array().unwrap().len(), 1);

    // The reverse lookup finds it.
    let found = json_body(
        get(
            app,
            &cookie,
            "/api/matrices/behaviors-for-file?path=src/x.rs",
        )
        .await,
    )
    .await;
    assert_eq!(found.as_array().unwrap().len(), 1);
    assert_eq!(found[0]["ruleTitle"], "R");
    assert_eq!(found[0]["dimensionName"], "D");
}

/// End-to-end status derivation: the view's precedence, driven through HTTP
/// rather than the unit test's synthetic rows.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn the_view_derives_cell_status_from_evidence(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let (m, cell) = a_cell(app.clone(), &cookie, "M").await;
    let view_path = format!("/api/matrices/{m}/view");

    let detail = json_body(get(app.clone(), &cookie, &format!("/api/matrices/{m}")).await).await;
    let key = format!(
        "{}:{}",
        detail["rules"][0]["id"].as_str().unwrap(),
        detail["dimensions"][0]["id"].as_str().unwrap()
    );

    // No evidence at all.
    let view = json_body(get(app.clone(), &cookie, &view_path).await).await;
    assert_eq!(view["cells"][&key]["status"], "unspecified");
    assert_eq!(view["summary"]["unspecified"], 1);
    assert_eq!(view["summary"]["total"], 1);

    // A passing test verifies it.
    send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/matrices/{m}/cells/{cell}/test-results"),
        serde_json::json!({ "testRef": "t1", "status": "pass" }),
    )
    .await;
    let view = json_body(get(app.clone(), &cookie, &view_path).await).await;
    assert_eq!(view["cells"][&key]["status"], "verified");
    assert_eq!(view["cells"][&key]["passingTestCount"], 1);
    assert_eq!(view["summary"]["verified"], 1);

    // One failing test outranks the passing one.
    send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/matrices/{m}/cells/{cell}/test-results"),
        serde_json::json!({ "testRef": "t2", "status": "fail", "detail": "boom" }),
    )
    .await;
    let view = json_body(get(app.clone(), &cookie, &view_path).await).await;
    assert_eq!(
        view["cells"][&key]["status"], "violated",
        "breakage outranks working"
    );
    assert_eq!(view["summary"]["violated"], 1);
    assert_eq!(view["summary"]["verified"], 0);

    // The test-results list is newest first.
    let results = json_body(
        get(
            app,
            &cookie,
            &format!("/api/matrices/{m}/cells/{cell}/test-results"),
        )
        .await,
    )
    .await;
    assert_eq!(results.as_array().unwrap().len(), 2);
    assert_eq!(results[0]["testRef"], "t2", "newest run first");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn recording_a_test_result_rejects_an_unknown_status(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let (m, cell) = a_cell(app.clone(), &cookie, "M").await;

    let res = send(
        app,
        &cookie,
        "POST",
        &format!("/api/matrices/{m}/cells/{cell}/test-results"),
        serde_json::json!({ "testRef": "t", "status": "skipped" }),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

/// A `cellId` from a matrix the caller ALSO owns is rejected — the route must
/// pass its `{id}` through, not just the session.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn a_cell_id_from_another_of_your_own_matrices_is_rejected(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "a@b.test", "Alice").await;
    let (mine, _) = a_cell(app.clone(), &cookie, "Mine").await;
    let (_, other_cell) = a_cell(app.clone(), &cookie, "Other").await;

    let res = get(
        app,
        &cookie,
        &format!("/api/matrices/{mine}/cells/{other_cell}/code"),
    )
    .await;
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "the matrix id in the path must constrain the cell id"
    );
}
