//! `GET /api/capabilities` and `fubbik.toml` route gating.
//!
//! Neither test touches the database: the pool is lazy and never connects,
//! because the capabilities endpoint and a 404 for an unmounted route both
//! answer without a query.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn app(features: fubbik_config::Features) -> axum::Router {
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
        features,
    })
}

async fn send(app: axum::Router, method: Method, path: &str) -> (StatusCode, Vec<u8>) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, body.to_vec())
}

#[tokio::test]
async fn capabilities_lists_enabled_features() {
    // Given
    let app = app(fubbik_config::Features::default());

    // When
    let (status, body) = send(app, Method::GET, "/api/capabilities").await;

    // Then
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "features": [
                "plans",
                "requirements",
                "matrices",
                "coordination",
                "learning_paths",
                "overlays",
                "ai"
            ]
        })
    );
}

#[tokio::test]
async fn disabled_group_routes_are_not_mounted() {
    // Given
    let features = fubbik_config::Config::from_toml_str("[features]\nai = false\n")
        .unwrap()
        .features;

    // When
    let (caps_status, caps_body) =
        send(app(features.clone()), Method::GET, "/api/capabilities").await;
    let (ai_status, _) = send(app(features), Method::POST, "/api/ai/summarize").await;

    // Then
    assert_eq!(caps_status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&caps_body).unwrap();
    let keys: Vec<&str> = json["features"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(!keys.contains(&"ai"), "ai must be omitted: {keys:?}");
    assert_eq!(keys.len(), 6);
    assert_eq!(ai_status, StatusCode::NOT_FOUND);
}
