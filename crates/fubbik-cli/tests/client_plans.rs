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
async fn task_claim_reads_the_plan_then_updates_its_first_task() {
    // Given
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/plans/plan-1"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "plan": {"id": "plan-1", "title": "Ship it", "status": "in_progress"},
            "requirements": [],
            "analyze": {"chunk": [], "file": [], "risk": [], "assumption": [], "question": []},
            "tasks": [{"id": "task-1", "planId": "plan-1", "title": "Ship it", "status": "pending"}],
            "dependencies": []
        })))
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("PATCH"))
        .and(wiremock::matchers::path("/api/plans/plan-1/tasks/task-1"))
        .and(wiremock::matchers::body_json(
            serde_json::json!({"status": "in_progress"}),
        ))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "task-1", "planId": "plan-1", "title": "Ship it", "status": "in_progress"
            })),
        )
        .mount(&server)
        .await;

    let client = Client::new(server.uri());
    // When
    let task = client
        .set_quick_task_status("plan-1", "in_progress")
        .await
        .unwrap();
    // Then
    assert_eq!(task.id, "task-1");
    assert_eq!(task.status, "in_progress");
}

#[tokio::test]
async fn task_completion_forwards_the_note_to_the_compatibility_endpoint() {
    // Given a quick task plan and a completion endpoint that expects a note
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/plans/plan-1"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "plan": {"id": "plan-1", "title": "Ship it", "status": "in_progress"},
            "requirements": [],
            "analyze": {},
            "tasks": [{"id": "task-1", "planId": "plan-1", "title": "Ship it", "status": "in_progress"}],
            "dependencies": []
        })))
        .mount(&server)
        .await;
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
    let (task, plan) = Client::new(server.uri())
        .complete_quick_task("plan-1", Some("Verified in production"))
        .await
        .unwrap();

    // Then the returned task and plan both reflect completion
    assert_eq!(task.status, "done");
    assert_eq!(plan.status, "completed");
}
