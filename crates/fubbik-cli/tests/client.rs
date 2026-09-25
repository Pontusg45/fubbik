use fubbik_cli::client::Client;

#[tokio::test]
async fn quick_chunk_creation_forwards_space_tags_and_update_tag() {
    // Given a chunk endpoint expecting the quick-command creation contract
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/chunks"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "title": "Remember this",
            "content": "Piped details\n",
            "type": "note",
            "tags": ["agent", "memory"],
            "spaceIds": ["space-1"],
            "updateTag": "session-42"
        })))
        .respond_with(
            wiremock::ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "id": "chunk-1", "title": "Remember this", "content": "Piped details\n",
                "type": "note", "updatedAt": "2026-09-24T10:00:00"
            })),
        )
        .mount(&server)
        .await;

    // When a quick chunk is created with all optional metadata
    let chunk = Client::new(server.uri())
        .create_chunk_with_update_tag(
            "Remember this",
            "Piped details\n",
            "note",
            &["agent".into(), "memory".into()],
            &["space-1".into()],
            Some("session-42"),
        )
        .await
        .unwrap();

    // Then the created chunk is returned
    assert_eq!(chunk.id, "chunk-1");
}

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
async fn update_listing_forwards_tag_and_space_filters() {
    // Given an update endpoint expecting a tag and space filter
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/chunks/updates"))
        .and(wiremock::matchers::query_param("tag", "release-1"))
        .and(wiremock::matchers::query_param("spaceId", "space-1"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "updates": [{"chunkId": "chunk-1", "chunkTitle": "Tagged", "version": 0}]
            })),
        )
        .mount(&server)
        .await;

    // When updates are listed through the CLI client
    let value = Client::new(server.uri())
        .list_updates("release-1", Some("space-1"))
        .await
        .unwrap();

    // Then the matching update envelope is returned
    assert_eq!(value["updates"][0]["chunkId"], "chunk-1");
}

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
async fn source_documentation_posts_a_versioned_manifest_to_the_import_endpoint() {
    // Given
    let server = wiremock::MockServer::start().await;
    let manifest = fubbik_core::source_docs::SourceManifest {
        version: 1,
        project: "example".into(),
        language: fubbik_core::source_docs::SourceLanguage::Java,
        extractor: "fixture".into(),
        complete: false,
        diagnostics: vec![],
        symbols: vec![],
    };
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/documents/import-source"))
        .and(wiremock::matchers::body_json(
            serde_json::json!({"spaceId":"space-1","manifest":manifest}),
        ))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"documentId":"doc-1","created":0})),
        )
        .expect(1)
        .mount(&server)
        .await;
    // When
    let result = Client::new(server.uri())
        .import_source_docs("space-1", &manifest)
        .await
        .unwrap();
    // Then
    assert_eq!(result["documentId"], "doc-1");
}

#[tokio::test]
async fn space_list_uses_the_active_rust_api() {
    // Given
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

    // When
    let spaces = Client::new(server.uri()).list_spaces().await.unwrap();
    // Then
    assert_eq!(spaces.len(), 1);
    assert_eq!(spaces[0].name, "fubbik");
}

#[tokio::test]
async fn space_references_accept_an_exact_name_or_id() {
    // Given
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

    // When
    let client = Client::new(server.uri());
    // Then
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
    // Given
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

    // When
    let resolved = Client::new(server.uri())
        .resolve_spaces(&["frontend".into(), "space-2".into()])
        .await
        .unwrap();
    // Then
    assert_eq!(resolved, ["space-1", "space-2"]);
}

#[tokio::test]
async fn space_detection_accepts_the_rust_apis_empty_no_match_response() {
    // Given
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/spaces/detect"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_raw("", "text/plain"))
        .mount(&server)
        .await;

    // When
    let detected = Client::new(server.uri())
        .detect_space(Some("/tmp/unknown"), None)
        .await
        .unwrap();
    // Then
    assert!(detected.is_none());
}

