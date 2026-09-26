use fubbik_cli::client::Client;

#[tokio::test]
async fn context_about_forwards_the_semantic_query_contract() {
    // Given
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/context/about"))
        .and(wiremock::matchers::query_param("q", "authentication"))
        .and(wiremock::matchers::query_param("spaceId", "space-1"))
        .and(wiremock::matchers::query_param("maxTokens", "2400"))
        .and(wiremock::matchers::query_param("format", "structured-json"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"sections": [], "totalTokens": 0})),
        )
        .mount(&server)
        .await;

    // When
    let context = Client::new(server.uri())
        .context_about("authentication", Some("space-1"), 2400, "structured-json")
        .await
        .unwrap();

    // Then
    assert_eq!(context["totalTokens"], 0);
}

#[tokio::test]
async fn plan_context_forwards_the_plan_budget_and_format() {
    // Given a plan context endpoint with an exact query contract
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/context/for-plan"))
        .and(wiremock::matchers::query_param("planId", "plan-1"))
        .and(wiremock::matchers::query_param("maxTokens", "6000"))
        .and(wiremock::matchers::query_param("format", "structured-json"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "format": "structured-json", "sections": [], "totalChunks": 0
            })),
        )
        .mount(&server)
        .await;

    // When plan-scoped context is requested
    let value = Client::new(server.uri())
        .context_for_plan("plan-1", 6000, "structured-json")
        .await
        .unwrap();

    // Then the structured response is returned
    assert_eq!(value["format"], "structured-json");
}

#[tokio::test]
async fn multi_file_context_joins_paths_and_forwards_the_space() {
    // Given a multi-file context endpoint with two changed paths
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/context/for-files"))
        .and(wiremock::matchers::query_param(
            "paths",
            "src/lib.rs,src/main.rs",
        ))
        .and(wiremock::matchers::query_param("spaceId", "space-1"))
        .and(wiremock::matchers::query_param("maxTokens", "8000"))
        .and(wiremock::matchers::query_param("format", "structured-md"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "format": "structured-md", "content": "# Context", "totalChunks": 1
            })),
        )
        .mount(&server)
        .await;

    // When context is requested for both paths
    let value = Client::new(server.uri())
        .context_for_files(
            &["src/lib.rs".into(), "src/main.rs".into()],
            Some("space-1"),
            8000,
            "structured-md",
        )
        .await
        .unwrap();

    // Then the endpoint response is returned intact
    assert_eq!(value["content"], "# Context");
}

#[tokio::test]
async fn context_snapshot_creation_sends_all_supported_selectors() {
    // Given a snapshot endpoint expecting plan, task, concept, files, and budget fields
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/context/snapshot"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "planId": "plan-1",
            "taskId": "task-1",
            "concept": "authentication",
            "filePaths": ["src/auth.rs"],
            "spaceId": "space-1",
            "maxTokens": 5000
        })))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "snapshotId": "snapshot-1", "tokenCount": 120, "chunkCount": 2,
                "createdAt": "2026-09-24T10:00:00"
            })),
        )
        .mount(&server)
        .await;

    // When a snapshot is created through the client
    let value = Client::new(server.uri())
        .create_context_snapshot(
            Some("plan-1"),
            Some("task-1"),
            Some("authentication"),
            &["src/auth.rs".into()],
            Some("space-1"),
            5000,
        )
        .await
        .unwrap();

    // Then the snapshot identifier is returned
    assert_eq!(value["snapshotId"], "snapshot-1");
}

#[tokio::test]
async fn context_snapshot_deletion_accepts_an_empty_success_response() {
    // Given a snapshot delete endpoint returning the legacy empty response
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("DELETE"))
        .and(wiremock::matchers::path("/api/context/snapshot/snapshot-1"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(&server)
        .await;

    // When the snapshot is deleted
    let result = Client::new(server.uri())
        .delete_context_snapshot("snapshot-1")
        .await;

    // Then the empty body is treated as success
    assert!(result.is_ok());
}

#[tokio::test]
async fn instruction_generation_uses_the_space_route_and_requested_format() {
    // Given an instruction endpoint for a resolved space and AGENTS.md format
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path(
            "/api/spaces/space-1/generate-instructions",
        ))
        .and(wiremock::matchers::query_param("format", "agents"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "format": "agents", "content": "# Agent instructions\n"
            })),
        )
        .mount(&server)
        .await;

    // When AGENTS.md instructions are generated
    let value = Client::new(server.uri())
        .generate_instructions("space-1", "agents")
        .await
        .unwrap();

    // Then the generated content is returned
    assert_eq!(value["content"], "# Agent instructions\n");
}

#[tokio::test]
async fn context_export_builds_budget_and_path_query() {
    // Given
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/chunks/export/context"))
        .and(wiremock::matchers::query_param("spaceId", "space-1"))
        .and(wiremock::matchers::query_param("maxTokens", "6000"))
        .and(wiremock::matchers::query_param("format", "markdown"))
        .and(wiremock::matchers::query_param("forPath", "src/lib.rs"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "format": "markdown", "tokens": 12, "content": "# Context"
            })),
        )
        .mount(&server)
        .await;

    // When
    let value = Client::new(server.uri())
        .export_context(Some("space-1"), 6000, "markdown", Some("src/lib.rs"))
        .await
        .unwrap();
    // Then
    assert_eq!(value["content"], "# Context");
}
