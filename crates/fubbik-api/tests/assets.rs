use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unknown_api_path_is_404_not_spa_fallback(pool: sqlx::PgPool) {
    let app = fubbik_api::router(fubbik_api::AppState {
        pool,
        implicit_dev_session: true,
        better_auth_secret: "test-secret".into(),
        ai: fubbik_ai::OllamaClient::new("http://127.0.0.1:1"),
    });

    let res = app
        .oneshot(
            Request::get("/api/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // The SPA fallback must never swallow unmatched API routes — doing so
    // returns HTML to a fetch() caller and produces confusing parse errors.
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
