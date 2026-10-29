use fubbik_cli::capabilities::{disabled_message, enabled_features, explain_not_found};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

async fn capabilities(server: &MockServer, template: ResponseTemplate) {
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/capabilities"))
        .respond_with(template)
        .mount(server)
        .await;
}

#[tokio::test]
async fn enabled_features_reads_the_capability_list() {
    // Given a server reporting two enabled features
    let server = MockServer::start().await;
    capabilities(
        &server,
        ResponseTemplate::new(200).set_body_json(serde_json::json!({"features": ["plans", "ai"]})),
    )
    .await;
    // When the CLI asks
    let features = enabled_features(&server.uri()).await;
    // Then it gets exactly those keys
    assert_eq!(features, Some(vec!["plans".into(), "ai".into()]));
}

#[tokio::test]
async fn unknown_capabilities_leave_the_original_error_unchanged() {
    // Given an older server without /api/capabilities
    let server = MockServer::start().await;
    capabilities(&server, ResponseTemplate::new(404)).await;
    let original = anyhow::anyhow!("GET /api/plans failed with 404 Not Found");
    // When a 404 is explained
    let error = explain_not_found(&server.uri(), "plans", original).await;
    // Then the original error comes back
    assert_eq!(
        error.to_string(),
        "GET /api/plans failed with 404 Not Found"
    );
}

#[tokio::test]
async fn enabled_feature_keeps_the_original_404() {
    // Given a server where plans are enabled (so the 404 is a real missing plan)
    let server = MockServer::start().await;
    capabilities(
        &server,
        ResponseTemplate::new(200).set_body_json(serde_json::json!({"features": ["plans"]})),
    )
    .await;
    // When the 404 is explained
    let error = explain_not_found(
        &server.uri(),
        "plans",
        anyhow::anyhow!("GET /api/plans/x failed with 404 Not Found"),
    )
    .await;
    // Then it is reported unchanged
    assert!(error.to_string().contains("/api/plans/x"));
}

#[test]
fn disabled_message_names_the_feature() {
    // Given a feature key
    // When the message is built
    // Then it matches the documented wording
    assert_eq!(
        disabled_message("matrices"),
        "feature \"matrices\" is disabled on this server"
    );
}
