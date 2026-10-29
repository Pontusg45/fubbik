//! `GET /api/openapi.json` and `GET /api/docs` — the machine-readable spec and a
//! Swagger UI page that renders it.
//!
//! **Deliberately unauthenticated**, like `capabilities`: the spec describes
//! only route shapes, and the page is a static shell. Swagger UI itself is
//! loaded from the jsdelivr CDN at a pinned version, so the server carries no
//! frontend assets.

use std::sync::LazyLock;

use axum::Router;
use axum::http::header;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use utoipa::OpenApi;

use crate::AppState;

/// The one place the `swagger-ui-dist` version is pinned.
pub const SWAGGER_UI_VERSION: &str = "5.17.14";

static SPEC_JSON: LazyLock<String> = LazyLock::new(|| {
    serde_json::to_string(&crate::openapi::ApiDoc::openapi())
        .expect("the OpenAPI document serializes")
});

static DOCS_HTML: LazyLock<String> =
    LazyLock::new(|| DOCS_TEMPLATE.replace("{{VERSION}}", SWAGGER_UI_VERSION));

const DOCS_TEMPLATE: &str = r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Fubbik API</title>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/swagger-ui-dist@{{VERSION}}/swagger-ui.css">
</head>
<body>
<p style="margin: 8px 16px; font-family: sans-serif"><a href="/api/openapi.json">Raw OpenAPI JSON</a></p>
<div id="swagger-ui"></div>
<script src="https://cdn.jsdelivr.net/npm/swagger-ui-dist@{{VERSION}}/swagger-ui-bundle.js"></script>
<script>
window.onload = () => {
  window.ui = SwaggerUIBundle({ url: "/api/openapi.json", dom_id: "#swagger-ui" });
};
</script>
</body>
</html>
"##;

#[utoipa::path(get, path = "/api/openapi.json",
    responses((status = 200, description = "This API's OpenAPI document", content_type = "application/json")))]
pub async fn openapi_json() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json")],
        SPEC_JSON.as_str(),
    )
}

pub async fn docs() -> Html<&'static str> {
    Html(DOCS_HTML.as_str())
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/openapi.json", get(openapi_json))
        .route("/api/docs", get(docs))
}
