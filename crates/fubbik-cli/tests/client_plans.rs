use fubbik_cli::client::Client;

#[tokio::test]
async fn plan_task_creation_uses_the_nested_plan_endpoint() {
    // Given
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/plans/plan-1/tasks"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "title": "Port MCP", "description": "Replace the TypeScript process"
        })))
        .respond_with(
            wiremock::ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "id": "task-1", "planId": "plan-1", "title": "Port MCP",
                "description": "Replace the TypeScript process", "status": "pending"
            })),
        )
        .mount(&server)
        .await;

    // When
    let task = Client::new(server.uri())
        .create_plan_task("plan-1", "Port MCP", Some("Replace the TypeScript process"))
        .await
        .unwrap();

    // Then
    assert_eq!(task.id, "task-1");
}

#[tokio::test]
async fn linking_a_requirement_uses_the_plan_requirement_contract() {
    // Given
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/plans/plan-1/requirements"))
        .and(wiremock::matchers::body_json(
            serde_json::json!({"requirementId": "req-1"}),
        ))
        .respond_with(
            wiremock::ResponseTemplate::new(201)
                .set_body_json(serde_json::json!({"planId":"plan-1","requirementId":"req-1"})),
        )
        .mount(&server)
        .await;

    // When
    let link = Client::new(server.uri())
        .link_plan_requirement("plan-1", "req-1")
        .await
        .unwrap();

    // Then
    assert_eq!(link["requirementId"], "req-1");
}

#[tokio::test]
async fn task_creation_uses_queue_endpoint_with_priority() {
    // Given a queue endpoint accepting priority and space
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/tasks"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "title": "Ship it", "description": "Now", "spaceId": "space-1", "priority": "high"
        })))
        .respond_with(
            wiremock::ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "id": "plan-1", "title": "Ship it", "status": "in_progress"
            })),
        )
        .mount(&server)
        .await;
    // When the client creates a quick task
    let task = Client::new(server.uri())
        .create_quick_task("Ship it", Some("Now"), Some("space-1"), "high")
        .await
        .unwrap();
    // Then the queue plan is returned
    assert_eq!(task.id, "plan-1");
}

#[tokio::test]
async fn task_list_uses_queue_only_endpoint() {
    // Given the queue list endpoint
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/tasks"))
        .and(wiremock::matchers::query_param("status", "completed"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"id": "plan-1", "title": "Ship it", "status": "completed"}
            ])),
        )
        .mount(&server)
        .await;
    // When completed tasks are listed
    let tasks = Client::new(server.uri())
        .list_quick_tasks("completed")
        .await
        .unwrap();
    // Then only queue results are returned
    assert_eq!(tasks[0].id, "plan-1");
}

#[tokio::test]
async fn task_claim_uses_queue_endpoint() {
    // Given the queue claim endpoint
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/tasks/plan-1/claim"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "plan": {"id": "plan-1", "title": "Ship it", "status": "in_progress"},
                "tasks": [], "requirements": [], "analyze": {}, "dependencies": []
            })),
        )
        .mount(&server)
        .await;
    // When the task is claimed
    let detail = Client::new(server.uri())
        .claim_quick_task("plan-1")
        .await
        .unwrap();
    // Then its plan is returned
    assert_eq!(detail.plan.id, "plan-1");
}

#[tokio::test]
async fn task_completion_forwards_the_note_to_the_compatibility_endpoint() {
    // Given a completion endpoint that expects a note
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/tasks/plan-1/complete"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "note": "Verified in production"
        })))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "plan-1", "title": "Ship it", "status": "completed"
            })),
        )
        .mount(&server)
        .await;

    // When the client completes the quick task with that note
    let plan = Client::new(server.uri())
        .complete_quick_task("plan-1", Some("Verified in production"))
        .await
        .unwrap();

    // Then the completed queue plan is returned
    assert_eq!(plan.status, "completed");
}
