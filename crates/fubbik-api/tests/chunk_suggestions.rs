mod common;

use axum::http::StatusCode;
use common::{TestApp, TestUser};

async fn create_chunk(app: &TestApp, user: &TestUser, title: &str, tags: &[&str]) -> String {
    let response = app
        .post(
            user,
            "/api/chunks",
            serde_json::json!({ "title": title, "content": "body", "tags": tags }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    TestApp::json(response).await["id"]
        .as_str()
        .expect("created chunk has an id")
        .to_owned()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn suggestions_prioritize_shared_tags_then_similar_titles(pool: sqlx::PgPool) {
    // Given
    let app = TestApp::new(pool);
    let owner = app.signup("suggestions@example.com", "Owner").await;
    let outsider = app.signup("suggestions-other@example.com", "Other").await;

    let target = create_chunk(&app, &owner, "Authentication flow", &["auth", "security"]).await;
    let tag_match = create_chunk(&app, &owner, "Database migration", &["auth", "security"]).await;
    let title_match = create_chunk(&app, &owner, "Authentication flows", &[]).await;
    create_chunk(
        &app,
        &outsider,
        "Authentication flow guide",
        &["auth", "security"],
    )
    .await;

    // When
    let response = app
        .get(&owner, &format!("/api/chunks/{target}/suggestions"))
        .await;
    // Then
    assert_eq!(response.status(), StatusCode::OK);
    let suggestions = TestApp::json(response).await;
    let suggestions = suggestions.as_array().expect("suggestions are an array");

    assert_eq!(suggestions[0]["id"], tag_match);
    assert_eq!(suggestions[0]["reason"], "shares 2 tags");
    assert_eq!(suggestions[1]["id"], title_match);
    assert_eq!(suggestions[1]["reason"], "similar title");
    assert_eq!(
        suggestions.len(),
        2,
        "the target and another user's chunks stay excluded"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn suggestions_require_an_owned_chunk(pool: sqlx::PgPool) {
    // Given
    let app = TestApp::new(pool);
    let owner = app.signup("suggestions-missing@example.com", "Owner").await;

    // When
    let response = app.get(&owner, "/api/chunks/missing/suggestions").await;
    // Then
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
