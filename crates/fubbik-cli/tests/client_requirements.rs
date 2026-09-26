use fubbik_cli::client::Client;

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
