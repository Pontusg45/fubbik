use fubbik_cli::client::Client;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

#[tokio::test]
async fn a_failed_get_reports_method_path_status_request_id_and_message() {
    // Given
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/health"))
        .respond_with(
            ResponseTemplate::new(503)
                .insert_header("x-request-id", "request-42")
                .set_body_json(serde_json::json!({"message": "database unavailable"})),
        )
        .mount(&server)
        .await;

    // When
    let error = Client::new(server.uri())
        .health()
        .await
        .unwrap_err()
        .to_string();

    // Then
    assert!(error.contains("GET /api/health"), "{error}");
    assert!(error.contains("503"), "{error}");
    assert!(error.contains("request-42"), "{error}");
    assert!(error.contains("database unavailable"), "{error}");
}

#[tokio::test]
async fn a_failed_mutation_decodes_error_and_normalizes_base_url() {
    // Given
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/tags"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": "name is required"
        })))
        .mount(&server)
        .await;
    let client = Client::new(format!("{}/", server.uri()));

    // When
    let error = client.create_tag("", None).await.unwrap_err().to_string();

    // Then
    assert_eq!(client.base_url(), server.uri());
    assert!(error.contains("POST /api/tags"), "{error}");
    assert!(error.contains("name is required"), "{error}");
}

#[tokio::test]
async fn chunk_collector_fetches_every_page_in_order() {
    // Given
    let server = MockServer::start().await;
    for (offset, id) in [(0, "chunk-1"), (1, "chunk-2")] {
        Mock::given(matchers::method("GET"))
            .and(matchers::path("/api/chunks"))
            .and(matchers::query_param("offset", offset.to_string()))
            .and(matchers::query_param("spaceId", "space-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "chunks": [{
                    "id": id, "title": id, "type": "note", "content": "body",
                    "updatedAt": "2026-09-26T12:00:00.000Z"
                }],
                "total": 2
            })))
            .mount(&server)
            .await;
    }

    // When
    let chunks = Client::new(server.uri())
        .list_all_chunks(Some("space-1"))
        .await
        .unwrap();

    // Then
    assert_eq!(
        chunks
            .iter()
            .map(|chunk| chunk.id.as_str())
            .collect::<Vec<_>>(),
        ["chunk-1", "chunk-2"]
    );
}

#[tokio::test]
async fn chunk_collector_rejects_a_truncated_page() {
    // Given
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/chunks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "chunks": [], "total": 2
        })))
        .mount(&server)
        .await;

    // When
    let error = Client::new(server.uri())
        .list_all_chunks(None)
        .await
        .unwrap_err()
        .to_string();

    // Then
    assert!(error.contains("ended before the reported total"), "{error}");
}
