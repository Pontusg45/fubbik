use fubbik_cli::client::Client;

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
async fn filtered_list_paginates_with_stable_order_and_space() {
    // Given two pages in one space
    let server = wiremock::MockServer::start().await;
    for (offset, id) in [(0, "a"), (1, "b")] {
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/chunks"))
            .and(wiremock::matchers::query_param("type", "note"))
            .and(wiremock::matchers::query_param("spaceId", "space-1"))
            .and(wiremock::matchers::query_param("sort", "alpha"))
            .and(wiremock::matchers::query_param(
                "offset",
                offset.to_string(),
            ))
            .and(wiremock::matchers::query_param("limit", "1"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "chunks": [chunk_json(id, id)], "total": 2, "limit": 1, "offset": offset
                })),
            )
            .mount(&server)
            .await;
    }
    // When all pages are requested
    let chunks = Client::new(server.uri())
        .query_chunks(Some("note"), None, Some("space-1"), 1, 0, true)
        .await
        .unwrap();
    // Then both ordered results are returned
    assert_eq!(
        chunks
            .iter()
            .map(|chunk| chunk.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
}

#[tokio::test]
async fn filtered_search_respects_offset_without_fetching_another_page() {
    // Given a search page at a requested offset
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/chunks"))
        .and(wiremock::matchers::query_param("search", "rust"))
        .and(wiremock::matchers::query_param("offset", "5"))
        .and(wiremock::matchers::query_param("limit", "10"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "chunks": [chunk_json("sixth", "Rust")], "total": 6, "limit": 10, "offset": 5
            })),
        )
        .expect(1)
        .mount(&server)
        .await;
    // When one page is requested
    let chunks = Client::new(server.uri())
        .query_chunks(None, Some("rust"), None, 10, 5, false)
        .await
        .unwrap();
    // Then it returns just that page
    assert_eq!(chunks[0].id, "sixth");
    server.verify().await;
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
async fn markdown_chunk_import_sends_relative_paths_and_space() {
    // Given a chunk document import endpoint with two Markdown files
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/chunks/import-docs"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "files": [
                {"path": "one.md", "content": "# One"},
                {"path": "nested/two.md", "content": "# Two"}
            ],
            "spaceId": "space-1"
        })))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "created": 2, "skipped": 0, "connections": 0, "errors": []
            })),
        )
        .mount(&server)
        .await;

    // When the Markdown batch is imported
    let result = Client::new(server.uri())
        .import_chunk_documents(
            &[
                ("one.md".into(), "# One".into()),
                ("nested/two.md".into(), "# Two".into()),
            ],
            "space-1",
        )
        .await
        .unwrap();

    // Then the server import summary is returned
    assert_eq!(result["created"], 2);
}

#[tokio::test]
async fn chunk_page_forwards_export_pagination_and_scope() {
    // Given the second page of a space-scoped alphabetical chunk listing
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/chunks"))
        .and(wiremock::matchers::query_param("limit", "100"))
        .and(wiremock::matchers::query_param("offset", "100"))
        .and(wiremock::matchers::query_param("sort", "alpha"))
        .and(wiremock::matchers::query_param("spaceId", "space-1"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "chunks": [], "total": 100, "limit": 100, "offset": 100
            })),
        )
        .mount(&server)
        .await;

    // When the export helper requests that page
    let page = Client::new(server.uri())
        .list_chunk_page(Some("space-1"), 100)
        .await
        .unwrap();

    // Then the total and empty terminal page are preserved
    assert_eq!(page.total, 100);
    assert!(page.chunks.is_empty());
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
async fn discovered_chunks_are_created_as_ai_drafts() {
    // Given a chunk endpoint expecting setup discovery provenance
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/chunks"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "title": "Tech Stack",
            "content": "## Frameworks\n- react",
            "type": "reference",
            "tags": ["tech-stack", "react"],
            "spaceIds": ["space-1"],
            "origin": "ai"
        })))
        .respond_with(
            wiremock::ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "id": "chunk-1",
                "title": "Tech Stack",
                "content": "## Frameworks\n- react",
                "type": "reference",
                "updatedAt": "2026-09-25T10:00:00"
            })),
        )
        .mount(&server)
        .await;

    // When setup imports an automatically discovered chunk
    let chunk = Client::new(server.uri())
        .create_discovered_chunk(
            "Tech Stack",
            "## Frameworks\n- react",
            "reference",
            &["tech-stack".into(), "react".into()],
            "space-1",
        )
        .await
        .unwrap();

    // Then the created draft is returned
    assert_eq!(chunk.id, "chunk-1");
}
