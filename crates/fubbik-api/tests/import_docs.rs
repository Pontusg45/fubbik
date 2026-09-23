mod common;

use axum::http::StatusCode;
use common::TestApp;

async fn create_space(app: &TestApp, user: &common::TestUser) -> String {
    let response = app
        .post(
            user,
            "/api/spaces",
            serde_json::json!({ "name": "Docs", "kind": "wiki" }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    TestApp::json(response).await["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn batch_import_is_idempotent_and_connects_folder_children_to_index(pool: sqlx::PgPool) {
    // Given
    let app = TestApp::new(pool.clone());
    let user = app.signup("docs-import@example.test", "Importer").await;
    let space_id = create_space(&app, &user).await;
    let body = serde_json::json!({
        "spaceId": space_id,
        "files": [
            { "path": "guide/index.md", "content": "# Guide\n\nOverview." },
            { "path": "guide/setup.md", "content": "# Setup\n\nInstall it." }
        ]
    });

    // When
    let response = app
        .post(&user, "/api/chunks/import-docs", body.clone())
        .await;
    // Then
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        TestApp::json(response).await,
        serde_json::json!({ "created": 2, "skipped": 0, "connections": 1, "errors": [] })
    );

    let response = app.post(&user, "/api/chunks/import-docs", body).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(TestApp::json(response).await["skipped"], 2);
    let count = sqlx::query_scalar!("SELECT COUNT(*) AS \"count!\" FROM chunk_connection")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn preview_matches_templates_and_override_extracts_fields(pool: sqlx::PgPool) {
    // Given
    let app = TestApp::new(pool.clone());
    let user = app.signup("template-import@example.test", "Template").await;
    let space_id = create_space(&app, &user).await;
    // When
    let template = app
        .post(
            &user,
            "/api/templates",
            serde_json::json!({
                "name": "Decision",
                "type": "reference",
                "content": "",
                "priority": 10,
                "tags": ["decision"],
                "matchRules": {
                    "minScore": 1,
                    "headings": [{ "patterns": ["Rationale"], "match": "exact", "level": 2, "required": true }],
                    "frontmatter": []
                },
                "fieldMappings": [{ "headings": ["Rationale"], "match": "exact", "target": "rationale" }]
            }),
        )
        .await;
    // Then
    assert_eq!(template.status(), StatusCode::CREATED);
    let template_id = TestApp::json(template).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let file = serde_json::json!({ "path": "adr/cache.md", "content": "# Cache policy\n\nIntro.\n\n## Rationale\nFast reads." });

    let preview = app
        .post(
            &user,
            "/api/chunks/import-docs/preview",
            serde_json::json!({ "spaceId": space_id, "files": [file.clone()] }),
        )
        .await;
    assert_eq!(preview.status(), StatusCode::OK);
    let preview = TestApp::json(preview).await;
    assert_eq!(preview["files"][0]["suggestedTemplate"]["id"], template_id);
    assert_eq!(
        preview["files"][0]["suggestedTemplate"]["extractedFields"]["rationale"],
        "Fast reads."
    );

    let response = app
        .post(
            &user,
            "/api/chunks/import-docs",
            serde_json::json!({
                "spaceId": space_id,
                "files": [file],
                "templateOverrides": { "adr/cache.md": template_id }
            }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let chunk = sqlx::query!(
        "SELECT title, type, rationale, content, origin, review_status FROM chunk LIMIT 1"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(chunk.title, "Cache policy");
    assert_eq!(chunk.r#type, "reference");
    assert_eq!(chunk.rationale.as_deref(), Some("Fast reads."));
    assert_eq!(chunk.origin, "ai");
    assert_eq!(chunk.review_status, "approved");
    assert!(!chunk.content.contains("## Rationale"));
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn stream_emits_file_and_done_events(pool: sqlx::PgPool) {
    // Given
    let app = TestApp::new(pool);
    let user = app.signup("stream-import@example.test", "Stream").await;
    let space_id = create_space(&app, &user).await;
    // When
    let response = app
        .post(
            &user,
            "/api/chunks/import-docs/stream",
            serde_json::json!({
                "spaceId": space_id,
                "files": [{ "path": "readme.md", "content": "# Home\n\nHello." }]
            }),
        )
        .await;
    // Then
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    let bytes = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let events = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(events.contains("event: file"));
    assert!(events.contains("\"status\":\"importing\""));
    assert!(events.contains("\"status\":\"created\""));
    assert!(events.contains("event: done"));
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn import_rejects_foreign_spaces_before_writing(pool: sqlx::PgPool) {
    // Given
    let app = TestApp::new(pool.clone());
    let owner = app.signup("space-owner@example.test", "Owner").await;
    let stranger = app.signup("space-stranger@example.test", "Stranger").await;
    let space_id = create_space(&app, &owner).await;
    // When
    let response = app
        .post(
            &stranger,
            "/api/chunks/import-docs",
            serde_json::json!({
                "spaceId": space_id,
                "files": [{ "path": "secret.md", "content": "# Secret" }]
            }),
        )
        .await;
    // Then
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        sqlx::query_scalar!("SELECT COUNT(*) AS \"count!\" FROM document")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn import_rate_limit_is_shared_by_batch_and_stream(pool: sqlx::PgPool) {
    // Given
    let app = TestApp::new(pool);
    let user = app.signup("rate-import@example.test", "Rate").await;
    let space_id = create_space(&app, &user).await;
    for attempt in 0..6 {
        let path = if attempt == 5 {
            "/api/chunks/import-docs/stream"
        } else {
            "/api/chunks/import-docs"
        };
        // When
        let response = app
            .post(
                &user,
                path,
                serde_json::json!({ "spaceId": space_id, "files": [] }),
            )
            .await;
        // Then
        assert_eq!(
            response.status(),
            if attempt < 5 {
                StatusCode::OK
            } else {
                StatusCode::TOO_MANY_REQUESTS
            }
        );
    }
}
