use fubbik_cli::client::Client;

#[tokio::test]
async fn list_builds_the_expected_query_string() {
    let server = wiremock::MockServer::start().await;

    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/chunks"))
        .and(wiremock::matchers::query_param("type", "note"))
        .and(wiremock::matchers::query_param("limit", "10"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "chunks": [],
                "total": 0,
                "limit": 10,
                "offset": 0
            })),
        )
        .mount(&server)
        .await;

    let client = Client::new(server.uri());
    let chunks = client.list_chunks(Some("note"), None, 10).await.unwrap();
    assert!(chunks.is_empty());
}

#[tokio::test]
async fn surfaces_server_errors_as_anyhow() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let client = Client::new(server.uri());
    let err = client.list_chunks(None, None, 50).await.unwrap_err();
    assert!(
        err.to_string().contains("401"),
        "error should mention the status: {err}"
    );
}

#[tokio::test]
async fn review_actions_use_the_proposal_endpoints() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/proposals/p1/approve"))
        .and(wiremock::matchers::body_json(
            serde_json::json!({"note": "looks good"}),
        ))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "p1",
                "chunkId": "c1",
                "changes": {"title": "New title"},
                "reason": null,
                "status": "approved",
                "proposedBy": "u1",
                "reviewedBy": "u1",
                "reviewedAt": null,
                "reviewNote": "looks good",
                "createdAt": "2026-01-01T00:00:00Z"
            })),
        )
        .mount(&server)
        .await;

    let client = Client::new(server.uri());
    let proposal = client
        .review_proposal("p1", "approve", Some("looks good"))
        .await
        .unwrap();
    assert_eq!(proposal.status, "approved");
}

#[tokio::test]
async fn task_claim_reads_the_plan_then_updates_its_first_task() {
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
    let task = client
        .set_quick_task_status("plan-1", "in_progress")
        .await
        .unwrap();
    assert_eq!(task.id, "task-1");
    assert_eq!(task.status, "in_progress");
}

fn chunk_json(id: &str, title: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "title": title,
        "type": "note",
        "content": "body",
        "updatedAt": "2026-01-01T00:00:00Z"
    })
}

#[tokio::test]
async fn get_accepts_the_enriched_chunk_detail_envelope() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/chunks/c1"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "chunk": chunk_json("c1", "Enveloped"),
                "connections": [],
                "tags": []
            })),
        )
        .mount(&server)
        .await;

    let chunk = Client::new(server.uri()).get_chunk("c1").await.unwrap();
    assert_eq!(chunk.title, "Enveloped");
}

#[tokio::test]
async fn update_sends_only_fields_the_user_supplied() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("PATCH"))
        .and(wiremock::matchers::path("/api/chunks/c1"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "title": "Renamed",
            "tags": ["rust", "cli"]
        })))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(chunk_json("c1", "Renamed")),
        )
        .mount(&server)
        .await;

    let tags = vec!["rust".to_string(), "cli".to_string()];
    let chunk = Client::new(server.uri())
        .update_chunk("c1", Some("Renamed"), None, None, Some(&tags), None)
        .await
        .unwrap();
    assert_eq!(chunk.title, "Renamed");
}

#[tokio::test]
async fn update_rejects_an_empty_patch_without_making_a_request() {
    let client = Client::new("http://127.0.0.1:1");
    let error = client
        .update_chunk("c1", None, None, None, None, None)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("nothing to update"));
}

#[tokio::test]
async fn context_export_builds_budget_and_path_query() {
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

    let value = Client::new(server.uri())
        .export_context(Some("space-1"), 6000, "markdown", Some("src/lib.rs"))
        .await
        .unwrap();
    assert_eq!(value["content"], "# Context");
}
