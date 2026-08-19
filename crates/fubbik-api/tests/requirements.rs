//! HTTP-level tests for the `requirements` domain: CRUD, chunk links,
//! stats, bulk actions, reorder, export, and batch create.
//!
//! Cross-user ownership tests here exercise the full stack (route ->
//! service -> repo) with `RequirementError`'s custom `IntoResponse`
//! wired in; the guard-removal proofs for `set_chunks`'s three-guard
//! join live at the repo layer (`fubbik-db/tests/requirement.rs`), per
//! this codebase's convention that a service-level 404 pre-check must
//! not be the only thing standing between a removed SQL guard and a
//! passing test.

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
    serde_json::from_slice(&body).unwrap()
}

async fn text_body(response: axum::response::Response) -> String {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(body.to_vec()).unwrap()
}

fn gwt_steps() -> serde_json::Value {
    serde_json::json!([
        {"keyword": "given", "text": "a user"},
        {"keyword": "when", "text": "they log in"},
        {"keyword": "then", "text": "they see the dashboard"},
    ])
}

async fn create_requirement(
    app: axum::Router,
    cookie: &str,
    title: &str,
) -> axum::response::Response {
    let body = serde_json::json!({"title": title, "steps": gwt_steps()});
    app.oneshot(
        Request::post("/api/requirements")
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
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
    assert_eq!(res.status(), StatusCode::OK);
    json_body(res).await["id"].as_str().unwrap().to_string()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_then_get_round_trip(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-req@b.test", "Alice").await;

    let res = create_requirement(app.clone(), &cookie, "Login flow").await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let created = json_body(res).await;
    assert_eq!(created["requirement"]["title"], "Login flow");
    assert_eq!(created["requirement"]["status"], "untested");
    assert_eq!(created["requirement"]["origin"], "human");
    assert_eq!(created["requirement"]["reviewStatus"], "approved");
    assert_eq!(created["warnings"], serde_json::json!([]));
    let id = created["requirement"]["id"].as_str().unwrap().to_string();

    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/requirements/{id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let detail = json_body(res).await;
    assert_eq!(
        detail["title"], "Login flow",
        "detail must flatten the requirement's own fields at the top level"
    );
    assert_eq!(detail["chunks"], serde_json::json!([]));
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_rejects_steps_that_fail_validation_with_structured_errors(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-invalid-steps@b.test", "Alice").await;

    let body = serde_json::json!({"title": "Bad", "steps": [{"keyword": "then", "text": "x"}]});
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/requirements")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let payload = json_body(res).await;
    assert_eq!(payload["message"], "Invalid steps");
    assert!(
        payload["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["error"] == "First step must be 'given'")
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn get_is_user_scoped(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice_cookie = signup(app.clone(), "alice-scope@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-scope@b.test", "Bob").await;

    let created =
        json_body(create_requirement(app.clone(), &alice_cookie, "Alice's requirement").await)
            .await;
    let id = created["requirement"]["id"].as_str().unwrap();

    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/requirements/{id}"))
                .header("cookie", &bob_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::NOT_FOUND,
        "Bob must not be able to read Alice's requirement"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_and_delete_round_trip(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-update@b.test", "Alice").await;
    let created = json_body(create_requirement(app.clone(), &cookie, "Original").await).await;
    let id = created["requirement"]["id"].as_str().unwrap().to_string();

    let res = app
        .clone()
        .oneshot(
            Request::patch(format!("/api/requirements/{id}"))
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(
                    serde_json::json!({"title": "Renamed"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let updated = json_body(res).await;
    assert_eq!(updated["requirement"]["title"], "Renamed");

    let res = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/requirements/{id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        json_body(res).await,
        serde_json::json!({"message": "Deleted"})
    );

    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/requirements/{id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_status_and_bare_number_bulk_response(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-status@b.test", "Alice").await;
    let created = json_body(create_requirement(app.clone(), &cookie, "Status target").await).await;
    let id = created["requirement"]["id"].as_str().unwrap().to_string();

    let res = app
        .clone()
        .oneshot(
            Request::patch(format!("/api/requirements/{id}/status"))
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(
                    serde_json::json!({"status": "passing"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let updated = json_body(res).await;
    assert_eq!(
        updated["status"], "passing",
        "status route returns the bare requirement, not wrapped"
    );

    // Bulk action returns a bare number, not an envelope — matches Node's
    // `bulkAction` returning `bulkUpdateRequirements`'s raw row count.
    let res = app
        .clone()
        .oneshot(
            Request::patch("/api/requirements/bulk")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(
                    serde_json::json!({"ids": [id], "action": "set_status", "status": "failing"})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = text_body(res).await;
    assert_eq!(
        body, "1",
        "bulk action must respond with a bare JSON number, not {{count: n}}"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn set_chunks_returns_join_rows_and_verifies_every_id(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-chunks@b.test", "Alice").await;
    let created = json_body(create_requirement(app.clone(), &cookie, "Chunked").await).await;
    let id = created["requirement"]["id"].as_str().unwrap().to_string();
    let chunk_id = create_chunk(app.clone(), &cookie, "A chunk").await;

    let res = app
        .clone()
        .oneshot(
            Request::put(format!("/api/requirements/{id}/chunks"))
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(
                    serde_json::json!({"chunkIds": [chunk_id]}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let links = json_body(res).await;
    assert_eq!(
        links,
        serde_json::json!([{"requirementId": id, "chunkId": chunk_id}])
    );

    // A nonexistent chunk id must reject the WHOLE call with 404, not
    // silently drop it.
    let res = app
        .clone()
        .oneshot(
            Request::put(format!("/api/requirements/{id}/chunks"))
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(
                    serde_json::json!({"chunkIds": ["does-not-exist"]}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn export_returns_bare_text(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-export@b.test", "Alice").await;
    let created = json_body(create_requirement(app.clone(), &cookie, "Login").await).await;
    let id = created["requirement"]["id"].as_str().unwrap().to_string();

    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/requirements/{id}/export?format=gherkin"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let text = text_body(res).await;
    assert!(
        text.starts_with("Feature: Login"),
        "export must return bare Gherkin text, not JSON: {text}"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn stats_reports_totals(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-stats@b.test", "Alice").await;
    create_requirement(app.clone(), &cookie, "One").await;
    create_requirement(app.clone(), &cookie, "Two").await;

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/requirements/stats")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let stats = json_body(res).await;
    assert_eq!(stats["total"], 2);
    assert_eq!(stats["untested"], 2);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn stats_filters_by_space_using_the_camel_case_param(pool: sqlx::PgPool) {
    // `StatsQuery` deserializes `space_id`, but Node's route takes `spaceId`
    // (`packages/api/src/requirements/routes.ts:34`) and so does every other
    // query in this domain. Without `rename_all = "camelCase"` the param is
    // silently IGNORED rather than rejected — stats come back unscoped, which
    // looks like a working endpoint returning wrong numbers. Serde ignores
    // unknown fields, so nothing surfaces the mistake but a test that asserts
    // the filter actually filtered.
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-stats-space@b.test", "Alice").await;

    let space_res = app
        .clone()
        .oneshot(
            Request::post("/api/spaces")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(
                    serde_json::json!({"name": "Scoped", "kind": "code"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(space_res.status(), StatusCode::CREATED);
    let space_id = json_body(space_res).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // One requirement inside the space, one outside it.
    let in_space = app
        .clone()
        .oneshot(
            Request::post("/api/requirements")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(
                    serde_json::json!({
                        "title": "In space",
                        "steps": gwt_steps(),
                        "spaceId": space_id,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(in_space.status(), StatusCode::CREATED);
    create_requirement(app.clone(), &cookie, "Outside space").await;

    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/requirements/stats?spaceId={space_id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let stats = json_body(res).await;
    assert_eq!(
        stats["total"], 1,
        "`spaceId` must scope the stats; got {stats} — if this is 2 the param \
         was ignored and the endpoint is reporting every requirement"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn batch_create_resolves_use_case_names(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "alice-batch@b.test", "Alice").await;

    let body = serde_json::json!({
        "requirements": [
            {"title": "R1", "steps": gwt_steps(), "useCaseName": "Checkout"},
            {"title": "R2", "steps": gwt_steps(), "useCaseName": "Checkout"},
        ]
    });
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/requirements/batch")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let result = json_body(res).await;
    assert_eq!(result["created"], 2);
    assert_eq!(
        result["useCasesCreated"].as_array().unwrap().len(),
        1,
        "the same use case name must be created only once across the batch"
    );
    let uc1 = result["requirements"][0]["useCaseId"].as_str().unwrap();
    let uc2 = result["requirements"][1]["useCaseId"].as_str().unwrap();
    assert_eq!(
        uc1, uc2,
        "both requirements must share the same resolved use case id"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn reorder_rejects_ids_not_owned_by_the_caller(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice_cookie = signup(app.clone(), "alice-reorder@b.test", "Alice").await;
    let bob_cookie = signup(app.clone(), "bob-reorder@b.test", "Bob").await;

    let a = json_body(create_requirement(app.clone(), &alice_cookie, "A").await).await;
    let a_id = a["requirement"]["id"].as_str().unwrap().to_string();
    let b = json_body(create_requirement(app.clone(), &bob_cookie, "B").await).await;
    let b_id = b["requirement"]["id"].as_str().unwrap().to_string();

    let res = app
        .clone()
        .oneshot(
            Request::patch("/api/requirements/reorder")
                .header("content-type", "application/json")
                .header("cookie", &alice_cookie)
                .body(Body::from(
                    serde_json::json!({"requirementIds": [a_id, b_id]}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "must reject a reorder list containing another user's requirement id"
    );
}
