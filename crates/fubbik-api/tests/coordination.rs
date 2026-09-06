mod common;

use axum::http::{Method, StatusCode};
use common::{TestApp, TestUser};

async fn signup(app: TestApp, email: &str) -> TestUser {
    app.signup(email, "Agent").await
}

async fn send(
    app: TestApp,
    user: &TestUser,
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
) -> axum::response::Response {
    app.request(
        Some(user),
        method.parse::<Method>().expect("test method must be valid"),
        path,
        body,
    )
    .await
}

async fn json(response: axum::response::Response) -> serde_json::Value {
    TestApp::json(response).await
}

async fn create_plan(app: TestApp, user: &TestUser) -> (String, String) {
    let response = send(
        app.clone(),
        user,
        "POST",
        "/api/plans",
        Some(serde_json::json!({ "title": "Coordinate", "tasks": [{ "title": "Research" }] })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let plan_id = json(response).await["id"].as_str().unwrap().to_string();
    let detail = json(send(app, user, "GET", &format!("/api/plans/{plan_id}"), None).await).await;
    let task_id = detail["tasks"][0]["id"].as_str().unwrap().to_string();
    (plan_id, task_id)
}

async fn join_run(app: TestApp, user: &TestUser, plan_id: &str, handle: &str) -> serde_json::Value {
    let response = send(
        app,
        user,
        "POST",
        &format!("/api/plans/{plan_id}/board/runs"),
        Some(serde_json::json!({
            "handle": handle,
            "externalKey": format!("integration/{handle}")
        })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

async fn write_entry(
    app: TestApp,
    user: &TestUser,
    plan_id: &str,
    body: serde_json::Value,
) -> serde_json::Value {
    let response = send(
        app,
        user,
        "POST",
        &format!("/api/plans/{plan_id}/board/entries"),
        Some(body),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn agent_can_join_claim_complete_and_reconnect(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);
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
    let app = TestApp::new(pool);
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

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn direct_messages_pagination_and_acknowledgement_work_over_http(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);
    let cookie = signup(app.clone(), "messages@coord.test").await;
    let (plan_id, _) = create_plan(app.clone(), &cookie).await;
    let root = join_run(app.clone(), &cookie, &plan_id, "root").await;
    let child = join_run(app.clone(), &cookie, &plan_id, "child").await;
    let sibling = join_run(app.clone(), &cookie, &plan_id, "sibling").await;

    let public_one = write_entry(
        app.clone(),
        &cookie,
        &plan_id,
        serde_json::json!({
            "runId": child["id"],
            "kind": "progress",
            "body": "Public one",
            "clientMutationId": "messages-1"
        }),
    )
    .await;
    write_entry(
        app.clone(),
        &cookie,
        &plan_id,
        serde_json::json!({
            "runId": child["id"],
            "recipientRunId": root["id"],
            "kind": "question",
            "body": "Root only",
            "clientMutationId": "messages-2"
        }),
    )
    .await;
    let public_two = write_entry(
        app.clone(),
        &cookie,
        &plan_id,
        serde_json::json!({
            "runId": child["id"],
            "kind": "note",
            "body": "Public two",
            "clientMutationId": "messages-3"
        }),
    )
    .await;

    let root_board = json(
        send(
            app.clone(),
            &cookie,
            "GET",
            &format!(
                "/api/plans/{plan_id}/board?runId={}",
                root["id"].as_str().unwrap()
            ),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(root_board["entries"].as_array().unwrap().len(), 3);

    let first_page = json(
        send(
            app.clone(),
            &cookie,
            "GET",
            &format!(
                "/api/plans/{plan_id}/board?runId={}&limit=1",
                sibling["id"].as_str().unwrap()
            ),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(first_page["entries"][0]["id"], public_one["id"]);
    assert_eq!(first_page["cursor"]["hasMore"], true);
    let cursor = first_page["cursor"]["nextSequence"].as_i64().unwrap();

    let second_page = json(
        send(
            app.clone(),
            &cookie,
            "GET",
            &format!(
                "/api/plans/{plan_id}/board?runId={}&afterSequence={cursor}&limit=1",
                sibling["id"].as_str().unwrap()
            ),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(second_page["entries"][0]["id"], public_two["id"]);
    assert_eq!(second_page["cursor"]["hasMore"], false);
    let final_cursor = second_page["cursor"]["nextSequence"].as_i64().unwrap();

    let ack = send(
        app.clone(),
        &cookie,
        "POST",
        &format!(
            "/api/plans/{plan_id}/board/runs/{}/ack",
            sibling["id"].as_str().unwrap()
        ),
        Some(serde_json::json!({ "throughSequence": final_cursor })),
    )
    .await;
    assert_eq!(ack.status(), StatusCode::OK);
    assert_eq!(json(ack).await["lastAckSequence"], final_cursor);

    let impossible_ack = send(
        app,
        &cookie,
        "POST",
        &format!(
            "/api/plans/{plan_id}/board/runs/{}/ack",
            sibling["id"].as_str().unwrap()
        ),
        Some(serde_json::json!({ "throughSequence": final_cursor + 1 })),
    )
    .await;
    assert_eq!(impossible_ack.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn journal_writes_are_idempotent_and_reject_mutation_key_reuse(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);
    let cookie = signup(app.clone(), "idempotency@coord.test").await;
    let (plan_id, task_id) = create_plan(app.clone(), &cookie).await;
    let worker = join_run(app.clone(), &cookie, &plan_id, "worker").await;
    let body = serde_json::json!({
        "runId": worker["id"],
        "taskId": task_id,
        "kind": "artifact",
        "body": "Commit abc123",
        "metadata": { "commit": "abc123" },
        "clientMutationId": "artifact-1"
    });

    let first = write_entry(app.clone(), &cookie, &plan_id, body.clone()).await;
    let retry = write_entry(app.clone(), &cookie, &plan_id, body).await;
    assert_eq!(retry["id"], first["id"]);
    assert_eq!(retry["sequence"], first["sequence"]);

    let reused = send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/plans/{plan_id}/board/entries"),
        Some(serde_json::json!({
            "runId": worker["id"],
            "taskId": task_id,
            "kind": "artifact",
            "body": "A different commit",
            "clientMutationId": "artifact-1"
        })),
    )
    .await;
    assert_eq!(reused.status(), StatusCode::CONFLICT);

    let board = json(
        send(
            app,
            &cookie,
            "GET",
            &format!("/api/plans/{plan_id}/board"),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(board["entries"].as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn only_the_claim_holder_can_renew_release_or_transition_a_task(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);
    let cookie = signup(app.clone(), "leases@coord.test").await;
    let (plan_id, task_id) = create_plan(app.clone(), &cookie).await;
    let holder = join_run(app.clone(), &cookie, &plan_id, "holder").await;
    let other = join_run(app.clone(), &cookie, &plan_id, "other").await;
    let claim_path = format!("/api/plans/{plan_id}/board/tasks/{task_id}/claim");

    let invalid_lease = send(
        app.clone(),
        &cookie,
        "POST",
        &claim_path,
        Some(serde_json::json!({ "runId": holder["id"], "action": "claim", "leaseSeconds": 10 })),
    )
    .await;
    assert_eq!(invalid_lease.status(), StatusCode::BAD_REQUEST);

    let claimed = send(
        app.clone(),
        &cookie,
        "POST",
        &claim_path,
        Some(serde_json::json!({ "runId": holder["id"], "action": "claim", "leaseSeconds": 60 })),
    )
    .await;
    assert_eq!(claimed.status(), StatusCode::OK);

    for action in ["renew", "release"] {
        let response = send(
            app.clone(),
            &cookie,
            "POST",
            &claim_path,
            Some(serde_json::json!({ "runId": other["id"], "action": action, "leaseSeconds": 60 })),
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::CONFLICT,
            "other run unexpectedly performed {action}"
        );
    }

    let renewed = send(
        app.clone(),
        &cookie,
        "POST",
        &claim_path,
        Some(serde_json::json!({ "runId": holder["id"], "action": "renew", "leaseSeconds": 120 })),
    )
    .await;
    assert_eq!(renewed.status(), StatusCode::OK);
    assert_eq!(json(renewed).await["action"], "renew");

    let released = send(
        app.clone(),
        &cookie,
        "POST",
        &claim_path,
        Some(serde_json::json!({ "runId": holder["id"], "action": "release" })),
    )
    .await;
    assert_eq!(released.status(), StatusCode::OK);
    assert_eq!(json(released).await["claim"], serde_json::Value::Null);

    let transition_without_claim = send(
        app,
        &cookie,
        "POST",
        &format!("/api/plans/{plan_id}/board/tasks/{task_id}/transition"),
        Some(serde_json::json!({
            "runId": holder["id"],
            "status": "in_progress",
            "clientMutationId": "after-release"
        })),
    )
    .await;
    assert_eq!(transition_without_claim.status(), StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn reconnect_identity_and_parent_links_are_plan_scoped(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);
    let cookie = signup(app.clone(), "identity@coord.test").await;
    let (first_plan, _) = create_plan(app.clone(), &cookie).await;
    let (second_plan, _) = create_plan(app.clone(), &cookie).await;
    let root = join_run(app.clone(), &cookie, &first_plan, "root").await;

    let child = send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/plans/{first_plan}/board/runs"),
        Some(serde_json::json!({
            "handle": "child",
            "parentRunId": root["id"],
            "externalKey": "identity/child"
        })),
    )
    .await;
    assert_eq!(child.status(), StatusCode::OK);
    assert_eq!(json(child).await["parentRunId"], root["id"]);

    let changed_identity = send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/plans/{first_plan}/board/runs"),
        Some(serde_json::json!({
            "handle": "renamed-child",
            "parentRunId": root["id"],
            "externalKey": "identity/child"
        })),
    )
    .await;
    assert_eq!(changed_identity.status(), StatusCode::CONFLICT);

    let foreign_parent = send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/plans/{second_plan}/board/runs"),
        Some(serde_json::json!({
            "handle": "misplaced-child",
            "parentRunId": root["id"]
        })),
    )
    .await;
    assert_eq!(foreign_parent.status(), StatusCode::NOT_FOUND);

    let unknown_reader = send(
        app,
        &cookie,
        "GET",
        &format!("/api/plans/{first_plan}/board?runId=missing-run"),
        None,
    )
    .await;
    assert_eq!(unknown_reader.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn completing_a_claimed_prerequisite_unblocks_dependents_and_retries_safely(
    pool: sqlx::PgPool,
) {
    let app = TestApp::new(pool);
    let cookie = signup(app.clone(), "dependencies@coord.test").await;
    let (plan_id, prerequisite_id) = create_plan(app.clone(), &cookie).await;
    let dependent_response = send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/plans/{plan_id}/tasks"),
        Some(serde_json::json!({ "title": "Implement" })),
    )
    .await;
    assert_eq!(dependent_response.status(), StatusCode::OK);
    let dependent = json(dependent_response).await;
    let dependent_id = dependent["id"].as_str().unwrap();

    let dependency = send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/plans/{plan_id}/tasks/{dependent_id}/dependencies"),
        Some(serde_json::json!({ "dependsOnTaskId": prerequisite_id })),
    )
    .await;
    assert_eq!(dependency.status(), StatusCode::OK);
    let blocked = send(
        app.clone(),
        &cookie,
        "PATCH",
        &format!("/api/plans/{plan_id}/tasks/{dependent_id}"),
        Some(serde_json::json!({ "status": "blocked" })),
    )
    .await;
    assert_eq!(blocked.status(), StatusCode::OK);

    let worker = join_run(app.clone(), &cookie, &plan_id, "dependency-worker").await;
    let claim = send(
        app.clone(),
        &cookie,
        "POST",
        &format!("/api/plans/{plan_id}/board/tasks/{prerequisite_id}/claim"),
        Some(serde_json::json!({ "runId": worker["id"], "action": "claim" })),
    )
    .await;
    assert_eq!(claim.status(), StatusCode::OK);

    let transition_body = serde_json::json!({
        "runId": worker["id"],
        "status": "done",
        "note": "Prerequisite complete",
        "clientMutationId": "dependency-complete-1"
    });
    let transition_path = format!("/api/plans/{plan_id}/board/tasks/{prerequisite_id}/transition");
    let first_response = send(
        app.clone(),
        &cookie,
        "POST",
        &transition_path,
        Some(transition_body.clone()),
    )
    .await;
    assert_eq!(first_response.status(), StatusCode::OK);
    let first = json(first_response).await;

    let retry_response = send(
        app.clone(),
        &cookie,
        "POST",
        &transition_path,
        Some(transition_body),
    )
    .await;
    assert_eq!(retry_response.status(), StatusCode::OK);
    let retry = json(retry_response).await;
    assert_eq!(retry["entry"]["id"], first["entry"]["id"]);

    let board = json(
        send(
            app,
            &cookie,
            "GET",
            &format!("/api/plans/{plan_id}/board"),
            None,
        )
        .await,
    )
    .await;
    let tasks = board["tasks"].as_array().unwrap();
    let prerequisite = tasks
        .iter()
        .find(|task| task["id"] == prerequisite_id)
        .unwrap();
    let dependent = tasks
        .iter()
        .find(|task| task["id"] == dependent_id)
        .unwrap();
    assert_eq!(prerequisite["status"], "done");
    assert_eq!(dependent["status"], "pending");
    assert_eq!(dependent["dependsOn"], serde_json::json!([prerequisite_id]));
    assert_eq!(board["claims"], serde_json::json!([]));
    assert_eq!(board["entries"].as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn every_coordination_endpoint_requires_a_session(pool: sqlx::PgPool) {
    let app = TestApp::new(pool);
    let requests = [
        ("GET", "/api/plans/plan/board", None),
        (
            "POST",
            "/api/plans/plan/board/runs",
            Some(serde_json::json!({ "handle": "worker" })),
        ),
        (
            "POST",
            "/api/plans/plan/board/tasks/task/claim",
            Some(serde_json::json!({ "runId": "run", "action": "claim" })),
        ),
        (
            "POST",
            "/api/plans/plan/board/tasks/task/transition",
            Some(serde_json::json!({
                "runId": "run",
                "status": "done",
                "clientMutationId": "transition"
            })),
        ),
        (
            "POST",
            "/api/plans/plan/board/entries",
            Some(serde_json::json!({
                "runId": "run",
                "kind": "note",
                "body": "hello",
                "clientMutationId": "entry"
            })),
        ),
        (
            "POST",
            "/api/plans/plan/board/runs/run/ack",
            Some(serde_json::json!({ "throughSequence": 0 })),
        ),
    ];

    for (method, path, body) in requests {
        let response = app
            .request(
                None,
                method.parse::<Method>().expect("test method must be valid"),
                path,
                body,
            )
            .await;
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "{method} {path} was not protected"
        );
    }
}
