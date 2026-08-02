use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../../apps/web/dist/"]
struct Assets;

/// Serves an embedded asset, falling back to index.html so client-side
/// routes resolve. API paths are excluded: an unmatched /api/* must 404
/// rather than return HTML.
pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if uri.path().starts_with("/api/") {
        return StatusCode::NOT_FOUND.into_response();
    }

    if let Some(file) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return ([(header::CONTENT_TYPE, mime.as_ref())], file.data).into_response();
    }

    match Assets::get("index.html") {
        Some(index) => ([(header::CONTENT_TYPE, "text/html")], index.data).into_response(),
        // A binary built without the web app still serves the API.
        None => (StatusCode::NOT_FOUND, "web UI not bundled").into_response(),
    }
}
