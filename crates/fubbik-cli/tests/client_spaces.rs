use fubbik_cli::client::Client;

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
