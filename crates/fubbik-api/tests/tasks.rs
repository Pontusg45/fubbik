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
    assert_eq!(TestApp::json(open).await.as_array().unwrap().len(), 1);

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
}
