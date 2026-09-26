use fubbik_cli::client::Client;

#[tokio::test]
async fn matrix_creation_forwards_scope_and_optional_fields() {
    // Given a matrix endpoint expecting the complete creation contract
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/matrices"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "name": "HTTP contracts",
            "layer": "contract",
            "description": "Public API rules",
            "spaceId": "space-1"
        })))
        .respond_with(
            wiremock::ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "id": "matrix-1",
                "name": "HTTP contracts",
                "layer": "contract",
                "description": "Public API rules"
            })),
        )
        .mount(&server)
        .await;

    // When a scoped matrix is created
    let matrix = Client::new(server.uri())
        .create_matrix(
            "HTTP contracts",
            "contract",
            Some("Public API rules"),
            Some("space-1"),
        )
        .await
        .unwrap();

    // Then the API response is decoded into the CLI model
    assert_eq!(matrix.id, "matrix-1");
    assert_eq!(matrix.layer, "contract");
}
