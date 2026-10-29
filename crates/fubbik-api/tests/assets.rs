use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use fubbik_api::assets;
use http_body_util::BodyExt;
use tempfile::TempDir;
use tower::ServiceExt;

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unknown_api_path_is_404_not_spa_fallback(pool: sqlx::PgPool) {
    // Given
    let root = fixture();
    let app = fubbik_api::router(fubbik_api::AppState {
        pool,
        implicit_dev_session: true,
        better_auth_secret: "test-secret".into(),
        ai: fubbik_ai::OllamaClient::new("http://127.0.0.1:1"),
        rate_limiter: Default::default(),
        features: Default::default(),
        background: Default::default(),
    })
    .merge(assets::router(root.path()));

    // When
    let res = app
        .oneshot(
            Request::get("/api/does-not-exist")
                .header("accept", "text/html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Then
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "application/json");
}

fn fixture() -> TempDir {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("assets")).unwrap();
    std::fs::write(root.path().join("index.html"), "<main>Fubbik</main>").unwrap();
    std::fs::write(root.path().join("assets/app-123.js"), "console.log('ok')").unwrap();
    root
}

async fn request(
    root: &TempDir,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
) -> axum::response::Response {
    let mut builder = Request::builder().method(method).uri(path);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    assets::router(root.path())
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn serves_shell_and_nested_browser_routes() {
    // Given
    let root = fixture();

    // When
    let home = request(&root, "GET", "/", &[("accept", "text/html")]).await;
    let nested = request(&root, "GET", "/plans/example", &[("accept", "text/html")]).await;

    // Then
    assert_eq!(home.status(), StatusCode::OK);
    assert_eq!(nested.status(), StatusCode::OK);
    assert_eq!(nested.headers()[header::CACHE_CONTROL], "no-cache");
    assert_eq!(
        nested.headers()[header::CONTENT_TYPE],
        "text/html; charset=utf-8"
    );
    assert_eq!(
        nested.into_body().collect().await.unwrap().to_bytes(),
        "<main>Fubbik</main>"
    );
}

#[tokio::test]
async fn serves_fingerprinted_assets_with_conditional_requests() {
    // Given
    let root = fixture();

    // When
    let asset = request(&root, "GET", "/assets/app-123.js", &[]).await;
    let etag = asset.headers()[header::ETAG].to_str().unwrap().to_owned();
    let unchanged = request(
        &root,
        "GET",
        "/assets/app-123.js",
        &[("if-none-match", &etag)],
    )
    .await;
    let head = request(&root, "HEAD", "/assets/app-123.js", &[]).await;

    // Then
    assert_eq!(asset.status(), StatusCode::OK);
    assert_eq!(
        asset.headers()[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    assert_eq!(
        asset.headers()[header::CONTENT_TYPE],
        "text/javascript; charset=utf-8"
    );
    assert_eq!(unchanged.status(), StatusCode::NOT_MODIFIED);
    assert!(
        unchanged
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .is_empty()
    );
    assert_eq!(head.status(), StatusCode::OK);
    assert!(
        head.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .is_empty()
    );
}

#[tokio::test]
async fn leaves_api_and_non_browser_paths_as_errors() {
    // Given
    let root = fixture();

    // When
    let api = request(&root, "GET", "/api/unknown", &[("accept", "text/html")]).await;
    let missing = request(&root, "GET", "/missing.js", &[("accept", "text/html")]).await;
    let missing_asset = request(&root, "GET", "/assets/missing", &[("accept", "text/html")]).await;
    let encoded_api = request(&root, "GET", "/api%2Funknown", &[("accept", "text/html")]).await;
    let data = request(
        &root,
        "GET",
        "/plans/example",
        &[("accept", "application/json")],
    )
    .await;
    let post = request(&root, "POST", "/plans/example", &[("accept", "text/html")]).await;

    // Then
    assert_eq!(api.status(), StatusCode::NOT_FOUND);
    assert_eq!(api.headers()[header::CONTENT_TYPE], "application/json");
    assert_eq!(encoded_api.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        encoded_api.headers()[header::CONTENT_TYPE],
        "application/json"
    );
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(missing_asset.status(), StatusCode::NOT_FOUND);
    assert_eq!(data.status(), StatusCode::NOT_FOUND);
    assert_eq!(post.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn rejects_encoded_traversal_and_hidden_files() {
    // Given
    let root = fixture();

    // When
    let parent = request(&root, "GET", "/%2e%2e/secret", &[("accept", "text/html")]).await;
    let hidden = request(&root, "GET", "/.env", &[]).await;
    let backslash = request(&root, "GET", "/assets%5csecret", &[]).await;

    // Then
    assert_eq!(parent.status(), StatusCode::BAD_REQUEST);
    assert_eq!(hidden.status(), StatusCode::BAD_REQUEST);
    assert_eq!(backslash.status(), StatusCode::BAD_REQUEST);
}

#[cfg(unix)]
#[tokio::test]
async fn rejects_symlinks_outside_asset_root() {
    // Given
    let root = fixture();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.txt"), "secret").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret.txt"),
        root.path().join("assets/link.txt"),
    )
    .unwrap();

    // When
    let response = request(&root, "GET", "/assets/link.txt", &[]).await;

    // Then
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
