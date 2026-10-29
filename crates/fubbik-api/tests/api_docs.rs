//! `GET /api/openapi.json` and `GET /api/docs`. Neither touches the database
//! or needs a session, so the pool is lazy and never connects.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use utoipa::OpenApi;

fn app() -> axum::Router {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused@127.0.0.1:1/unused")
        .expect("lazy pool needs only a parseable URL");
    fubbik_api::router(fubbik_api::AppState {
        pool,
        implicit_dev_session: false,
        better_auth_secret: "test-secret".into(),
        ai: fubbik_ai::OllamaClient::new("http://127.0.0.1:1"),
        rate_limiter: Default::default(),
        background: Default::default(),
        features: Default::default(),
    })
}

async fn get(path: &str) -> (StatusCode, String, Vec<u8>) {
    let request = Request::builder().uri(path).body(Body::empty()).unwrap();
    let response = app().oneshot(request).await.unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, content_type, body.to_vec())
}

#[tokio::test]
async fn openapi_json_matches_generated_document() {
    // Given
    let expected = serde_json::to_value(fubbik_api::openapi::ApiDoc::openapi()).unwrap();

    // When
    let (status, content_type, body) = get("/api/openapi.json").await;

    // Then
    assert_eq!(status, StatusCode::OK);
    assert!(
        content_type.starts_with("application/json"),
        "{content_type}"
    );
    let served: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(served, expected);
}

#[tokio::test]
async fn docs_page_serves_swagger_ui() {
    // Given
    let version = fubbik_api::api_docs::SWAGGER_UI_VERSION;

    // When
    let (status, content_type, body) = get("/api/docs").await;

    // Then
    assert_eq!(status, StatusCode::OK);
    assert!(content_type.starts_with("text/html"), "{content_type}");
    let html = String::from_utf8(body).unwrap();
    assert!(html.contains(&format!(
        "https://cdn.jsdelivr.net/npm/swagger-ui-dist@{version}/swagger-ui.css"
    )));
    assert!(html.contains(&format!(
        "https://cdn.jsdelivr.net/npm/swagger-ui-dist@{version}/swagger-ui-bundle.js"
    )));
    assert!(
        html.contains(r##"SwaggerUIBundle({ url: "/api/openapi.json", dom_id: "#swagger-ui" })"##)
    );
}

#[tokio::test]
async fn docs_page_links_raw_spec() {
    // Given / When
    let (_, _, body) = get("/api/docs").await;

    // Then
    let html = String::from_utf8(body).unwrap();
    assert!(html.contains(r#"<a href="/api/openapi.json">Raw OpenAPI JSON</a>"#));
}

#[tokio::test]
async fn spa_docs_path_is_not_served_by_api() {
    // Given: the web app owns `/docs`; Swagger UI lives under `/api/docs`.

    // When
    let (status, _, _) = get("/docs").await;

    // Then
    assert_eq!(status, StatusCode::NOT_FOUND);
}
