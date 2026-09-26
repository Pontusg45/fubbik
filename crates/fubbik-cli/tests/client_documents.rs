use fubbik_cli::client::Client;

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
