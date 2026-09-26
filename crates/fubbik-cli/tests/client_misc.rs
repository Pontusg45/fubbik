use fubbik_cli::client::Client;

#[tokio::test]
async fn knowledge_health_forwards_the_lint_space_scope() {
    // Given a space-scoped knowledge health endpoint
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/health/knowledge"))
        .and(wiremock::matchers::query_param("spaceId", "space-1"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "orphans":{"chunks":[],"total":0}, "stale":{"chunks":[],"total":0}
            })),
        )
        .mount(&server)
        .await;

    // When lint requests health for that space
    let health = Client::new(server.uri())
        .knowledge_health(Some("space-1"))
        .await
        .unwrap();

    // Then the scoped report is returned
    assert_eq!(health["orphans"]["total"], 0);
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
