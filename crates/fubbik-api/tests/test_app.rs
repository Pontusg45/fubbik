mod common;

use axum::http::StatusCode;
use common::TestApp;

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn signed_up_user_can_call_an_authenticated_route(pool: sqlx::PgPool) {
    // Given
    let app = TestApp::new(pool);
    let user = app.signup("test-app@example.com", "Test App").await;

    // When
    let response = app.get(&user, "/api/plans").await;

    // Then
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(TestApp::json(response).await, serde_json::json!([]));
}
