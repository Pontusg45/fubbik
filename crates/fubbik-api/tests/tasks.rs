mod common;

use axum::http::StatusCode;
use common::TestApp;

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn quick_task_moves_through_claim_and_completion(pool: sqlx::PgPool) {
    // Given a newly created single-task plan
    let app = TestApp::new(pool);
    let user = app.signup("quick-task@example.com", "Task User").await;
    let created = app
        .post(
            &user,
            "/api/tasks",
            serde_json::json!({"title": "Review migration", "description": "Check parity"}),
        )
        .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let plan_id = TestApp::json(created).await["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // When the task is claimed
    let claimed = app
        .post(
            &user,
            &format!("/api/tasks/{plan_id}/claim"),
            serde_json::json!({}),
        )
        .await;

    // Then its task and queue entry are in progress
    assert_eq!(claimed.status(), StatusCode::OK);
    let claimed = TestApp::json(claimed).await;
    assert_eq!(claimed["tasks"][0]["status"], "in_progress");
    let open = app.get(&user, "/api/tasks").await;
    assert_eq!(open.status(), StatusCode::OK);
    let open = TestApp::json(open).await;
    assert_eq!(open.as_array().unwrap().len(), 1);
    assert_eq!(open[0]["metadata"]["taskQueue"], true);
    assert_eq!(open[0]["metadata"]["priority"], "normal");

    // When the quick task is completed
    let completed = app
        .post(
            &user,
            &format!("/api/tasks/{plan_id}/complete"),
            serde_json::json!({"note": "Parity confirmed"}),
        )
        .await;

    // Then the plan is completed and no longer appears in the open queue
    assert_eq!(completed.status(), StatusCode::OK);
    assert_eq!(TestApp::json(completed).await["status"], "completed");
    let open = app.get(&user, "/api/tasks").await;
    assert!(TestApp::json(open).await.as_array().unwrap().is_empty());
    let completed = app.get(&user, "/api/tasks?status=completed").await;
    assert_eq!(TestApp::json(completed).await[0]["id"], plan_id);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn queue_routes_exclude_and_reject_ordinary_plans(pool: sqlx::PgPool) {
    // Given an ordinary in-progress plan with a task.
    let app = TestApp::new(pool);
    let user = app.signup("queue-boundary@example.com", "Task User").await;
    let created = app
        .post(
            &user,
            "/api/plans",
            serde_json::json!({"title": "Ordinary", "tasks": [{"title": "Work"}]}),
        )
        .await;
    let id = TestApp::json(created).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    app.patch(
        &user,
        &format!("/api/plans/{id}"),
        serde_json::json!({"status": "in_progress"}),
    )
    .await;

    // When queue endpoints receive that plan's id.
    let listed = app.get(&user, "/api/tasks").await;
    let claimed = app
        .post(
            &user,
            &format!("/api/tasks/{id}/claim"),
            serde_json::json!({}),
        )
        .await;
    let completed = app
        .post(
            &user,
            &format!("/api/tasks/{id}/complete"),
            serde_json::json!({}),
        )
        .await;

    // Then it is absent from the queue and neither action changes it.
    assert!(TestApp::json(listed).await.as_array().unwrap().is_empty());
    assert_eq!(claimed.status(), StatusCode::NOT_FOUND);
    assert_eq!(completed.status(), StatusCode::NOT_FOUND);
    let detail = app.get(&user, &format!("/api/plans/{id}")).await;
    assert_eq!(TestApp::json(detail).await["plan"]["status"], "in_progress");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn queue_priority_orders_active_tasks_and_validates_input(pool: sqlx::PgPool) {
    // Given low and urgent standalone tasks.
    let app = TestApp::new(pool);
    let user = app.signup("queue-priority@example.com", "Task User").await;
    for (title, priority) in [("Later", "low"), ("Now", "urgent")] {
        let created = app
            .post(
                &user,
                "/api/tasks",
                serde_json::json!({"title": title, "priority": priority}),
            )
            .await;
        assert_eq!(created.status(), StatusCode::CREATED);
    }

    // When the active queue is listed and an invalid priority is submitted.
    let listed = app.get(&user, "/api/tasks").await;
    let invalid = app
        .post(
            &user,
            "/api/tasks",
            serde_json::json!({"title": "Bad", "priority": "later"}),
        )
        .await;

    // Then priority determines order and invalid values are rejected.
    let listed = TestApp::json(listed).await;
    assert_eq!(listed[0]["title"], "Now");
    assert_eq!(listed[0]["metadata"]["priority"], "urgent");
    assert_eq!(listed[1]["title"], "Later");
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let invalid_status = app.get(&user, "/api/tasks?status=draft").await;
    assert_eq!(invalid_status.status(), StatusCode::BAD_REQUEST);
}
