use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{Method, StatusCode, header};
use axum::response::Response;
use percent_encoding::percent_decode_str;

/// Attach static file serving and browser history fallback to an API router.
/// Explicit API routes continue to take precedence over this fallback.
pub fn router(root: impl Into<PathBuf>) -> Router {
    let root = Arc::new(root.into());
    Router::new().fallback(move |request: Request| {
        let root = Arc::clone(&root);
        async move { serve(root.as_ref(), request).await }
    })
}

async fn serve(root: &Path, request: Request) -> Response {
    let path = request.uri().path();
    let Ok(decoded) = percent_decode_str(path).decode_utf8() else {
        return status(StatusCode::BAD_REQUEST);
    };
    if decoded == "/api" || decoded.starts_with("/api/") {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"Not found\"}"))
            .expect("static JSON 404 response");
    }
    if request.method() != Method::GET && request.method() != Method::HEAD {
        return status(StatusCode::METHOD_NOT_ALLOWED);
    }
    let relative = decoded.trim_start_matches('/');
    let parsed = Path::new(relative);
    if relative.contains('\\')
        || relative.contains('\0')
        || parsed.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        || relative
            .split('/')
            .any(|segment| segment == "." || segment.starts_with('.'))
    {
        return status(StatusCode::BAD_REQUEST);
    }

    let candidate = root.join(parsed);
    let file = if candidate.is_file() {
        candidate
    } else if accepts_html(&request)
        && !relative.starts_with("assets/")
        && !relative.rsplit('/').next().unwrap_or("").contains('.')
    {
        root.join("index.html")
    } else {
        return status(StatusCode::NOT_FOUND);
    };
    let Ok(root_real) = tokio::fs::canonicalize(root).await else {
        return status(StatusCode::NOT_FOUND);
    };
    let Ok(file_real) = tokio::fs::canonicalize(&file).await else {
        return status(StatusCode::NOT_FOUND);
    };
    if !file_real.starts_with(&root_real) {
        return status(StatusCode::BAD_REQUEST);
    }
    let Ok(metadata) = tokio::fs::metadata(&file_real).await else {
        return status(StatusCode::NOT_FOUND);
    };
    if !metadata.is_file() {
        return status(StatusCode::NOT_FOUND);
    }
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let etag = format!("W/\"{}-{modified}\"", metadata.len());
    let cache = if decoded.starts_with("/assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    if request
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        == Some(etag.as_str())
    {
        return Response::builder()
            .status(StatusCode::NOT_MODIFIED)
            .header(header::ETAG, etag)
            .header(header::CACHE_CONTROL, cache)
            .body(Body::empty())
            .expect("conditional response");
    }

    let body = if request.method() == Method::HEAD {
        Body::empty()
    } else {
        match tokio::fs::read(&file_real).await {
            Ok(bytes) => Body::from(bytes),
            Err(_) => return status(StatusCode::NOT_FOUND),
        }
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type(&file_real))
        .header(header::CACHE_CONTROL, cache)
        .header(header::ETAG, etag)
        .header(header::CONTENT_LENGTH, metadata.len().to_string())
        .body(body)
        .expect("asset response")
}

fn accepts_html(request: &Request) -> bool {
    request
        .headers()
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("text/html") || value.contains("application/xhtml+xml"))
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn status(code: StatusCode) -> Response {
    Response::builder()
        .status(code)
        .body(Body::empty())
        .expect("status response")
}
