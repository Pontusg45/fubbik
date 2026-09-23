//! Proves `auth::session::CurrentUser`'s implicit-dev fallback bootstraps
//! its own `user` row rather than 401ing against an unseeded database. This
//! is the *only* state the bug shows up in — a seeded database passes
//! whether or not the bootstrap exists, which would make the test
//! worthless (see `fubbik_db::repo::user::ensure_implicit_dev_user`).

use axum::body::Body;
use axum::http::Request;
use fubbik_db::repo::user;
use sqlx::PgPool;
use tower::ServiceExt;

async fn test_app_with_implicit_dev(pool: PgPool) -> axum::Router {
    fubbik_api::router(fubbik_api::AppState {
        pool,
        implicit_dev_session: true,
        better_auth_secret: "test-secret".into(),
        ai: fubbik_ai::OllamaClient::new("http://127.0.0.1:1"),
        rate_limiter: Default::default(),
        background: Default::default(),
    })
}

async fn get(app: &axum::Router, path: &str) -> axum::response::Response {
    app.clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn implicit_dev_session_creates_the_dev_user_on_an_empty_database(pool: PgPool) {
    // Given
    // The ONLY state in which the current bug appears. With a seeded DB it passes either way.
    let app = test_app_with_implicit_dev(pool.clone()).await;
    // When
    let res = get(&app, "/api/chunks").await;
    // Then
    assert_eq!(
        res.status(),
        200,
        "dev session must bootstrap its user, not 401"
    );

    let row = user::find_by_email(&pool, "dev@localhost").await.unwrap();
    assert_eq!(row.unwrap().id, "dev-user");
}
