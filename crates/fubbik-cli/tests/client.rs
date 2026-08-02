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
