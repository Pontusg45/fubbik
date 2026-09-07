mod common;

use axum::http::StatusCode;
use common::TestApp;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn app_with_response(pool: sqlx::PgPool, response: &str) -> (TestApp, MockServer) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "response": response })),
        )
        .mount(&server)
        .await;
    let app = TestApp::with_ai(pool, fubbik_ai::OllamaClient::new(server.uri()));
    (app, server)
}

async fn create_chunk(
    app: &TestApp,
    user: &common::TestUser,
    title: &str,
    content: &str,
) -> String {
    let response = app
        .post(
            user,
            "/api/chunks",
            serde_json::json!({ "title": title, "content": content, "type": "note" }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    TestApp::json(response).await["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn summarize_uses_owned_chunk_content(pool: sqlx::PgPool) {
    let (app, server) = app_with_response(pool, "A concise summary.").await;
    let user = app.signup("summary@example.test", "Summary").await;
    let id = create_chunk(&app, &user, "Retries", "Retry transient failures.").await;

    let response = app
        .post(
            &user,
            "/api/ai/summarize",
            serde_json::json!({ "chunkId": id }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        TestApp::json(response).await,
        serde_json::json!({ "summary": "A concise summary." })
    );

    let requests = server.received_requests().await.unwrap();
    let request = requests
        .iter()
        .find(|request| request.url.path() == "/api/generate")
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(body["model"], "llama3.2");
    assert_eq!(body["stream"], false);
    assert!(body["prompt"].as_str().unwrap().contains("Title: Retries"));
    assert!(
        body["prompt"]
            .as_str()
            .unwrap()
            .contains("Content: Retry transient failures.")
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn chunk_ai_routes_do_not_cross_tenant_boundaries(pool: sqlx::PgPool) {
    let (app, server) = app_with_response(pool, "unused").await;
    let owner = app.signup("owner-ai@example.test", "Owner").await;
    let stranger = app.signup("stranger-ai@example.test", "Stranger").await;
    let id = create_chunk(&app, &owner, "Private", "Secret").await;

    for endpoint in ["summarize", "suggest-connections"] {
        let response = app
            .post(
                &stranger,
                &format!("/api/ai/{endpoint}"),
                serde_json::json!({ "chunkId": id }),
            )
            .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn suggest_connections_only_exposes_the_callers_chunks(pool: sqlx::PgPool) {
    let response = r#"[{"id":"candidate","relation":"supports"}]"#;
    let (app, server) = app_with_response(pool, response).await;
    let user = app.signup("suggest@example.test", "Suggest").await;
    let other = app.signup("other-suggest@example.test", "Other").await;
    let target = create_chunk(&app, &user, "Target", "Target content").await;
    let candidate = create_chunk(&app, &user, "Candidate", "Candidate content").await;
    create_chunk(&app, &other, "Foreign", "Must stay hidden").await;

    let response = app
        .post(
            &user,
            "/api/ai/suggest-connections",
            serde_json::json!({ "chunkId": target }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        TestApp::json(response).await,
        serde_json::json!([{ "id": "candidate", "relation": "supports" }])
    );

    let requests = server.received_requests().await.unwrap();
    let request = requests
        .iter()
        .find(|request| request.url.path() == "/api/generate")
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
    let prompt = body["prompt"].as_str().unwrap();
    assert!(prompt.contains(&format!("- {candidate}: Candidate")));
    assert!(!prompt.contains("Foreign"));
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn generate_preserves_json_and_legacy_invalid_json_fallback(pool: sqlx::PgPool) {
    let generated =
        r#"{"title":"Runbooks","content":"Write them.","type":"document","tags":["ops"]}"#;
    let (app, _server) = app_with_response(pool.clone(), generated).await;
    let user = app.signup("generate@example.test", "Generate").await;
    let response = app
        .post(
            &user,
            "/api/ai/generate",
            serde_json::json!({ "prompt": "runbook" }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        TestApp::json(response).await,
        serde_json::json!({
            "title": "Runbooks",
            "content": "Write them.",
            "type": "document",
            "tags": ["ops"]
        })
    );

    let (fallback_app, _fallback_server) = app_with_response(pool, "plain model output").await;
    let response = fallback_app
        .post(
            &user,
            "/api/ai/generate",
            serde_json::json!({ "prompt": "fallback title" }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        TestApp::json(response).await,
        serde_json::json!({
            "title": "fallback title",
            "content": "plain model output",
            "type": "note",
            "tags": []
        })
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn structure_requirement_filters_invalid_steps_and_accepts_space_id(pool: sqlx::PgPool) {
    let structured = r#"{"steps":[{"keyword":"given","text":"a user"},{"keyword":"oops","text":"drop"},{"keyword":"then","text":"it works"}]}"#;
    let (app, _server) = app_with_response(pool, structured).await;
    let user = app.signup("structure@example.test", "Structure").await;
    let response = app
        .post(
            &user,
            "/api/ai/structure-requirement",
            serde_json::json!({ "description": "It should work", "spaceId": null }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        TestApp::json(response).await,
        serde_json::json!({ "steps": [
            { "keyword": "given", "text": "a user" },
            { "keyword": "then", "text": "it works" }
        ] })
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn ai_routes_require_authentication_and_validate_description_length(pool: sqlx::PgPool) {
    let (app, _) = app_with_response(pool, r#"{"steps":[]}"#).await;
    let response = app
        .request(
            None,
            axum::http::Method::POST,
            "/api/ai/generate",
            Some(serde_json::json!({ "prompt": "anything" })),
        )
        .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let user = app.signup("validation@example.test", "Validation").await;
    let response = app
        .post(
            &user,
            "/api/ai/structure-requirement",
            serde_json::json!({ "description": "x".repeat(5001) }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
