use fubbik_cli::client::{Client, Space};
use fubbik_cli::setup::{discover, import};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

#[test]
fn backend_only_project_exposes_routing_conventions() {
    // Given
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("package.json"),
        r#"{"dependencies":{"express":"5.0.0"}}"#,
    )
    .unwrap();
    std::fs::create_dir(root.path().join("routes")).unwrap();

    // When
    let discovery = discover(root.path()).unwrap();

    // Then
    assert!(discovery.chunks.iter().any(|chunk| {
        chunk.title == "Routing Conventions" && chunk.tags.contains(&"routing".into())
    }));
    assert!(discovery.chunks.iter().all(|chunk| chunk.tier > 1));
}

#[tokio::test]
async fn documentation_discovery_imports_through_the_client_interface() {
    // Given
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("README.md"), "# Knowledge\nDetails.").unwrap();
    let discovery = discover(root.path()).unwrap();
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .and(matchers::path("/api/chunks/import-docs"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "created": 1, "errors": []
        })))
        .mount(&server)
        .await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/api/chunks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "chunks": [], "total": 0
        })))
        .mount(&server)
        .await;
    let space = Space {
        id: "space-1".into(),
        name: "Project".into(),
        kind: "codebase".into(),
        description: None,
    };

    // When
    let report = import(&Client::new(server.uri()), &space, &discovery).await;

    // Then
    assert_eq!(report.chunks_created, 1);
    assert!(report.errors.is_empty());
}
