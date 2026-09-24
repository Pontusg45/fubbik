mod common;

use axum::http::StatusCode;
use common::TestApp;

fn steps() -> serde_json::Value {
    serde_json::json!([
        {"keyword": "given", "text": "a signed-in user"},
        {"keyword": "when", "text": "they submit the form"},
        {"keyword": "then", "text": "the action succeeds"}
    ])
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn suggestion_context_aggregates_and_filters_owned_knowledge(pool: sqlx::PgPool) {
    // Given a use case, grouped and ungrouped requirements, and matching chunks
    let app = TestApp::new(pool);
    let alice = app.signup("alice-suggestions@example.com", "Alice").await;
    let bob = app.signup("bob-suggestions@example.com", "Bob").await;

    let use_case_response = app
        .post(
            &alice,
            "/api/use-cases",
            serde_json::json!({"name": "Authentication"}),
        )
        .await;
    assert_eq!(use_case_response.status(), StatusCode::CREATED);
    let use_case_id = TestApp::json(use_case_response).await["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let grouped = app
        .post(
            &alice,
            "/api/requirements",
            serde_json::json!({
                "title": "Authentication requires MFA",
                "steps": steps(),
                "useCaseId": use_case_id
            }),
        )
        .await;
    assert_eq!(grouped.status(), StatusCode::CREATED);
    let ungrouped = app
        .post(
            &alice,
            "/api/requirements",
            serde_json::json!({"title": "Authentication audit trail", "steps": steps()}),
        )
        .await;
    assert_eq!(ungrouped.status(), StatusCode::CREATED);
    let unrelated = app
        .post(
            &alice,
            "/api/requirements",
            serde_json::json!({"title": "Billing receipt", "steps": steps()}),
        )
        .await;
    assert_eq!(unrelated.status(), StatusCode::CREATED);

    let chunk = app
        .post(
            &alice,
            "/api/chunks",
            serde_json::json!({
                "title": "Authentication policy",
                "content": "MFA is required for administrator accounts."
            }),
        )
        .await;
    assert_eq!(chunk.status(), StatusCode::CREATED);
    let hidden = app
        .post(
            &bob,
            "/api/chunks",
            serde_json::json!({"title": "Authentication secret", "content": "private"}),
        )
        .await;
    assert_eq!(hidden.status(), StatusCode::CREATED);

    // When Alice requests focused suggestion context
    let response = app
        .get(
            &alice,
            "/api/requirements/suggest-context?focus=authentication",
        )
        .await;

    // Then the response combines only Alice's matching requirement and chunk context
    assert_eq!(response.status(), StatusCode::OK);
    let body = TestApp::json(response).await;
    assert_eq!(body["useCases"][0]["name"], "Authentication");
    assert_eq!(
        body["useCases"][0]["requirements"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(body["ungroupedRequirements"].as_array().unwrap().len(), 1);
    assert_eq!(
        body["ungroupedRequirements"][0]["title"],
        "Authentication audit trail"
    );
    assert_eq!(body["coverageGaps"].as_array().unwrap().len(), 1);
    assert_eq!(body["coverageGaps"][0]["title"], "Authentication policy");
    assert_eq!(body["relevantChunks"].as_array().unwrap().len(), 1);
    assert_eq!(body["relevantChunks"][0]["title"], "Authentication policy");
    assert_eq!(body["healthIssueCounts"]["orphan"], 1);
    assert_eq!(body["healthIssueCounts"]["thin"], 1);
}