#[tokio::test]
async fn tag_list_preserves_server_counts() {
    // Given
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

    // When
    let tags = Client::new(server.uri()).list_tags().await.unwrap();
    // Then
    assert_eq!(tags[0].chunk_count, 3);
}

#[tokio::test]
async fn link_sends_the_connection_contract() {
    // Given
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

    // When
    let edge = Client::new(server.uri())
        .create_connection("a", "b", "supports")
        .await
        .unwrap();
    // Then
    assert_eq!(edge.id, "edge-1");
}

#[tokio::test]
async fn requirements_list_forwards_filters_and_unwraps_the_envelope() {
    // Given
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

    // When
    let requirements = Client::new(server.uri())
        .list_requirements(Some("space-1"), Some("failing"), None)
        .await
        .unwrap();
    // Then
    assert_eq!(requirements[0].id, "req-1");
}

#[tokio::test]
async fn requirement_creation_forwards_the_import_description_and_priority() {
    // Given a requirement endpoint expecting the fields parsed from Gherkin
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/requirements"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "title": "Sign in",
            "description": "Authentication",
            "steps": [{"keyword": "given", "text": "a registered user"}],
            "spaceId": "space-1",
            "priority": "must"
        })))
        .respond_with(
            wiremock::ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "requirement": {
                    "id": "req-1", "title": "Sign in", "status": "untested",
                    "priority": "must", "steps": []
                }
            })),
        )
        .mount(&server)
        .await;

    // When the client creates the imported requirement
    let requirement = Client::new(server.uri())
        .create_requirement(
            "Sign in",
            Some("Authentication"),
            &[serde_json::json!({
                "keyword": "given",
                "text": "a registered user"
            })],
            Some("space-1"),
            Some("must"),
        )
        .await
        .unwrap();

    // Then the created requirement is unwrapped from the API envelope
    assert_eq!(requirement.id, "req-1");
    assert_eq!(requirement.priority.as_deref(), Some("must"));
}

#[tokio::test]
async fn enrich_all_uses_the_bulk_endpoint() {
    // Given
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/chunks/enrich-all"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"enriched": 4, "failed": 0})),
        )
        .mount(&server)
        .await;

    // When
    let result = Client::new(server.uri()).enrich_all().await.unwrap();
    // Then
    assert_eq!(result["enriched"], 4);
}

#[tokio::test]
async fn document_import_sends_file_content_and_source_path() {
    // Given
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

    // When
    let imported = Client::new(server.uri())
        .import_document("/repo/README.md", "# Hello", Some("space-1"))
        .await
        .unwrap();
    // Then
    assert_eq!(imported["document"]["id"], "doc-1");
}

#[tokio::test]
async fn directory_import_sends_all_documents_in_one_request() {
    // Given a document import endpoint expecting two Markdown files and a space
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/documents/import-dir"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "files": [
                {"sourcePath": "/repo/README.md", "content": "# Readme"},
                {"sourcePath": "/repo/docs/guide.md", "content": "# Guide"}
            ],
            "spaceId": "space-1"
        })))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"document": {"id": "doc-1"}},
                {"document": {"id": "doc-2"}}
            ])),
        )
        .mount(&server)
        .await;

    // When the client imports the directory payload
    let results = Client::new(server.uri())
        .import_documents(
            &[
                ("/repo/README.md".into(), "# Readme".into()),
                ("/repo/docs/guide.md".into(), "# Guide".into()),
            ],
            Some("space-1"),
        )
        .await
        .unwrap();

    // Then both per-document results are returned
    assert_eq!(results.len(), 2);
    assert_eq!(results[1]["document"]["id"], "doc-2");
}

#[tokio::test]
async fn list_builds_the_expected_query_string() {
    // Given
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
    // When
    let chunks = client.list_chunks(Some("note"), None, 10).await.unwrap();
    // Then
    assert!(chunks.is_empty());
}

