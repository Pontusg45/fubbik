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

async fn signup(app: axum::Router, email: &str) -> String {
    let res = app
        .oneshot(
            Request::post("/api/auth/sign-up/email")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"email":"{email}","password":"hunter22","name":"Agent"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    res.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

async fn send(
    app: axum::Router,
    cookie: &str,
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("cookie", cookie);
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    app.oneshot(
        request
            .body(body.map_or_else(Body::empty, |v| Body::from(v.to_string())))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn json(response: axum::response::Response) -> serde_json::Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

async fn create_plan(app: axum::Router, cookie: &str) -> (String, String) {
    let response = send(
        app.clone(),
        cookie,
        "POST",
        "/api/plans",
        Some(serde_json::json!({ "title": "Coordinate", "tasks": [{ "title": "Research" }] })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let plan_id = json(response).await["id"].as_str().unwrap().to_string();
    let detail = json(send(app, cookie, "GET", &format!("/api/plans/{plan_id}"), None).await).await;
    let task_id = detail["tasks"][0]["id"].as_str().unwrap().to_string();
    (plan_id, task_id)
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn agent_can_join_claim_complete_and_reconnect(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let cookie = signup(app.clone(), "workflow@coord.test").await;
    let (plan_id, task_id) = create_plan(app.clone(), &cookie).await;
    let join_body = serde_json::json!({ "handle": "worker", "externalKey": "thread/worker" });
    let joined = json(
        send(
            app.clone(),
            &cookie,
            "POST",
            &format!("/api/plans/{plan_id}/board/runs"),
            Some(join_body.clone()),
        )
        .await,
    )
    .await;
    let run_id = joined["id"].as_str().unwrap();

    let claim = send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/plans/{plan_id}/board/tasks/{task_id}/claim"),
        Some(serde_json::json!({ "runId": run_id, "action": "claim" })),
    )
    .await;
    assert_eq!(claim.status(), StatusCode::OK);

    let transition = send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/plans/{plan_id}/board/tasks/{task_id}/transition"),
        Some(serde_json::json!({
            "runId": run_id,
            "status": "done",
            "note": "Delivered",
            "clientMutationId": "complete-1"
        })),
    )
    .await;
    assert_eq!(transition.status(), StatusCode::OK);

    let board = json(
        send(
            app.clone(),
            &cookie,
            "GET",
            &format!("/api/plans/{plan_id}/board?runId={run_id}"),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(board["tasks"][0]["status"], "done");
    assert_eq!(board["entries"][0]["body"], "Delivered");
    assert_eq!(board["claims"], serde_json::json!([]));

    let rejoined = json(
        send(
            app,
            &cookie,
            "POST",
            &format!("/api/plans/{plan_id}/board/runs"),
            Some(join_body),
        )
        .await,
    )
    .await;
    assert_eq!(rejoined["id"], joined["id"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn board_is_user_scoped_and_claim_conflicts_are_409(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool));
    let alice = signup(app.clone(), "alice@coord.test").await;
    let bob = signup(app.clone(), "bob@coord.test").await;
    let (plan_id, task_id) = create_plan(app.clone(), &alice).await;
    let a = json(
        send(
            app.clone(),
            &alice,
            "POST",
            &format!("/api/plans/{plan_id}/board/runs"),
            Some(serde_json::json!({ "handle": "a" })),
        )
        .await,
    )
    .await;
    let b = json(
        send(
            app.clone(),
            &alice,
            "POST",
            &format!("/api/plans/{plan_id}/board/runs"),
            Some(serde_json::json!({ "handle": "b" })),
        )
        .await,
    )
    .await;

    let first = send(
        app.clone(),
        &alice,
        "POST",
        &format!("/api/plans/{plan_id}/board/tasks/{task_id}/claim"),
        Some(serde_json::json!({ "runId": a["id"], "action": "claim" })),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    let second = send(
        app.clone(),
        &alice,
        "POST",
        &format!("/api/plans/{plan_id}/board/tasks/{task_id}/claim"),
        Some(serde_json::json!({ "runId": b["id"], "action": "claim" })),
    )
    .await;
    assert_eq!(second.status(), StatusCode::CONFLICT);

    let foreign = send(
        app,
        &bob,
        "GET",
        &format!("/api/plans/{plan_id}/board"),
        None,
    )
    .await;
    assert_eq!(foreign.status(), StatusCode::NOT_FOUND);
}
