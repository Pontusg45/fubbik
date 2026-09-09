use fubbik_cli::client::Client;

#[tokio::test]
async fn space_list_uses_the_active_rust_api() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/spaces"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                "id": "space-1", "name": "fubbik", "kind": "code",
                "description": null, "userId": "u1",
                "createdAt": "2026-01-01T00:00:00Z", "updatedAt": "2026-01-01T00:00:00Z"
            }])),
        )
        .mount(&server)
        .await;

    let spaces = Client::new(server.uri()).list_spaces().await.unwrap();
    assert_eq!(spaces.len(), 1);
    assert_eq!(spaces[0].name, "fubbik");
}

#[tokio::test]
async fn space_references_accept_an_exact_name_or_id() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/spaces"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                "id": "space-1", "name": "fubbik", "kind": "code",
                "userId": "u1", "createdAt": "2026-01-01T00:00:00Z",
                "updatedAt": "2026-01-01T00:00:00Z"
            }])),
        )
        .mount(&server)
        .await;

    let client = Client::new(server.uri());
    assert_eq!(
        client
            .resolve_space(Some("fubbik"))
            .await
            .unwrap()
            .as_deref(),
        Some("space-1")
    );
    assert_eq!(
        client
            .resolve_space(Some("space-1"))
            .await
            .unwrap()
            .as_deref(),
        Some("space-1")
    );
}

#[tokio::test]
async fn multiple_space_references_are_resolved_in_one_lookup() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/spaces"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"id": "space-1", "name": "frontend", "kind": "code"},
                {"id": "space-2", "name": "backend", "kind": "code"}
            ])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let resolved = Client::new(server.uri())
        .resolve_spaces(&["frontend".into(), "space-2".into()])
        .await
        .unwrap();
    assert_eq!(resolved, ["space-1", "space-2"]);
}

#[tokio::test]
async fn space_detection_accepts_the_rust_apis_empty_no_match_response() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/spaces/detect"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_raw("", "text/plain"))
        .mount(&server)
        .await;

    let detected = Client::new(server.uri())
        .detect_space(Some("/tmp/unknown"), None)
        .await
        .unwrap();
    assert!(detected.is_none());
}

#[tokio::test]
async fn tag_list_preserves_server_counts() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/tags"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(
                serde_json::json!([{"id": "tag-1", "name": "rust", "chunkCount": 3}]),
            ),
        )
        .mount(&server)
        .await;

    let tags = Client::new(server.uri()).list_tags().await.unwrap();
    assert_eq!(tags[0].chunk_count, 3);
}

#[tokio::test]
async fn link_sends_the_connection_contract() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/connections"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "sourceId": "a", "targetId": "b", "relation": "supports", "origin": "human"
        })))
        .respond_with(
            wiremock::ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "id": "edge-1", "sourceId": "a", "targetId": "b",
                "relation": "supports", "createdAt": "2026-01-01T00:00:00Z",
                "origin": "human", "reviewStatus": "approved", "weight": 1
            })),
        )
        .mount(&server)
        .await;

    let edge = Client::new(server.uri())
        .create_connection("a", "b", "supports")
        .await
        .unwrap();
    assert_eq!(edge.id, "edge-1");
}

#[tokio::test]
async fn requirements_list_forwards_filters_and_unwraps_the_envelope() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/requirements"))
        .and(wiremock::matchers::query_param("spaceId", "space-1"))
        .and(wiremock::matchers::query_param("status", "failing"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"requirements": [{
                "id": "req-1", "title": "Login works", "steps": [], "order": 0,
                "status": "failing", "priority": "must", "userId": "u1",
                "createdAt": "2026-01-01T00:00:00Z", "updatedAt": "2026-01-01T00:00:00Z",
                "origin": "human", "reviewStatus": "approved"
            }], "total": 1}),
        ))
        .mount(&server)
        .await;

    let requirements = Client::new(server.uri())
        .list_requirements(Some("space-1"), Some("failing"), None)
        .await
        .unwrap();
    assert_eq!(requirements[0].id, "req-1");
}

#[tokio::test]
async fn enrich_all_uses_the_bulk_endpoint() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/chunks/enrich-all"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"enriched": 4, "failed": 0})),
        )
        .mount(&server)
        .await;

    let result = Client::new(server.uri()).enrich_all().await.unwrap();
    assert_eq!(result["enriched"], 4);
}

#[tokio::test]
async fn document_import_sends_file_content_and_source_path() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/documents/import"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "sourcePath": "/repo/README.md", "content": "# Hello", "spaceId": "space-1"
        })))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"document": {"id": "doc-1", "title": "Hello"}, "created": 1, "updated": 0, "status": "created"}),
        ))
        .mount(&server)
        .await;

    let imported = Client::new(server.uri())
        .import_document("/repo/README.md", "# Hello", Some("space-1"))
        .await
        .unwrap();
    assert_eq!(imported["document"]["id"], "doc-1");
}

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