#[tokio::test]
async fn recent_chunk_listing_forwards_recap_filters() {
    // Given a recent-chunk endpoint scoped to one space
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/chunks"))
        .and(wiremock::matchers::query_param("after", "14"))
        .and(wiremock::matchers::query_param("sort", "updated"))
        .and(wiremock::matchers::query_param("limit", "100"))
        .and(wiremock::matchers::query_param("spaceId", "space-1"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "chunks": [{
                    "id": "chunk-1", "title": "Changed", "type": "note",
                    "createdAt": "2026-01-01T00:00:00Z",
                    "updatedAt": "2026-01-02T00:00:00Z"
                }],
                "total": 1, "limit": 100, "offset": 0
            })),
        )
        .mount(&server)
        .await;

    // When the recap's recent chunks are requested
    let page = Client::new(server.uri())
        .list_recent_chunks(14, Some("space-1"))
        .await
        .unwrap();

    // Then the typed chunk is returned
    assert_eq!(page.total, 1);
    assert_eq!(page.chunks.len(), 1);
    assert_eq!(page.chunks[0].id, "chunk-1");
}

#[tokio::test]
async fn prompt_search_uses_the_prompt_tag_and_single_result_limit() {
    // Given a prompt-tagged chunk search endpoint
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/chunks"))
        .and(wiremock::matchers::query_param("tags", "prompt"))
        .and(wiremock::matchers::query_param("search", "review"))
        .and(wiremock::matchers::query_param("limit", "1"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "chunks": [{
                    "id": "prompt-1", "title": "Review", "content": "Review this",
                    "type": "note", "updatedAt": "2026-01-02T00:00:00Z"
                }],
                "total": 1, "limit": 1, "offset": 0
            })),
        )
        .mount(&server)
        .await;

    // When a prompt is searched by title
    let chunks = Client::new(server.uri())
        .list_prompt_chunks(Some("review"))
        .await
        .unwrap();

    // Then only the matching prompt chunk is returned
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].id, "prompt-1");
}

#[tokio::test]
async fn applies_to_update_uses_the_bare_array_contract() {
    // Given an applies-to endpoint expecting the active bare-array contract
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("PUT"))
        .and(wiremock::matchers::path("/api/chunks/chunk-1/applies-to"))
        .and(wiremock::matchers::body_json(serde_json::json!([
            {"pattern": "src/**/*.rs"},
            {"pattern": "Cargo.toml"}
        ])))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    // When file scopes are assigned to a chunk
    let value = Client::new(server.uri())
        .set_applies_to("chunk-1", &["src/**/*.rs".into(), "Cargo.toml".into()])
        .await
        .unwrap();

    // Then the accepted API response is returned
    assert_eq!(value, serde_json::json!([]));
}

#[tokio::test]
async fn surfaces_server_errors_as_anyhow() {
    // Given
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let client = Client::new(server.uri());
    // When
    let err = client.list_chunks(None, None, 50).await.unwrap_err();
    // Then
    assert!(
        err.to_string().contains("401"),
        "error should mention the status: {err}"
    );
}

#[tokio::test]
async fn review_actions_use_the_proposal_endpoints() {
    // Given
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
    // When
    let proposal = client
        .review_proposal("p1", "approve", Some("looks good"))
        .await
        .unwrap();
    // Then
    assert_eq!(proposal.status, "approved");
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
    // Given
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

    // When
    let chunk = Client::new(server.uri()).get_chunk("c1").await.unwrap();
    // Then
    assert_eq!(chunk.title, "Enveloped");
}

#[tokio::test]
async fn update_sends_only_fields_the_user_supplied() {
    // Given
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
    // When
    let chunk = Client::new(server.uri())
        .update_chunk("c1", Some("Renamed"), None, None, Some(&tags), None)
        .await
        .unwrap();
    // Then
    assert_eq!(chunk.title, "Renamed");
}

#[tokio::test]
async fn update_rejects_an_empty_patch_without_making_a_request() {
    // Given
    let client = Client::new("http://127.0.0.1:1");
    // When
    let error = client
        .update_chunk("c1", None, None, None, None, None)
        .await
        .unwrap_err();
    // Then
    assert!(error.to_string().contains("nothing to update"));
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
